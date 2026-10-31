//! Where session overlay helpers listen. Every helper drawing on one
//! compositor listens in one private directory of the compositor's runtime
//! directory, so a capture finds them all; each socket is named for the
//! state root and session it serves.

use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

use agent_desktop_core::AdapterError;

pub(crate) const PROTOCOL: &str = "v1";
const MAX_SOCKET_PATH: usize = 107;
const DEFAULT_DISPLAY: &str = "wayland-0";

fn hash(parts: &[&[u8]]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for part in parts {
        for byte in part.iter().chain(&[0]) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

fn unusable(detail: impl std::fmt::Display) -> AdapterError {
    AdapterError::internal("The cursor overlay socket directory is not usable")
        .with_platform_detail(detail.to_string())
}

/// The helpers' directory for the compositor `display` names inside `runtime`.
pub(crate) fn directory_for(runtime: &Path, display: &str) -> PathBuf {
    runtime.join(format!("ad-cursor-{:016x}", hash(&[display.as_bytes()])))
}

/// The helpers' directory for this process's compositor.
pub(crate) fn directory() -> Result<PathBuf, AdapterError> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| unusable("XDG_RUNTIME_DIR is not set"))?;
    let display = std::env::var("WAYLAND_DISPLAY")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_DISPLAY.to_owned());
    Ok(directory_for(Path::new(&runtime), &display))
}

/// The socket of the helper serving `session_id` of the state root `root`.
pub(crate) fn socket_in(
    directory: &Path,
    root: &Path,
    session_id: &str,
) -> Result<PathBuf, AdapterError> {
    let name = hash(&[
        root.as_os_str().as_bytes(),
        session_id.as_bytes(),
        PROTOCOL.as_bytes(),
    ]);
    let socket = directory.join(format!("{name:016x}.sock"));
    if socket.as_os_str().len() > MAX_SOCKET_PATH {
        return Err(unusable(format!(
            "{} is longer than a socket address holds",
            socket.display()
        )));
    }
    Ok(socket)
}

/// Creates `directory` private to this user, or checks that it already is.
pub(crate) fn ensure_private(directory: &Path) -> Result<(), AdapterError> {
    match std::fs::DirBuilder::new().mode(0o700).create(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(unusable(error)),
    }
    let metadata = std::fs::symlink_metadata(directory).map_err(unusable)?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(unusable(format!(
            "{} is not a directory private to this user",
            directory.display()
        )));
    }
    Ok(())
}

fn is_socket_name(name: &str) -> bool {
    name.strip_suffix(".sock")
        .is_some_and(|stem| stem.len() == 16 && stem.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// Every helper socket in `directory`; a missing directory has none.
pub(crate) fn listed(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut sockets: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_str().is_some_and(is_socket_name))
        .map(|entry| entry.path())
        .collect();
    sockets.sort();
    sockets
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
