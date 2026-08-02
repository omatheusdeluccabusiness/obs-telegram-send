use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use obs_telegram_agent::{
    jobs::{JobError, JobService},
    telegram::TelegramClient,
};

#[derive(Clone, Default)]
struct FakeGateway {
    uploads: Arc<Mutex<Vec<(PathBuf, String)>>>,
}

#[derive(Clone, Default)]
struct FailingGateway {
    uploads: Arc<Mutex<usize>>,
}

impl FakeGateway {
    fn uploads(&self) -> Vec<(PathBuf, String)> {
        self.uploads.lock().unwrap().clone()
    }
}

#[async_trait]
impl TelegramClient for FakeGateway {
    async fn upload_file(&self, path: PathBuf, display_name: String) -> Result<(), String> {
        self.uploads.lock().unwrap().push((path, display_name));
        Ok(())
    }
}

#[async_trait]
impl TelegramClient for FailingGateway {
    async fn upload_file(&self, _path: PathBuf, _display_name: String) -> Result<(), String> {
        *self.uploads.lock().unwrap() += 1;
        Err("network unavailable".to_owned())
    }
}

#[test]
fn rejects_a_file_larger_than_two_gibibytes_before_upload() {
    assert_eq!(
        JobService::<FakeGateway>::validate_size(2 * 1024_u64.pow(3) + 1),
        Err(JobError::FileTooLarge)
    );
}

#[tokio::test]
async fn enqueue_never_calls_telegram_until_the_plugin_posts_a_job() {
    let gateway = FakeGateway::default();
    let service = JobService::new(gateway.clone());

    assert!(gateway.uploads().is_empty());
    assert!(service.jobs().is_empty());
}

#[tokio::test]
async fn retry_is_the_only_way_to_reupload_a_failed_job() {
    let gateway = FailingGateway::default();
    let service = JobService::new(gateway.clone());
    let file = tempfile::NamedTempFile::new().unwrap();
    let job_id = service
        .enqueue(file.path().to_path_buf(), "recording.mp4".to_owned())
        .await
        .unwrap();

    assert_eq!(*gateway.uploads.lock().unwrap(), 0);
    service.start_upload(&job_id).await.unwrap();
    assert_eq!(*gateway.uploads.lock().unwrap(), 1);
    assert_eq!(
        service.start_upload(&job_id).await,
        Err(JobError::NotRetryable)
    );

    service.retry(&job_id).await.unwrap();
    assert_eq!(*gateway.uploads.lock().unwrap(), 2);
}
