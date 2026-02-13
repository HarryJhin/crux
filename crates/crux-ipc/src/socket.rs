//! Socket path resolution and discovery.

use std::path::PathBuf;

/// Determine the socket path for this Crux instance.
///
/// Priority:
/// 1. `$CRUX_SOCKET` environment variable
/// 2. `$XDG_RUNTIME_DIR/crux/gui-sock-$PID`
/// 3. `/tmp/crux-$UID/gui-sock-$PID`
pub fn socket_path() -> PathBuf {
    let env = SocketEnv::from_env();
    let path = resolve_socket_path(&env);

    // Ensure the parent directory exists with restricted permissions.
    if env.crux_socket.is_none() {
        let dir = resolve_runtime_dir(env.xdg_runtime_dir.as_deref());
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("failed to create socket directory {}: {}", dir.display(), e);
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Err(e) = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)) {
                    log::warn!("failed to set socket dir permissions: {e}");
                }
            }
        }
    }

    path
}

/// Discover an existing Crux server socket.
///
/// Used by CLI clients to find a running server:
/// 1. Check `$CRUX_SOCKET`
/// 2. Scan runtime directory for the most recent `gui-sock-*` file
pub fn discover_socket() -> Option<PathBuf> {
    let env = SocketEnv::from_env();
    discover_socket_with(&env)
}

/// Resolved environment for socket path determination.
///
/// Captures environment variables once, enabling pure-function testing
/// without global state manipulation.
struct SocketEnv {
    crux_socket: Option<String>,
    xdg_runtime_dir: Option<String>,
}

impl SocketEnv {
    fn from_env() -> Self {
        Self {
            crux_socket: std::env::var("CRUX_SOCKET").ok(),
            xdg_runtime_dir: std::env::var("XDG_RUNTIME_DIR").ok(),
        }
    }
}

/// Pure socket path resolution — no filesystem side effects.
fn resolve_socket_path(env: &SocketEnv) -> PathBuf {
    if let Some(ref path) = env.crux_socket {
        return PathBuf::from(path);
    }

    let dir = resolve_runtime_dir(env.xdg_runtime_dir.as_deref());
    let pid = std::process::id();
    dir.join(format!("gui-sock-{pid}"))
}

/// Pure runtime directory resolution.
fn resolve_runtime_dir(xdg_runtime_dir: Option<&str>) -> PathBuf {
    if let Some(xdg) = xdg_runtime_dir {
        return PathBuf::from(xdg).join("crux");
    }

    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/crux-{uid}"))
}

/// Pure socket discovery — operates on resolved env and scans given directory.
fn discover_socket_with(env: &SocketEnv) -> Option<PathBuf> {
    // 1. Explicit override
    if let Some(ref path) = env.crux_socket {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Scan runtime directory for most recent socket
    let dir = resolve_runtime_dir(env.xdg_runtime_dir.as_deref());
    scan_socket_dir(&dir)
}

/// Scan a directory for the most recently modified `gui-sock-*` file.
fn scan_socket_dir(dir: &std::path::Path) -> Option<PathBuf> {
    let read_dir = std::fs::read_dir(dir).ok()?;

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
    fn resolve_default_path_contains_pid() {
        let env = SocketEnv {
            crux_socket: None,
            xdg_runtime_dir: None,
        };
        let path = resolve_socket_path(&env);
        let pid = std::process::id();
        let filename = path.file_name().unwrap().to_string_lossy();
        assert_eq!(filename, format!("gui-sock-{pid}"));
    }

    #[test]
    fn resolve_crux_socket_override() {
        let env = SocketEnv {
            crux_socket: Some("/tmp/crux-test-socket".into()),
            xdg_runtime_dir: None,
        };
        let path = resolve_socket_path(&env);
        assert_eq!(path, PathBuf::from("/tmp/crux-test-socket"));
    }

    #[test]
    fn resolve_xdg_runtime_dir() {
        let env = SocketEnv {
            crux_socket: None,
            xdg_runtime_dir: Some("/run/user/1000".into()),
        };
        let path = resolve_socket_path(&env);
        let pid = std::process::id();
        assert_eq!(
            path,
            PathBuf::from(format!("/run/user/1000/crux/gui-sock-{pid}"))
        );
    }

    #[test]
    fn resolve_crux_socket_takes_priority_over_xdg() {
        let env = SocketEnv {
            crux_socket: Some("/custom/socket".into()),
            xdg_runtime_dir: Some("/run/user/1000".into()),
        };
        let path = resolve_socket_path(&env);
        assert_eq!(path, PathBuf::from("/custom/socket"));
    }

    #[test]
    fn discover_returns_none_for_nonexistent_dir() {
        let env = SocketEnv {
            crux_socket: None,
            xdg_runtime_dir: Some("/tmp/crux-test-nonexistent-dir-12345".into()),
        };
        let result = discover_socket_with(&env);
        assert!(result.is_none());
    }

    #[test]
    fn runtime_dir_falls_back_to_tmp() {
        let dir = resolve_runtime_dir(None);
        let uid = unsafe { libc::getuid() };
        assert_eq!(dir, PathBuf::from(format!("/tmp/crux-{uid}")));
    }

    #[test]
    fn runtime_dir_uses_xdg_when_set() {
        let dir = resolve_runtime_dir(Some("/run/user/501"));
        assert_eq!(dir, PathBuf::from("/run/user/501/crux"));
    }
}
