use std::io::Read;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;

use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{self, Processor};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use crate::event::{CruxEventListener, TerminalEvent};
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

/// Extract the directory path from an OSC 7 URI payload.
///
/// OSC 7 format: `file://hostname/path` or `file:///path`.
/// Returns `None` if the URI is not a valid `file://` URL.
/// Percent-encoded characters (e.g. `%20`) are decoded.
fn parse_osc7_uri(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;

    // Skip the hostname — the path starts at the next '/'.
    let path_start = rest.find('/')?;
    let encoded_path = &rest[path_start..];

    // Percent-decode the path.
    let mut decoded = Vec::with_capacity(encoded_path.len());
    let bytes = encoded_path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                decoded.push(byte);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(decoded).ok()
}

/// Scan a byte buffer for OSC 7 sequences and emit `CwdChanged` events.
///
/// OSC 7 is: `ESC ] 7 ; <uri> ST` where ST is `ESC \` or `BEL` (0x07).
/// This scanner is stateless per call — it only finds complete sequences
/// within a single buffer. Sequences split across reads are missed, which
/// is acceptable since OSC 7 payloads are short (~80 bytes) and the PTY
/// read buffer is 4KB.
fn scan_osc7(buf: &[u8], event_tx: &mpsc::Sender<TerminalEvent>) {
    // OSC introducer: ESC ] (0x1b 0x5d)
    let mut i = 0;
    while i + 4 < buf.len() {
        // Look for ESC ]
        if buf[i] != 0x1b || buf[i + 1] != 0x5d {
            i += 1;
            continue;
        }

        // Check for "7;" after ESC ]
        if buf[i + 2] != b'7' || buf[i + 3] != b';' {
            i += 2;
            continue;
        }

        // Find the string terminator: BEL (0x07) or ESC \ (0x1b 0x5c).
        let payload_start = i + 4;
        let mut end = payload_start;
        let mut found = false;
        while end < buf.len() {
            if buf[end] == 0x07 {
                found = true;
                break;
            }
            if buf[end] == 0x1b && end + 1 < buf.len() && buf[end + 1] == 0x5c {
                found = true;
                break;
            }
            end += 1;
        }

        if found {
            if let Ok(uri) = std::str::from_utf8(&buf[payload_start..end]) {
                if let Some(path) = parse_osc7_uri(uri) {
                    log::debug!("OSC 7 CWD: {}", path);
                    let _ = event_tx.send(TerminalEvent::CwdChanged(path));
                }
            }
            // Skip past the terminator.
            i = if buf[end] == 0x07 { end + 1 } else { end + 2 };
        } else {
            // Incomplete sequence — skip the ESC ] and continue.
            i += 2;
        }
    }
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
                        // Scan for OSC 7 before feeding to the VTE parser.
                        // alacritty_terminal does not handle OSC 7, so we
                        // intercept it here. The VTE parser will log it as
                        // "unhandled osc_dispatch" but otherwise ignore it.
                        scan_osc7(&buf[..n], &event_tx);

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

    #[test]
    fn test_parse_osc7_uri_basic() {
        let result = parse_osc7_uri("file://hostname/Users/jjh/Projects");
        assert_eq!(result, Some("/Users/jjh/Projects".to_string()));
    }

    #[test]
    fn test_parse_osc7_uri_empty_hostname() {
        let result = parse_osc7_uri("file:///home/user/code");
        assert_eq!(result, Some("/home/user/code".to_string()));
    }

    #[test]
    fn test_parse_osc7_uri_percent_encoded() {
        let result = parse_osc7_uri("file://host/Users/jjh/My%20Documents");
        assert_eq!(result, Some("/Users/jjh/My Documents".to_string()));
    }

    #[test]
    fn test_parse_osc7_uri_not_file_scheme() {
        assert_eq!(parse_osc7_uri("http://example.com/path"), None);
    }

    #[test]
    fn test_parse_osc7_uri_no_path() {
        assert_eq!(parse_osc7_uri("file://hostname"), None);
    }

    #[test]
    fn test_parse_osc7_uri_root_path() {
        let result = parse_osc7_uri("file://localhost/");
        assert_eq!(result, Some("/".to_string()));
    }

    #[test]
    fn test_scan_osc7_bel_terminated() {
        let (tx, rx) = mpsc::channel();
        // ESC ] 7 ; file://host/tmp BEL
        let buf = b"\x1b]7;file://host/tmp\x07";
        scan_osc7(buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, TerminalEvent::CwdChanged(p) if p == "/tmp"));
    }

    #[test]
    fn test_scan_osc7_st_terminated() {
        let (tx, rx) = mpsc::channel();
        // ESC ] 7 ; file:///home/user ESC backslash
        let buf = b"\x1b]7;file:///home/user\x1b\\";
        scan_osc7(buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, TerminalEvent::CwdChanged(p) if p == "/home/user"));
    }

    #[test]
    fn test_scan_osc7_embedded_in_other_output() {
        let (tx, rx) = mpsc::channel();
        // Some text, then OSC 7, then more text.
        let mut buf = Vec::new();
        buf.extend_from_slice(b"hello world ");
        buf.extend_from_slice(b"\x1b]7;file://host/Users/jjh\x07");
        buf.extend_from_slice(b" more text");
        scan_osc7(&buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, TerminalEvent::CwdChanged(p) if p == "/Users/jjh"));
        assert!(rx.try_recv().is_err(), "should only emit one event");
    }

    #[test]
    fn test_scan_osc7_no_osc7_present() {
        let (tx, rx) = mpsc::channel();
        let buf = b"just some normal terminal output\r\n";
        scan_osc7(buf, &tx);
        assert!(rx.try_recv().is_err(), "no events should be emitted");
    }

    #[test]
    fn test_scan_osc7_other_osc_ignored() {
        let (tx, rx) = mpsc::channel();
        // OSC 0 (set title) should not trigger CwdChanged.
        let buf = b"\x1b]0;my title\x07";
        scan_osc7(buf, &tx);
        assert!(rx.try_recv().is_err(), "OSC 0 should not emit CwdChanged");
    }
}
