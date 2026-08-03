use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use directories_next::BaseDirs;
use rand::RngCore;
use secrecy::SecretString;

use crate::private_fs::{ensure_private_directory, ensure_private_file, private_create_new};

pub const APP_SUPPORT_DIRECTORY_NAME: &str = "OBS-Telegram-Send";
pub const INSTALL_BEARER_FILE_NAME: &str = "loopback-install-bearer";
const READ_RETRY_COUNT: usize = 10;
const READ_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(5);

pub struct InstallBearerStore {
    directory: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InstallBearerStoreError {
    Unavailable,
    InvalidStoredBearer,
}

impl InstallBearerStore {
    pub fn in_app_support() -> Result<Self, InstallBearerStoreError> {
        let base_directories = BaseDirs::new().ok_or(InstallBearerStoreError::Unavailable)?;
        Ok(Self::at(
            base_directories
                .data_local_dir()
                .join(APP_SUPPORT_DIRECTORY_NAME),
        ))
    }

    pub fn at(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn path(&self) -> PathBuf {
        self.directory.join(INSTALL_BEARER_FILE_NAME)
    }

    pub fn load_or_create(&self) -> Result<SecretString, InstallBearerStoreError> {
        ensure_owner_only_directory(&self.directory)?;
        let path = self.path();

        for attempt in 0..READ_RETRY_COUNT {
            match fs::read_to_string(&path) {
                Ok(value) if valid_bearer(&value) => {
                    ensure_owner_only_file(&path)?;
                    return Ok(SecretString::from(value));
                }
                Ok(_) if attempt + 1 < READ_RETRY_COUNT => std::thread::sleep(READ_RETRY_DELAY),
                Ok(_) => return Err(InstallBearerStoreError::InvalidStoredBearer),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return self.create_bearer(&path)
                }
                Err(_) => return Err(InstallBearerStoreError::Unavailable),
            }
        }

        unreachable!("read retry loop always returns")
    }

    fn create_bearer(&self, path: &Path) -> Result<SecretString, InstallBearerStoreError> {
        let bearer = generated_bearer();
        let temporary_path = self.temporary_path();
        match private_create_new(&temporary_path) {
            Ok(mut file) => {
                file.write_all(bearer.as_bytes())
                    .and_then(|()| file.sync_all())
                    .map_err(|_| InstallBearerStoreError::Unavailable)?;
                ensure_owner_only_file(&temporary_path)?;
                drop(file);

                match fs::hard_link(&temporary_path, path) {
                    Ok(()) => {
                        fs::remove_file(&temporary_path)
                            .map_err(|_| InstallBearerStoreError::Unavailable)?;
                        Ok(SecretString::from(bearer))
                    }
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        let _ = fs::remove_file(&temporary_path);
                        self.load_or_create()
                    }
                    Err(_) => {
                        let _ = fs::remove_file(&temporary_path);
                        Err(InstallBearerStoreError::Unavailable)
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => self.create_bearer(path),
            Err(_) => Err(InstallBearerStoreError::Unavailable),
        }
    }

    fn temporary_path(&self) -> PathBuf {
        self.directory.join(format!(
            ".{INSTALL_BEARER_FILE_NAME}.tmp-{:016x}",
            rand::random::<u64>()
        ))
    }
}

fn ensure_owner_only_directory(path: &Path) -> Result<(), InstallBearerStoreError> {
    ensure_private_directory(path).map_err(|_| InstallBearerStoreError::Unavailable)
}

fn ensure_owner_only_file(path: &Path) -> Result<(), InstallBearerStoreError> {
    ensure_private_file(path).map_err(|_| InstallBearerStoreError::Unavailable)
}

fn valid_bearer(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn generated_bearer() -> String {
    let mut bytes = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
