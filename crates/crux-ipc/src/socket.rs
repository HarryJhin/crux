//! Socket path resolution and discovery.

use std::path::PathBuf;

/// Determine the socket path for this Crux instance.
///
/// Priority:
/// 1. `$CRUX_SOCKET` environment variable
/// 2. `$XDG_RUNTIME_DIR/crux/gui-sock-$PID`
/// 3. `/tmp/crux-$UID/gui-sock-$PID`
pub fn socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("CRUX_SOCKET") {
        return PathBuf::from(path);
    }

    let dir = runtime_directory();

    // Ensure the directory exists with restricted permissions.
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("failed to create socket directory {}: {}", dir.display(), e);
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }
    }

    let pid = std::process::id();
    dir.join(format!("gui-sock-{pid}"))
}

/// Get the runtime directory for socket files.
fn runtime_directory() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(xdg).join("crux");
    }

    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/crux-{uid}"))
}

/// Discover an existing Crux server socket.
///
/// Used by CLI clients to find a running server:
/// 1. Check `$CRUX_SOCKET`
/// 2. Scan runtime directory for the most recent `gui-sock-*` file
pub fn discover_socket() -> Option<PathBuf> {
    // 1. Explicit environment variable
    if let Ok(path) = std::env::var("CRUX_SOCKET") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Scan runtime directory for most recent socket
    let dir = runtime_directory();
    let read_dir = std::fs::read_dir(&dir).ok()?;

    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;

    for entry in read_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("gui-sock-") {
            continue;
        }
        let path = entry.path();
        if let Ok(meta) = entry.metadata() {
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(_, t)| modified > *t) {
                best = Some((path, modified));
            }
        }
    }

    best.map(|(p, _)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_contains_pid() {
        // Clear env to test default path
        std::env::remove_var("CRUX_SOCKET");
        let path = socket_path();
        let pid = std::process::id();
        let filename = path.file_name().unwrap().to_string_lossy();
        assert_eq!(filename, format!("gui-sock-{pid}"));
    }

    #[test]
    fn socket_path_respects_env_override() {
        let test_path = "/tmp/crux-test-socket";
        std::env::set_var("CRUX_SOCKET", test_path);
        let path = socket_path();
        assert_eq!(path, PathBuf::from(test_path));
        std::env::remove_var("CRUX_SOCKET");
    }

    #[test]
    fn discover_socket_returns_none_when_empty() {
        std::env::remove_var("CRUX_SOCKET");
        // With a non-existent directory, discover should return None
        std::env::set_var("XDG_RUNTIME_DIR", "/tmp/crux-test-nonexistent-dir");
        let result = discover_socket();
        assert!(result.is_none());
        std::env::remove_var("XDG_RUNTIME_DIR");
    }
}
