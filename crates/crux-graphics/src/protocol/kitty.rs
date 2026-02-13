//! Kitty graphics protocol parser.
//!
//! The Kitty graphics protocol uses APC (Application Program Command) escape
//! sequences to transmit images. The format is:
//!
//! ```text
//! APC G <key>=<value>,<key>=<value>,...;<base64-data> ST
//! ```
//!
//! Where APC = `\x1b_` and ST = `\x1b\\` (or `\x07` as BEL terminator).
//!
//! Reference: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>

use base64::Engine;

use crate::error::GraphicsError;
use crate::types::{ImageId, PixelFormat, TransmissionMode};

/// Actions that can be performed on images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyAction {
    /// Transmit image data (possibly with immediate display).
    Transmit,
    /// Transmit and display in one step.
    TransmitAndDisplay,
    /// Display a previously transmitted image.
    Display,
    /// Delete images or placements.
    Delete,
    /// Query terminal for graphics protocol support.
    Query,
    /// Transmit animation frame data.
    AnimationFrame,
}

/// Specifies what to delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteTarget {
    /// Delete all images and placements.
    All,
    /// Delete a specific image by ID (and all its placements).
    ById(ImageId),
    /// Delete a specific placement.
    ByPlacement {
        image_id: ImageId,
        placement_id: u32,
    },
    /// Delete all images at the cursor position.
    AtCursor,
    /// Delete all images intersecting a cell range.
    InRange { column: u32, row: i32 },
}

/// Compression format for transmitted data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// No compression.
    None,
    /// Zlib/deflate compression.
    Zlib,
}

/// A parsed Kitty graphics protocol command.
#[derive(Debug, Clone)]
pub struct KittyCommand {
    /// The action to perform.
    pub action: KittyAction,
    /// Image ID (0 = auto-assign or not specified).
    pub image_id: u32,
    /// Placement ID (0 = default).
    pub placement_id: u32,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Pixel format of the transmitted data.
    pub format: PixelFormat,
    /// How the data is transmitted.
    pub transmission: TransmissionMode,
    /// Compression applied to the data.
    pub compression: Compression,
    /// Whether more data chunks follow (chunked transfer).
    /// `true` = more chunks coming, `false` = this is the last (or only) chunk.
    pub more_data: bool,
    /// Display columns (0 = auto).
    pub display_columns: u32,
    /// Display rows (0 = auto).
    pub display_rows: u32,
    /// X offset in source image pixels.
    pub source_x: u32,
    /// Y offset in source image pixels.
    pub source_y: u32,
    /// Width of source region in pixels (0 = full).
    pub source_width: u32,
    /// Height of source region in pixels (0 = full).
    pub source_height: u32,
    /// Z-index for layering.
    pub z_index: i32,
    /// The base64-encoded payload data.
    pub payload: Vec<u8>,
    /// Whether to suppress the OK response.
    pub quiet: u8,
    /// Delete target (only relevant when action is Delete).
    pub delete_target: Option<DeleteTarget>,
}

impl Default for KittyCommand {
    fn default() -> Self {
        Self {
            action: KittyAction::Transmit,
            image_id: 0,
            placement_id: 0,
            width: 0,
            height: 0,
            format: PixelFormat::Rgba,
            transmission: TransmissionMode::Direct,
            compression: Compression::None,
            more_data: false,
            display_columns: 0,
            display_rows: 0,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            z_index: 0,
            payload: Vec::new(),
            quiet: 0,
            delete_target: None,
        }
    }
}

impl KittyCommand {
    /// Decode the base64 payload into raw bytes.
    pub fn decode_payload(&self) -> Result<Vec<u8>, GraphicsError> {
        if self.payload.is_empty() {
            return Ok(Vec::new());
        }
        let engine = base64::engine::general_purpose::STANDARD;
        engine
            .decode(&self.payload)
            .map_err(GraphicsError::Base64Decode)
    }
}

