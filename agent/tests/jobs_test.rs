use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use obs_telegram_agent::{
    jobs::{JobError, JobService},
    telegram::{TelegramClient, UploadError},
};

#[derive(Clone, Default)]
struct FakeGateway {
    uploads: Arc<Mutex<Vec<(PathBuf, String)>>>,
}

#[derive(Clone, Default)]
struct FailingGateway {
    uploads: Arc<Mutex<usize>>,
}

#[cfg(unix)]
#[derive(Clone)]
struct PersistBlockingGateway {
    directory: PathBuf,
}

#[derive(Clone, Default)]
struct UncertainGateway;

impl FakeGateway {
    fn uploads(&self) -> Vec<(PathBuf, String)> {
        self.uploads.lock().unwrap().clone()
    }
}

#[async_trait]
impl TelegramClient for FakeGateway {
    async fn upload_file(&self, path: PathBuf, display_name: String) -> Result<(), UploadError> {
        self.uploads.lock().unwrap().push((path, display_name));
        Ok(())
    }
}

#[async_trait]
impl TelegramClient for FailingGateway {
    async fn upload_file(&self, _path: PathBuf, _display_name: String) -> Result<(), UploadError> {
        *self.uploads.lock().unwrap() += 1;
        Err(UploadError::PreUploadRejected)
    }
}

#[async_trait]
#[cfg(unix)]
impl TelegramClient for PersistBlockingGateway {
    async fn upload_file(&self, _path: PathBuf, _display_name: String) -> Result<(), UploadError> {
        std::fs::set_permissions(
            &self.directory,
            std::os::unix::fs::PermissionsExt::from_mode(0o500),
        )
        .map_err(|_| UploadError::PreUploadRejected)
    }
}

#[async_trait]
impl TelegramClient for UncertainGateway {
    async fn upload_file(&self, _path: PathBuf, _display_name: String) -> Result<(), UploadError> {
        Err(UploadError::TransportUncertain)
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

    assert_eq!(
        service.retry(&job_id).await.unwrap().state,
        obs_telegram_agent::jobs::JobState::Queued
    );
    service.start_upload(&job_id).await.unwrap();
    assert_eq!(*gateway.uploads.lock().unwrap(), 2);
}

#[tokio::test]
async fn completed_jobs_do_not_persist_the_recording_path_but_failed_jobs_survive_restart() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::NamedTempFile::new().unwrap();
    let service = JobService::at(FailingGateway::default(), directory.path()).unwrap();
    let id = service
        .enqueue(source.path().to_path_buf(), "ignored.mp4".to_owned())
        .await
        .unwrap();
    service.start_upload(&id).await.unwrap();

    let restored = JobService::at(FailingGateway::default(), directory.path()).unwrap();
    assert_eq!(
        restored.status(&id).await.unwrap().state,
        obs_telegram_agent::jobs::JobState::Failed
    );
    let serialized = std::fs::read_to_string(directory.path().join("upload-jobs.json")).unwrap();
    assert!(serialized.contains(&source.path().display().to_string()));
}

#[tokio::test]
async fn completing_a_persisted_job_erases_its_protected_source_path() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::NamedTempFile::new().unwrap();
    let service = JobService::at(FakeGateway::default(), directory.path()).unwrap();
    let id = service
        .enqueue(source.path().to_path_buf(), "recording.mp4".to_owned())
        .await
        .unwrap();

    service.start_upload(&id).await.unwrap();

    let serialized = std::fs::read_to_string(directory.path().join("upload-jobs.json")).unwrap();
    assert!(!serialized.contains(&source.path().display().to_string()));
}

#[tokio::test]
async fn queued_jobs_restored_after_a_crash_become_manually_retryable_failures() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::NamedTempFile::new().unwrap();
    let service = JobService::at(FakeGateway::default(), directory.path()).unwrap();
    let id = service
        .enqueue(source.path().to_path_buf(), "recording.mp4".to_owned())
        .await
        .unwrap();

    let restored = JobService::at(FakeGateway::default(), directory.path()).unwrap();
    assert_eq!(
        restored.status(&id).await.unwrap().state,
        obs_telegram_agent::jobs::JobState::Failed
    );
}

