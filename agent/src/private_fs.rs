use std::{fs, io, path::Path};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn ensure_private_directory(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    set_private_directory_permissions(path)
}

pub fn ensure_private_file(path: &Path) -> io::Result<()> {
    set_private_file_permissions(path)
}

pub fn private_create_new(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    apply_private_mode(&mut options);
    let file = options.open(path)?;
    ensure_private_file(path)?;
    Ok(file)
}

pub fn private_create_truncate(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    apply_private_mode(&mut options);
    let file = options.open(path)?;
    ensure_private_file(path)?;
    Ok(file)
}

#[cfg(unix)]
fn apply_private_mode(options: &mut fs::OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn apply_private_mode(_: &mut fs::OpenOptions) {}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_: &Path) -> io::Result<()> {
    // All runtime data is installed below the current user's LocalAppData.
    // Windows ACL inheritance keeps this private to that user; POSIX modes do
    // not exist on this platform.
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_: &Path) -> io::Result<()> {
    Ok(())
}
