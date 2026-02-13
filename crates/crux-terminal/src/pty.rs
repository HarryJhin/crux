use std::io::Read;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;

use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{self, Processor};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use crate::event::{CruxEventListener, TerminalEvent};
use crate::osc_scanner::{scan_osc133, scan_osc7};
use crate::TerminalSize;

/// Typed error for PTY spawn failures.
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("failed to open PTY pair: {0}")]
    OpenPty(#[source] anyhow::Error),
    #[error("failed to spawn command: {0}")]
    SpawnCommand(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("failed to get writer: {0}")]
    GetWriter(#[source] anyhow::Error),
}

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
        let base = std::path::Path::new(&dir);
        if base.join(&letter_dir).join(name).exists() || base.join(&hex_dir).join(name).exists() {
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
            let base = std::path::Path::new(dir);
            if base.join(&letter_dir).join(name).exists() || base.join(&hex_dir).join(name).exists()
            {
                return true;
            }
        }
    }

    // $HOME/.terminfo (check both letter and hex subdirectories)
    if let Ok(home) = std::env::var("HOME") {
        let user_terminfo = std::path::Path::new(&home).join(".terminfo");
        if user_terminfo.join(&letter_dir).join(name).exists()
            || user_terminfo.join(&hex_dir).join(name).exists()
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
    shell_args: &[String],
    size: &TerminalSize,
    cwd: Option<&str>,
    command: Option<&[String]>,
    env: Option<&std::collections::HashMap<String, String>>,
) -> anyhow::Result<(
    Box<dyn MasterPty + Send>,
    Box<dyn portable_pty::Child + Send + Sync>,
)> {
    // Guard against empty command slice to prevent panic.
    if let Some(args) = command {
        if args.is_empty() {
            return Err(anyhow::anyhow!("command must have at least one element"));
        }
    }

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: u16::try_from(size.rows).unwrap_or(u16::MAX),
        cols: u16::try_from(size.cols).unwrap_or(u16::MAX),
        pixel_width: u16::try_from((size.cols as f32 * size.cell_width) as usize)
            .unwrap_or(u16::MAX),
        pixel_height: u16::try_from((size.rows as f32 * size.cell_height) as usize)
            .unwrap_or(u16::MAX),
    })?;

    let mut cmd = if let Some(args) = command {
        // Run a specific command instead of the default shell.
        let mut builder = CommandBuilder::new(&args[0]);
        for arg in &args[1..] {
            builder.arg(arg);
        }
        builder
    } else {
        let mut builder = CommandBuilder::new(shell);
        // Use shell_args from config instead of hardcoded "-l"
        for arg in shell_args {
            builder.arg(arg);
        }
        builder
    };

    // Set working directory if specified.
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }

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

    // Set additional environment variables from params.
    if let Some(extra_env) = env {
        for (key, value) in extra_env {
            cmd.env(key, value);
        }
    }

    let child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave); // Must drop slave FD after spawn so reader gets EOF on child exit
    Ok((pair.master, child))
}

/// Start a background thread that reads PTY output and feeds it into the
/// alacritty_terminal parser, then signals wakeup.
///
/// The `event_tx` channel is used to emit events that alacritty_terminal
/// does not handle natively (e.g. OSC 7 CWD changes).
///
/// The thread exits when the PTY reader returns EOF or an error.
pub fn start_pty_read_loop(
    term: Arc<FairMutex<Term<CruxEventListener>>>,
    mut reader: Box<dyn Read + Send>,
    event_tx: mpsc::Sender<TerminalEvent>,
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
                        // Scan for OSC sequences before feeding to the VTE parser.
                        // alacritty_terminal does not handle OSC 7 or OSC 133,
                        // so we intercept them here. The VTE parser will log
                        // them as "unhandled osc_dispatch" but otherwise ignore them.
                        scan_osc7(&buf[..n], &event_tx);
                        scan_osc133(&buf[..n], &event_tx);

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
        .expect("failed to spawn PTY reader thread: system resource exhaustion or invalid thread configuration")
}

/// Ensure the xterm-crux terminfo is installed on the system.
///
/// This function:
/// 1. Checks if xterm-crux is already available
/// 2. If not, embeds the terminfo source and compiles it with `tic`
/// 3. Installs to `~/.terminfo/` (user-local directory)
/// 4. Verifies installation succeeded
///
/// Returns `true` if the terminfo is available after this call.
pub fn ensure_terminfo_installed() -> bool {
    // Check if already installed
    if check_terminfo_available("xterm-crux") {
        log::info!("xterm-crux terminfo already installed");
        return true;
    }

    log::info!("xterm-crux terminfo not found, installing...");

    // Embed the terminfo source from the repository
    const TERMINFO_SRC: &str = include_str!("../../../extra/crux.terminfo");

    // Create a temporary file for the terminfo source
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("crux-terminfo-{}.src", std::process::id()));

    // Write the terminfo source to the temp file
    if let Err(e) = std::fs::write(&temp_path, TERMINFO_SRC) {
        log::warn!("Failed to write terminfo source to temp file: {}", e);
        return false;
    }

    // Run tic to compile and install the terminfo
    let result = std::process::Command::new("tic")
        .args(["-x", "-e", "xterm-crux,crux,crux-direct"])
        .arg(&temp_path)
        .output();

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    match result {
        Ok(output) => {
            if output.status.success() {
                // Verify installation
                if check_terminfo_available("xterm-crux") {
                    log::info!("Successfully installed xterm-crux terminfo to ~/.terminfo/");
                    true
                } else {
                    log::warn!("tic succeeded but xterm-crux still not found");
                    false
                }
            } else {
                log::warn!(
                    "tic failed with exit code {:?}: {}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                );
                false
            }
        }
        Err(e) => {
            log::warn!("Failed to run tic command: {}", e);
            false
        }
    }
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

    #[test]
    fn test_ensure_terminfo_installed() {
        // This test verifies that ensure_terminfo_installed() returns true,
        // either because xterm-crux is already installed or because it
        // successfully installs it.
        assert!(
            ensure_terminfo_installed(),
            "ensure_terminfo_installed should return true after installation"
        );

        // Verify that xterm-crux is now available
        assert!(
            check_terminfo_available("xterm-crux"),
            "xterm-crux should be available after ensure_terminfo_installed"
        );
    }
}
