use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use rand::RngCore;
use serde::Serialize;

use crate::telegram::TelegramClient;

pub const MAX_FILE_SIZE_BYTES: u64 = 2 * 1024_u64.pow(3);

pub type JobId = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    FileTooLarge,
    FileUnavailable,
    NotFound,
    NotRetryable,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Uploading,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct JobStatus {
    pub job_id: JobId,
    pub filename: String,
    pub state: JobState,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable_error: Option<String>,
}

struct Job {
    status: JobStatus,
    recording_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct JobService<G> {
    gateway: G,
    jobs: Arc<Mutex<HashMap<JobId, Job>>>,
}

impl<G: TelegramClient> JobService<G> {
    pub fn new(gateway: G) -> Self {
        Self {
            gateway,
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn validate_size(size: u64) -> Result<(), JobError> {
        if size > MAX_FILE_SIZE_BYTES {
            Err(JobError::FileTooLarge)
        } else {
            Ok(())
        }
    }

    pub async fn enqueue(&self, path: PathBuf, display_name: String) -> Result<JobId, JobError> {
        let size = tokio::fs::metadata(&path)
            .await
            .map_err(|_| JobError::FileUnavailable)?
            .len();
        Self::validate_size(size)?;

        let job_id = new_job_id();
        let status = JobStatus {
            job_id: job_id.clone(),
            filename: display_name,
            state: JobState::Queued,
            size,
            retryable_error: None,
        };
        self.jobs.lock().unwrap().insert(
            job_id.clone(),
            Job {
                status,
                recording_path: Some(path),
            },
        );
        Ok(job_id)
    }

    pub async fn status(&self, job_id: &str) -> Result<JobStatus, JobError> {
        self.jobs
            .lock()
            .unwrap()
            .get(job_id)
            .map(|job| job.status.clone())
            .ok_or(JobError::NotFound)
    }

    pub fn jobs(&self) -> Vec<JobStatus> {
        self.jobs
            .lock()
            .unwrap()
            .values()
            .map(|job| job.status.clone())
            .collect()
    }

    // Called only by the explicit job POST after user confirmation.
    pub async fn start_upload(&self, job_id: &str) -> Result<(), JobError> {
        let (path, display_name) = {
            let mut jobs = self.jobs.lock().unwrap();
            let job = jobs.get_mut(job_id).ok_or(JobError::NotFound)?;
            if !matches!(job.status.state, JobState::Queued) {
                return Err(JobError::NotRetryable);
            }
            let path = job.recording_path.clone().ok_or(JobError::NotRetryable)?;
            job.status.state = JobState::Uploading;
            job.status.retryable_error = None;
            (path, job.status.filename.clone())
        };

        let result = self.gateway.upload_file(path, display_name).await;
        let mut jobs = self.jobs.lock().unwrap();
        let Some(job) = jobs.get_mut(job_id) else {
            return Ok(());
        };
        match result {
            Ok(()) => {
                job.status.state = JobState::Completed;
                job.status.retryable_error = None;
                // The source location is held only while a retry is possible.
                job.recording_path = None;
            }
            Err(_) => {
                job.status.state = JobState::Failed;
                job.status.retryable_error = Some("Telegram upload failed. Try again.".to_owned());
            }
        }
        Ok(())
    }

    pub async fn retry(&self, job_id: &str) -> Result<(), JobError> {
        {
            let mut jobs = self.jobs.lock().unwrap();
            let job = jobs.get_mut(job_id).ok_or(JobError::NotFound)?;
            if job.status.state != JobState::Failed {
                return Err(JobError::NotRetryable);
            }
            job.status.state = JobState::Queued;
            job.status.retryable_error = None;
        }
        if self.start_upload(job_id).await.is_err() {
            return Err(JobError::NotRetryable);
        }
        Ok(())
    }
}

fn new_job_id() -> JobId {
    let mut bytes = [0_u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
