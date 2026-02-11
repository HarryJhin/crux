---
title: "macOS Accessibility (VoiceOver)"
description: "NSAccessibility protocol, AccessKit integration, GPUI accessibility gaps, terminal a11y patterns from Warp/Ghostty, WCAG criteria, system settings, implementation priority"
date: 2026-02-12
phase: [future]
topics: [accessibility, voiceover, accesskit, wcag]
status: final
related:
  - terminal-architecture.md
  - ../gpui/framework.md
---

# macOS Accessibility (VoiceOver)

> 작성일: 2026-02-12
> 목적: Crux 터미널의 접근성(Accessibility) 구현 전략 — VoiceOver 지원, AccessKit 통합, WCAG 준수, 시스템 접근성 설정 반영

---

## 목차

1. [개요](#1-개요)
2. [macOS Accessibility Architecture](#2-macos-accessibility-architecture)
3. [GPUI Accessibility Status](#3-gpui-accessibility-status)
4. [AccessKit — Recommended Approach](#4-accesskit--recommended-approach)
5. [Existing Terminal Accessibility Patterns](#5-existing-terminal-accessibility-patterns)
6. [WCAG Criteria for Terminals](#6-wcag-criteria-for-terminals)
7. [System Accessibility Settings](#7-system-accessibility-settings)
8. [Implementation Priority](#8-implementation-priority)
9. [Crux Implementation Recommendations](#9-crux-implementation-recommendations)

---

## 1. 개요

Terminal accessibility is historically poor. Most terminal emulators provide minimal or no screen reader support, forcing visually impaired users to rely on platform-level screen reading of raw window content — which produces incoherent output for terminal UIs.

The challenge:
- Terminal content is a **character grid**, not a structured document
- Content changes rapidly (scrolling output, animations, progress bars)
- TUI applications (vim, tmux) have their own internal structure invisible to the OS
- GPUI (Crux's rendering framework) has **zero built-in accessibility support**

Despite these challenges, meaningful accessibility is achievable through careful design.

Sources: [macOS Accessibility Programming Guide](https://developer.apple.com/library/archive/documentation/Accessibility/Conceptual/AccessibilityMacOSX/), [AccessKit](https://github.com/AccessKit/accesskit), [WCAG 2.1](https://www.w3.org/WAI/WCAG21/quickref/), [Warp Blog: Accessibility](https://www.warp.dev/blog/accessibility)

---

## 2. macOS Accessibility Architecture

### NSAccessibility Protocol

macOS accessibility is built on the NSAccessibility protocol. Applications expose an **accessibility tree** of elements, each with:

- **Role**: What the element is (window, text area, button, etc.)
- **Value**: Current content (text, number, etc.)
- **Label**: Human-readable description
- **Actions**: What can be done (press, increment, etc.)
- **Notifications**: State changes (value changed, selection changed, etc.)

### Key Roles for Terminals

| NSAccessibility Role | Terminal Usage |
|---------------------|----------------|
| `AXWindow` | Terminal window |
| `AXTabGroup` | Tab bar |
| `AXTextArea` | Terminal content area |
| `AXStaticText` | Individual text lines or command regions |
| `AXGroup` | Command boundary regions (with OSC 133) |

### How VoiceOver Interacts with Text

VoiceOver reads text content from `AXTextArea` elements:

1. **AXValue**: The full text content
2. **AXSelectedText**: Currently selected text
3. **AXVisibleCharacterRange**: What's currently on screen
4. **AXNumberOfCharacters**: Total character count
5. **AXInsertionPointLineNumber**: Current cursor line

For terminals, the "text area" is the visible grid content plus scrollback.

### Accessibility Notifications

| Notification | When to Fire |
|-------------|--------------|
| `AXValueChanged` | Terminal content changed (new output) |
| `AXSelectedTextChanged` | Selection changed |
| `AXFocusedUIElementChanged` | Focus moved between panes/tabs |
| `AXLayoutChanged` | Terminal resized, pane layout changed |
| `AXAnnouncementRequested` | Important events (command completed, error) |

---

## 3. GPUI Accessibility Status

### Current State: No Accessibility Support

GPUI has **zero built-in accessibility support** as of v0.2.x. This is confirmed by the Zed team:

- No accessibility tree construction
- No NSAccessibility protocol implementation
- No VoiceOver, Voice Control, or Switch Control support
- No keyboard accessibility for GPUI UI elements

### Implications for Crux

Since GPUI doesn't provide accessibility primitives, Crux must implement them directly:

1. **Bypass GPUI** for accessibility: Interface with the macOS accessibility APIs directly via `objc2-app-kit`
2. **Or use AccessKit**: A cross-platform accessibility toolkit that bridges to NSAccessibility

### Zed's Accessibility Plans

The Zed editor has acknowledged accessibility as a gap. As of early 2025, there were discussions about adding AccessKit integration to GPUI, but no implementation has been merged. Crux should not depend on upstream GPUI accessibility landing.

---

## 4. AccessKit — Recommended Approach

### Overview

[AccessKit](https://github.com/AccessKit/accesskit) is a cross-platform accessibility toolkit for Rust. It provides:

- A platform-agnostic accessibility tree API
- Platform adapters for macOS (NSAccessibility), Windows (UI Automation), Linux (AT-SPI)
- Integration with winit, egui, and other Rust UI frameworks

### Architecture

```
Crux Terminal State
    │
    ▼
AccessKit Tree Builder ──→ AccessKit Node Tree
    │                          │
    ▼                          ▼
accesskit_macos ──→ NSAccessibility Protocol ──→ VoiceOver
```

### Key AccessKit Types

```rust
use accesskit::{NodeBuilder, NodeId, Role, Tree, TreeUpdate};

fn build_accessibility_tree(terminal: &Term) -> TreeUpdate {
    let mut nodes = Vec::new();

    // Root node: the terminal
    let mut root = NodeBuilder::new(Role::Terminal);
    root.set_name("Terminal");

    // Add visible lines as text nodes
    let grid = terminal.grid();
    for (i, line) in grid.display_iter().enumerate() {
        let text: String = line.iter().map(|cell| cell.c).collect();
        let text = text.trim_end();

        if !text.is_empty() {
            let mut line_node = NodeBuilder::new(Role::StaticText);
            line_node.set_name(text.to_string());
            line_node.set_value(text.to_string());

            let line_id = NodeId(i as u64 + 1);
            root.push_child(line_id);
            nodes.push((line_id, line_node.build()));
        }
    }

    let root_id = NodeId(0);
    nodes.push((root_id, root.build()));

    TreeUpdate {
        nodes,
        tree: Some(Tree::new(root_id)),
        focus: root_id,
    }
}
```

### macOS Adapter

```rust
use accesskit_macos::Adapter;

// Create adapter (once, during window creation)
let adapter = Adapter::new(
    ns_view,  // The NSView for the terminal
    initial_tree_update,
    Box::new(action_handler),
);

// On terminal content change:
adapter.update(new_tree_update);
```

### Crate: `accesskit = "0.17"`, `accesskit_macos = "0.18"`

AccessKit is actively maintained, used by egui, and has stable macOS support.

---

## 5. Existing Terminal Accessibility Patterns

### Warp's Announcement Approach

Warp (a Rust-based terminal) uses a simplified accessibility model:

1. **Announcement-based**: Instead of exposing the full terminal buffer, Warp announces significant events:
   - "Command completed with exit code 0"
   - "Error: command not found"
   - New prompt ready
2. **Block-based navigation**: Users navigate between command blocks rather than individual lines
3. **Structured output**: Because Warp has its own input editor (not a traditional terminal), it can provide rich accessibility for the input area

**Pros**: Simple, focused on what matters
**Cons**: Misses arbitrary terminal content, doesn't help with TUI apps

### Ghostty's AccessKit Plans

Ghostty (by Mitchell Hashimoto) has stated plans to integrate AccessKit for accessibility. As of early 2025, this was not yet implemented but was on the roadmap.

### iTerm2's Accessibility

iTerm2 provides:
- Full text area accessibility (AXTextArea)
- Per-line text exposure to VoiceOver
- Selection tracking
- "Announce" mode for important output

### Terminal.app's Accessibility

macOS Terminal.app has the best built-in accessibility of any terminal:
- Full VoiceOver support
- Per-line navigation
- Automatic announcement of new output
- Integration with macOS accessibility settings

This is the gold standard Crux should aspire to.

---

## 6. WCAG Criteria for Terminals

### Applicable WCAG 2.1 Success Criteria

| Level | Criterion | Terminal Relevance |
|-------|-----------|-------------------|
| A | 1.1.1 Non-text Content | Icons need text alternatives |
| A | 1.3.1 Info and Relationships | Command regions (OSC 133) provide structure |
| A | 1.4.1 Use of Color | Don't convey info by color alone (exit status) |
| AA | 1.4.3 Contrast (Minimum) | 4.5:1 for normal text, 3:1 for large text |
| AA | 1.4.4 Resize Text | Support font size scaling up to 200% |
| AA | 1.4.11 Non-text Contrast | UI controls need 3:1 against adjacent |
| A | 2.1.1 Keyboard | All functionality available via keyboard |
| A | 2.1.2 No Keyboard Trap | User can always exit focus |
| AA | 2.4.7 Focus Visible | Clear focus indicator on active pane/tab |
| A | 4.1.2 Name, Role, Value | All UI components exposed to AT |

### Contrast Requirements

```rust
fn contrast_ratio(fg: Color, bg: Color) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(c: Color) -> f64 {
    let r = linearize(c.r as f64 / 255.0);
    let g = linearize(c.g as f64 / 255.0);
    let b = linearize(c.b as f64 / 255.0);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn linearize(v: f64) -> f64 {
    if v <= 0.03928 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

// WCAG AA requires:
// - Normal text: ratio >= 4.5
// - Large text (14pt bold or 18pt): ratio >= 3.0
```

---

## 7. System Accessibility Settings

### macOS Accessibility Preferences

Users can enable system-wide accessibility settings. Crux should respect these:

#### Reduce Motion

```rust
use objc2_app_kit::NSWorkspace;

fn reduce_motion_enabled() -> bool {
    unsafe {
        NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    }
}
```

When enabled:
- Disable cursor blink animation
- Disable smooth scrolling (use instant jump)
- Disable tab transition animations
- Disable window opacity animations

#### Increase Contrast

```rust
fn increase_contrast_enabled() -> bool {
    unsafe {
        NSWorkspace::sharedWorkspace().accessibilityDisplayShouldIncreaseContrast()
    }
}
```

When enabled:
- Use high-contrast color scheme
- Add borders to focused elements
- Increase selection highlight contrast

#### Reduce Transparency

```rust
fn reduce_transparency_enabled() -> bool {
    unsafe {
        NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceTransparency()
    }
}
```

When enabled:
- Set window opacity to 1.0 (ignore configured transparency)
- Use solid backgrounds instead of blur effects

#### Differentiate Without Color

```rust
fn differentiate_without_color_enabled() -> bool {
    unsafe {
        NSWorkspace::sharedWorkspace().accessibilityDisplayShouldDifferentiateWithoutColor()
    }
}
```

When enabled:
- Add symbols/icons alongside color-coded indicators
- Exit status: use checkmark/X in addition to green/red

### Observing Changes

```rust
// Listen for accessibility setting changes
NSWorkspace::sharedWorkspace().notificationCenter()
    .addObserver_selector_name_object(
        observer,
        sel!(accessibilitySettingsChanged:),
        NSWorkspace::accessibilityDisplayOptionsDidChangeNotification(),
        None,
    );
```

---

## 8. Implementation Priority

### Priority 1: System Settings (Low Effort, High Impact)

These require no AccessKit and directly improve usability for all users:

| Feature | Effort | Impact |
|---------|--------|--------|
| Respect Reduce Motion | Low | Epilepsy safety |
| Respect Increase Contrast | Low | Low vision |
| Respect Reduce Transparency | Low | Low vision |
| WCAG-compliant default theme | Medium | All users |
| Font scaling (Cmd+/Cmd-) | Low | Low vision |
| Focus visible indicator | Low | Keyboard users |

### Priority 2: AccessKit Text Exposure (Medium Effort, High Impact)

| Feature | Effort | Impact |
|---------|--------|--------|
| Expose terminal buffer as AXTextArea | Medium | Screen reader users |
| Line-by-line navigation | Medium | VoiceOver users |
| Selection change notifications | Low | Screen reader users |
| Cursor position tracking | Low | Screen reader users |

### Priority 3: Semantic Navigation (Higher Effort, Medium Impact)

| Feature | Effort | Impact |
|---------|--------|--------|
| Command block navigation (OSC 133) | Medium | Power screen reader users |
| Announce command completion | Low | Screen reader users |
| Announce errors | Low | Screen reader users |
| Tab/pane focus announcements | Low | Screen reader users |

### Priority 4: Full VoiceOver Parity (High Effort)

| Feature | Effort | Impact |
|---------|--------|--------|
| VoiceOver rotor integration | High | Advanced VoiceOver users |
| Braille display support | High | Braille users |
| Voice Control support | High | Motor impaired users |

---

## 9. Crux Implementation Recommendations

### Immediate (All Phases)

1. **WCAG-compliant default color scheme**: Ensure all default colors meet 4.5:1 contrast ratio
2. **System settings observation**: React to Reduce Motion, Increase Contrast, Reduce Transparency
3. **Keyboard-first design**: Every feature must be keyboard accessible (no mouse-only interactions)
4. **Font scaling**: Cmd+Plus/Minus/Zero for zoom

### Phase 2–3

5. **AccessKit integration**: Expose terminal buffer as AXTextArea
6. **New output announcements**: Use `AXAnnouncementRequested` for command completion
7. **Focus management**: Proper focus tracking across tabs/panes

### Future Phase

8. **OSC 133 semantic navigation**: Navigate between commands via accessibility tree
9. **Structured command blocks**: Expose command regions as AXGroup nodes
10. **VoiceOver rotor**: Custom rotor items for command navigation

### Architecture

```rust
/// Accessibility module for Crux terminal
pub struct TerminalAccessibility {
    adapter: accesskit_macos::Adapter,
    /// Cached text content for diff-based updates
    last_content: Vec<String>,
    /// System settings cache
    reduce_motion: bool,
    increase_contrast: bool,
    reduce_transparency: bool,
}

impl TerminalAccessibility {
    pub fn update(&mut self, terminal: &Term) {
        let current_content = extract_visible_text(terminal);

        if current_content != self.last_content {
            let tree_update = build_tree_update(&current_content, terminal);
            self.adapter.update(tree_update);
            self.last_content = current_content;
        }
    }

    pub fn announce(&self, message: &str) {
        // Fire AXAnnouncementRequested notification
        let update = TreeUpdate {
            nodes: vec![(
                ANNOUNCEMENT_NODE_ID,
                NodeBuilder::new(Role::StaticText)
                    .set_live(Live::Assertive)
                    .set_name(message.to_string())
                    .build(),
            )],
            ..Default::default()
        };
        self.adapter.update(update);
    }
}
```

### Key Crate Dependencies

```toml
[dependencies]
accesskit = "0.17"
accesskit_macos = "0.18"
objc2-app-kit = "0.2"  # For system accessibility settings
```

---

## Sources

- [macOS Accessibility Programming Guide](https://developer.apple.com/library/archive/documentation/Accessibility/Conceptual/AccessibilityMacOSX/) — Apple official documentation
- [AccessKit](https://github.com/AccessKit/accesskit) — Cross-platform accessibility toolkit for Rust
- [WCAG 2.1 Quick Reference](https://www.w3.org/WAI/WCAG21/quickref/) — W3C accessibility guidelines
- [Warp Accessibility Blog Post](https://www.warp.dev/blog/making-warp-accessible) — Warp's approach to terminal accessibility
- [NSWorkspace Accessibility API](https://developer.apple.com/documentation/appkit/nsworkspace) — System accessibility settings
- [VoiceOver User Guide](https://support.apple.com/guide/voiceover/welcome/mac) — macOS VoiceOver reference
- [accesskit_macos crate](https://docs.rs/accesskit_macos/latest/accesskit_macos/) — macOS AccessKit adapter
