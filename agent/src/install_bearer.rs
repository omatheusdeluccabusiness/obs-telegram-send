use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use directories_next::BaseDirs;
use rand::RngCore;
use secrecy::SecretString;

pub const APP_SUPPORT_DIRECTORY_NAME: &str = "OBS-Telegram-Send";
pub const INSTALL_BEARER_FILE_NAME: &str = "loopback-install-bearer";

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

        match fs::read_to_string(&path) {
            Ok(value) => {
                ensure_owner_only_file(&path)?;
                valid_bearer(&value)
                    .then_some(SecretString::from(value))
                    .ok_or(InstallBearerStoreError::InvalidStoredBearer)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.create_bearer(&path),
            Err(_) => Err(InstallBearerStoreError::Unavailable),
        }
    }

    fn create_bearer(&self, path: &Path) -> Result<SecretString, InstallBearerStoreError> {
        let bearer = generated_bearer();
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
        {
            Ok(mut file) => {
                file.write_all(bearer.as_bytes())
                    .and_then(|()| file.sync_all())
                    .map_err(|_| InstallBearerStoreError::Unavailable)?;
                ensure_owner_only_file(path)?;
                Ok(SecretString::from(bearer))
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => self.load_or_create(),
            Err(_) => Err(InstallBearerStoreError::Unavailable),
        }
    }
}

fn ensure_owner_only_directory(path: &Path) -> Result<(), InstallBearerStoreError> {
    fs::create_dir_all(path).map_err(|_| InstallBearerStoreError::Unavailable)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| InstallBearerStoreError::Unavailable)
}

fn ensure_owner_only_file(path: &Path) -> Result<(), InstallBearerStoreError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| InstallBearerStoreError::Unavailable)
}

fn valid_bearer(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn generated_bearer() -> String {
    let mut bytes = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
