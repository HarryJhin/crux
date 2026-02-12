use std::io::Read;
use std::sync::Arc;
use std::thread::JoinHandle;

use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{self, Processor};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use crate::event::CruxEventListener;
use crate::TerminalSize;

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
    cmd.env("TERM", "xterm-crux");
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

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        {
                            let mut term = term.lock();
                            parser.advance(&mut *term, &buf[..n]);
                        }
                        wakeup();
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::Interrupted {
                            break;
                        }
                    }
                }
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
