use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::{directory_for, ensure_private, listed, socket_in};

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("ad-wlr-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    path
}

#[test]
fn one_compositor_shares_a_directory_and_each_session_has_its_socket() {
    let runtime = Path::new("/run/user/1000");
    let directory = directory_for(runtime, "wayland-1");
    assert_eq!(directory, directory_for(runtime, "wayland-1"));
    assert_ne!(directory, directory_for(runtime, "wayland-2"));
    assert_eq!(directory.parent(), Some(runtime));
    let root = Path::new("/home/u/.agent-desktop");
    let first = socket_in(&directory, root, "run-1").expect("socket");
    assert_eq!(first, socket_in(&directory, root, "run-1").expect("socket"));
    assert_ne!(first, socket_in(&directory, root, "run-2").expect("socket"));
    assert_ne!(
        first,
        socket_in(&directory, Path::new("/elsewhere"), "run-1").expect("socket")
    );
    assert_eq!(first.parent(), Some(directory.as_path()));
}

#[test]
fn a_socket_path_longer_than_an_address_is_refused() {
    let deep = Path::new("/tmp").join("d".repeat(120));
    assert!(socket_in(&deep, Path::new("/r"), "run-1").is_err());
}

#[test]
fn the_directory_is_made_private_and_a_shared_one_is_refused() {
    let directory = scratch("private");
    ensure_private(&directory).expect("created");
    let mode = std::fs::metadata(&directory)
        .expect("exists")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
    ensure_private(&directory).expect("already private");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    assert!(ensure_private(&directory).is_err());
    std::fs::remove_dir_all(&directory).expect("cleanup");
}

#[test]
fn only_helper_socket_names_are_listed() {
    let directory = scratch("listed");
    assert!(listed(&directory).is_empty());
    ensure_private(&directory).expect("created");
    for name in [
        "0123456789abcdef.sock",
        "short.sock",
        "0123456789abcdef.txt",
    ] {
        std::fs::write(directory.join(name), b"").expect("write");
    }
    assert_eq!(
        listed(&directory),
        [directory.join("0123456789abcdef.sock")]
    );
    std::fs::remove_dir_all(&directory).expect("cleanup");
}
