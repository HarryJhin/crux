---
title: "Terminal Config System Design"
description: "Configuration format comparison (TOML vs YAML vs KDL vs Lua), XDG-first file locations, hot-reload with notify crate, figment for layered config, schema validation, deprecated field handling, GUI settings window architecture, bidirectional config sync, terminal settings UX patterns"
date: 2026-02-12
phase: [5]
topics: [config, toml, hot-reload, settings, gui, gpui, preferences]
status: final
related:
  - terminal-architecture.md
  - ../gpui/framework.md
  - ../gpui/widgets-integration.md
---

# Terminal Config System Design

> 작성일: 2026-02-12
> 목적: Crux 터미널의 설정 시스템 설계 — 포맷 선택, 파일 위치, 핫 리로드, 스키마 검증, 레이어드 설정, GUI 설정 창

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
10. [GUI Settings Window Architecture](#10-gui-settings-window-architecture)
11. [Terminal Settings UI Patterns](#11-terminal-settings-ui-patterns)
12. [Bidirectional Config Sync](#12-bidirectional-config-sync)
13. [Settings UX Components](#13-settings-ux-components)

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

---

## 10. GUI Settings Window Architecture

### Why Both GUI and TOML?

| Approach | Pros | Cons | Terminals |
|----------|------|------|-----------|
| GUI only | Discoverable, beginner-friendly | Not version-controllable, opaque | iTerm2, Warp |
| File only | Version-controllable, scriptable | Steep learning curve, no preview | Alacritty, Ghostty, WezTerm |
| **GUI + File (bidirectional)** | **Best of both worlds** | Implementation complexity | **Crux**, VS Code |

Crux's approach: **TOML is the single source of truth**. The GUI is a visual editor that reads and writes TOML. This means:
- `dotfiles` repos, `chezmoi`, and team config sharing all work naturally
- Power users edit TOML directly; GUI users never need to touch a file
- No hidden state, no proprietary formats

### GPUI Window Management

GPUI supports multiple windows. The settings window is a secondary window opened via ⌘,:

```rust
use gpui::*;

fn open_settings_window(cx: &mut AppContext) {
    let bounds = Bounds::centered(None, size(px(720.), px(560.)), cx);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Settings".into()),
                ..Default::default()
            }),
            kind: WindowKind::Normal,
            ..Default::default()
        },
        |cx| cx.new_view(|cx| SettingsWindow::new(cx)),
    );
}
```

macOS HIG requirements for the settings window:
- **Non-modal**: Doesn't block main terminal window
- **Singleton**: Only one instance at a time
- **⌘,** shortcut: Standard macOS convention
- **Minimize/maximize disabled but visible**: Traffic light buttons present but grayed
- **Resizable**: With sensible min/max constraints (720×560 default)

### Tab-Based Layout with gpui-component

Using `gpui-component`'s `TabPanel` for settings categories:

```rust
use gpui::*;
use gpui_component::tab::{Tab, TabPanel};

struct SettingsWindow {
    active_tab: SettingsTab,
    config_model: Model<ConfigModel>,
}

#[derive(Clone, PartialEq)]
enum SettingsTab {
    General,
    Appearance,
    Terminal,
    Keybindings,
    Ime,
    Mcp,
}

impl Render for SettingsWindow {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let config = self.config_model.read(cx);

        v_flex()
            .size_full()
            .child(
                // Toolbar-style tab bar (macOS HIG)
                h_flex()
                    .child(tab_button("General", SettingsTab::General, &self.active_tab, cx))
                    .child(tab_button("Appearance", SettingsTab::Appearance, &self.active_tab, cx))
                    .child(tab_button("Terminal", SettingsTab::Terminal, &self.active_tab, cx))
                    .child(tab_button("Keybindings", SettingsTab::Keybindings, &self.active_tab, cx))
                    .child(tab_button("IME", SettingsTab::Ime, &self.active_tab, cx))
                    .child(tab_button("MCP", SettingsTab::Mcp, &self.active_tab, cx))
            )
            .child(
                // Tab content
                match self.active_tab {
                    SettingsTab::General => self.render_general(config, cx),
                    SettingsTab::Appearance => self.render_appearance(config, cx),
                    SettingsTab::Terminal => self.render_terminal(config, cx),
                    SettingsTab::Keybindings => self.render_keybindings(config, cx),
                    SettingsTab::Ime => self.render_ime(config, cx),
                    SettingsTab::Mcp => self.render_mcp(config, cx),
                }
            )
    }
}
```

### GPUI Data Binding Pattern

GPUI uses `Model<T>` as the reactive state container. When the model is updated via `cx.notify()`, all views observing it re-render automatically:

```rust
pub struct ConfigModel {
    config: AppConfig,
}

impl ConfigModel {
    pub fn update_font_size(&mut self, size: f32, cx: &mut ModelContext<Self>) {
        self.config.font.size = size;
        cx.notify(); // Triggers re-render of all observing views
    }
}

// In a settings view:
fn render_font_size_slider(&self, config: &ConfigModel, cx: &mut ViewContext<Self>) -> impl IntoElement {
    let model = self.config_model.clone();

    h_flex()
        .child(label("Font Size"))
        .child(
            slider()
                .min(6.0)
                .max(72.0)
                .value(config.config.font.size)
                .on_change(move |value, cx| {
                    model.update(cx, |m, cx| {
                        m.update_font_size(value, cx);
                    });
                    // Also write to TOML file (debounced)
                    schedule_config_write(cx);
                })
        )
        .child(label(format!("{:.0}pt", config.config.font.size)))
}
```

### Available gpui-component Widgets for Settings

| Widget | Settings Use Case | gpui-component |
|--------|-------------------|----------------|
| `Slider` | Font size, opacity, scrollback | `slider()` |
| `Switch` / `Toggle` | Cursor blink, ligatures, blur | `switch()` |
| `Dropdown` / `Select` | Cursor style, shell, theme | `dropdown()` |
| `TextInput` | Font family, shell path, socket path | `text_input()` |
| `ColorPicker` | Foreground, background, ANSI colors | `color_picker()` |
| `NumberInput` | Scrollback lines, line height | `number_input()` |
| `Button` | Reset, Open Config File | `button()` |
| `TabBar` / `Tab` | Settings categories | `TabBar::new()` |
| `Settings` | **Complete multi-page settings UI** | `Settings::new()` |
| `SettingPage` / `SettingGroup` / `SettingItem` | Settings structure | Part of Settings API |
| `ScrollArea` | Long settings lists | `scroll_area()` |
| `Divider` | Section separators | `divider()` |
| `Label` | Setting names, descriptions | `label()` |

### gpui-component Built-In Settings Widget

The `gpui-component` crate provides a **complete `Settings` component** with sidebar navigation, grouping, and automatic field rendering. This is the recommended approach for fastest implementation:

```rust
use gpui_component::{Settings, SettingPage, SettingGroup, SettingItem, SettingField};

Settings::new("crux-settings")
    .pages(vec![
        SettingPage::new("General")
            .default_open(true)
            .group(
                SettingGroup::new()
                    .title("Shell")
                    .items(vec![
                        SettingItem::new(
                            "Default Shell",
                            SettingField::dropdown(
                                vec![("/bin/zsh", "zsh"), ("/bin/bash", "bash")],
                                |cx| AppSettings::global(cx).shell.clone(),
                                |val, cx| {
                                    AppSettings::update(cx, |s| s.shell = val.to_string());
                                },
                            )
                        ),
                    ])
            ),
        SettingPage::new("Appearance")
            .groups(vec![
                SettingGroup::new()
                    .title("Font")
                    .items(vec![
                        SettingItem::new(
                            "Font Size",
                            SettingField::number_input(
                                NumberFieldOptions { min: 8.0, max: 72.0, step: 1.0, ..Default::default() },
                                |cx| AppSettings::global(cx).font_size as f64,
                                |val, cx| {
                                    AppSettings::update(cx, |s| s.font_size = val as f32);
                                    cx.emit(SettingsChanged::FontSize(val as f32));
                                },
                            )
                        ),
                        SettingItem::new(
                            "Theme Color",
                            SettingField::color_picker(
                                |cx| AppSettings::global(cx).theme_color,
                                |color, cx| {
                                    AppSettings::update(cx, |s| s.theme_color = color);
                                },
                            )
                        ),
                    ]),
            ]),
    ])
```

This renders as a **macOS-style sidebar settings window** with automatic save/reset support.

**Implementation approach comparison**:

| Approach | Speed | Flexibility | macOS Native Feel |
|----------|-------|-------------|-------------------|
| `Settings` component | **Fast** (pre-built) | Limited | **Excellent** |
| Custom `TabBar` + forms | Moderate | **Full control** | Good (manual work) |
| Hybrid (Settings + custom tabs) | Moderate | Good | **Excellent** |

**Recommendation**: Start with the `Settings` component for initial implementation, then customize or replace individual pages as needed.

### Zed's Settings Architecture Lessons

From [How We Rebuilt Settings in Zed](https://zed.dev/blog/settings-ui):

1. **Files as the organizing principle**: Treat the config file structure (not UI abstractions) as the primary organizational structure. Settings UI maps directly to the TOML file sections.
2. **Strongly-typed settings**: Use a single consolidated `CruxSettings` struct with `Global` trait, not scattered registrations.
3. **Direct mapping**: Map setting types directly to UI controls without intermediate macro layers.

```rust
// Zed's approach: Type → UI control mapping
// CruxSettings struct field → SettingField widget → config.toml section

struct CruxSettings {
    font: FontConfig,       // → SettingPage("Appearance") → [font] in TOML
    terminal: TermConfig,   // → SettingPage("Terminal")   → [terminal] in TOML
    window: WindowConfig,   // → SettingPage("Appearance") → [window] in TOML
    shell: ShellConfig,     // → SettingPage("General")    → [shell] in TOML
}
```

### GPUI Global Settings Pattern

For settings that affect multiple windows (terminal view + settings window), use GPUI's `Global` trait:

```rust
use gpui::Global;

impl Global for CruxSettings {}

// Initialize at app startup
cx.set_global(CruxSettings::load_from_toml());

// Read from any window (lock-free)
let settings = cx.global::<CruxSettings>();

// Update from settings window (triggers re-render of all observers)
cx.update_global::<CruxSettings, _>(|settings, _| {
    settings.font.size = new_size;
});
```

**Important GPUI 0.2.x API change**: The modern API passes `Window` and `Context<Self>` explicitly. The old `WindowContext` and `ViewContext<T>` types are deprecated:

```rust
// Correct (GPUI 0.2.x):
impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ...
    }
}

// Incorrect (old API):
// fn render(&mut self, cx: &mut ViewContext<Self>) { ... }
```

---

## 11. Terminal Settings UI Patterns

### iTerm2: The Gold Standard

iTerm2 uses a **two-tier hierarchy**: global settings + profile-specific settings.

**Top-Level Tabs**: General, Appearance, Profiles, Keys, Arrangements, Advanced

**Profile Subtabs**: General, Colors, Text, Window, Terminal, Session, Keys, Advanced

**Key architectural features**:
- **Profile inheritance**: Custom profiles inherit from Default, override only changed values
- **Dynamic Profiles (JSON)**: Version-controllable profile definitions
- **Non-modal window**: Settings stays open, changes apply immediately
- **Search bar**: Full-text search across all preferences
- **Profile tags**: Search/filter profiles by keywords

**What to adopt for Crux**: Profile inheritance model, search across settings, non-modal window.

### Warp: Modern UX

**Approach**: Minimal, command-palette-driven settings.

**Access**: ⌘, (traditional) + ⌘P command palette (search settings)

**Settings Structure**: Appearance, Features, Session, Keybindings (only 4 categories)

**Key patterns**:
- Command palette searches settings as well as commands
- Live theme preview with sample terminal output
- Immediate apply (no Apply button)
- Community theme gallery

**What to adopt for Crux**: Command palette integration (⌘K), live theme preview, minimal category count.

### VS Code: Dual GUI/JSON

**Architecture**: GUI editor is a rendered view over `settings.json`. Both views show the same data.

**Three-tier hierarchy**: Default (read-only) → User (`~/.config/Code/User/settings.json`) → Workspace (`.vscode/settings.json`)

**Key features**:
- `@modified` filter: Show only non-default settings
- Blue vertical line: Visual indicator for modified settings
- Gear icon per setting: Reset to default
- Fuzzy search: Matches setting key, display name, description, enum values
- Scope toggles: Switch between User/Workspace views

**What to adopt for Crux**: `@modified` filter, per-setting reset icon, fuzzy search, blue modified indicator.

### macOS Human Interface Guidelines

Apple's HIG for Settings windows:

| Guideline | Recommendation |
|-----------|---------------|
| Window title | "Settings" (modern macOS 13+) |
| Shortcut | ⌘, (mandatory) |
| Tab navigation | Toolbar-based icons (not NSTabView) |
| Tab shortcuts | ⌘1 through ⌘9 |
| Window behavior | Non-modal, singleton |
| Traffic lights | Minimize/maximize disabled, not removed |
| Tab icons | SF Symbols for consistency |
| Nesting | Max 2 levels deep |
| First tab | Always "General" |

### Settings UI Comparison Matrix

| Feature | iTerm2 | Warp | VS Code | **Crux (Planned)** |
|---------|--------|------|---------|---------------------|
| GUI settings | Yes | Yes | Yes | **Yes** |
| Config file | plist (hidden) | Internal | JSON | **TOML (visible)** |
| Bidirectional sync | No | No | Yes | **Yes** |
| Search settings | Yes | Via palette | Yes | **Yes** |
| Live preview | Partial | Themes only | No | **Yes** |
| Profile inheritance | Yes | No | Workspace | **Yes** |
| Modified indicator | No | No | Yes | **Yes** |
| Per-setting reset | No | No | Yes | **Yes** |
| Command palette | No | Yes | Yes | **Yes** |

---

## 12. Bidirectional Config Sync

### Architecture

```
┌──────────────────────────────────────────────────┐
│              ConfigManager (singleton)            │
│  ┌───────────────────────────────────────────┐   │
│  │   ArcSwap<AppConfig>  (source of truth)   │   │
│  └───────────────────────────────────────────┘   │
│         ↑ write              ↑ write             │
│    ┌────┴─────┐        ┌────┴──────┐            │
│    │ GUI Edit │        │ File Edit │            │
│    │ (slider) │        │ (vim/code)│            │
│    └────┬─────┘        └────┬──────┘            │
│         │                    │                    │
│    toml_edit              notify                  │
│    write-back             watcher                 │
│         ↓                    ↓                    │
│    ┌─────────────────────────────────────┐       │
│    │      config.toml (persistent)       │       │
│    └─────────────────────────────────────┘       │
└──────────────────────────────────────────────────┘
```

### toml_edit for Format-Preserving Writes

The `toml` crate destroys comments and formatting on serialization. Use `toml_edit` instead:

```rust
use toml_edit::{DocumentMut, value};

fn update_config_preserving_format(
    config_path: &Path,
    key: &str,
    section: &str,
    new_value: toml_edit::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(config_path)?;
    let mut doc = content.parse::<DocumentMut>()?;

    // Update single value, preserving all comments and formatting
    doc[section][key] = toml_edit::Item::Value(new_value);

    std::fs::write(config_path, doc.to_string())?;
    Ok(())
}

// Example: Update font size while keeping all comments intact
update_config_preserving_format(
    &config_path,
    "size",
    "font",
    value(16.0),
)?;
```

**Before** (user's hand-crafted config):
```toml
# My terminal config
[font]
family = "JetBrains Mono"  # Love this font
size = 14.0                 # Default size
ligatures = false
```

**After** (GUI changes font size to 16):
```toml
# My terminal config
[font]
family = "JetBrains Mono"  # Love this font
size = 16.0                 # Default size
ligatures = false
```

Comments, whitespace, and ordering are all preserved.

**Crate comparison**:

| Crate | Read | Write | Preserves Comments | Preserves Formatting |
|-------|------|-------|--------------------|----------------------|
| `toml` 0.8 | Yes | Yes | **No** | **No** |
| `toml_edit` 0.23 | Yes | Yes | **Yes** | **Yes** |

**Rule: Use `toml` for reading, `toml_edit` for writing.**

### Preventing Write-Read Loops

The critical problem: GUI writes → file watcher fires → GUI reloads → potential infinite loop.

**Solution: Timestamp tracking**

```rust
use std::time::{Duration, SystemTime};
use std::sync::Mutex;

struct ConfigManager {
    last_write_time: Mutex<Option<SystemTime>>,
    config_path: PathBuf,
}

impl ConfigManager {
    fn write_config(&self, config: &AppConfig) -> Result<()> {
        let toml_str = self.serialize_preserving_format(config)?;
        std::fs::write(&self.config_path, toml_str)?;

        // Record when WE wrote the file
        let metadata = std::fs::metadata(&self.config_path)?;
        *self.last_write_time.lock().unwrap() = Some(metadata.modified()?);
        Ok(())
    }

    fn on_file_changed(&self) -> Result<()> {
        let metadata = std::fs::metadata(&self.config_path)?;
        let file_mtime = metadata.modified()?;

        let last_write = self.last_write_time.lock().unwrap();
        if let Some(our_write_time) = *last_write {
            // 200ms threshold accounts for filesystem timestamp granularity
            if file_mtime <= our_write_time + Duration::from_millis(200) {
                return Ok(()); // Our own write, ignore
            }
        }
        drop(last_write);

        // External modification — reload
        self.reload_config_from_disk()?;
        Ok(())
    }
}
```

**Alternative approaches**:

| Strategy | Mechanism | Robustness |
|----------|-----------|------------|
| Timestamp tracking | Compare file mtime vs last write | **Best** (recommended) |
| Generation counter | Atomic counter incremented on write | Good, but race-prone |
| Ignore-next flag | AtomicBool set before write, cleared on event | Simple but fragile |

### Conflict Resolution: GUI-Wins-During-Focus

When the settings window is open and the file changes externally:

```rust
fn on_file_changed(&self) -> Result<()> {
    let new_config = self.load_from_disk()?;

    if self.settings_window_is_open() {
        // Defer reload — show notification instead
        self.pending_file_version = Some(new_config);
        self.show_notification("Config file changed externally. Reload?");
    } else {
        // No settings window — auto-reload immediately
        self.apply_config(new_config);
    }
    Ok(())
}
```

**On parse error**: Always keep the old configuration. Show user-facing error notification. Never leave the terminal in a broken state.

### State Management with GPUI Model

```rust
use gpui::*;
use arc_swap::ArcSwap;

pub struct ConfigModel {
    config: AppConfig,
    watcher_active: bool,
}

impl ConfigModel {
    pub fn update_setting<F>(&mut self, updater: F, cx: &mut ModelContext<Self>)
    where
        F: FnOnce(&mut AppConfig),
    {
        updater(&mut self.config);
        cx.notify(); // Re-render all observing views

        // Debounced write to TOML (100ms after last change)
        cx.spawn(|this, mut cx| async move {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            this.update(&mut cx, |this, _| {
                this.write_to_disk();
            }).ok();
        }).detach();
    }
}
```

---

## 13. Settings UX Components

### Fuzzy Search

Implement fzf-style fuzzy search across all settings:

```
Search scope: setting key + display label + description + enum values

Example: typing "fosi" matches:
  → font.size (key match)
  → Font Size (label match)
  → "Font size in points" (description match)

Special filters:
  @modified  — Show only non-default settings
  @tab:appearance — Filter by tab
```

### Modified Indicator

Visual indicator for settings that differ from their default value:

```
┌────────────────────────────────────────┐
│ ▌ Font Size        [16    ] ←→   ⟲    │  ← Blue bar + reset icon
│   Font Family      [Menlo         ▼]  │  ← No indicator (default)
│ ▌ Ligatures        [✓]            ⟲    │  ← Modified
│   Cursor Style     [Block         ▼]  │  ← Default
└────────────────────────────────────────┘
```

- **Blue vertical bar**: Setting has been changed from default
- **⟲ Reset icon**: Appears only on modified settings, click to restore default
- **Bold label**: Optional, for emphasis on modified values

### Font Preview Panel

Terminal-specific font preview showing critical characters:

```
┌─ Font Settings ──────────────────────────┐
│ Family: [JetBrains Mono           ▼]    │
│ Size:   [14    ] ←─────────────→         │
│ ☑ Enable ligatures                       │
│                                          │
│ Preview:                                 │
│ ┌──────────────────────────────────────┐ │
│ │ The quick brown fox jumps over 0O1l  │ │
│ │ fn main() { println!("한글 테스트"); } │ │
│ │ != => -> >= <= /* */ // ===           │ │
│ │ ┌─┐ └─┘ ├─┤ ─── ═══                 │ │
│ │     ⎇  λ  ∑  ∞                   │ │
│ └──────────────────────────────────────┘ │
└──────────────────────────────────────────┘
```

Preview includes:
- Basic ASCII with commonly confused characters (0O, 1l, Il)
- CJK characters (한글) to verify fallback chain
- Programming ligatures (if enabled): `!=`, `=>`, `->`, `>=`
- Box drawing characters: `┌─┐ └─┘ ├─┤`
- Powerline/Nerd Font symbols: `  ⎇`
- Unicode symbols: `λ ∑ ∞`

### Color Scheme Editor

```
┌─ Colors ─────────────────────────────────┐
│ Theme: [Tokyo Night          ▼]          │
│                                          │
│ ┌── Preview ───────────────────────────┐ │
│ │ $ ls -la                             │ │
│ │ drwxr-xr-x  user  Documents/        │ │
│ │ -rw-r--r--  user  README.md          │ │
│ │ $ git status                         │ │
│ │ On branch main                       │ │
│ │ Changes not staged:                  │ │
│ │   modified: src/main.rs              │ │
│ └──────────────────────────────────────┘ │
│                                          │
│ Foreground  [■ #c0caf5]                  │
│ Background  [■ #1a1b26]                  │
│ Cursor      [■ #c0caf5]                  │
│ Selection   [■ #33467c]                  │
│                                          │
│ ANSI Colors:                             │
│ Normal: ■ ■ ■ ■ ■ ■ ■ ■                 │
│ Bright: ■ ■ ■ ■ ■ ■ ■ ■                 │
│                                          │
│ [Import Theme...] [Export Theme...]      │
└──────────────────────────────────────────┘
```

Features:
- Embedded terminal preview with curated sample output
- Theme dropdown with instant live preview
- Individual color wells for fine-tuning
- Import/export in JSON, iTerm2, terminal.sexy formats
- 16 ANSI color grid (8 normal + 8 bright)

### Key Binding Recorder

```
┌─ Keybindings ────────────────────────────┐
│ Search: [                           🔍]  │
│                                          │
│ Action              Keybinding    Reset  │
│ ─────────────────────────────────────── │
│ New Tab             ⌘T                   │
│ Close Tab           ⌘W                   │
│ Split Right         ⌘D            ⟲     │
│ ▌Split Down         ⌘⇧D           ⟲     │  ← Modified
│ Next Pane           ⌘]                   │
│ Previous Pane       ⌘[                   │
│                                          │
│ ⚠ ⌘⇧D conflicts with "Bookmark" (built-in) │
│   [Keep Both] [Replace "Bookmark"]       │
└──────────────────────────────────────────┘
```

**Recording flow**:
1. Click on keybinding cell → enters recording mode (cell highlights)
2. Press desired key combination → display as `⌘⇧P`
3. Check for conflicts against system, built-in, and custom bindings
4. If conflict: show inline warning with resolution options
5. ESC cancels recording

**Conflict detection priority**:

| Priority | Source | Override Allowed |
|----------|--------|-----------------|
| 1 (highest) | macOS system (⌘Q, ⌘W, ⌘⌥Esc) | No |
| 2 | Built-in Crux bindings | Yes (with warning) |
| 3 | User custom bindings | Yes |

### IME Settings Tab

```
┌─ IME ────────────────────────────────────┐
│ ☑ Enable Vim auto-switch                 │
│   Switch to ASCII in Normal mode,        │
│   restore in Insert mode                 │
│                                          │
│ Composition overlay style:               │
│   ○ Inline (next to cursor)             │
│   ● Floating (above cursor)             │
│   ○ Status bar                           │
│                                          │
│ Input source for Normal mode:            │
│   [ABC - English            ▼]           │
│                                          │
│ ☐ Show input source indicator            │
└──────────────────────────────────────────┘
```

### MCP Security Tab

```
┌─ MCP Server ─────────────────────────────┐
│ ☑ Enable MCP server                      │
│                                          │
│ Socket path: [~/.crux/mcp.sock     ]    │
│                                          │
│ Security policy:                         │
│   ● Ask before executing commands        │
│   ○ Allow all (trusted environment)      │
│   ○ Read-only (inspection tools only)    │
│                                          │
│ Command whitelist:                       │
│   [ls, cat, git, cargo, npm       ]     │
│   (comma-separated, empty = allow all)   │
│                                          │
│ Blocked tools:                           │
│   [crux_send_keys                  ]    │
│   (comma-separated)                      │
│                                          │
│ Rate limit: [60    ] calls/minute        │
└──────────────────────────────────────────┘
```

---

## Additional Crate Dependencies (GUI Settings)

```toml
[dependencies]
toml_edit = "0.23"     # Format-preserving TOML write-back
arc-swap = "1.7"       # Lock-free config access
notify-debouncer-mini = "0.5"  # Debounced file watching
# gpui and gpui-component already in workspace
```

---

## Additional Sources (GUI Settings)

### Terminal Settings UX
- [iTerm2 Preferences Documentation](https://iterm2.com/documentation-preferences.html) — Profile system, tab organization
- [iTerm2 Dynamic Profiles](https://iterm2.com/documentation-dynamic-profiles.html) — JSON-based profile inheritance
- [Warp Theme Design Blog](https://www.warp.dev/blog/how-we-designed-themes-for-the-terminal-a-peek-into-our-process) — Modern theme UX
- [VS Code Settings Architecture](https://code.visualstudio.com/docs/getstarted/settings) — Dual GUI/JSON model, @modified filter
- [macOS HIG: Settings](https://developer.apple.com/design/human-interface-guidelines/settings) — Apple guidelines for preferences windows

### Bidirectional Sync
- [toml_edit crate](https://crates.io/crates/toml_edit) — Format-preserving TOML manipulation
- [toml_edit vs toml comparison](https://epage.github.io/blog/2023/01/toml-vs-toml-edit/) — When to use which
- [arc-swap documentation](https://docs.rs/arc-swap/latest/arc_swap/) — Lock-free atomic pointer swap
- [arc-swap patterns guide](https://docs.rs/arc-swap/latest/arc_swap/docs/patterns/index.html) — Observer and state patterns
- [notify-debouncer-mini](https://docs.rs/notify-debouncer-mini/latest/notify_debouncer_mini/) — Debounced file watcher
- [Runtime Configuration Reloading in Rust](https://vorner.github.io/2019/08/11/runtime-configuration-reloading.html) — arc-swap author's guide

### GPUI
- [GPUI Technical Overview](https://beckmoulton.medium.com/gpui-a-technical-overview-of-the-high-performance-rust-ui-framework-powering-zed-ac65975cda9f) — Model/View reactive architecture
- [gpui-component crate](https://crates.io/crates/gpui-component) — 60+ widgets for settings UI
- [Zed Editor Configuration](https://zed.dev/docs/configuring-zed) — Reference for GPUI-based settings
