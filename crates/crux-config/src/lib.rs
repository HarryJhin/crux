//! Configuration system for Crux terminal emulator.
//!
//! Supports TOML configuration files with sensible defaults matching current
//! hardcoded values. Config file is optional - the application works with zero config.
//!
//! # Config file locations
//!
//! Priority order:
//! 1. `$CRUX_CONFIG` environment variable
//! 2. macOS: `~/Library/Application Support/crux/config.toml`
//! 3. XDG: `~/.config/crux/config.toml`

pub mod watcher;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse TOML: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Invalid configuration: {0}")]
    ValidationError(String),

    #[error("File watcher error: {0}")]
    WatchError(String),
}

/// Main configuration structure.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct CruxConfig {
    pub window: WindowConfig,
    pub font: FontConfig,
    pub colors: ColorConfig,
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub keybindings: Vec<KeyBinding>,
}

impl CruxConfig {
    /// Load configuration from the default location.
    ///
    /// Returns default configuration if no config file exists.
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path();
        Self::load_from(&path)
    }

    /// Load configuration from a specific path.
    ///
    /// Returns default configuration if the file doesn't exist.
    pub fn load_from(path: &PathBuf) -> Result<Self, ConfigError> {
        if !path.exists() {
            log::info!(
                "No config file found at {}, using defaults",
                path.display()
            );
            return Ok(Self::default());
        }

        log::info!("Loading config from {}", path.display());
        let contents = std::fs::read_to_string(path)?;
        let config: CruxConfig = toml::from_str(&contents)?;
        config.validate()?;
        Ok(config)
    }

    /// Get the config file path based on environment and platform.
    pub fn config_path() -> PathBuf {
        // 1. Check $CRUX_CONFIG environment variable
        if let Ok(path) = std::env::var("CRUX_CONFIG") {
            return PathBuf::from(path);
        }

        // 2. macOS primary location
        #[cfg(target_os = "macos")]
        {
            if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "crux") {
                return proj_dirs.config_dir().join("config.toml");
            }
        }

        // 3. XDG fallback
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/crux/config.toml")
    }

    /// Validate configuration values.
    fn validate(&self) -> Result<(), ConfigError> {
        // Validate font size
        if !(6.0..=72.0).contains(&self.font.size) {
            return Err(ConfigError::ValidationError(format!(
                "font.size must be between 6.0 and 72.0, got {}",
                self.font.size
            )));
        }

        // Validate scrollback lines
        if self.terminal.scrollback_lines > 1_000_000 {
            return Err(ConfigError::ValidationError(format!(
                "terminal.scrollback_lines must be <= 1,000,000, got {}",
                self.terminal.scrollback_lines
            )));
        }

        // Validate window dimensions
        if self.window.width < 100.0 || self.window.width > 10000.0 {
            return Err(ConfigError::ValidationError(format!(
                "window.width must be between 100.0 and 10000.0, got {}",
                self.window.width
            )));
        }

        if self.window.height < 100.0 || self.window.height > 10000.0 {
            return Err(ConfigError::ValidationError(format!(
                "window.height must be between 100.0 and 10000.0, got {}",
                self.window.height
            )));
        }

        // Validate opacity
        if !(0.0..=1.0).contains(&self.window.opacity) {
            return Err(ConfigError::ValidationError(format!(
                "window.opacity must be between 0.0 and 1.0, got {}",
                self.window.opacity
            )));
        }

        Ok(())
    }
}

/// Window appearance configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct WindowConfig {
    /// Window width in pixels.
    pub width: f32,
    /// Window height in pixels.
    pub height: f32,
    /// Window opacity (0.0 = transparent, 1.0 = opaque).
    pub opacity: f32,
    /// Show window decorations (titlebar, etc.).
    pub decorations: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 800.0,
            height: 600.0,
            opacity: 1.0,
            decorations: true,
        }
    }
}

/// Font configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct FontConfig {
    /// Primary font family name.
    pub family: String,
    /// Font size in points.
    pub size: f32,
    /// Enable font ligatures.
    pub ligatures: bool,
    /// Fallback fonts for missing glyphs.
    #[serde(default)]
    pub fallback: Vec<String>,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "Menlo".to_string(),
            size: 14.0,
            ligatures: true,
            fallback: vec![
                "Menlo".to_string(),
                "Monaco".to_string(),
                "Courier New".to_string(),
            ],
        }
    }
}

/// Color scheme configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct ColorConfig {
    /// Named theme (reserved for future use).
    pub theme: Option<String>,
    /// Background color (RGB hex, e.g., 0x1e1e2e).
    pub background: u32,
    /// Foreground color (RGB hex).
    pub foreground: u32,
    /// Cursor color (RGB hex).
    pub cursor: u32,
    /// Normal ANSI colors (0-7).
    pub normal: [u32; 8],
    /// Bright ANSI colors (8-15).
    pub bright: [u32; 8],
}