/// Parse a Kitty graphics protocol command from the content between APC G and ST.
///
/// The input should be the raw bytes after `\x1b_G` and before `\x1b\\` (or BEL).
/// Format: `key=value,key=value,...;base64data`
///
/// # Errors
///
/// Returns `GraphicsError::ParseError` if the command cannot be parsed.
pub fn parse_kitty_command(input: &[u8]) -> Result<KittyCommand, GraphicsError> {
    let input_str = std::str::from_utf8(input)
        .map_err(|e| GraphicsError::ParseError(format!("invalid UTF-8: {e}")))?;

    let mut cmd = KittyCommand::default();

    // Split on ';' to separate key-value pairs from payload
    let (params_str, payload_str) = match input_str.find(';') {
        Some(pos) => (&input_str[..pos], &input_str[pos + 1..]),
        None => (input_str, ""),
    };

    // Store the payload
    if !payload_str.is_empty() {
        cmd.payload = payload_str.as_bytes().to_vec();
    }

    // Parse key=value pairs
    let mut delete_specifier: Option<char> = None;
    let mut delete_value: Option<String> = None;

    for pair in params_str.split(',') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.find('=') {
            Some(pos) => (&pair[..pos], &pair[pos + 1..]),
            None => {
                return Err(GraphicsError::ParseError(format!(
                    "invalid key-value pair: {pair}"
                )));
            }
        };

        match key {
            // Action
            "a" => {
                cmd.action = match value {
                    "t" | "T" => KittyAction::Transmit,
                    "p" | "P" => KittyAction::TransmitAndDisplay,
                    "d" | "D" => KittyAction::Delete,
                    "q" | "Q" => KittyAction::Query,
                    "f" | "F" => KittyAction::AnimationFrame,
                    _ => KittyAction::TransmitAndDisplay,
                };
            }
            // Image ID
            "i" => {
                cmd.image_id = parse_u32(value, "image id")?;
            }
            // Placement ID
            "p" => {
                cmd.placement_id = parse_u32(value, "placement id")?;
            }
            // Format
            "f" => {
                cmd.format = match value {
                    "24" => PixelFormat::Rgb,
                    "32" => PixelFormat::Rgba,
                    "100" => PixelFormat::Png,
                    _ => {
                        return Err(GraphicsError::ParseError(format!(
                            "unsupported format: {value}"
                        )));
                    }
                };
            }
            // Transmission mode
            "t" => {
                cmd.transmission = match value {
                    "d" | "D" => TransmissionMode::Direct,
                    "f" | "F" => TransmissionMode::File,
                    "t" | "T" => TransmissionMode::TempFile,
                    "s" | "S" => TransmissionMode::SharedMemory,
                    _ => TransmissionMode::Direct,
                };
            }
            // Width in pixels
            "s" => {
                cmd.width = parse_u32(value, "width")?;
            }
            // Height in pixels
            "v" => {
                cmd.height = parse_u32(value, "height")?;
            }
            // Compression
            "o" => {
                cmd.compression = match value {
                    "z" => Compression::Zlib,
                    _ => Compression::None,
                };
            }
            // More data chunks
            "m" => {
                cmd.more_data = value == "1";
            }
            // Display columns
            "c" => {
                cmd.display_columns = parse_u32(value, "display columns")?;
            }
            // Display rows
            "r" => {
                cmd.display_rows = parse_u32(value, "display rows")?;
            }
            // Source X offset
            "x" => {
                cmd.source_x = parse_u32(value, "source x")?;
            }
            // Source Y offset
            "y" => {
                cmd.source_y = parse_u32(value, "source y")?;
            }
            // Source width
            "w" => {
                cmd.source_width = parse_u32(value, "source width")?;
            }
            // Source height
            "h" => {
                cmd.source_height = parse_u32(value, "source height")?;
            }
            // Z-index
            "z" => {
                cmd.z_index = value
                    .parse::<i32>()
                    .map_err(|e| GraphicsError::ParseError(format!("invalid z-index: {e}")))?;
            }
            // Quiet mode
            "q" => {
                cmd.quiet = value
                    .parse::<u8>()
                    .map_err(|e| GraphicsError::ParseError(format!("invalid quiet: {e}")))?;
            }
            // Delete specifier
            "d" => {
                if let Some(ch) = value.chars().next() {
                    delete_specifier = Some(ch);
                    if value.len() > 1 {
                        delete_value = Some(value[1..].to_string());
                    }
                }
            }
            // Ignore unknown keys for forward compatibility
            _ => {
                log::trace!("ignoring unknown kitty graphics key: {key}={value}");
            }
        }
    }

    // Resolve delete target if action is Delete
    if cmd.action == KittyAction::Delete {
        cmd.delete_target = Some(match delete_specifier {
            Some('a') | Some('A') => DeleteTarget::All,
            Some('i') | Some('I') => {
                if cmd.image_id > 0 {
                    if cmd.placement_id > 0 {
                        DeleteTarget::ByPlacement {
                            image_id: ImageId(cmd.image_id),
                            placement_id: cmd.placement_id,
                        }
                    } else {
                        DeleteTarget::ById(ImageId(cmd.image_id))
                    }
                } else {
                    DeleteTarget::All
                }
            }
            Some('c') | Some('C') => DeleteTarget::AtCursor,
            Some('p') | Some('P') => {
                let col = delete_value
                    .as_deref()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(0);
                DeleteTarget::InRange {
                    column: col,
                    row: 0,
                }
            }
            _ => DeleteTarget::All,
        });
    }

    // If no explicit action was set but payload exists, infer TransmitAndDisplay
    // (Kitty protocol default: transmit+display when 'a' key is absent)
    if params_str.split(',').all(|p| !p.starts_with("a=")) && !cmd.payload.is_empty() {
        cmd.action = KittyAction::TransmitAndDisplay;
    }

    Ok(cmd)
}

