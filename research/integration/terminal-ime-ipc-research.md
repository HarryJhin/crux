---
title: "Terminal Emulator IME & IPC Research Report"
description: "Comparative analysis of IME auto-switch, IPC event subscriptions, and image drag-drop across open source terminal emulators"
phase: 3
topics: ["ime", "ipc", "event-subscription", "drag-drop", "terminal-research"]
related: ["ime-clipboard.md", "ipc-protocol-design.md"]
date: 2026-02-13
---

# Terminal Emulator IME & IPC Research Report

Research on open source terminal emulator implementations for Phase 3 remaining features.

## Table of Contents

1. [Vim IME Auto-Switch: Restore on Insert Mode](#1-vim-ime-auto-switch-restore-on-insert-mode)
2. [IPC Event Subscription Implementation](#2-ipc-event-subscription-implementation)
3. [Image Drag-and-Drop in Terminals](#3-image-drag-and-drop-in-terminals)
4. [Terminal IME State Exposure](#4-terminal-ime-state-exposure)
5. [Key Takeaways](#key-takeaways)

---

## 1. Vim IME Auto-Switch: Restore on Insert Mode

### Overview

The problem: When Vim switches to Normal mode, terminals disable IME (switching to ASCII). When returning to Insert mode with Beam/Bar cursor, the previous IME input source should be restored automatically.

### Research Findings by Terminal

#### Alacritty (Rust)

**Implementation Status:** No native implementation found

**Architecture:**
- Uses `winit` library for window management and input handling
- `winit` PR #518 implemented NSTextInputClient for macOS
- DECSCUSR escape sequence support for cursor shape changes

**Key Files:**
- IME handling: Delegated to `winit` library (not in Alacritty core)
- Platform layer: `alacritty/src/` (platform-specific)
- Terminal core: `alacritty_terminal/src/` (VT emulation)

**Issues:**
- No cursor shape → IME switching integration
- Issue #748: "macOS: cannot switch input source, cannot use Pinyin"
- Issue #6631: DECSCUSR blinking mode not working correctly
- PR #7883: Attempt to show cursor in preedit area (blocked by winit #3617)

**Design Decision:** Alacritty relies on application-level plugins (Vim plugins) rather than terminal-level IME switching

**References:**
- [Alacritty GitHub](https://github.com/alacritty/alacritty)
- [winit NSTextInputClient PR #518](https://github.com/rust-windowing/winit/pull/518)
- [Alacritty Issue #748](https://github.com/alacritty/alacritty/issues/748)
- [Alacritty PR #7883](https://github.com/alacritty/alacritty/pull/7883)

---

#### WezTerm (Rust)

**Implementation Status:** Configuration-based, no automatic cursor-shape detection

**Architecture:**
- `use_ime` config option (boolean) to enable/disable IME globally
- `default_cursor_style` config: SteadyBlock, BlinkingBlock, SteadyUnderline, BlinkingUnderline, SteadyBar, BlinkingBar
- Escape sequences can override cursor style per mode

**Key Features:**
- IME processing controlled at window level via `use_ime` setting
- No built-in correlation between cursor shape and IME state
- Users must use Vim plugins for auto-switching

**Design Decision:** User-configured IME on/off, not cursor-reactive

**References:**
- [WezTerm use_ime config](https://wezterm.org/config/lua/config/use_ime.html)
- [WezTerm default_cursor_style](https://wezterm.org/config/lua/config/default_cursor_style.html)

---

#### Kitty (Python/C)

**Implementation Status:** Limited IME support, no auto-switch

**Architecture:**
- Uses GLFW library (fundamental IME limitation)
- IBus IME support via `GLFW_IM_MODULE=ibus` environment variable
- No native cursor-shape → IME switching

**Limitations:**
- "IME does not work with kitty thanks to limitations of GLFW"
- No built-in configuration for auto IME switching

**Design Decision:** GLFW constraints prevent sophisticated IME integration

**References:**
- [Kitty IME Issue #469](https://github.com/kovidgoyal/kitty/issues/469)
- [Kitty ArchWiki](https://wiki.archlinux.org/title/Kitty)

---

#### Ghostty (Zig)

**Implementation Status:** Active IME development, several known issues

**Architecture:**
- Platform-native UI with comptime interfaces for platform-specific code
- macOS: Uses NSTextInputClient (likely in apprt layer)
- Linux: GTK4 with IME support

**Known Issues:**
- macOS: Pre-edit text disappears when pressing modifier keys (Issue #4634)
- macOS: IME position incorrect with window padding (Issue #4933)
- Linux: fcitx activation issues (Discussion #3628)
- Panic when IME input occurs in scrolled-off shell (Discussion #9954)

**Key Insight:** Input method switching while preedit is active should commit the text

**Design Decision:** Platform-native IME, but no cursor-shape auto-switch documented

**References:**
- [Ghostty GitHub](https://github.com/ghostty-org/ghostty)
- [Ghostty Issue #4634](https://github.com/ghostty-org/ghostty/issues/4634)
- [Ghostty Issue #4933](https://github.com/ghostty-org/ghostty/issues/4933)
- [Ghostty Discussion #3628](https://github.com/ghostty-org/ghostty/discussions/3628)

---

#### iTerm2 (Objective-C)

**Implementation Status:** Partial support, relies on macOS settings

**Architecture:**
- `PTYTextView.m`: Implements NSTextInputClient protocol
- `hasMarkedText`, `markedRange`, `setMarkedText:selectedRange:replacementRange:`, `unmarkText`
- `inputMethodEditorLength`: Computes cell width for IME display
- `_drawingHelper.numberOfIMELines`: Tracks additional lines for IME rendering

**Known Issues:**
- Issue #10973: Dropdown mode doesn't change keyboard layout
- Issue #11356: Force keyboard only works with "Automatically switch to document's input source" off
- No automatic restore of previous input source on cursor shape change

**Key Implementation Details:**
```objc
// PTYTextView.m extracts
- (BOOL)hasMarkedText;
- (NSRange)markedRange;
- (void)setMarkedText:(id)string
       selectedRange:(NSRange)selectedRange
    replacementRange:(NSRange)replacementRange;
- (void)unmarkText;
- (NSInteger)inputMethodEditorLength; // Cell width calculation
```

**Design Decision:** Relies on macOS "Automatically switch to document's input source" system setting, not terminal-driven

**References:**
- [iTerm2 PTYTextView.m](https://github.com/gnachman/iTerm2/blob/master/sources/PTYTextView.m)
- [iTerm2 Issue #10973](https://gitlab.com/gnachman/iterm2/-/issues/10973)
- [iTerm2 Issue #11356](https://gitlab.com/gnachman/iterm2/-/issues/11356)

---

### External Tool Ecosystem

Since terminals don't natively implement cursor-shape → IME switching, the Vim/Neovim ecosystem uses external tools:

#### macism (macOS)

- **Repository:** [laishulu/macism](https://github.com/laishulu/macism)
- **Advantage:** Reliable CJKV input source switching (works around macOS bug)
- **Implementation:** Uses Accessibility API to force input source changes
- **Usage:** `macism <input-source-id>` from CLI
- **Key Insight:** Default macOS APIs fail for CJK languages when called from CLI (non-GUI). macism implements workaround.

#### im-select (Cross-platform)

- **Repository:** [daipeihust/im-select](https://github.com/daipeihust/im-select)
- **Platforms:** macOS, Windows
- **Usage:** `im-select <input-source>`

#### Vim Plugin Pattern

**vim-macos-ime:**
- **Repository:** [laishulu/vim-macos-ime](https://github.com/laishulu/vim-macos-ime)
- **Approach:**
  - On `InsertLeave`: Switch to ASCII input source
  - On `InsertEnter`: Restore previous input source
  - Uses `macism` CLI tool
- **Configuration:** Requires `set ttimeoutlen=100` to avoid Esc delay

**Mac-input.vim:**
- **Repository:** [BenSYZ/Mac-input.vim](https://github.com/BenSYZ/Mac-input.vim)
- **Feature:** Stores IME state per buffer, restores on buffer switch

**vim-barbaric:**
- **Repository:** [rlue/vim-barbaric](https://github.com/rlue/vim-barbaric)
- **Support:** fcitx, ibus, xkb-switch (Linux), macOS

---

### Cursor Shape Detection

**Challenge:** Terminals process DECSCUSR (`CSI Ps SP q`) but don't expose cursor shape state to external APIs.

**DECSCUSR Parameters:**
- 0 or 1: Blinking block
- 2: Steady block
- 3: Blinking underline
- 4: Steady underline
- 5: Blinking bar (xterm)
- 6: Steady bar (xterm)

**Implementation Issues:**
- No standard query sequence for current cursor shape
- No callback/notification when cursor shape changes
- Applications (Vim) send DECSCUSR but can't confirm it was processed

**Workaround Pattern:**
1. Vim plugin tracks mode internally (autocmd `InsertEnter`, `InsertLeave`)
2. Plugin calls external tool (macism) on mode change
3. Vim sends DECSCUSR to terminal independently

**Key Insight:** IME switching is driven by application logic (Vim autocmd), not terminal cursor state detection

**References:**
- [xterm.js Issue #3293](https://github.com/xtermjs/xterm.js/issues/3293)
- [Alacritty Issue #6631](https://github.com/alacritty/alacritty/issues/6631)
- [Microsoft Terminal Issue #4106](https://github.com/microsoft/terminal/issues/4106)

---

### Key Findings Summary

| Terminal | Native IME Auto-Switch | Cursor Shape API | Recommended Approach |
|----------|------------------------|------------------|---------------------|
| **Alacritty** | No | DECSCUSR in | Vim plugin + macism |
| **WezTerm** | No | DECSCUSR in | Vim plugin + macism |
| **Kitty** | No (GLFW limitation) | DECSCUSR in | Vim plugin + macism |
| **Ghostty** | No (in development) | DECSCUSR in | Vim plugin + macism |
| **iTerm2** | Partial (macOS setting) | DECSCUSR in | Vim plugin + macism |

**Consensus Pattern:**
- Terminals accept DECSCUSR for cursor shape changes
- No terminal exposes cursor shape state via API
- IME switching is application-driven (Vim plugins with autocmd hooks)
- External tools (macism, im-select) perform actual input source switching

---

## 2. IPC Event Subscription Implementation

### Overview

How terminals expose events (keystrokes, screen updates, mode changes) to external clients via IPC.

---

#### WezTerm (Lua API)

**Architecture:** Lua-based event system with `wezterm.on()` registration

**Event Registration:**
```lua
wezterm.on(event_name, callback)
```

**Callback Signature:**
```lua
function(window, pane)
  -- window: GUI window object
  -- pane: active pane object
  return false  -- Prevent later callbacks and default actions
end
```

**Control Flow:**
- Multiple callbacks can register for same event
- Callbacks execute in registration order
- Return `false` to stop propagation and prevent default actions

**Event Categories:**

1. **Predefined Events:** Window focus, config reload, bell notifications
2. **Custom Events:** User-defined events (avoid names conflicting with future WezTerm versions)

**Event Emission:**
- `wezterm.emit(name, ...)` - Programmatic emission
- `EmitEvent` key assignment - User-triggered emission

**Limitations:**
- No de-registration (handlers cleared on config reload)
- Lua state rebuilds on config reload
- No external process subscription (Lua runs inside WezTerm)

**Design Decision:** Configuration-embedded scripting, not external IPC

**References:**
- [wezterm.on() documentation](https://wezterm.org/config/lua/wezterm/on.html)
- [Window Events](https://wezterm.org/config/lua/window-events/index.html)
- [EmitEvent](https://wezterm.org/config/lua/keyassignment/EmitEvent.html)

---

#### Kitty (Unix Socket / JSON-RPC 2.0)

**Architecture:** JSON-based remote control protocol over Unix socket or TCP

**Protocol Format:**
```
<ESC>P@kitty-cmd<JSON><ESC>\
```

**JSON Structure:**
```json
{
  "cmd": "command_name",
  "version": [0, 14, 2],
  "no_response": false,
  "payload": { ... }
}
```

**Listen Modes:**

1. **In-band (default):** `allow_remote_control=yes` - watches terminal output stream for escape codes
2. **Socket mode:** `--listen-on=unix:/tmp/mykitty` - dedicated socket connection

**Async Requests:**
- For commands requiring user interaction (e.g., `select-window`)
- Include `"async": "random-unique-id"` in JSON
- Client can cancel with `cancel_async` field

**Streaming Requests:**
- Large data transfers split into chunks
- Each chunk: `"stream": true` and same `"stream_id"`
- End-of-data: empty chunk with matching `stream_id`

**Security:**
- Encrypted communication (AES-256-GCM + X25519 ECDH)
- Time-based nonces (5-minute window) prevent replay attacks
- Public keys via `KITTY_PUBLIC_KEY` environment variable

**Available Commands (40+):**
- **Window:** launch, new-window, close-window, focus-window, resize-window
- **Tab:** close-tab, focus-tab, set-tab-title, set-tab-color
- **Display:** set-font-size, set-colors, set-background-opacity
- **Content:** get-text, get-colors, ls (list windows/tabs)
- **Input:** send-text, send-key, signal-child

**Event Subscription:**
- No explicit subscription mechanism documented
- Clients poll via `ls` command or maintain state from async responses

**Design Decision:** Command/response model, not push-based event streaming

**References:**
- [Kitty Remote Control Protocol](https://sw.kovidgoyal.net/kitty/rc_protocol/)
- [Control kitty from scripts](https://sw.kovidgoyal.net/kitty/remote-control/)

---

#### iTerm2 (Python API / WebSocket)

**Architecture:** Python asyncio library with WebSocket connection

**Notification Subscriptions (9 types):**

1. **New Session:** `async_subscribe_to_new_session_notification()`
2. **Keystroke:** `async_subscribe_to_keystroke_notification(filter, patterns_to_ignore)`
3. **Screen Update:** `async_subscribe_to_screen_update_notification()`
4. **Prompt:** `async_subscribe_to_prompt_notification()`
5. **Location Change:** `async_subscribe_to_location_change_notification()` (host/directory)
6. **Custom Escape Sequence:** `async_subscribe_to_custom_escape_sequence_notification()` (OSC 1337)
7. **Session Termination:** `async_subscribe_to_terminate_session_notification()`
8. **Layout Change:** `async_subscribe_to_layout_change_notification()` (window/tab/session relationships)
9. **Focus Change:** `async_subscribe_to_focus_change_notification()`

**Callback Signature:**
```python
async def callback(connection, notification):
    # connection: Connection object
    # notification: Protocol buffer object (type-specific)
    pass
```

**Subscription Management:**
- Returns `SubscriptionToken` for later unsubscription
- `async_unsubscribe(token)` to remove subscription
- Optional session filtering to limit scope

**Keystroke Pattern Filtering:**
```python
KeystrokePattern(
    required_modifiers=[],
    forbidden_modifiers=[],
    characters="abc",
    keycodes=[13],
    characters_ignoring_modifiers="xyz"
)
```

**Design Decision:** Push-based event streaming over WebSocket with fine-grained filtering

**References:**
- [iTerm2 Python API Notifications](https://iterm2.com/python-api/notifications.html)
- [iTerm2 Python API](https://iterm2.com/python-api/)

---

#### Zellij (Rust WASM Plugins)

**Architecture:** WebAssembly plugins with Protocol Buffer API

**Plugin Interface:**
```rust
use zellij_tile::prelude::*;

#[derive(Default)]
struct MyPlugin;

impl ZellijPlugin for MyPlugin {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // Subscribe to events
        subscribe(&[
            EventType::ModeUpdate,
            EventType::KeyPress,
            EventType::TabUpdate,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::ModeUpdate(mode_info) => { /* ... */ },
            Event::KeyPress(key) => { /* ... */ },
            _ => {}
        }
        false  // Return true to re-render
    }
}
```

**Event Types (30+ variants):**

**User Input:**
- `Key` - Key press in plugin pane
- `Mouse` - Mouse interaction
- `InterceptedKeyPress` - Captured keyboard input
- `InputReceived` - General input anywhere

**UI State:**
- `ModeUpdate` - Mode information changes
- `TabUpdate` - Tab state modifications
- `PaneUpdate` - Pane manifest changes
- `Visible(bool)` - Plugin visibility toggle

**File System:**
- `FileSystemCreate`, `FileSystemRead`, `FileSystemUpdate`, `FileSystemDelete`
- Each contains `Vec<(PathBuf, Option<Metadata>)>`

**Process Management:**
- `CommandPaneOpened`, `CommandPaneExited`, `EditPaneOpened`, `EditPaneExited`
- `RunCommandResult(exit_code, stdout, stderr, context)`

**System Integration:**
- `CopyToClipboard` - Text copied anywhere in app
- `PastedText` - Paste event
- `WebRequestResult` - HTTP response
- `CustomMessage(sender, message)` - Inter-plugin communication

**Subscription Mechanism:**
```rust
subscribe(&[EventType::ModeUpdate, EventType::KeyPress]);
```

**Update Trigger:**
Once subscribed, `update()` method called on matching events.

**Design Decision:** WASM sandbox with strongly-typed event enum, synchronous update callback

**References:**
- [Zellij Event enum](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.Event.html)
- [Zellij Plugin API](https://zellij.dev/documentation/plugin-api.html)
- [Developing a Rust Plugin](https://zellij.dev/tutorials/developing-a-rust-plugin/)

---

### Event Subscription Comparison

| Terminal | Protocol | Transport | Subscription Model | Event Filtering | Language |
|----------|----------|-----------|-------------------|-----------------|----------|
| **WezTerm** | Lua callbacks | In-process | `wezterm.on()` | Callback return value | Lua |
| **Kitty** | JSON-RPC 2.0 | Unix socket / TCP | Command/response | N/A (polling) | Any (socket client) |
| **iTerm2** | Protocol Buffers | WebSocket | Async subscribe functions | Session + pattern filtering | Python asyncio |
| **Zellij** | Protocol Buffers | WASM FFI | `subscribe(&[EventType])` | Event type enum | Rust (WASM) |

**Key Patterns:**

1. **In-Process (WezTerm):** Lua runs inside terminal, no external IPC
2. **Command/Response (Kitty):** Client sends commands, receives responses, polls for state
3. **Push Subscription (iTerm2):** WebSocket push events, async callbacks, token-based unsubscribe
4. **Plugin Sandbox (Zellij):** WASM plugins, strongly-typed events, synchronous update

**Backpressure Handling:**
- **WezTerm:** N/A (in-process)
- **Kitty:** Client-managed (async/streaming for large data)
- **iTerm2:** Python asyncio queue management
- **Zellij:** WASM synchronous call (no explicit backpressure)

---

## 3. Image Drag-and-Drop in Terminals

### Overview

How terminals handle dropping image data (not file paths) into the terminal window.

---

#### Kitty Graphics Protocol

**Protocol Design:**
- **Goal:** Render arbitrary pixel graphics in terminal
- **Approach:** APC (Application Programming Command) escape sequences
- **Format:** `<ESC>_G<control-data>;<payload><ESC>\`
- **Control Data:** Comma-separated `key=value` pairs
- **Payload:** Base64-encoded binary image data

**Key Features:**
- Graphics integrate with text (alpha blending, z-ordering)
- Graphics scroll with text
- Individual pixel positioning with X/Y offsets within cells
- Terminals don't need to understand image formats (client encodes)

**Placement:**
- Rendered at current cursor position (upper-left of cell)
- Supports extra X/Y pixel offsets

**Image Drag-and-Drop:**
- No explicit documentation for drag-and-drop → graphics protocol conversion
- Users would need to:
  1. Detect drop event (OS-level)
  2. Read image data
  3. Encode as Kitty graphics protocol
  4. Emit to terminal

**Design Decision:** In-band protocol for rendering, no native drag-and-drop integration

**References:**
- [Kitty Graphics Protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
- [icat kitten](https://sw.kovidgoyal.net/kitty/kittens/icat/)

---

#### WezTerm

**Image Protocol Support:**
- iTerm2 inline image protocol
- Kitty graphics protocol (`enable_kitty_graphics=true`)
- Sixel graphics

**Image Pasting:**
- Active community discussion (Issue #7272)
- No native image paste implementation yet
- Workaround: Community script converts Kitty protocol to stdout

**OSC 52 Clipboard:**
- Supports setting/clearing clipboard via OSC 52
- No clipboard query support
- Works for text, not images

**Drag-and-Drop:**
- No documented image drag-and-drop support
- File path dropping works (pastes path as text)

**Design Decision:** Focus on rendering protocols, not drag-and-drop integration

**References:**
- [WezTerm Issue #7272](https://github.com/wezterm/wezterm/issues/7272)
- [WezTerm PasteFrom](https://wezterm.org/config/lua/keyassignment/PasteFrom.html)

---

#### iTerm2

**Image Protocol:**
- iTerm2 inline images via OSC 1337
- Base64-encoded image data in escape sequence

**Drag-and-Drop Behavior:**
- Documented: Drag files → pastes file paths
- Drag selected text → pastes text
- No explicit image drag-and-drop documentation

**NSPasteboard Implementation:**
- `PTYTextView.m` likely handles drag-and-drop via NSView drag destination protocol
- Expected pasteboard types: `NSPasteboardTypeTIFF`, `NSPasteboardTypePNG`
- Would need to check `draggingEntered:`, `performDragOperation:` implementations

**Potential Pattern (unconfirmed):**
```objc
- (NSDragOperation)draggingEntered:(id<NSDraggingInfo>)sender {
    NSPasteboard *pb = [sender draggingPasteboard];
    if ([pb availableTypeFromArray:@[NSPasteboardTypeTIFF, NSPasteboardTypePNG]]) {
        return NSDragOperationCopy;
    }
    return NSDragOperationNone;
}
```

**Design Decision:** Standard macOS drag-and-drop, likely file-path focused

**References:**
- [NSPasteboard Drag and Drop (macOS)](https://www.appcoda.com/nspasteboard-macos/)
- [iTerm2 Issues](https://gitlab.com/gnachman/iterm2/-/issues)

---

#### Alacritty

**Image Support:** None (text-only terminal)

**Drag-and-Drop:**
- No image support
- No image drag-and-drop

**Design Decision:** Minimalist terminal, text-focused

---

#### Ghostty

**Image Support:**
- Kitty graphics protocol
- Sixel graphics

**Drag-and-Drop:**
- No documented image drag-and-drop
- Standard file drag-and-drop expected

**Design Decision:** Graphics rendering support, drag-and-drop likely file-path based

---

### Image Drag-and-Drop Summary

| Terminal | Image Rendering | Drag File Path | Drag Image Data | Implementation Approach |
|----------|----------------|----------------|-----------------|-------------------------|
| **Alacritty** | No | Unknown | No | N/A |
| **WezTerm** | Yes (Kitty, iTerm2, Sixel) | Yes (pastes path) | No (requested) | Future feature |
| **Kitty** | Yes (Kitty protocol) | Yes (pastes path) | Unknown | Potentially via protocol |
| **Ghostty** | Yes (Kitty, Sixel) | Likely yes | Unknown | Likely file-path |
| **iTerm2** | Yes (OSC 1337) | Yes (pastes path) | Unknown | NSPasteboard capable |

**Key Insight:** Most terminals support image *rendering* protocols but not direct image *drag-and-drop*. Standard behavior is dropping files → paste file path.

**Potential Implementation for Crux:**
1. Detect image drag via `NSPasteboard` types (`NSPasteboardTypeTIFF`, `NSPasteboardTypePNG`)
2. Extract image data from pasteboard
3. Convert to Kitty graphics protocol or custom Crux protocol
4. Emit escape sequence to PTY

---

## 4. Terminal IME State Exposure

### Overview

Do terminals expose IME composition state (preedit text, cursor position) via external API?

---

#### iTerm2 Python API

**IME State Exposure:** No documented API for IME composition state

**Available Notifications:**
- Keystroke (but not during IME preedit)
- Screen update (reflects committed text, not preedit)
- Custom escape sequence

**PTYTextView Internal State:**
```objc
@property (nonatomic) NSAttributedString *markedText;
@property (nonatomic) NSRange markedRange;
@property (nonatomic) NSInteger numberOfIMELines;
```

**Not Exposed:**
- `markedText` - Current preedit text
- `markedRange` - Preedit position
- `numberOfIMELines` - IME overlay lines

**Design Decision:** IME state is rendering detail, not exposed to scripting API

**References:**
- [iTerm2 Python API](https://iterm2.com/python-api/)
- [PTYTextView.m](https://github.com/gnachman/iTerm2/blob/master/sources/PTYTextView.m)

---

#### WezTerm Lua API

**IME State Exposure:** No documented IME state in Lua API

**Available:**
- Window and pane objects
- No IME preedit or composition state

**Design Decision:** Lua API focuses on window/pane management, not input state

---

#### Zellij Plugin API

**IME State Exposure:** No IME-specific events

**Event Types:**
- `Key` - Key press (likely blocked during IME preedit)
- `InputReceived` - General input event
- No `ImeComposition` or `ImePreedit` variant

**Design Decision:** Plugin API for UI/workflow, not low-level input

---

#### Kitty Remote Control

**IME State Exposure:** No IME state in protocol

**Available Commands:**
- `send-text` - Send committed text
- `send-key` - Send key events
- No IME composition query

**Design Decision:** Control protocol for automation, not input monitoring

---

### IME State Exposure Summary

| Terminal | External API | IME State Exposed | Preedit Text | Composition Events |
|----------|--------------|-------------------|--------------|-------------------|
| **Alacritty** | None | No | No | No |
| **WezTerm** | Lua (in-process) | No | No | No |
| **Kitty** | JSON-RPC | No | No | No |
| **Ghostty** | None | No | No | No |
| **iTerm2** | Python | No | No | No |
| **Zellij** | WASM Plugins | No | No | No |

**Consensus:** No terminal exposes IME composition state via external API.

**Rationale:**
1. IME preedit is transient UI state (overlay rendering)
2. Not part of terminal content (doesn't write to PTY)
3. Exposed to application via `NSTextInputClient` (macOS) or XIM (Linux), not external clients

**Use Case for Exposure:**
- Claude Code Agent Teams could query IME state before sending commands
- Avoid interrupting user during active composition
- Wait for commit before pane switching

**Implementation Challenge:**
- Preedit text never touches PTY
- Lives only in view layer
- Would need explicit IPC event: `ime_composition_start`, `ime_composition_update`, `ime_composition_commit`

---

## Key Takeaways

### 1. Vim IME Auto-Switch

**Finding:** No terminal natively implements cursor-shape → IME switching.

**Standard Pattern:**
- Vim plugins detect mode changes via autocmd (`InsertEnter`, `InsertLeave`)
- Plugin calls external tool (macism, im-select) to switch input source
- Vim sends DECSCUSR to terminal for cursor shape
- Terminal and IME switch are independent operations

**Recommendation for Crux:**
- Document macism/im-select integration pattern
- Consider future feature: expose cursor shape change events via IPC
- Allow Claude Code to subscribe to cursor shape changes
- Enable agent-driven IME switching for Vim-mode terminals

---

### 2. IPC Event Subscription

**Finding:** Four distinct models with different tradeoffs.

**Models:**
1. **In-Process Scripting (WezTerm):** Lua callbacks, no external IPC, config reload to update
2. **Command/Response (Kitty):** JSON-RPC, polling-based, async for user interaction
3. **Push Subscription (iTerm2):** WebSocket, protocol buffers, token-based unsubscribe, fine-grained filtering
4. **WASM Plugins (Zellij):** Sandboxed, strongly-typed events, synchronous update

**Recommendation for Crux:**
- Use iTerm2-style push subscription for Claude Code integration
- Unix socket server with JSON-RPC 2.0 (matches current plan)
- Subscription tokens for management
- Event filtering (session, pattern, event type)
- Consider both JSON and Protocol Buffers for efficiency

**Reference Implementation:**
- iTerm2 Python API for subscription patterns
- Kitty for async/streaming large data
- Zellij for strongly-typed event enum

---

### 3. Image Drag-and-Drop

**Finding:** Most terminals support image rendering but not image drag-and-drop.

**Standard Behavior:**
- Drag file → paste file path as text
- Drag image → no special handling (or paste path if from file)

**Graphics Protocols Supported:**
- Kitty graphics protocol (Kitty, WezTerm, Ghostty)
- iTerm2 inline images (WezTerm, iTerm2)
- Sixel (WezTerm, Ghostty)

**Recommendation for Crux:**
- Implement `NSPasteboard` drag destination
- Detect image types (`NSPasteboardTypeTIFF`, `NSPasteboardTypePNG`)
- Convert to Kitty graphics protocol
- Emit to PTY for rendering
- Expose via IPC for Claude Code (image data, position, metadata)

---

### 4. IME State Exposure

**Finding:** No terminal exposes IME composition state via API.

**Rationale:**
- Preedit is transient view state, not terminal content
- Only visible to NSTextInputClient (macOS) or XIM (Linux)
- Never written to PTY

**Recommendation for Crux:**
- Expose IME state via IPC as differentiator
- Events: `ime_composition_start`, `ime_composition_update`, `ime_composition_commit`, `ime_composition_cancel`
- Payload: Preedit text, cursor position, candidate window state
- Use case: Claude Code can avoid interrupting during active composition

**Novel Feature:** Would be first terminal to expose IME state to external clients.

---

## Implementation Priorities for Crux

### High Priority

1. **IPC Event Subscription (Phase 2 carryover)**
   - JSON-RPC 2.0 over Unix socket
   - Push-based event streaming (iTerm2 pattern)
   - Subscription tokens with filtering
   - Events: pane focus, tab switch, mode change, process exit

2. **IME State IPC Events (Phase 3)**
   - `ime_composition_start`, `ime_composition_update`, `ime_composition_commit`
   - Payload: preedit text, cursor position
   - Novel feature for Claude Code integration

### Medium Priority

3. **Cursor Shape Change Events (Phase 3)**
   - Detect DECSCUSR processing
   - Emit IPC event: `cursor_shape_changed(shape, blinking)`
   - Enable agent-driven IME auto-switch

4. **Image Drag-and-Drop (Phase 3)**
   - `NSPasteboard` drag destination for images
   - Convert to Kitty graphics protocol
   - Emit to PTY + IPC notification

### Research-Only (No Implementation)

5. **Vim IME Auto-Switch**
   - Document macism integration in user guide
   - No terminal-level implementation (app-driven pattern)

---

## Sources

### Terminals Researched

- [Alacritty GitHub](https://github.com/alacritty/alacritty)
- [WezTerm Documentation](https://wezterm.org/)
- [Kitty Documentation](https://sw.kovidgoyal.net/kitty/)
- [Ghostty GitHub](https://github.com/ghostty-org/ghostty)
- [iTerm2 Documentation](https://iterm2.com/documentation-one-page.html)
- [Zellij Documentation](https://zellij.dev/documentation/)

### Key References

**IME Auto-Switch:**
- [vim-macos-ime](https://github.com/laishulu/vim-macos-ime)
- [macism CLI](https://github.com/laishulu/macism)
- [vim-barbaric](https://github.com/rlue/vim-barbaric)
- [Mac-input.vim](https://github.com/BenSYZ/Mac-input.vim)

**IPC Event Subscription:**
- [WezTerm Event System](https://wezterm.org/config/lua/wezterm/on.html)
- [Kitty Remote Control Protocol](https://sw.kovidgoyal.net/kitty/rc_protocol/)
- [iTerm2 Python API Notifications](https://iterm2.com/python-api/notifications.html)
- [Zellij Plugin API Events](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.Event.html)

**Graphics Protocols:**
- [Kitty Graphics Protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
- [WezTerm Features](https://wezterm.org/features.html)

**Low-Level APIs:**
- [winit IME enum](https://docs.rs/winit/latest/winit/event/enum.Ime.html)
- [NSTextInputClient (Apple)](https://developer.apple.com/documentation/appkit/nstextinputclient)
- [iTerm2 PTYTextView.m](https://github.com/gnachman/iTerm2/blob/master/sources/PTYTextView.m)

---

**Research Date:** 2026-02-13
**Researcher:** Claude (oh-my-claudecode:researcher agent)
**Status:** Complete