impl Default for ColorConfig {
    fn default() -> Self {
        // Catppuccin Mocha palette (current hardcoded values)
        Self {
            theme: None,
            background: 0x1e1e2e,
            foreground: 0xcdd6f4,
            cursor: 0xf5e0dc,
            normal: [
                0x1e1e2e, // black
                0xf38ba8, // red
                0xa6e3a1, // green
                0xf9e2af, // yellow
                0x89b4fa, // blue
                0xcba6f7, // magenta
                0x94e2d5, // cyan
                0xcdd6f4, // white
            ],
            bright: [
                0x585b70, // bright black
                0xeba0ac, // bright red
                0x94e2d5, // bright green (note: same as normal cyan in current code)
                0xf5e0dc, // bright yellow
                0x74c7ec, // bright blue
                0xf5c2e7, // bright magenta
                0x89dceb, // bright cyan
                0xffffff, // bright white
            ],
        }
    }
}

/// Terminal behavior configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct TerminalConfig {
    /// Scrollback history size in lines.
    pub scrollback_lines: usize,
    /// Shell to execute.
    ///
    /// If None, uses shell detection logic:
    /// 1. $SHELL environment variable
    /// 2. macOS dscl UserShell lookup
    /// 3. /bin/zsh fallback
    pub shell: Option<String>,
    /// Shell arguments (e.g., ["-l"] for login shell).
    #[serde(default)]
    pub shell_args: Vec<String>,
    /// Additional environment variables to pass to the shell.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            scrollback_lines: 10_000,
            shell: None,
            shell_args: vec!["-l".to_string()],
            env: HashMap::new(),
        }
    }
}

/// Keybinding configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyBinding {
    /// Key combination (e.g., "cmd-t", "ctrl-shift-c").
    pub key: String,
    /// Modifiers (e.g., ["cmd"], ["ctrl", "shift"]).
    #[serde(default)]
    pub mods: Vec<String>,
    /// Action to perform (e.g., "new_tab", "close_tab").
    pub action: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CruxConfig::default();
        assert_eq!(config.window.width, 800.0);
        assert_eq!(config.window.height, 600.0);
        assert_eq!(config.font.family, "Menlo");
        assert_eq!(config.font.size, 14.0);
        assert_eq!(config.terminal.scrollback_lines, 10_000);
        assert_eq!(config.colors.background, 0x1e1e2e);
    }

    #[test]
    fn test_config_validation() {
        let mut config = CruxConfig::default();

        // Invalid font size
        config.font.size = 100.0;
        assert!(config.validate().is_err());

        config.font.size = 14.0;
        assert!(config.validate().is_ok());

        // Invalid scrollback
        config.terminal.scrollback_lines = 2_000_000;
        assert!(config.validate().is_err());

        config.terminal.scrollback_lines = 10_000;
        assert!(config.validate().is_ok());

        // Invalid window size
        config.window.width = 50.0;
        assert!(config.validate().is_err());

        config.window.width = 800.0;
        assert!(config.validate().is_ok());

        // Invalid opacity
        config.window.opacity = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_load_from_string() {
        let toml = r#"
[window]
width = 1024.0
height = 768.0

[font]
family = "JetBrains Mono"
size = 16.0

[terminal]
scrollback_lines = 50000
"#;

        let config: CruxConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.window.width, 1024.0);
        assert_eq!(config.font.family, "JetBrains Mono");
        assert_eq!(config.terminal.scrollback_lines, 50_000);
    }

    #[test]
    fn test_deny_unknown_fields() {
        let toml = r#"
[window]
width = 800.0
unknown_field = "oops"
"#;

        let result: Result<CruxConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_color_defaults() {
        let config = CruxConfig::default();
        // Verify Catppuccin Mocha colors
        assert_eq!(config.colors.background, 0x1e1e2e);
        assert_eq!(config.colors.foreground, 0xcdd6f4);
        assert_eq!(config.colors.cursor, 0xf5e0dc);
        assert_eq!(config.colors.normal[0], 0x1e1e2e); // black
        assert_eq!(config.colors.normal[1], 0xf38ba8); // red
        assert_eq!(config.colors.bright[0], 0x585b70); // bright black
    }

    #[test]
    fn test_missing_config_file_uses_defaults() {
        let path = PathBuf::from("/nonexistent/config.toml");
        let config = CruxConfig::load_from(&path).unwrap();
        assert_eq!(config.window.width, 800.0);
        assert_eq!(config.font.size, 14.0);
    }
}
