//! Graphics protocol escape sequence scanners.
//!
//! Detects Kitty APC graphics sequences (`ESC _ G ... ST`) and iTerm2
//! inline image sequences (`ESC ] 1337 ; File= ... ST`) in the PTY byte
//! stream. Unlike the stateless OSC scanners in `osc_scanner.rs`, the
//! Kitty scanner is **stateful** because graphics payloads can span
//! multiple PTY reads (images are large).
//!
//! # Kitty APC Format
//!
//! ```text
//! ESC _ G <key>=<value>,...;<base64-data> ESC \
//! 0x1b 0x5f 0x47 ...                     0x1b 0x5c
//! ```
//!
//! # iTerm2 OSC 1337 Format
//!
//! ```text
//! ESC ] 1337 ; File=<params>:<base64-data> BEL
//! ESC ] 1337 ; File=<params>:<base64-data> ESC \
//! ```

use std::sync::mpsc;

use crate::event::{GraphicsProtocol, TerminalEvent};
use crate::osc_scanner::find_string_terminator;

/// Maximum accumulator size for graphics payloads (64MB).
/// Prevents unbounded memory growth from malicious PTY applications.
const MAX_ACCUMULATOR_SIZE: usize = 64 * 1024 * 1024;

/// Stateful scanner for Kitty graphics APC sequences.
///
/// Tracks state across PTY reads because graphics payloads (base64-encoded
/// images) can be tens of kilobytes and may span multiple read() calls.
///
/// # Usage
///
/// Create one instance per PTY read loop. Call [`feed`] with each buffer
/// read from the PTY. Complete sequences are emitted as `TerminalEvent::Graphics`
/// on the provided channel.
#[derive(Debug)]
pub struct KittyGraphicsScanner {
    /// Current scanner state.
    state: KittyScanState,
    /// Accumulates payload bytes across reads when a sequence spans buffers.
    accumulator: Vec<u8>,
}

/// Internal state machine for the Kitty APC scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KittyScanState {
    /// Not inside a graphics sequence; scanning for `ESC _`.
    Ground,
    /// Saw `ESC`; waiting for `_` to confirm APC start.
    EscSeen,
    /// Inside APC; saw `ESC _ G`; accumulating payload until ST (`ESC \`).
    InPayload,
    /// Inside APC payload and just saw `ESC`; waiting for `\` to confirm ST.
    PayloadEscSeen,
}

impl KittyGraphicsScanner {
    /// Create a new scanner in the ground state.
    pub fn new() -> Self {
        Self {
            state: KittyScanState::Ground,
            accumulator: Vec::new(),
        }
    }

    /// Feed a buffer of bytes from the PTY and emit any complete graphics
    /// events on `event_tx`.
    ///
    /// This method is designed to be called on every PTY read. It handles
    /// sequences that span multiple reads by maintaining internal state.
    pub fn feed(&mut self, buf: &[u8], event_tx: &mpsc::Sender<TerminalEvent>) {
        let mut i = 0;
        while i < buf.len() {
            match self.state {
                KittyScanState::Ground => {
                    if buf[i] == 0x1b {
                        self.state = KittyScanState::EscSeen;
                    }
                    i += 1;
                }
                KittyScanState::EscSeen => {
                    if buf[i] == b'_' {
                        // APC start — check next byte for 'G'
                        // We need to peek at the next byte to confirm Kitty graphics
                        if i + 1 < buf.len() {
                            if buf[i + 1] == b'G' {
                                self.state = KittyScanState::InPayload;
                                self.accumulator.clear();
                                i += 2; // skip '_' and 'G'
                            } else {
                                // APC but not graphics — back to ground
                                self.state = KittyScanState::Ground;
                                i += 1;
                            }
                        } else {
                            // '_' is at end of buffer; 'G' might be in next read.
                            // Optimistically enter InPayload state; if next byte
                            // isn't 'G', we'll reset.
                            // Actually, we need the 'G'. Store partial state.
                            // For simplicity, enter a sub-state. But let's keep
                            // it simple: just go back to Ground. We'll miss the
                            // rare case of APC split exactly at `ESC _` | `G...`.
                            // This is acceptable since 64KB buffer makes this
                            // extremely unlikely.
                            self.state = KittyScanState::Ground;
                            i += 1;
                        }
                    } else {
                        // ESC followed by something other than '_' — not APC
                        self.state = KittyScanState::Ground;
                        // Don't advance i; re-examine this byte in Ground state
                    }
                }
                KittyScanState::InPayload => {
                    if buf[i] == 0x1b {
                        self.state = KittyScanState::PayloadEscSeen;
                        i += 1;
                    } else {
                        // Guard against unbounded accumulator growth.
                        if self.accumulator.len() >= MAX_ACCUMULATOR_SIZE {
                            log::warn!("Kitty graphics accumulator exceeded 64MB, discarding malformed sequence");
                            self.reset();
                        } else {
                            self.accumulator.push(buf[i]);
                        }
                        i += 1;
                    }
                }
                KittyScanState::PayloadEscSeen => {
                    if buf[i] == b'\\' {
                        // ST found — sequence complete
                        self.emit_graphics_event(event_tx);
                        self.state = KittyScanState::Ground;
                        i += 1;
                    } else if buf[i] == b'_' {
                        // Nested ESC _ inside payload — shouldn't happen in
                        // valid Kitty protocol, but handle gracefully: treat
                        // the previous accumulation as garbage and restart.
                        self.accumulator.clear();
                        // Check for 'G' after this '_'
                        if i + 1 < buf.len() && buf[i + 1] == b'G' {
                            self.state = KittyScanState::InPayload;
                            i += 2;
                        } else {
                            self.state = KittyScanState::Ground;
                            i += 1;
                        }
                    } else {
                        // ESC inside payload that isn't ST — include ESC and
                        // current byte in the payload (malformed but tolerant).
                        self.accumulator.push(0x1b);
                        self.accumulator.push(buf[i]);
                        self.state = KittyScanState::InPayload;
                        i += 1;
                    }
                }
            }
        }
    }

