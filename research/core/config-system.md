---
title: "Terminal Config System Design"
description: "Configuration format comparison (TOML vs YAML vs KDL vs Lua), XDG-first file locations, hot-reload with notify crate, figment for layered config, schema validation, deprecated field handling"
date: 2026-02-12
phase: [5]
topics: [config, toml, hot-reload, settings]
status: final
related:
  - terminal-architecture.md
---

# Terminal Config System Design

> 작성일: 2026-02-12
> 목적: Crux 터미널의 설정 시스템 설계 — 포맷 선택, 파일 위치, 핫 리로드, 스키마 검증, 레이어드 설정

---

## 목차

1. [개요](#1-개요)
2. [Configuration Format Comparison](#2-configuration-format-comparison)
3. [File Locations and Precedence](#3-file-locations-and-precedence)
4. [Hot Reload](#4-hot-reload)
5. [Schema Validation with Serde](#5-schema-validation-with-serde)
6. [Layered Config with figment](#6-layered-config-with-figment)
7. [Deprecated Field Handling](#7-deprecated-field-handling)
8. [Default Config Generation](#8-default-config-generation)
9. [Crux Implementation Recommendations](#9-crux-implementation-recommendations)

---

## 1. 개요

A terminal emulator's configuration system must balance:

- **Discoverability**: Users should easily understand available options
- **Type safety**: Catch typos and invalid values at parse time
- **Hot reload**: Changes apply without restarting the terminal
- **Layered merging**: CLI flags override env vars override config file override defaults
- **Ecosystem fit**: Align with Rust tooling conventions

### What Other Terminals Use

| Terminal | Format | Hot Reload | Validation |
|----------|--------|------------|------------|
| Alacritty | TOML (was YAML, migrated in v0.13) | Yes | serde + manual |
| Kitty | Custom (INI-like `.conf`) | Yes | Custom parser |
| Ghostty | Custom (key=value `.conf`) | Partial | Custom parser |
| WezTerm | Lua | Yes (full scripting) | Runtime errors |
| Rio | TOML | Yes | serde |
| Warp | Internal (not user-facing) | N/A | N/A |

---

## 2. Configuration Format Comparison

### TOML — **Recommended**

```toml
[font]
family = "JetBrains Mono"
size = 14.0
ligatures = false

[font.fallback]
families = ["Apple SD Gothic Neo", "Noto Sans Mono CJK KR"]

[colors]
foreground = "#c0caf5"
background = "#1a1b26"

[terminal]
scrollback_lines = 10000
cursor_style = "block"
cursor_blink = false

[shell]
program = "/bin/zsh"
args = ["-l"]
integration = true

[window]
opacity = 1.0
blur = false
decorations = "full"
```

**Pros**:
- Native to Rust ecosystem (Cargo.toml is TOML)
- Strong typing: integers, floats, strings, arrays, tables — no implicit conversions
- Excellent serde support via `toml` crate
- Comments supported (`#`)
- Alacritty validated this choice after migrating from YAML (v0.13)
- Human-readable, minimal syntax noise

**Cons**:
- Deeply nested tables can be verbose
- No scripting/conditionals (feature, not a bug for configs)
- Array of tables syntax (`[[section]]`) can confuse newcomers

**Crate**: `toml = "0.8"` (serde-based, 10M+ downloads)

### YAML

```yaml
font:
  family: JetBrains Mono
  size: 14.0
  ligatures: false
```

**Pros**: Familiar, compact for simple configs
**Cons**: Significant whitespace, "Norway problem" (`NO` → boolean), implicit type coercion, security history (arbitrary code execution in some parsers), Alacritty migrated away from it
**Verdict**: **Do not use.** The YAML→TOML migration in Alacritty was painful. Learn from their mistake.

### KDL

```kdl
font {
    family "JetBrains Mono"
    size 14.0
    ligatures false
}
```

**Pros**: Clean syntax, designed for config files, good error messages
**Cons**: Not widely adopted in Rust ecosystem, less tooling support, unfamiliar to most users
**Crate**: `kdl = "4.6"` — well-maintained but niche
**Verdict**: Interesting but premature. Re-evaluate if ecosystem grows.

### Lua

```lua
config.font_family = "JetBrains Mono"
config.font_size = 14.0

-- Conditional config
if os.getenv("SSH_CONNECTION") then
    config.cursor_blink = true
end
```

**Pros**: Full scripting, conditional logic, WezTerm proves it works
**Cons**: Complexity explosion, security surface (arbitrary code execution), error handling is difficult, harder to validate/lint, harder to generate defaults
**Crate**: `mlua = "0.10"` (bindings to Lua 5.4/LuaJIT)
**Verdict**: **Do not use for Crux.** WezTerm's Lua config is powerful but produces unreadable configs and hard-to-debug errors. Terminal config should be declarative.

### Decision Matrix

| Criterion | TOML | YAML | KDL | Lua |
|-----------|------|------|-----|-----|
| Rust ecosystem fit | ★★★ | ★★ | ★★ | ★ |
| Type safety | ★★★ | ★ | ★★★ | ★ |
| User familiarity | ★★★ | ★★★ | ★ | ★★ |
| Error messages | ★★★ | ★★ | ★★★ | ★ |
| Serde integration | ★★★ | ★★★ | ★★ | ★ |
| **Total** | **15** | **11** | **11** | **6** |

**Winner: TOML**

---

## 3. File Locations and Precedence

### XDG Base Directory Specification

Crux follows XDG on all platforms, with macOS-native fallbacks:

```
Primary:   $XDG_CONFIG_HOME/crux/config.toml
           → Default: ~/.config/crux/config.toml

macOS alt: ~/Library/Application Support/com.crux.terminal/config.toml

Legacy:    ~/.crux.toml (deprecated, emit warning)
```

### Full Precedence (highest → lowest)

```
1. CLI flags          --font-size=16
2. Environment vars   CRUX_FONT_SIZE=16
3. XDG config file    ~/.config/crux/config.toml
4. macOS native       ~/Library/Application Support/com.crux.terminal/config.toml
5. System defaults    /etc/crux/config.toml (optional)
6. Built-in defaults  Compiled into binary
```

### Config Discovery Logic

```rust
use dirs::config_dir;
use std::path::PathBuf;

fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // XDG_CONFIG_HOME/crux/config.toml
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(xdg).join("crux/config.toml"));
    }

    // ~/.config/crux/config.toml (XDG default)
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".config/crux/config.toml"));
    }

    // macOS native location
    if let Some(app_support) = dirs::config_dir() {
        paths.push(app_support.join("com.crux.terminal/config.toml"));
    }

    // Legacy location (deprecated)
    if let Some(home) = dirs::home_dir() {
        let legacy = home.join(".crux.toml");
        if legacy.exists() {
            eprintln!("Warning: ~/.crux.toml is deprecated. \
                       Move to ~/.config/crux/config.toml");
            paths.push(legacy);
        }
    }

    paths
}
```

### Environment Variable Naming Convention

```
CRUX_<SECTION>_<KEY>=<value>

CRUX_FONT_FAMILY="JetBrains Mono"
CRUX_FONT_SIZE=16
CRUX_TERMINAL_SCROLLBACK_LINES=50000
CRUX_WINDOW_OPACITY=0.95
```

---

## 4. Hot Reload

### Architecture

```
[File System] --(notify crate)--> [Debounce 10ms] --> [Parse TOML]
    --> [Validate] --> [Diff against current] --> [Apply changes]
```

### Implementation with `notify`

```rust
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::sync::mpsc;
use std::time::Duration;

fn watch_config(config_path: &Path) -> notify::Result<()> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    let _ = tx.send(());
                }
                _ => {}
            }
        }
    })?;

    // Watch the PARENT directory, not the file itself
    // This handles editor atomic saves (write to temp → rename)
    let parent = config_path.parent().unwrap();
    watcher.watch(parent, RecursiveMode::NonRecursive)?;

    // Debounce loop
    std::thread::spawn(move || {
        let mut last_reload = std::time::Instant::now();
        while let Ok(()) = rx.recv() {
            let now = std::time::Instant::now();
            if now.duration_since(last_reload) < Duration::from_millis(10) {
                continue;  // Debounce: skip rapid successive events
            }
            last_reload = now;
            // Trigger config reload on main thread
            reload_config();
        }
    });

    Ok(())
}
```

### Why Watch Parent Directory?

Many editors (vim, VS Code, JetBrains) use **atomic saves**:

1. Write to `.config/crux/config.toml.tmp`
2. Rename `.config/crux/config.toml.tmp` → `.config/crux/config.toml`

If you watch the file directly, the `rename` removes the watch. Watching the parent directory catches both direct writes and atomic renames.

### Error Handling on Reload

```rust
fn reload_config(path: &Path, current: &Config) -> ReloadResult {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Cannot read config: {e}");
            return ReloadResult::KeepCurrent;
        }
    };

    let new_config: Config = match toml::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            // Show error notification in terminal, keep old config
            log::warn!("Config parse error: {e}");
            notify_user(&format!("Config error: {e}"));
            return ReloadResult::KeepCurrent;
        }
    };

    if let Err(warnings) = new_config.validate() {
        for w in &warnings {
            log::warn!("Config warning: {w}");
        }
    }

    // Diff and apply only changed fields
    let diff = current.diff(&new_config);
    ReloadResult::Apply(new_config, diff)
}
```

**Key principle**: On parse error, keep the old configuration. Never leave the terminal in a broken state.

### Hot-Reloadable vs Restart-Required

| Category | Hot Reloadable | Restart Required |
|----------|---------------|------------------|
| Colors/theme | Yes | — |
| Font family/size | Yes | — |
| Cursor style/blink | Yes | — |
| Window opacity | Yes | — |
| Keybindings | Yes | — |
| Scrollback size | — | Yes (buffer reallocation) |
| Shell program | — | Yes (per-tab) |
| Window decorations | — | Yes (platform limitation) |
| IPC socket path | — | Yes |

---

## 5. Schema Validation with Serde

### Deny Unknown Fields

Catch typos immediately:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub font: FontConfig,
    #[serde(default)]
    pub colors: ColorConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub shell: ShellConfig,
    #[serde(default)]
    pub keybindings: Vec<Keybinding>,
}
```

With `deny_unknown_fields`, a typo like `[fontt]` immediately produces:

```
Error: unknown field `fontt`, expected one of `font`, `colors`, `terminal`, `window`, `shell`, `keybindings`
```

### Default Values

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontConfig {
    #[serde(default = "default_font_family")]
    pub family: String,

    #[serde(default = "default_font_size")]
    pub size: f32,

    #[serde(default)]
    pub ligatures: bool,  // false by default

    #[serde(default)]
    pub fallback: FontFallbackConfig,
}

fn default_font_family() -> String {
    "Menlo".to_string()  // macOS system monospace
}

fn default_font_size() -> f32 {
    14.0
}
```

### Range Validation

```rust
impl Config {
    pub fn validate(&self) -> Result<Vec<Warning>, Vec<Error>> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if self.font.size < 6.0 || self.font.size > 128.0 {
            errors.push(Error::OutOfRange {
                field: "font.size",
                value: self.font.size.to_string(),
                range: "6.0..128.0",
            });
        }

        if self.terminal.scrollback_lines > 1_000_000 {
            warnings.push(Warning::HighValue {
                field: "terminal.scrollback_lines",
                value: self.terminal.scrollback_lines.to_string(),
                suggestion: "Values above 1M may use significant memory",
            });
        }

        if self.window.opacity < 0.0 || self.window.opacity > 1.0 {
            errors.push(Error::OutOfRange {
                field: "window.opacity",
                value: self.window.opacity.to_string(),
                range: "0.0..1.0",
            });
        }

        if errors.is_empty() {
            Ok(warnings)
        } else {
            Err(errors)
        }
    }
}
```

### Color Parsing

```rust
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_hex_color(&s).map_err(serde::de::Error::custom)
    }
}

fn parse_hex_color(s: &str) -> Result<Color, String> {
    let s = s.strip_prefix('#').unwrap_or(s);
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
            Ok(Color { r, g, b })
        }
        _ => Err(format!("Invalid color: #{s} (expected 6 hex digits)")),
    }
}
```

---

## 6. Layered Config with figment

### Overview

The `figment` crate provides a powerful abstraction for layered configuration merging:

```rust
use figment::{Figment, providers::{Toml, Env, Serialized}};

fn load_config() -> Result<Config, figment::Error> {
    let config: Config = Figment::new()
        // Layer 1: Built-in defaults (lowest priority)
        .merge(Serialized::defaults(Config::default()))
        // Layer 2: System config
        .merge(Toml::file("/etc/crux/config.toml").nested())
        // Layer 3: User XDG config
        .merge(Toml::file(xdg_config_path()).nested())
        // Layer 4: Environment variables (CRUX_ prefix)
        .merge(Env::prefixed("CRUX_").split("_"))
        // Extract merged result
        .extract()?;

    Ok(config)
}
```

### figment Features

| Feature | Description |
|---------|-------------|
| **Layered merging** | Later providers override earlier ones |
| **Nested keys** | `CRUX_FONT_SIZE` maps to `font.size` |
| **Profile support** | `[debug]` vs `[release]` sections |
| **Error tracking** | Knows which provider set each value |
| **Metadata** | Can tell user "this value came from env var CRUX_FONT_SIZE" |

### Crate: `figment = "0.10"`

**Pros**: Elegant API, excellent error messages, profile support
**Cons**: Additional dependency, learning curve

**Alternative**: Manual layering with `Config::merge()` method is simpler if figment feels too heavy.

---

## 7. Deprecated Field Handling

### Strategy

Support old field names for one major version, emit warnings, then remove:

```rust
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub font: FontConfig,

    // Deprecated: renamed to `font` in v0.3
    #[serde(default, rename = "fonts")]
    pub _deprecated_fonts: Option<FontConfig>,
}

impl Config {
    pub fn migrate(&mut self) -> Vec<DeprecationWarning> {
        let mut warnings = Vec::new();

        if let Some(fonts) = self._deprecated_fonts.take() {
            warnings.push(DeprecationWarning {
                old_field: "fonts",
                new_field: "font",
                since: "0.3.0",
                removal: "1.0.0",
            });
            if self.font == FontConfig::default() {
                self.font = fonts;  // Only apply if new field isn't set
            }
        }

        warnings
    }
}
```

### User-Facing Warning

```
⚠ Config deprecation: `fonts` has been renamed to `font`
  (deprecated in v0.3.0, will be removed in v1.0.0)
  Please update your config at ~/.config/crux/config.toml
```

---

## 8. Default Config Generation

### Annotated Default Config

Generate a well-commented default config that serves as documentation:

```rust
fn generate_default_config() -> String {
    r#"# Crux Terminal Configuration
# Location: ~/.config/crux/config.toml
# Changes are applied immediately (hot reload).

# Font configuration
[font]
# family = "Menlo"           # Font family name
# size = 14.0                # Font size in points (6.0 - 128.0)
# ligatures = false          # Enable font ligatures

# CJK fallback chain (tried in order)
# [font.fallback]
# families = ["Apple SD Gothic Neo", "PingFang SC", "Noto Sans Mono CJK KR"]

# Color scheme
[colors]
# foreground = "#c0caf5"
# background = "#1a1b26"

# Terminal behavior
[terminal]
# scrollback_lines = 10000   # Number of scrollback lines (0 = disabled)
# cursor_style = "block"     # block, underline, beam
# cursor_blink = false       # Enable cursor blinking

# Shell configuration
[shell]
# program = "/bin/zsh"       # Shell to launch (default: $SHELL or /bin/zsh)
# args = ["-l"]              # Arguments passed to shell
# integration = true         # Enable shell integration (OSC 7, 133)

# Window appearance
[window]
# opacity = 1.0              # Window opacity (0.0 - 1.0)
# blur = false               # Enable background blur (macOS only)
# decorations = "full"       # full, none

# Keybindings (list of {key, mods, action})
# [[keybindings]]
# key = "n"
# mods = "super"
# action = "new_window"
"#.to_string()
}
```

### CLI Command

```bash
# Generate default config
crux --generate-config > ~/.config/crux/config.toml

# Validate existing config
crux --check-config
```

---

## 9. Crux Implementation Recommendations

### Phase 5 Implementation Plan

1. **Define `Config` struct** with serde derives, `deny_unknown_fields`, defaults
2. **Config discovery**: XDG-first with macOS fallback
3. **TOML parsing**: `toml = "0.8"` with detailed error messages
4. **Layered merging**: Start with manual merge; add figment if complexity warrants
5. **Hot reload**: `notify` crate watching parent directory, 10ms debounce
6. **Validation**: Range checks, color parsing, font existence check
7. **CLI integration**: `--generate-config`, `--check-config`, `--config-path`
8. **Deprecation framework**: Field migration with versioned warnings

### Crate Dependencies

```toml
[dependencies]
toml = "0.8"
serde = { version = "1", features = ["derive"] }
dirs = "5"
notify = "7"

# Optional: for layered config
# figment = { version = "0.10", features = ["toml", "env"] }
```

### What Not To Do

- **Do not use YAML**: Alacritty learned this lesson; the migration was painful
- **Do not use Lua**: WezTerm's power comes at a complexity cost that Crux doesn't need
- **Do not invent a custom format**: KDL and TOML cover all needs
- **Do not validate lazily**: Catch all errors at parse time, not at use time
- **Do not watch the file directly**: Watch parent directory for atomic save compatibility

---

## Sources

- [Alacritty Config Migration (YAML → TOML)](https://github.com/alacritty/alacritty/blob/master/CHANGELOG.md#0130) — v0.13 changelog
- [figment documentation](https://docs.rs/figment/latest/figment/) — Layered config framework
- [notify crate](https://docs.rs/notify/latest/notify/) — File system watching
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html) — Standard config paths
- [TOML Specification](https://toml.io/en/v1.0.0) — Format reference
- [Kitty Config Documentation](https://sw.kovidgoyal.net/kitty/conf/) — Example of custom format
- [WezTerm Lua Config](https://wezfurlong.org/wezterm/config/files.html) — Lua-based config example
- [Rio Configuration](https://raphamorim.io/rio/docs/configuration) — TOML config example