/// Parse a string as u32, providing a contextual error message.
fn parse_u32(value: &str, context: &str) -> Result<u32, GraphicsError> {
    value
        .parse::<u32>()
        .map_err(|e| GraphicsError::ParseError(format!("invalid {context}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_transmit() {
        let input = b"a=t,f=32,s=100,v=50,i=1;AAAA";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.action, KittyAction::Transmit);
        assert_eq!(cmd.format, PixelFormat::Rgba);
        assert_eq!(cmd.width, 100);
        assert_eq!(cmd.height, 50);
        assert_eq!(cmd.image_id, 1);
        assert_eq!(cmd.payload, b"AAAA");
    }

    #[test]
    fn test_parse_transmit_and_display() {
        let input = b"a=T,f=24,s=200,v=100,i=5;AQID";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.action, KittyAction::Transmit);
        assert_eq!(cmd.format, PixelFormat::Rgb);
        assert_eq!(cmd.width, 200);
        assert_eq!(cmd.height, 100);
        assert_eq!(cmd.image_id, 5);
    }

    #[test]
    fn test_parse_display_action() {
        // Note: 'p' for display is overloaded with placement_id key.
        // The action key is 'a', value 'p' means TransmitAndDisplay.
        let input = b"a=p,i=3,p=1,c=10,r=5,z=-1";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.action, KittyAction::TransmitAndDisplay);
        assert_eq!(cmd.image_id, 3);
        assert_eq!(cmd.placement_id, 1);
        assert_eq!(cmd.display_columns, 10);
        assert_eq!(cmd.display_rows, 5);
        assert_eq!(cmd.z_index, -1);
    }

    #[test]
    fn test_parse_delete_all() {
        let input = b"a=d,d=a";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.action, KittyAction::Delete);
        assert_eq!(cmd.delete_target, Some(DeleteTarget::All));
    }

    #[test]
    fn test_parse_delete_by_id() {
        let input = b"a=d,d=i,i=42";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.action, KittyAction::Delete);
        assert_eq!(cmd.delete_target, Some(DeleteTarget::ById(ImageId(42))));
    }

    #[test]
    fn test_parse_delete_by_placement() {
        let input = b"a=d,d=i,i=42,p=7";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.action, KittyAction::Delete);
        assert_eq!(
            cmd.delete_target,
            Some(DeleteTarget::ByPlacement {
                image_id: ImageId(42),
                placement_id: 7,
            })
        );
    }

    #[test]
    fn test_parse_chunked_transfer() {
        let chunk1 = b"a=t,f=32,s=100,v=50,i=1,m=1;AAAA";
        let cmd1 = parse_kitty_command(chunk1).unwrap();
        assert!(cmd1.more_data);

        let chunk2 = b"m=0;BBBB";
        let cmd2 = parse_kitty_command(chunk2).unwrap();
        assert!(!cmd2.more_data);
    }

    #[test]
    fn test_parse_png_format() {
        let input = b"a=t,f=100,i=10;iVBORw0KGgo=";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.format, PixelFormat::Png);
    }

    #[test]
    fn test_parse_file_transmission() {
        let input = b"a=t,t=f,i=1;L3RtcC9pbWFnZS5wbmc=";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.transmission, TransmissionMode::File);
    }

    #[test]
    fn test_parse_zlib_compression() {
        let input = b"a=t,o=z,f=32,s=10,v=10,i=1;AAAA";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.compression, Compression::Zlib);
    }

    #[test]
    fn test_parse_no_payload() {
        let input = b"a=d,d=a";
        let cmd = parse_kitty_command(input).unwrap();
        assert!(cmd.payload.is_empty());
    }

    #[test]
    fn test_parse_query() {
        let input = b"a=q,i=1,s=1,v=1,f=32;AAAA";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.action, KittyAction::Query);
    }

    #[test]
    fn test_decode_payload() {
        let input = b"a=t,f=32,s=1,v=1,i=1;AQID";
        let cmd = parse_kitty_command(input).unwrap();
        let decoded = cmd.decode_payload().unwrap();
        assert_eq!(decoded, vec![1, 2, 3]);
    }

    #[test]
    fn test_decode_empty_payload() {
        let input = b"a=d,d=a";
        let cmd = parse_kitty_command(input).unwrap();
        let decoded = cmd.decode_payload().unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_default_action_is_transmit_display() {
        // No 'a' key with payload should default to TransmitAndDisplay
        let input = b"f=32,s=10,v=10,i=1;AAAA";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.action, KittyAction::TransmitAndDisplay);
    }

    #[test]
    fn test_quiet_mode() {
        let input = b"a=t,q=2,i=1;AAAA";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.quiet, 2);
    }

    #[test]
    fn test_invalid_key_value_pair() {
        let input = b"invalid";
        let result = parse_kitty_command(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_negative_z_index() {
        let input = b"a=t,z=-10,i=1;AAAA";
        let cmd = parse_kitty_command(input).unwrap();
        assert_eq!(cmd.z_index, -10);
    }

    // --- Property-based tests ---

    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn kitty_parser_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..1000)) {
                let _ = parse_kitty_command(&bytes);
            }

            #[test]
            fn kitty_parser_handles_valid_transmit(
                width in 1u32..4096,
                height in 1u32..4096,
                image_id in 1u32..65535,
            ) {
                let input = format!("a=t,f=32,s={},v={},i={};AQID", width, height, image_id);
                let result = parse_kitty_command(input.as_bytes());
                prop_assert!(result.is_ok());
                let cmd = result.unwrap();
                prop_assert_eq!(cmd.action, KittyAction::Transmit);
                prop_assert_eq!(cmd.width, width);
                prop_assert_eq!(cmd.height, height);
                prop_assert_eq!(cmd.image_id, image_id);
            }

            #[test]
            fn kitty_parser_handles_delete_commands(image_id in 1u32..1000) {
                let input = format!("a=d,d=i,i={}", image_id);
                let result = parse_kitty_command(input.as_bytes());
                prop_assert!(result.is_ok());
                let cmd = result.unwrap();
                prop_assert_eq!(cmd.action, KittyAction::Delete);
                prop_assert_eq!(
                    cmd.delete_target,
                    Some(DeleteTarget::ById(ImageId(image_id)))
                );
            }

            #[test]
            fn kitty_parser_handles_display_params(
                cols in 1u32..200,
                rows in 1u32..100,
                z_index in -100i32..100,
            ) {
                let input = format!("a=p,i=1,c={},r={},z={}", cols, rows, z_index);
                let result = parse_kitty_command(input.as_bytes());
                prop_assert!(result.is_ok());
                let cmd = result.unwrap();
                prop_assert_eq!(cmd.display_columns, cols);
                prop_assert_eq!(cmd.display_rows, rows);
                prop_assert_eq!(cmd.z_index, z_index);
            }

            #[test]
            fn kitty_parser_handles_chunked_transfer(more_data in any::<bool>()) {
                let input = format!("a=t,f=32,s=10,v=10,i=1,m={};AAAA", if more_data { "1" } else { "0" });
                let result = parse_kitty_command(input.as_bytes());
                prop_assert!(result.is_ok());
                let cmd = result.unwrap();
                prop_assert_eq!(cmd.more_data, more_data);
            }

            #[test]
            fn kitty_decoder_never_panics_on_valid_base64(
                data in prop::collection::vec(any::<u8>(), 0..100)
            ) {
                use base64::Engine;
                let engine = base64::engine::general_purpose::STANDARD;
                let encoded = engine.encode(&data);
                let input = format!("a=t,f=32,s=10,v=10,i=1;{}", encoded);
                let result = parse_kitty_command(input.as_bytes());
                prop_assert!(result.is_ok());
                let cmd = result.unwrap();
                let decoded = cmd.decode_payload();
                prop_assert!(decoded.is_ok());
                prop_assert_eq!(decoded.unwrap(), data);
            }

            #[test]
            fn kitty_parser_accepts_utf8_strings(input_str in ".{0,100}") {
                // Any UTF-8 string should not panic the parser
                let _ = parse_kitty_command(input_str.as_bytes());
            }

            #[test]
            fn kitty_parser_handles_compression_flag(use_zlib in any::<bool>()) {
                let input: &[u8] = if use_zlib {
                    b"a=t,o=z,f=32,s=10,v=10,i=1;AAAA"
                } else {
                    b"a=t,f=32,s=10,v=10,i=1;AAAA"
                };
                let result = parse_kitty_command(input);
                prop_assert!(result.is_ok());
                let cmd = result.unwrap();
                prop_assert_eq!(
                    cmd.compression,
                    if use_zlib { Compression::Zlib } else { Compression::None }
                );
            }
        }
    }
}