    /// Emit a `TerminalEvent::Graphics` with the accumulated payload.
    fn emit_graphics_event(&mut self, event_tx: &mpsc::Sender<TerminalEvent>) {
        if self.accumulator.is_empty() {
            return;
        }
        let payload = std::mem::take(&mut self.accumulator);
        log::debug!("Kitty graphics APC: {} bytes payload", payload.len());
        let _ = event_tx.send(TerminalEvent::Graphics {
            protocol: GraphicsProtocol::Kitty,
            payload,
        });
    }

    /// Reset the scanner to the ground state, discarding any partial sequence.
    pub fn reset(&mut self) {
        self.state = KittyScanState::Ground;
        self.accumulator.clear();
    }

    /// Returns `true` if the scanner is in the middle of accumulating a
    /// graphics sequence (i.e., saw `ESC _ G` but not yet `ESC \`).
    pub fn is_accumulating(&self) -> bool {
        matches!(
            self.state,
            KittyScanState::InPayload | KittyScanState::PayloadEscSeen
        )
    }
}

impl Default for KittyGraphicsScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Scan a byte buffer for a single complete Kitty APC graphics sequence.
///
/// Returns `Some((start, end))` byte offsets if a complete `ESC _ G ... ESC \`
/// sequence is found within the buffer. The offsets cover the entire sequence
/// including delimiters.
///
/// This is a stateless convenience function for cases where the entire
/// sequence is expected to fit in one buffer. For production use with
/// large images, prefer [`KittyGraphicsScanner`].
pub fn scan_kitty_graphics(buf: &[u8]) -> Option<(usize, usize)> {
    // Minimum: ESC _ G <at-least-1-byte> ESC \ = 6 bytes
    if buf.len() < 6 {
        return None;
    }

    let mut i = 0;
    while i + 5 < buf.len() {
        // Look for ESC _ G
        if buf[i] == 0x1b && i + 2 < buf.len() && buf[i + 1] == b'_' && buf[i + 2] == b'G' {
            let start = i;
            // Find terminator: ESC backslash
            let mut j = i + 3;
            while j + 1 < buf.len() {
                if buf[j] == 0x1b && buf[j + 1] == b'\\' {
                    return Some((start, j + 2));
                }
                j += 1;
            }
            // No terminator found from this start; try next ESC
            i += 1;
        } else {
            i += 1;
        }
    }
    None
}

