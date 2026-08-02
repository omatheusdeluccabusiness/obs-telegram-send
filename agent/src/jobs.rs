use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use directories_next::BaseDirs;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::{install_bearer::APP_SUPPORT_DIRECTORY_NAME, telegram::TelegramClient};

pub const MAX_FILE_SIZE_BYTES: u64 = 2 * 1024_u64.pow(3);
const JOBS_FILE_NAME: &str = "upload-jobs.json";

pub type JobId = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    FileTooLarge,
    FileUnavailable,
    InvalidDisplayName,
    StorageUnavailable,
    NotFound,
    NotRetryable,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Uploading,
    Completed,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobStatus {
    pub job_id: JobId,
    pub filename: String,
    pub state: JobState,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable_error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Job {
    status: JobStatus,
    // Kept only in an owner-only state file while a manual retry remains possible.
    recording_path: Option<PathBuf>,
}

#[derive(Clone)]
struct JobStore {
    path: Option<PathBuf>,
}

impl JobStore {
    fn memory() -> Self {
        Self { path: None }
    }

    fn at(directory: &Path) -> Result<Self, JobError> {
        fs::create_dir_all(directory).map_err(|_| JobError::StorageUnavailable)?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|_| JobError::StorageUnavailable)?;
        Ok(Self {
            path: Some(directory.join(JOBS_FILE_NAME)),
        })
    }

    fn load(&self) -> Result<HashMap<JobId, Job>, JobError> {
        let Some(path) = &self.path else {
            return Ok(HashMap::new());
        };
        let serialized = match fs::read_to_string(path) {
            Ok(serialized) => serialized,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(_) => return Err(JobError::StorageUnavailable),
        };
        let mut jobs: HashMap<JobId, Job> =
            serde_json::from_str(&serialized).map_err(|_| JobError::StorageUnavailable)?;
        for job in jobs.values_mut() {
            if job.status.state == JobState::Queued {
                job.status.state = JobState::Failed;
                job.status.retryable_error =
                    Some("Agent restarted before upload. Try again.".to_owned());
            }
            if job.status.state == JobState::Uploading {
                job.status.state = JobState::Unknown;
                job.status.retryable_error = Some(
                    "Telegram may have received this recording. Verify before sending again."
                        .to_owned(),
                );
                job.recording_path = None;
            }
            if matches!(job.status.state, JobState::Completed | JobState::Unknown) {
                job.recording_path = None;
            }
        }
        Ok(jobs)
    }

    fn save(&self, jobs: &HashMap<JobId, Job>) -> Result<(), JobError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let serialized = serde_json::to_vec(jobs).map_err(|_| JobError::StorageUnavailable)?;
        let temporary = path.with_file_name(format!(
            ".{JOBS_FILE_NAME}.{:016x}.tmp",
            rand::random::<u64>()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| JobError::StorageUnavailable)?;
        if file
            .write_all(&serialized)
            .and_then(|_| file.sync_all())
            .is_err()
        {
            let _ = fs::remove_file(&temporary);
            return Err(JobError::StorageUnavailable);
        }
        drop(file);
        if fs::rename(&temporary, path).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(JobError::StorageUnavailable);
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| JobError::StorageUnavailable)
    }
}

#[derive(Clone)]
pub struct JobService<G> {
    gateway: G,
    jobs: Arc<Mutex<HashMap<JobId, Job>>>,
    store: JobStore,
}

impl<G: TelegramClient> JobService<G> {
    pub fn new(gateway: G) -> Self {
        Self {
            gateway,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            store: JobStore::memory(),
        }
    }

    pub fn at(gateway: G, directory: &Path) -> Result<Self, JobError> {
        let store = JobStore::at(directory)?;
        let jobs = store.load()?;
        store.save(&jobs)?;
        Ok(Self {
            gateway,
            jobs: Arc::new(Mutex::new(jobs)),
            store,
        })
    }

