//! OSC sequence scanners for PTY output.
//!
//! Scans raw byte buffers for OSC 7 (CWD change) and OSC 133 (FinalTerm
//! shell integration) sequences that `alacritty_terminal` does not handle
//! natively. These scanners run before the VTE parser so we can intercept
//! and emit the appropriate `TerminalEvent`s.

use std::sync::mpsc;

use crate::event::{SemanticZoneType, TerminalEvent};

/// Extract the directory path from an OSC 7 URI payload.
///
/// OSC 7 format: `file://hostname/path` or `file:///path`.
/// Returns `None` if the URI is not a valid `file://` URL.
/// Percent-encoded characters (e.g. `%20`) are decoded.
pub(crate) fn parse_osc7_uri(uri: &str) -> Option<String> {
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
pub(crate) fn scan_osc7(buf: &[u8], event_tx: &mpsc::Sender<TerminalEvent>) {
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

/// Scan a byte buffer for OSC 133 (FinalTerm) prompt-marking sequences.
///
/// OSC 133 markers:
///   `ESC ] 133 ; A ST` — Prompt start
///   `ESC ] 133 ; B ST` — Command start (user pressed Enter)
///   `ESC ] 133 ; C ST` — Output start
///   `ESC ] 133 ; D ST` — Command complete (optionally `133;D;N` with exit code)
///
/// ST is BEL (0x07) or ESC \ (0x1b 0x5c).
///
/// Like `scan_osc7`, this is stateless per call — sequences split across
/// reads are missed, which is acceptable given the short payload size.
pub(crate) fn scan_osc133(buf: &[u8], event_tx: &mpsc::Sender<TerminalEvent>) {
    // OSC introducer: ESC ] (0x1b 0x5d)
    // Minimum sequence: ESC ] 1 3 3 ; A BEL = 7 bytes
    let mut i = 0;
    while i + 6 < buf.len() {
        // Look for ESC ]
        if buf[i] != 0x1b || buf[i + 1] != 0x5d {
            i += 1;
            continue;
        }

        // Check for "133;" after ESC ]
        if i + 5 >= buf.len()
            || buf[i + 2] != b'1'
            || buf[i + 3] != b'3'
            || buf[i + 4] != b'3'
            || buf[i + 5] != b';'
        {
            i += 2;
            continue;
        }

        // Read the payload after "133;" (at least the marker letter).
        let payload_start = i + 6;
        if payload_start >= buf.len() {
            break;
        }

        // Find the string terminator: BEL (0x07) or ESC \ (0x1b 0x5c).
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
            if let Ok(payload) = std::str::from_utf8(&buf[payload_start..end]) {
                let event = match payload.as_bytes().first() {
                    Some(b'A') => Some(TerminalEvent::PromptMark {
                        mark: SemanticZoneType::Prompt,
                        exit_code: None,
                    }),
                    Some(b'B') => Some(TerminalEvent::PromptMark {
                        mark: SemanticZoneType::Input,
                        exit_code: None,
                    }),
                    Some(b'C') => Some(TerminalEvent::PromptMark {
                        mark: SemanticZoneType::Output,
                        exit_code: None,
                    }),
                    Some(b'D') => {
                        // Parse optional exit code: "D" or "D;N"
                        let exit_code = payload
                            .strip_prefix("D;")
                            .and_then(|s| s.parse::<i32>().ok());
                        Some(TerminalEvent::PromptMark {
                            mark: SemanticZoneType::Output,
                            exit_code,
                        })
                    }
                    _ => None,
                };
                if let Some(event) = event {
                    log::debug!("OSC 133: {}", payload);
                    let _ = event_tx.send(event);
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

#[cfg(test)]
mod tests {
    use super::*;

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

    // --- OSC 133 tests ---

    #[test]
    fn test_scan_osc133_prompt_start_bel() {
        let (tx, rx) = mpsc::channel();
        let buf = b"\x1b]133;A\x07";
        scan_osc133(buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Prompt,
                exit_code: None,
            }
        ));
    }

    #[test]
    fn test_scan_osc133_prompt_start_st() {
        let (tx, rx) = mpsc::channel();
        // ESC ] 133;A ESC backslash
        let buf = b"\x1b]133;A\x1b\\";
        scan_osc133(buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Prompt,
                exit_code: None,
            }
        ));
    }

    #[test]
    fn test_scan_osc133_command_start() {
        let (tx, rx) = mpsc::channel();
        let buf = b"\x1b]133;B\x07";
        scan_osc133(buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Input,
                exit_code: None,
            }
        ));
    }

    #[test]
    fn test_scan_osc133_output_start() {
        let (tx, rx) = mpsc::channel();
        let buf = b"\x1b]133;C\x07";
        scan_osc133(buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Output,
                exit_code: None,
            }
        ));
    }

    #[test]
    fn test_scan_osc133_command_complete_no_exit_code() {
        let (tx, rx) = mpsc::channel();
        let buf = b"\x1b]133;D\x07";
        scan_osc133(buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Output,
                exit_code: None,
            }
        ));
    }

    #[test]
    fn test_scan_osc133_command_complete_with_exit_code() {
        let (tx, rx) = mpsc::channel();
        let buf = b"\x1b]133;D;0\x07";
        scan_osc133(buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Output,
                exit_code: Some(0),
            }
        ));
    }

    #[test]
    fn test_scan_osc133_command_complete_nonzero_exit() {
        let (tx, rx) = mpsc::channel();
        let buf = b"\x1b]133;D;127\x07";
        scan_osc133(buf, &tx);
        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Output,
                exit_code: Some(127),
            }
        ));
    }

    #[test]
    fn test_scan_osc133_embedded_in_output() {
        let (tx, rx) = mpsc::channel();
        let mut buf = Vec::new();
        buf.extend_from_slice(b"some output ");
        buf.extend_from_slice(b"\x1b]133;A\x07");
        buf.extend_from_slice(b"$ ");
        buf.extend_from_slice(b"\x1b]133;B\x07");
        scan_osc133(&buf, &tx);
        let event1 = rx.try_recv().unwrap();
        assert!(matches!(
            event1,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Prompt,
                ..
            }
        ));
        let event2 = rx.try_recv().unwrap();
        assert!(matches!(
            event2,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Input,
                ..
            }
        ));
        assert!(rx.try_recv().is_err(), "should only emit two events");
    }

    #[test]
    fn test_scan_osc133_no_osc133_present() {
        let (tx, rx) = mpsc::channel();
        let buf = b"just some normal terminal output\r\n";
        scan_osc133(buf, &tx);
        assert!(rx.try_recv().is_err(), "no events should be emitted");
    }

    #[test]
    fn test_scan_osc133_other_osc_ignored() {
        let (tx, rx) = mpsc::channel();
        // OSC 7 should not trigger PromptMark.
        let buf = b"\x1b]7;file://host/tmp\x07";
        scan_osc133(buf, &tx);
        assert!(rx.try_recv().is_err(), "OSC 7 should not emit PromptMark");
    }

    #[test]
    fn test_scan_osc133_full_prompt_cycle() {
        let (tx, rx) = mpsc::channel();
        // Simulate a full shell integration cycle: A B C D
        let mut buf = Vec::new();
        buf.extend_from_slice(b"\x1b]133;A\x07");
        buf.extend_from_slice(b"$ ");
        buf.extend_from_slice(b"\x1b]133;B\x07");
        buf.extend_from_slice(b"ls\r\n");
        buf.extend_from_slice(b"\x1b]133;C\x07");
        buf.extend_from_slice(b"file1 file2\r\n");
        buf.extend_from_slice(b"\x1b]133;D;0\x07");
        scan_osc133(&buf, &tx);

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert_eq!(events.len(), 4, "should emit 4 events for A/B/C/D");
        assert!(matches!(
            events[0],
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Prompt,
                ..
            }
        ));
        assert!(matches!(
            events[1],
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Input,
                ..
            }
        ));
        assert!(matches!(
            events[2],
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Output,
                exit_code: None,
            }
        ));
        assert!(matches!(
            events[3],
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Output,
                exit_code: Some(0),
            }
        ));
    }
}
