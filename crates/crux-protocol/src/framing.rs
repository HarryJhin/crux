//! Length-prefix framing for IPC message transport.

use std::fmt;

/// Maximum frame payload size (16 MB).
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Errors that can occur during frame encoding/decoding.
#[derive(Debug)]
pub enum FrameError {
    /// The message exceeds [`MAX_FRAME_SIZE`].
    MessageTooLarge(usize),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::MessageTooLarge(size) => {
                write!(f, "message too large: {size} bytes (max {MAX_FRAME_SIZE})")
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Encode a message with a 4-byte big-endian length prefix.
pub fn encode_frame(msg: &[u8]) -> Result<Vec<u8>, FrameError> {
    let len: u32 = msg
        .len()
        .try_into()
        .map_err(|_| FrameError::MessageTooLarge(msg.len()))?;
    if msg.len() > MAX_FRAME_SIZE {
        return Err(FrameError::MessageTooLarge(msg.len()));
    }
    let mut frame = Vec::with_capacity(4 + msg.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(msg);
    Ok(frame)
}

/// Decode a frame from a buffer.
///
/// Returns `Ok(Some((total_consumed_bytes, payload)))` if a complete frame is
/// available, `Ok(None)` if the buffer is incomplete, or `Err` if the frame
/// exceeds the size limit.
pub fn decode_frame(buf: &[u8]) -> Result<Option<(usize, Vec<u8>)>, FrameError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_FRAME_SIZE {
        return Err(FrameError::MessageTooLarge(len));
    }
    if buf.len() < 4 + len {
        return Ok(None);
    }
    Ok(Some((4 + len, buf[4..4 + len].to_vec())))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trip() {
        let payload = b"hello world";
        let frame = encode_frame(payload).expect("encode");
        let (consumed, decoded) = decode_frame(&frame)
            .expect("no error")
            .expect("should decode");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, payload);
    }

    #[test]
    fn frame_decode_incomplete_header() {
        assert!(decode_frame(&[0x00, 0x00]).unwrap().is_none());
    }

    #[test]
    fn frame_decode_incomplete_payload() {
        let frame = encode_frame(b"hello").expect("encode");
        // Chop off the last byte so payload is incomplete.
        assert!(decode_frame(&frame[..frame.len() - 1]).unwrap().is_none());
    }

    #[test]
    fn frame_rejects_oversized() {
        // Craft a header claiming a payload larger than MAX_FRAME_SIZE.
        let huge_len = (MAX_FRAME_SIZE + 1) as u32;
        let mut buf = huge_len.to_be_bytes().to_vec();
        buf.push(0); // at least one byte so header is complete
        assert!(decode_frame(&buf).is_err());
    }
}