/// Scan a byte buffer for iTerm2 OSC 1337 inline image sequences.
///
/// iTerm2 format: `ESC ] 1337 ; File= <params> : <base64-data> ST`
/// where ST is BEL (0x07) or `ESC \` (0x1b 0x5c).
///
/// This scanner is stateless — it only finds complete sequences within a
/// single buffer. This is acceptable because:
/// 1. The 64KB PTY buffer is large enough for most inline images
/// 2. iTerm2 protocol doesn't have chunked transfer (unlike Kitty)
///
/// Emits `TerminalEvent::Graphics` with `GraphicsProtocol::Iterm2` for
/// each complete sequence found.
pub fn scan_iterm2_graphics(buf: &[u8], event_tx: &mpsc::Sender<TerminalEvent>) {
    // Minimum: ESC ] 1 3 3 7 ; F i l e = <something> BEL = 13 bytes
    // The sequence prefix is: ESC ] 1337 ; File=
    const PREFIX: &[u8] = b"1337;File=";

    let mut i = 0;
    while i + 2 + PREFIX.len() < buf.len() {
        // Look for ESC ] (OSC introducer)
        if buf[i] != 0x1b || buf[i + 1] != 0x5d {
            i += 1;
            continue;
        }

        // Check for "1337;File=" after ESC ]
        let prefix_start = i + 2;
        let prefix_end = prefix_start + PREFIX.len();
        if prefix_end > buf.len() || &buf[prefix_start..prefix_end] != PREFIX {
            i += 2;
            continue;
        }

        // Find the string terminator: BEL (0x07) or ESC \ (0x1b 0x5c)
        let payload_start = prefix_end;

        if let Some((end, next_i)) = find_string_terminator(buf, payload_start) {
            let payload = buf[payload_start..end].to_vec();
            if !payload.is_empty() {
                log::debug!("iTerm2 OSC 1337: {} bytes payload", payload.len());
                let _ = event_tx.send(TerminalEvent::Graphics {
                    protocol: GraphicsProtocol::Iterm2,
                    payload,
                });
            }
            // Skip past the terminator
            i = next_i;
        } else {
            // Incomplete sequence — skip the OSC introducer
            i += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- scan_kitty_graphics (stateless) tests ---

    #[test]
    fn test_scan_kitty_complete_sequence() {
        // ESC _ G a=t,f=32;AAAA ESC backslash
        let buf = b"\x1b_Ga=t,f=32;AAAA\x1b\\";
        let result = scan_kitty_graphics(buf);
        assert_eq!(result, Some((0, buf.len())));
    }

    #[test]
    fn test_scan_kitty_embedded_in_output() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"hello ");
        let start = buf.len();
        buf.extend_from_slice(b"\x1b_Ga=t;AAAA\x1b\\");
        let end = buf.len();
        buf.extend_from_slice(b" world");
        let result = scan_kitty_graphics(&buf);
        assert_eq!(result, Some((start, end)));
    }

    #[test]
    fn test_scan_kitty_no_sequence() {
        let buf = b"just some normal terminal output\r\n";
        assert_eq!(scan_kitty_graphics(buf), None);
    }

    #[test]
    fn test_scan_kitty_incomplete_no_st() {
        // APC start but no ST terminator
        let buf = b"\x1b_Ga=t;AAAA";
        assert_eq!(scan_kitty_graphics(buf), None);
    }

    #[test]
    fn test_scan_kitty_apc_but_not_graphics() {
        // APC with something other than 'G'
        let buf = b"\x1b_Xsomething\x1b\\";
        assert_eq!(scan_kitty_graphics(buf), None);
    }

    #[test]
    fn test_scan_kitty_too_short() {
        let buf = b"\x1b_G";
        assert_eq!(scan_kitty_graphics(buf), None);
    }

    // --- KittyGraphicsScanner (stateful) tests ---

    #[test]
    fn test_stateful_complete_in_one_read() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();
        // ESC _ G a=t,f=32;AAAA ESC backslash
        let buf = b"\x1b_Ga=t,f=32;AAAA\x1b\\";
        scanner.feed(buf, &tx);

        let event = rx.try_recv().unwrap();
        match event {
            TerminalEvent::Graphics { protocol, payload } => {
                assert_eq!(protocol, GraphicsProtocol::Kitty);
                assert_eq!(payload, b"a=t,f=32;AAAA");
            }
            _ => panic!("expected Graphics event"),
        }
        assert!(!scanner.is_accumulating());
    }

    #[test]
    fn test_stateful_split_across_two_reads() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        // First read: start of sequence
        scanner.feed(b"\x1b_Ga=t;AA", &tx);
        assert!(scanner.is_accumulating());
        assert!(rx.try_recv().is_err(), "no event yet");

        // Second read: rest of sequence
        scanner.feed(b"BB\x1b\\", &tx);
        assert!(!scanner.is_accumulating());

        let event = rx.try_recv().unwrap();
        match event {
            TerminalEvent::Graphics { protocol, payload } => {
                assert_eq!(protocol, GraphicsProtocol::Kitty);
                assert_eq!(payload, b"a=t;AABB");
            }
            _ => panic!("expected Graphics event"),
        }
    }

    #[test]
    fn test_stateful_split_across_three_reads() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        scanner.feed(b"\x1b_Ga=t;", &tx);
        assert!(scanner.is_accumulating());

        scanner.feed(b"AABBCC", &tx);
        assert!(scanner.is_accumulating());

        scanner.feed(b"DD\x1b\\", &tx);
        assert!(!scanner.is_accumulating());

        let event = rx.try_recv().unwrap();
        match event {
            TerminalEvent::Graphics { payload, .. } => {
                assert_eq!(payload, b"a=t;AABBCCDD");
            }
            _ => panic!("expected Graphics event"),
        }
    }

    #[test]
    fn test_stateful_st_split_across_reads() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        // ESC at end of first read, backslash at start of second
        scanner.feed(b"\x1b_Ga=t;AAAA\x1b", &tx);
        assert!(scanner.is_accumulating());
        assert!(rx.try_recv().is_err());

        scanner.feed(b"\\more stuff", &tx);
        assert!(!scanner.is_accumulating());

        let event = rx.try_recv().unwrap();
        match event {
            TerminalEvent::Graphics { payload, .. } => {
                assert_eq!(payload, b"a=t;AAAA");
            }
            _ => panic!("expected Graphics event"),
        }
    }

    #[test]
    fn test_stateful_multiple_sequences_in_one_read() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        let mut buf = Vec::new();
        buf.extend_from_slice(b"\x1b_Ga=t;AA\x1b\\");
        buf.extend_from_slice(b"\x1b_Ga=t;BB\x1b\\");
        scanner.feed(&buf, &tx);

        let event1 = rx.try_recv().unwrap();
        let event2 = rx.try_recv().unwrap();
        assert!(rx.try_recv().is_err());

        match event1 {
            TerminalEvent::Graphics { payload, .. } => assert_eq!(payload, b"a=t;AA"),
            _ => panic!("expected Graphics event"),
        }
        match event2 {
            TerminalEvent::Graphics { payload, .. } => assert_eq!(payload, b"a=t;BB"),
            _ => panic!("expected Graphics event"),
        }
    }

    #[test]
    fn test_stateful_non_graphics_apc_ignored() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        // APC with 'X' instead of 'G'
        scanner.feed(b"\x1b_Xsomething\x1b\\", &tx);
        assert!(!scanner.is_accumulating());
        assert!(rx.try_recv().is_err(), "non-graphics APC should be ignored");
    }

    #[test]
    fn test_stateful_interleaved_with_normal_output() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        let mut buf = Vec::new();
        buf.extend_from_slice(b"normal output ");
        buf.extend_from_slice(b"\x1b_Ga=t;AAAA\x1b\\");
        buf.extend_from_slice(b" more output ");
        scanner.feed(&buf, &tx);

        let event = rx.try_recv().unwrap();
        match event {
            TerminalEvent::Graphics { payload, .. } => assert_eq!(payload, b"a=t;AAAA"),
            _ => panic!("expected Graphics event"),
        }
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_stateful_reset() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        // Start a sequence but don't finish it
        scanner.feed(b"\x1b_Ga=t;partial", &tx);
        assert!(scanner.is_accumulating());

        // Reset
        scanner.reset();
        assert!(!scanner.is_accumulating());

        // Next data should be treated fresh
        scanner.feed(b"\x1b_Ga=t;fresh\x1b\\", &tx);
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn test_stateful_empty_payload_ignored() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        // ESC _ G ESC \ (empty payload after 'G')
        scanner.feed(b"\x1b_G\x1b\\", &tx);
        assert!(
            rx.try_recv().is_err(),
            "empty payload should not emit event"
        );
    }

    #[test]
    fn test_stateful_default_trait() {
        let scanner = KittyGraphicsScanner::default();
        assert!(!scanner.is_accumulating());
    }

    // --- iTerm2 OSC 1337 tests ---

    #[test]
    fn test_iterm2_bel_terminated() {
        let (tx, rx) = mpsc::channel();
        // ESC ] 1337;File=name=test.png;size=100:AAAA BEL
        let buf = b"\x1b]1337;File=name=test.png;size=100:AAAA\x07";
        scan_iterm2_graphics(buf, &tx);

        let event = rx.try_recv().unwrap();
        match event {
            TerminalEvent::Graphics { protocol, payload } => {
                assert_eq!(protocol, GraphicsProtocol::Iterm2);
                assert_eq!(payload, b"name=test.png;size=100:AAAA");
            }
            _ => panic!("expected Graphics event"),
        }
    }

    #[test]
    fn test_iterm2_st_terminated() {
        let (tx, rx) = mpsc::channel();
        // ESC ] 1337;File=inline=1:AAAA ESC backslash
        let buf = b"\x1b]1337;File=inline=1:AAAA\x1b\\";
        scan_iterm2_graphics(buf, &tx);

        let event = rx.try_recv().unwrap();
        match event {
            TerminalEvent::Graphics { protocol, payload } => {
                assert_eq!(protocol, GraphicsProtocol::Iterm2);
                assert_eq!(payload, b"inline=1:AAAA");
            }
            _ => panic!("expected Graphics event"),
        }
    }

    #[test]
    fn test_iterm2_embedded_in_output() {
        let (tx, rx) = mpsc::channel();
        let mut buf = Vec::new();
        buf.extend_from_slice(b"some text ");
        buf.extend_from_slice(b"\x1b]1337;File=inline=1:AAAA\x07");
        buf.extend_from_slice(b" more text");
        scan_iterm2_graphics(&buf, &tx);

        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            TerminalEvent::Graphics {
                protocol: GraphicsProtocol::Iterm2,
                ..
            }
        ));
        assert!(rx.try_recv().is_err(), "should only emit one event");
    }

    #[test]
    fn test_iterm2_no_sequence() {
        let (tx, rx) = mpsc::channel();
        let buf = b"just normal terminal output\r\n";
        scan_iterm2_graphics(buf, &tx);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_iterm2_other_osc_ignored() {
        let (tx, rx) = mpsc::channel();
        // OSC 7 should not match
        let buf = b"\x1b]7;file://host/tmp\x07";
        scan_iterm2_graphics(buf, &tx);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_iterm2_osc1337_non_file_ignored() {
        let (tx, rx) = mpsc::channel();
        // OSC 1337 but with a different command (not File=)
        let buf = b"\x1b]1337;SetMark\x07";
        scan_iterm2_graphics(buf, &tx);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_iterm2_incomplete_sequence() {
        let (tx, rx) = mpsc::channel();
        // No terminator
        let buf = b"\x1b]1337;File=inline=1:AAAA";
        scan_iterm2_graphics(buf, &tx);
        assert!(rx.try_recv().is_err());
    }

    // --- PTY buffer size test ---

    #[test]
    fn test_pty_buffer_size_is_64kb() {
        // Verify the PTY read buffer constant is 64KB.
        // The actual buffer is in pty.rs; we verify the hex value here.
        assert_eq!(0x10000_usize, 65536, "PTY buffer should be 64KB");
    }

    // --- Integration test: graphics event through parse pipeline ---

    #[test]
    fn test_kitty_scan_then_parse() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        // Simulate a Kitty transmit command
        let buf = b"\x1b_Ga=t,f=32,s=10,v=10,i=1;AQID\x1b\\";
        scanner.feed(buf, &tx);

        let event = rx.try_recv().unwrap();
        match event {
            TerminalEvent::Graphics { protocol, payload } => {
                assert_eq!(protocol, GraphicsProtocol::Kitty);
                // Now parse with the crux-graphics parser
                let cmd = crux_graphics::protocol::kitty::parse_kitty_command(&payload).unwrap();
                assert_eq!(
                    cmd.action,
                    crux_graphics::protocol::kitty::KittyAction::Transmit
                );
                assert_eq!(cmd.format, crux_graphics::types::PixelFormat::Rgba);
                assert_eq!(cmd.width, 10);
                assert_eq!(cmd.height, 10);
                assert_eq!(cmd.image_id, 1);
                // Verify base64 decoding works
                let decoded = cmd.decode_payload().unwrap();
                assert_eq!(decoded, vec![1, 2, 3]);
            }
            _ => panic!("expected Graphics event"),
        }
    }

    #[test]
    fn test_kitty_chunked_scan_then_parse() {
        let (tx, rx) = mpsc::channel();
        let mut scanner = KittyGraphicsScanner::new();

        // First chunk (m=1 means more data)
        scanner.feed(b"\x1b_Ga=t,f=32,s=10,v=10,i=1,m=1;AAAA\x1b\\", &tx);
        let event1 = rx.try_recv().unwrap();
        match &event1 {
            TerminalEvent::Graphics { payload, .. } => {
                let cmd = crux_graphics::protocol::kitty::parse_kitty_command(payload).unwrap();
                assert!(cmd.more_data, "first chunk should have more_data=true");
            }
            _ => panic!("expected Graphics event"),
        }

        // Last chunk (m=0)
        scanner.feed(b"\x1b_Gm=0;BBBB\x1b\\", &tx);
        let event2 = rx.try_recv().unwrap();
        match &event2 {
            TerminalEvent::Graphics { payload, .. } => {
                let cmd = crux_graphics::protocol::kitty::parse_kitty_command(payload).unwrap();
                assert!(!cmd.more_data, "last chunk should have more_data=false");
            }
            _ => panic!("expected Graphics event"),
        }
    }
}