#[tokio::test]
#[cfg(unix)]
async fn a_persist_failure_before_upload_leaves_a_pollable_failed_job_without_calling_telegram() {
    let directory = tempfile::tempdir().unwrap();
    let gateway = FakeGateway::default();
    let service = JobService::at(gateway.clone(), directory.path()).unwrap();
    let source = tempfile::NamedTempFile::new().unwrap();
    let id = service
        .enqueue(source.path().to_path_buf(), "recording.mp4".to_owned())
        .await
        .unwrap();
    std::fs::set_permissions(
        directory.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o500),
    )
    .unwrap();

    assert_eq!(
        service.start_upload(&id).await,
        Err(JobError::StorageUnavailable)
    );
    assert_eq!(
        service.status(&id).await.unwrap().state,
        obs_telegram_agent::jobs::JobState::Failed
    );
    assert!(gateway.uploads().is_empty());
}

#[tokio::test]
#[cfg(unix)]
async fn a_completion_persist_failure_marks_the_job_unknown_and_never_retryable() {
    let directory = tempfile::tempdir().unwrap();
    let service = JobService::at(
        PersistBlockingGateway {
            directory: directory.path().to_path_buf(),
        },
        directory.path(),
    )
    .unwrap();
    let source = tempfile::NamedTempFile::new().unwrap();
    let id = service
        .enqueue(source.path().to_path_buf(), "recording.mp4".to_owned())
        .await
        .unwrap();

    assert_eq!(
        service.start_upload(&id).await,
        Err(JobError::StorageUnavailable)
    );
    assert_eq!(
        service.status(&id).await.unwrap().state,
        obs_telegram_agent::jobs::JobState::Unknown
    );
    assert_eq!(service.retry(&id).await, Err(JobError::NotRetryable));
}

#[tokio::test]
async fn an_uncertain_transport_result_is_unknown_and_never_offered_for_retry() {
    let service = JobService::new(UncertainGateway);
    let source = tempfile::NamedTempFile::new().unwrap();
    let id = service
        .enqueue(source.path().to_path_buf(), "recording.mp4".to_owned())
        .await
        .unwrap();

    service.start_upload(&id).await.unwrap();

    assert_eq!(
        service.status(&id).await.unwrap().state,
        obs_telegram_agent::jobs::JobState::Unknown
    );
    assert_eq!(service.retry(&id).await, Err(JobError::NotRetryable));
}

#[tokio::test]
#[cfg(unix)]
async fn retry_save_failure_rolls_back_to_the_failed_pollable_state() {
    let directory = tempfile::tempdir().unwrap();
    let service = JobService::at(FailingGateway::default(), directory.path()).unwrap();
    let source = tempfile::NamedTempFile::new().unwrap();
    let id = service
        .enqueue(source.path().to_path_buf(), "recording.mp4".to_owned())
        .await
        .unwrap();
    service.start_upload(&id).await.unwrap();
    std::fs::set_permissions(
        directory.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o500),
    )
    .unwrap();

    assert_eq!(service.retry(&id).await, Err(JobError::StorageUnavailable));
    assert_eq!(
        service.status(&id).await.unwrap().state,
        obs_telegram_agent::jobs::JobState::Failed
    );
}

#[tokio::test]
async fn display_name_is_derived_from_the_source_basename() {
    let gateway = FakeGateway::default();
    let service = JobService::new(gateway);
    let source = tempfile::NamedTempFile::with_suffix(".mp4").unwrap();
    let id = service
        .enqueue(source.path().to_path_buf(), "ignored.mp4".to_owned())
        .await
        .unwrap();

    assert_eq!(
        service.status(&id).await.unwrap().filename,
        source.path().file_name().unwrap().to_str().unwrap()
    );
}

#[tokio::test]
async fn display_name_with_a_path_separator_is_rejected() {
    let service = JobService::new(FakeGateway::default());
    let source = tempfile::NamedTempFile::new().unwrap();

    assert_eq!(
        service
            .enqueue(source.path().to_path_buf(), "../recording.mp4".to_owned())
            .await,
        Err(JobError::InvalidDisplayName)
    );
}