    pub fn in_app_support(gateway: G) -> Result<Self, JobError> {
        let base = BaseDirs::new().ok_or(JobError::StorageUnavailable)?;
        Self::at(
            gateway,
            &base.data_local_dir().join(APP_SUPPORT_DIRECTORY_NAME),
        )
    }

    pub fn validate_size(size: u64) -> Result<(), JobError> {
        if size > MAX_FILE_SIZE_BYTES {
            Err(JobError::FileTooLarge)
        } else {
            Ok(())
        }
    }

    pub async fn enqueue(&self, path: PathBuf, supplied_name: String) -> Result<JobId, JobError> {
        validate_display_name(&supplied_name)?;
        let filename = basename(&path)?;
        let size = size_of_open_file(&path).await?;
        Self::validate_size(size)?;

        let job_id = new_job_id();
        let status = JobStatus {
            job_id: job_id.clone(),
            filename,
            state: JobState::Queued,
            size,
            retryable_error: None,
        };
        let mut jobs = self.jobs.lock().unwrap();
        jobs.insert(
            job_id.clone(),
            Job {
                status,
                recording_path: Some(path),
            },
        );
        if self.store.save(&jobs).is_err() {
            jobs.remove(&job_id);
            return Err(JobError::StorageUnavailable);
        }
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
            if job.status.state != JobState::Queued {
                return Err(JobError::NotRetryable);
            }
            let path = job.recording_path.clone().ok_or(JobError::NotRetryable)?;
            job.status.state = JobState::Uploading;
            job.status.retryable_error = None;
            let display_name = job.status.filename.clone();
            (path, display_name)
        };
        if self.store.save(&self.jobs.lock().unwrap()).is_err() {
            let mut jobs = self.jobs.lock().unwrap();
            if let Some(job) = jobs.get_mut(job_id) {
                job.status.state = JobState::Failed;
                job.status.retryable_error = Some(
                    "Upload could not be started safely. Try again after storage is available."
                        .to_owned(),
                );
            }
            return Err(JobError::StorageUnavailable);
        }

        let result = self.gateway.upload_file(path, display_name).await;
        let mut jobs = self.jobs.lock().unwrap();
        let Some(job) = jobs.get_mut(job_id) else {
            return Ok(());
        };
        let remote_success = result.is_ok();
        match result {
            Ok(()) => {
                job.status.state = JobState::Completed;
                job.status.retryable_error = None;
                job.recording_path = None;
            }
            Err(_) => {
                job.status.state = JobState::Failed;
                job.status.retryable_error = Some("Telegram upload failed. Try again.".to_owned());
            }
        }
        if self.store.save(&jobs).is_err() {
            if remote_success {
                let job = jobs
                    .get_mut(job_id)
                    .expect("job is retained while uploading");
                job.status.state = JobState::Unknown;
                job.status.retryable_error = Some(
                    "Telegram may have received this recording. Verify before sending again."
                        .to_owned(),
                );
                job.recording_path = None;
            }
            return Err(JobError::StorageUnavailable);
        }
        Ok(())
    }

    // Marks a failed job pollable as queued. The route starts it in a separate task.
    pub async fn retry(&self, job_id: &str) -> Result<JobStatus, JobError> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs.get_mut(job_id).ok_or(JobError::NotFound)?;
        if job.status.state != JobState::Failed {
            return Err(JobError::NotRetryable);
        }
        job.status.state = JobState::Queued;
        job.status.retryable_error = None;
        let status = job.status.clone();
        self.store.save(&jobs)?;
        Ok(status)
    }
}

async fn size_of_open_file(path: &Path) -> Result<u64, JobError> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| JobError::FileUnavailable)?;
    file.metadata()
        .await
        .map_err(|_| JobError::FileUnavailable)
        .map(|metadata| metadata.len())
}

fn basename(path: &Path) -> Result<String, JobError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .ok_or(JobError::InvalidDisplayName)
}

fn validate_display_name(name: &str) -> Result<(), JobError> {
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        Err(JobError::InvalidDisplayName)
    } else {
        Ok(())
    }
}

fn new_job_id() -> JobId {
    let mut bytes = [0_u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
