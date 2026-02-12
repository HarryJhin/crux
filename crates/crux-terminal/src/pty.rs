use std::io::Read;
use std::sync::Arc;
use std::thread::JoinHandle;

use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{self, Processor};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use crate::event::CruxEventListener;
use crate::TerminalSize;

/// Check if a terminfo entry is available on the system.
///
/// Searches the standard terminfo directories in order:
/// 1. `$TERMINFO/{first_char}/{name}` (user override)
/// 2. `$TERMINFO_DIRS` paths
/// 3. `$HOME/.terminfo/{first_char}/{name}`
/// 4. `/usr/share/terminfo/{first_char}/{name}` (letter subdir, Linux/BSD)
/// 5. `/usr/share/terminfo/{hex}/{name}` (hex subdir, macOS)
fn check_terminfo_available(name: &str) -> bool {
    let first_char = match name.chars().next() {
        Some(c) => c,
        None => return false,
    };
    let letter_dir = first_char.to_string();
    let hex_dir = format!("{:x}", first_char as u32);

    // $TERMINFO (single directory override)
    if let Ok(dir) = std::env::var("TERMINFO") {
        if std::path::Path::new(&dir)
            .join(&letter_dir)
            .join(name)
            .exists()
        {
            return true;
        }
    }

    // $TERMINFO_DIRS (colon-separated list)
    if let Ok(dirs) = std::env::var("TERMINFO_DIRS") {
        for dir in dirs.split(':') {
            let dir = if dir.is_empty() {
                "/usr/share/terminfo"
            } else {
                dir
            };
            if std::path::Path::new(dir)
                .join(&letter_dir)
                .join(name)
                .exists()
            {
                return true;
            }
        }
    }

    // $HOME/.terminfo
    if let Ok(home) = std::env::var("HOME") {
        if std::path::Path::new(&home)
            .join(".terminfo")
            .join(&letter_dir)
            .join(name)
            .exists()
        {
            return true;
        }
    }

    // System default: letter subdirectory (Linux/BSD)
    if std::path::Path::new("/usr/share/terminfo")
        .join(&letter_dir)
        .join(name)
        .exists()
    {
        return true;
    }

    // System default: hex subdirectory (macOS)
    if std::path::Path::new("/usr/share/terminfo")
        .join(&hex_dir)
        .join(name)
        .exists()
    {
        return true;
    }

    false
}

/// Spawn a PTY with the given shell and size.
///
/// Returns the master PTY handle and the child process.
pub fn spawn_pty(
    shell: &str,
    size: &TerminalSize,
) -> anyhow::Result<(
    Box<dyn MasterPty + Send>,
    Box<dyn portable_pty::Child + Send + Sync>,
)> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: size.rows as u16,
        cols: size.cols as u16,
        pixel_width: (size.cols as f32 * size.cell_width) as u16,
        pixel_height: (size.rows as f32 * size.cell_height) as u16,
    })?;

    let mut cmd = CommandBuilder::new(shell);
    cmd.arg("-l"); // login shell

    // Set terminal environment variables.
    let term_name = if check_terminfo_available("xterm-crux") {
        "xterm-crux"
    } else {
        log::warn!("xterm-crux terminfo not found, falling back to xterm-256color");
        "xterm-256color"
    };
    cmd.env("TERM", term_name);
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "Crux");
    cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));

    let child = pair.slave.spawn_command(cmd)?;
    Ok((pair.master, child))
}

/// Start a background thread that reads PTY output and feeds it into the
/// alacritty_terminal parser, then signals wakeup.
///
/// The thread exits when the PTY reader returns EOF or an error.
pub fn start_pty_read_loop(
    term: Arc<FairMutex<Term<CruxEventListener>>>,
    mut reader: Box<dyn Read + Send>,
    wakeup: impl Fn() + Send + 'static,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("crux-pty-reader".into())
        .spawn(move || {
            let mut buf = [0u8; 0x1000]; // 4KB read buffer
            let mut parser: Processor = ansi::Processor::new();
            let mut pending_bytes: usize = 0;
            let mut last_wakeup = std::time::Instant::now();

            /// Maximum time between wakeup notifications.
            const BATCH_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(4);
            /// Maximum bytes to accumulate before forcing a wakeup.
            const BATCH_MAX_BYTES: usize = 4096;

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        {
                            let mut term = term.lock();
                            parser.advance(&mut *term, &buf[..n]);
                        }
                        pending_bytes += n;

                        // Batch wakeup: flush after timeout or byte threshold.
                        let now = std::time::Instant::now();
                        if now.duration_since(last_wakeup) >= BATCH_TIMEOUT
                            || pending_bytes >= BATCH_MAX_BYTES
                        {
                            wakeup();
                            last_wakeup = now;
                            pending_bytes = 0;
                        }
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::Interrupted {
                            break;
                        }
                    }
                }
            }

            // Final wakeup for any remaining buffered data.
            if pending_bytes > 0 {
                wakeup();
            }
        })
        .expect("failed to spawn PTY reader thread")
}

/// Detect the user's default shell.
///
/// Priority order:
/// 1. $SHELL environment variable
/// 2. macOS dscl UserShell lookup
/// 3. /bin/zsh fallback
pub fn detect_shell() -> String {
    // Try $SHELL environment variable
    if let Ok(shell) = std::env::var("SHELL") {
        if !shell.is_empty() {
            return shell;
        }
    }

    // Try macOS dscl lookup
    #[cfg(target_os = "macos")]
    {
        if let Ok(username) = std::env::var("USER") {
            if let Ok(output) = std::process::Command::new("dscl")
                .args([".", "-read", &format!("/Users/{}", username), "UserShell"])
                .output()
            {
                if output.status.success() {
                    if let Ok(stdout) = String::from_utf8(output.stdout) {
                        // Output format: "UserShell: /bin/zsh"
                        if let Some(shell) = stdout.split_whitespace().nth(1) {
                            return shell.to_string();
                        }
                    }
                }
            }
        }
    }

    // Final fallback
    "/bin/zsh".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell_returns_nonempty() {
        let shell = detect_shell();
        assert!(
            !shell.is_empty(),
            "detect_shell should return a non-empty string"
        );
    }

    #[test]
    fn test_detect_shell_returns_absolute_path() {
        let shell = detect_shell();
        assert!(
            shell.starts_with('/'),
            "shell path should be absolute: {}",
            shell
        );
    }

    #[test]
    fn test_check_terminfo_xterm_256color_exists() {
        // xterm-256color should be present on virtually every system.
        assert!(
            check_terminfo_available("xterm-256color"),
            "xterm-256color terminfo should be available"
        );
    }

    #[test]
    fn test_check_terminfo_nonexistent() {
        assert!(
            !check_terminfo_available("nonexistent-terminal-xyz-999"),
            "nonexistent terminfo should not be found"
        );
    }

    #[test]
    fn test_check_terminfo_empty_name() {
        assert!(
            !check_terminfo_available(""),
            "empty name should return false"
        );
    }
}
