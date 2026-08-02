use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use obs_telegram_agent::install_bearer::InstallBearerStore;
use secrecy::ExposeSecret;

fn temporary_app_support_directory() -> PathBuf {
    std::env::temp_dir().join(format!(
        "obs-telegram-send-bearer-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn install_bearer_is_stable_across_store_restarts() {
    let directory = temporary_app_support_directory();
    let first = InstallBearerStore::at(directory.clone())
        .load_or_create()
        .unwrap();
    let second = InstallBearerStore::at(directory.clone())
        .load_or_create()
        .unwrap();

    assert_eq!(first.expose_secret(), second.expose_secret());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn install_bearer_store_uses_owner_only_directory_and_file_permissions() {
    let directory = temporary_app_support_directory();
    let store = InstallBearerStore::at(directory.clone());
    store.load_or_create().unwrap();

    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let stored_value = fs::read_to_string(store.path()).unwrap();
    assert_eq!(stored_value.len(), 64);
    assert!(!stored_value.contains("bot_token"));
    assert!(!stored_value.contains("api_hash"));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn reader_retries_until_a_concurrently_written_bearer_is_complete() {
    let directory = temporary_app_support_directory();
    let store = InstallBearerStore::at(directory.clone());
    fs::create_dir_all(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(store.path(), "").unwrap();
    fs::set_permissions(store.path(), fs::Permissions::from_mode(0o600)).unwrap();

    let path = store.path();
    let expected = "a".repeat(64);
    let writer = thread::spawn({
        let expected = expected.clone();
        move || {
            thread::sleep(std::time::Duration::from_millis(20));
            fs::write(path, expected).unwrap();
        }
    });

    let bearer = store.load_or_create().unwrap();
    writer.join().unwrap();
    assert_eq!(bearer.expose_secret(), expected);

    fs::remove_dir_all(directory).unwrap();
}
