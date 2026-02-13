# Phase 3: IME & Rich Clipboard — Detailed Implementation Plan

> Created: 2026-02-13
> Status: Ready for implementation
> Depends on: Phase 1 (complete), Phase 2 (complete)

---

## Executive Summary

Phase 3 has **6 work items** (3.A-3.F). Critical finding: **most of PLAN.md section 3.1-3.3 is already implemented**. GPUI handles NSTextInputClient natively; Crux's `CruxTerminalView` already implements all 7 `EntityInputHandler` methods with Korean IME hardening (modifier isolation, event dedup, NFC normalization, composition overlay). The remaining work focuses on a small cursor bug fix, rich clipboard integration, drag-and-drop, IPC protocol extensions, and Vim IME auto-switch.

### Review Findings (2026-02-13)

4명의 전문 리뷰어(아키텍처/보안/API/코드)가 본 계획을 검토한 결과:

| 리뷰 | 점수 | 핵심 발견 |
|------|------|----------|
| 아키텍처 | 4.5/5 | CursorShapeChanged 이벤트 발행 방식 명확화 필요 |
| 보안 | 3/5 | CRITICAL 3건 (임시파일, OSC 52, IPC 인증) |
| API 설계 | 3.5/5 | ClipboardReadResult tagged union 재설계 필요 |
| 코드 품질 | 4/5 | GPUI API 확인됨, 에러 처리 개선 필요 |

아래 각 섹션에 **[리뷰 반영]** 태그로 수정사항을 표시함.

### Sub-phases for Incremental Delivery

| Sub-phase | Items | Can Ship Independently | Estimated Effort |
|-----------|-------|----------------------|------------------|
| **3-Alpha** | 3.A Wide char cursor fix | Yes | ~1 hour |
| **3-Beta** | 3.B Rich clipboard + 3.C Drag & drop | Yes (together) | ~2-3 days |
| **3-Gamma** | 3.D IPC protocol extensions | Yes | ~1-2 days |
| **3-Delta** | 3.E Vim IME auto-switch | Yes | ~1-2 days |
| **3-Epsilon** | 3.F IME crash resilience | Yes (low priority) | ~2 hours |

### Dependency Graph

```
3.A (standalone — no deps)
3.B → 3.C (drag-and-drop reuses clipboard image-save logic)
3.B → 3.D (IPC clipboard methods need crux-clipboard wired in)
3.E (standalone — no deps)
3.F (standalone — no deps)
```

---

## PLAN.md Updates — Items Already Done

The following PLAN.md Phase 3 items should be marked `[x]` based on the current codebase:

### 3.1 NSTextInputClient Implementation — ALL DONE
GPUI implements `NSTextInputClient` at the platform layer (`platform/mac/window.rs`). Crux does NOT need direct objc2 implementation. Instead, GPUI delegates to `EntityInputHandler`, which `CruxTerminalView` implements at `view.rs:796-1054`.

| PLAN.md Item | Status | Evidence |
|---|---|---|
| `insertText:replacementRange:` | DONE | `replace_text_in_range()` at view.rs:902-982 |
| `setMarkedText:selectedRange:replacementRange:` | DONE | `replace_and_mark_text_in_range()` at view.rs:984-1023 |
| `unmarkText` | DONE | `unmark_text()` at view.rs:897-900 |
| `hasMarkedText`, `markedRange`, `selectedRange` | DONE | `marked_text_range()` at view.rs:865-895, `selected_text_range()` at view.rs:840-863 |
| `firstRectForCharacterRange:actualRange:` | DONE | `bounds_for_range()` at view.rs:1025-1042 |
| `doCommandBySelector:` | DONE | Handled by GPUI platform layer |
| `validAttributesForMarkedText` | DONE | Handled by GPUI platform layer |
| `characterIndexForPoint:` | DONE | `character_index_for_point()` at view.rs:1044-1053 |

### 3.2 Composition Overlay Rendering — ALL DONE

| PLAN.md Item | Status | Evidence |
|---|---|---|
| Preedit text rendered as overlay | DONE | element.rs:342-381 (shapes IME preedit overlay) |
| Underline style for composition text | DONE | element.rs:358-362 (UnderlineStyle on preedit run) |
| Distinct color for composing vs committed | DONE | element.rs:371-378 (blue-ish Hsla background) |
| Correct overlay positioning for wide chars | DONE | element.rs:348-352 (uses cursor col * cell_width) |
| Overlay cleanup on composition cancel/commit | DONE | view.rs:897-900 (unmark_text clears), view.rs:911 (replace_text clears) |

### 3.3 Korean IME Hardening — ALL DONE

| PLAN.md Item | Status | Evidence |
|---|---|---|
| Modifier key isolation (Ghostty #4634) | DONE | view.rs:226-231 (`is_standalone_modifier` check during `hasMarkedText`) |
| Event deduplication (Alacritty #8079) | DONE | view.rs:255-263 + view.rs:919-926 (10ms dedup window) |
| IME crash resilience (100ms timeout) | PARTIAL | Low priority; macOS IME architecture prevents hard deadlocks |
| NFD normalization | DONE | view.rs:931 (`text.nfc().collect()`) |
| Wide character cursor | **NOT DONE** | See 3.A below |

### Summary: Mark these PLAN.md items as [x]
- [x] 3.1 — All 8 NSTextInputClient methods
- [x] 3.2 — All 5 composition overlay items
- [x] 3.3 — 4 of 5 hardening items (wide char cursor remains)

---

## 3.A Wide Character Cursor Fix

**Priority**: High (visual bug)
**Effort**: ~5 lines of code, ~1 hour with testing
**Dependencies**: None

### Problem

When the cursor is on a CJK wide character (2-cell width), the cursor quad is drawn at single `cell_width`. This makes the cursor appear to cover only half the character.

### Root Cause

In `element.rs:302-340`, the cursor quad always uses `size(cell_width, cell_height)` regardless of whether the cell under the cursor has the `WIDE_CHAR` flag.

### Files to Modify

**`crates/crux-terminal-view/src/element.rs`** — cursor quad construction

### Implementation

In the cursor quad building section (line ~302-340), after computing `cursor_row` and `cursor_col`, check if the cell at the cursor position has the `WIDE_CHAR` flag:

```rust
// element.rs, inside the cursor quad section (~line 302)
let cursor_row = content.cursor.point.line.0 as usize;
let cursor_col = content.cursor.point.column.0;

// Check if cursor is on a wide character (CJK 2-cell).
let cursor_cell_idx = cursor_row * content.cols + cursor_col;
let is_wide = cursor_cell_idx < content.cells.len()
    && content.cells[cursor_cell_idx]
        .flags
        .contains(CellFlags::WIDE_CHAR);
let cursor_width = if is_wide { cell_width * 2.0 } else { cell_width };

let cx_pos = point(
    origin.x + cell_width * cursor_col as f32,
    origin.y + cell_height * cursor_row as f32,
);
let cell_bounds = Bounds::new(cx_pos, size(cursor_width, cell_height));
```

Update all cursor shape variants to use `cursor_width` instead of `cell_width`:
- `CursorShape::Block` — use `cell_bounds` (already uses it)
- `CursorShape::Beam` — no change (beam is always 2px wide)
- `CursorShape::Underline` — use `cursor_width` instead of `cell_width`

### Test Strategy

1. Manual test: Open terminal, type `echo "한글테스트"`, move cursor over CJK characters with arrow keys, verify cursor spans 2 cells.
2. Unit test: Create a `TerminalContent` with a WIDE_CHAR cell at cursor position, verify the generated cursor quad width is `2 * cell_width`.

---

## 3.B Rich Clipboard

**Priority**: High (core differentiator for Claude Code image paste)
**Effort**: ~1.5 days
**Dependencies**: None (crux-clipboard already exists with NSPasteboard bindings)

### Current State

- `crux-clipboard` crate exists at `crates/crux-clipboard/src/lib.rs`
- Already implements: `Clipboard::read()`, `read_text()`, `read_image()`, `write_text()`, `write_image()`, `available_types()`
- Already detects: Text, HTML, Image (PNG/TIFF), FilePaths
- **Missing**: TIFF-to-PNG conversion, temp file saving, file URL reading, wiring into crux-app
- **crux-app does NOT depend on crux-clipboard** — paste currently uses GPUI's built-in `cx.read_from_clipboard()` (text-only)

### Sub-tasks

#### 3.B.1 Add `image` crate for TIFF-to-PNG conversion

**File**: `crates/crux-clipboard/Cargo.toml`

Add dependency:
```toml
[dependencies]
image = { version = "0.25", default-features = false, features = ["png", "tiff"] }
```

**File**: `crates/crux-clipboard/src/lib.rs`

Add a `tiff_to_png()` helper function and update `read_image_internal()` to auto-convert:

```rust
/// Convert TIFF image data to PNG format.
/// **[리뷰 반영]** 에러 컨텍스트 보존 (MAJOR fix)
fn tiff_to_png(tiff_data: &[u8]) -> Result<Vec<u8>, ClipboardError> {
    let img = image::load_from_memory_with_format(tiff_data, image::ImageFormat::Tiff)
        .map_err(|e| {
            log::warn!("TIFF decode failed: {e}");
            ClipboardError::ImageDecode(e.to_string())
        })?;
    let mut png_buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png_buf), image::ImageFormat::Png)
        .map_err(|e| {
            log::warn!("PNG encode failed: {e}");
            ClipboardError::ImageEncode(e.to_string())
        })?;
    Ok(png_buf)
}
```

Update `read_image_internal()`: When TIFF data is read, convert to PNG before returning.

#### 3.B.2 Temp file saving for image paste

**File**: `crates/crux-clipboard/src/lib.rs`

Add a new public function:

```rust
/// Save clipboard image to a temp file and return the path.
/// **[리뷰 반영]** tempfile crate 사용, 파일명 충돌 방지, 보안 강화 (CRITICAL + MAJOR fix)
pub fn save_image_to_temp(png_data: &[u8]) -> Result<std::path::PathBuf, ClipboardError> {
    use std::io::Write;
    // Use $TMPDIR/crux-clipboard/ with 0700 permissions (symlink attack prevention)
    let dir = std::env::temp_dir().join("crux-clipboard");
    std::fs::create_dir_all(&dir).map_err(|_| ClipboardError::WriteFailed)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| ClipboardError::WriteFailed)?;
    }
    // Atomic counter + PID for collision prevention
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pid = std::process::id();
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = dir.join(format!("paste-{pid}-{timestamp}-{seq}.png"));
    // Write with exclusive create (O_EXCL) to prevent TOCTOU
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| ClipboardError::WriteFailed)?;
    file.write_all(png_data).map_err(|_| ClipboardError::WriteFailed)?;
    Ok(path)
}
```

#### 3.B.3 File URL reading from NSPasteboard

**File**: `crates/crux-clipboard/src/lib.rs`

Implement `read_file_paths_internal()` (currently returns `NotImplemented`):

```rust
fn read_file_paths_internal(pasteboard: &NSPasteboard) -> Result<Vec<PathBuf>, ClipboardError> {
    // Use propertyListForType to get an array of file URL strings.
    let file_url_type = unsafe { NSPasteboardTypeFileURL };
    let data = unsafe { pasteboard.stringForType(file_url_type) };
    if let Some(url_string) = data {
        let url_str = url_string.to_string();
        // File URLs are percent-encoded: file:///path/to/file
        if let Ok(url) = url::Url::parse(&url_str) {
            if let Ok(path) = url.to_file_path() {
                return Ok(vec![path]);
            }
        }
        // Fallback: try as plain path
        return Ok(vec![PathBuf::from(url_str)]);
    }
    Err(ClipboardError::NotImplemented)
}
```

Add `url` dependency to `Cargo.toml`:
```toml
url = "2"
```

#### 3.B.4 Wire crux-clipboard into crux-app

**File**: `crates/crux-app/Cargo.toml`

Add dependency:
```toml
crux-clipboard.workspace = true
```

**File**: `crates/crux-terminal-view/src/view.rs`

Enhance `paste_from_clipboard()` (line 599-615) to handle images:

The current implementation only reads text via GPUI's `cx.read_from_clipboard()`. We need to add a parallel path that checks NSPasteboard for images. However, since `CruxTerminalView` runs on the GPUI main thread (which IS the macOS main thread), we can safely call `crux_clipboard::Clipboard` methods.

**Approach**: Add a feature flag or compile-time check. When clipboard contains an image:
1. Read image data via `crux_clipboard::Clipboard::read_image()`
2. Convert TIFF to PNG if needed
3. Save to temp file via `save_image_to_temp()`
4. Insert the file path as text into PTY (sideband for Crux-aware apps can come later)

```rust
fn paste_from_clipboard(&mut self, cx: &mut Context<Self>) {
    // Try image paste first (requires MainThreadMarker).
    if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
        if let Ok(content) = crux_clipboard::Clipboard::read(mtm) {
            match content {
                crux_clipboard::ClipboardContent::Image { png_data } => {
                    if let Ok(path) = crux_clipboard::save_image_to_temp(&png_data) {
                        let path_str = path.to_string_lossy().to_string();
                        self.terminal.write_to_pty(path_str.as_bytes());
                        return;
                    }
                }
                crux_clipboard::ClipboardContent::FilePaths(paths) => {
                    // Insert paths separated by spaces.
                    let text = paths.iter()
                        .map(|p| shell_escape::escape(p.to_string_lossy()))
                        .collect::<Vec<_>>()
                        .join(" ");
                    self.terminal.write_to_pty(text.as_bytes());
                    return;
                }
                _ => {} // Fall through to text paste below
            }
        }
    }

    // Default: text paste via GPUI clipboard API.
    if let Some(item) = cx.read_from_clipboard() {
        // ... existing text paste logic ...
    }
}
```

**[리뷰 반영] 클립보드 붙여넣기 시 ANSI 이스케이프 필터링 (HIGH fix)**:

모든 클립보드 텍스트를 PTY에 전송하기 전에 제어 문자를 필터링한다:
```rust
/// Strip dangerous control characters from clipboard text before pasting.
fn sanitize_paste_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .collect()
}
```

Bracketed paste mode가 활성화된 경우 `\x1b[200~`..`\x1b[201~`로 감싸서 전송 (기존 구현 활용).
```

**File**: `crates/crux-terminal-view/Cargo.toml`

Add dependencies:
```toml
crux-clipboard.workspace = true
objc2-foundation = { version = "0.2", features = ["NSThread"] }  # for MainThreadMarker
```

#### 3.B.5 OSC 52 clipboard integration (standard terminals)

**File**: `crates/crux-terminal/src/pty.rs`

The PTY read loop already scans for OSC 7 and OSC 133. Add OSC 52 scanning for base64-encoded clipboard operations. This allows terminal programs to read/write the clipboard via escape sequences.

Add a new `TerminalEvent` variant:
```rust
// In crates/crux-terminal/src/event.rs
pub enum TerminalEvent {
    // ... existing variants ...
    ClipboardSet { data: String },  // OSC 52 clipboard write
}
```

Add OSC 52 scanning in the PTY read loop byte scanner.

Handle `TerminalEvent::ClipboardSet` in `CruxTerminalView::process_events()` by writing to the system clipboard via GPUI's `cx.write_to_clipboard()`.

**[리뷰 반영] OSC 52 보안 정책 (CRITICAL fix)**:

OSC 52 클립보드 접근은 보안 위험이 높다. 기본 정책:
- **Write (복사)**: 허용 (프로그램이 시스템 클립보드에 쓰기)
- **Read (쿼리)**: 기본 차단 (사용자 동의 후에만 허용, iTerm2 방식)

```rust
/// OSC 52 clipboard access policy.
#[derive(Debug, Clone, Copy, Default)]
pub enum Osc52Policy {
    #[default]
    WriteOnly,   // Default: programs can write, not read
    ReadWrite,   // User opted in: programs can read and write
    Disabled,    // No clipboard access via OSC 52
}
```

설정은 향후 Phase 5 config 시스템에서 토글 가능.

### Test Strategy

1. **Unit tests** for `tiff_to_png()`: Feed known TIFF bytes, verify PNG output magic bytes (`\x89PNG`).
2. **Unit tests** for `save_image_to_temp()`: Create temp file, verify it exists and contains correct data.
3. **Manual test**: Copy an image in Preview.app, Cmd+V in Crux, verify path is pasted.
4. **Manual test**: Copy file in Finder, Cmd+V in Crux, verify escaped path is pasted.

---

## 3.C Drag & Drop

**Priority**: Medium
**Effort**: ~1 day
**Dependencies**: 3.B (reuses image temp-file logic)

### Current State

No drag-and-drop handling exists. GPUI provides `FileDropEvent` / `ExternalPaths` — no need for `NSDraggingDestination` protocol.

### Files to Modify

**`crates/crux-terminal-view/src/view.rs`** — Add drag event handlers
**`crates/crux-terminal-view/src/element.rs`** — Add drop indicator overlay

### Implementation

#### 3.C.1 Add drag state to CruxTerminalView

```rust
// In CruxTerminalView struct:
/// Whether a file drag is currently over the terminal.
drag_active: bool,
```

Initialize to `false` in the constructor.

#### 3.C.2 Handle FileDropEvent in render()

GPUI's `div()` supports `.on_drop()` for external file drops. The `ExternalPaths` type wraps `Vec<PathBuf>`.

In `CruxTerminalView::render()`, add to the div chain:

```rust
.on_drop(cx.listener(|this: &mut Self, paths: &ExternalPaths, _window, cx| {
    this.drag_active = false;
    let escaped_paths: Vec<String> = paths.paths()
        .iter()
        .map(|p| shell_escape::escape(p.to_string_lossy()).to_string())
        .collect();
    let text = escaped_paths.join(" ");
    this.terminal.write_to_pty(text.as_bytes());
    cx.notify();
}))
```

For drag-enter/exit visual feedback, use GPUI's drag state detection. The `drag_active` flag controls whether the drop indicator overlay is rendered.

#### 3.C.3 Drop indicator overlay

In `element.rs`, add an optional drop indicator to `TerminalPrepaintState`:

```rust
pub struct TerminalPrepaintState {
    // ... existing fields ...
    /// Drop indicator border overlay when file drag is active.
    drop_indicator: bool,
}
```

In the paint phase, if `drop_indicator` is true, paint a colored border:

```rust
if state.drop_indicator {
    let border_color = Hsla { h: 0.58, s: 0.7, l: 0.5, a: 0.8 }; // Blue accent
    let thickness = px(3.0);
    // Paint 4 border rectangles (top, bottom, left, right)
    window.paint_quad(fill(
        Bounds::new(bounds.origin, size(bounds.size.width, thickness)),
        border_color,
    ));
    // ... bottom, left, right borders ...
}
```

#### 3.C.4 Image drops

For image file drops (detected by extension: `.png`, `.jpg`, `.jpeg`, `.gif`, `.tiff`, `.bmp`):
- If the image is already a file, just insert the path
- No conversion needed (unlike clipboard paste which may be raw TIFF data)

### Test Strategy

1. **Manual test**: Drag a text file from Finder into Crux terminal, verify path appears.
2. **Manual test**: Drag multiple files, verify space-separated escaped paths.
3. **Manual test**: Verify blue border indicator appears during drag-over and disappears on drop/exit.

---

## 3.D IPC Protocol Extensions

**Priority**: Medium
**Effort**: ~1-2 days
**Dependencies**: 3.B (clipboard must be wired in)

### Current State

- IPC server handles 9 methods (handshake + 8 pane/window methods)
- Protocol types in `crux-protocol`, handlers in `crux-ipc`, dispatch in `crux-app`
- MCP server wraps IPC — new IPC methods automatically become available to MCP tools

### New IPC Methods

#### 3.D.1 `crux:clipboard/read`

**crux-protocol/src/lib.rs** — Add types:

```rust
/// Parameters for `crux:clipboard/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardReadParams {
    /// Preferred content type: "text", "image", "auto" (default: "auto")
    #[serde(default = "default_clipboard_type")]
    pub content_type: String,
}

fn default_clipboard_type() -> String { "auto".to_string() }

/// Result of `crux:clipboard/read`.
/// **[리뷰 반영]** Tagged union으로 재설계 — 불가능한 상태 원천 방지 (P0 API fix)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "content_type")]
pub enum ClipboardReadResult {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { image_path: String },
    #[serde(rename = "html")]
    Html { html: String },
    #[serde(rename = "file_paths")]
    FilePaths { paths: Vec<String> },
}
```

**crux-protocol/src/lib.rs** — Add method constants:

```rust
pub mod method {
    // ... existing methods ...
    pub const CLIPBOARD_READ: &str = "crux:clipboard/read";
    pub const CLIPBOARD_WRITE: &str = "crux:clipboard/write";
    pub const IME_GET_STATE: &str = "crux:ime/get-state";
    pub const IME_SET_INPUT_SOURCE: &str = "crux:ime/set-input-source";
    pub const EVENTS_SUBSCRIBE: &str = "crux:events/subscribe";
}
```

#### 3.D.2 `crux:clipboard/write`

```rust
/// Parameters for `crux:clipboard/write`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardWriteParams {
    pub content_type: String,  // "text", "image"
    pub text: Option<String>,
    pub image_path: Option<String>,  // path to PNG file
}
```

#### 3.D.3 `crux:ime/get-state`

```rust
/// Result of `crux:ime/get-state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImeStateResult {
    pub composing: bool,
    pub preedit_text: Option<String>,
    pub input_source: Option<String>,  // e.g. "com.apple.inputmethod.Korean.2SetKorean"
}
```

Implementation: Read `marked_text` from the active pane's `CruxTerminalView`. For `input_source`, use `TISCopyCurrentKeyboardInputSource()` FFI.

#### 3.D.4 `crux:ime/set-input-source`

```rust
/// Parameters for `crux:ime/set-input-source`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImeSetInputSourceParams {
    pub input_source: String,  // e.g. "com.apple.keylayout.ABC"
}
```

Implementation: Use `TISSelectInputSource()` FFI (same as Vim auto-switch, see 3.E).

#### 3.D.5 `crux:events/subscribe`

```rust
/// Parameters for `crux:events/subscribe`.
/// **[리뷰 반영]** 이벤트 타입을 enum으로 변경 — typo 방지, IDE 지원 (P0 API fix)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsSubscribeParams {
    pub events: Vec<PaneEventType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneEventType {
    PaneCreated,
    PaneClosed,
    PaneFocused,
    PaneResized,
    TitleChanged,
    ClipboardSet,
}
```

Implementation: Use the existing `pane_events` buffer in `CruxApp`. Send JSON-RPC notifications to subscribed clients when events occur. This requires adding a subscriber list to the IPC server.

### Adding to IPC Handler

**crux-ipc/src/command.rs** — Add new IpcCommand variants:

```rust
pub enum IpcCommand {
    // ... existing variants ...
    ClipboardRead {
        params: ClipboardReadParams,
        reply: oneshot::Sender<anyhow::Result<ClipboardReadResult>>,
    },
    ClipboardWrite {
        params: ClipboardWriteParams,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    ImeGetState {
        reply: oneshot::Sender<anyhow::Result<ImeStateResult>>,
    },
    ImeSetInputSource {
        params: ImeSetInputSourceParams,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
}
```

**crux-ipc/src/handler.rs** — Add dispatch cases in `dispatch_request()`.

**crux-app/src/app.rs** — Add handlers in `handle_ipc_command()`.

### Adding MCP Tools

**crux-mcp/src/tools/mod.rs** — Add new tool modules or extend existing ones.

For each new IPC method, add a corresponding MCP tool that calls through to IPC.

### Test Strategy

1. **Integration test**: Connect IPC client, call `crux:clipboard/write` with text, then `crux:clipboard/read`, verify round-trip.
2. **Integration test**: Call `crux:ime/get-state` and verify response structure.
3. **Unit test**: Verify serialization/deserialization of all new protocol types.

---

## 3.E Vim IME Auto-Switch

**Priority**: Medium (key differentiator for Korean Vim users)
**Effort**: ~1-2 days
**Dependencies**: None

### Design

When a terminal application changes cursor shape (via DECSCUSR escape sequence `\e[N q`), Crux detects the transition and switches the macOS input source:

| Transition | Meaning | Action |
|---|---|---|
| Beam/Underline → Block | Entering Normal mode | Switch to ASCII (e.g., `com.apple.keylayout.ABC`) |
| Block → Beam/Underline | Entering Insert mode | Restore previous IME (no switch needed — user will activate manually) |

### Files to Modify

**`crates/crux-terminal/src/terminal.rs`** — Track cursor shape transitions
**`crates/crux-terminal/src/event.rs`** — New event for cursor shape change
**`crates/crux-terminal-view/src/view.rs`** — Handle cursor shape change event, trigger IME switch
**New file**: `crates/crux-terminal-view/src/ime_switch.rs` — TIS FFI bindings

### Implementation

#### 3.E.1 Cursor shape change detection

**`crates/crux-terminal/src/event.rs`**:

```rust
pub enum TerminalEvent {
    // ... existing variants ...
    CursorShapeChanged {
        old_shape: CursorShape,
        new_shape: CursorShape,
    },
}
```

**`crates/crux-terminal/src/terminal.rs`**:

In `CruxTerminal::content()`, compare the current cursor shape against a stored `last_cursor_shape`. If different, emit a `CursorShapeChanged` event.

```rust
// In CruxTerminal struct:
last_cursor_shape: CursorShape,

// In content() method, after getting cursor state:
if cursor.shape != self.last_cursor_shape {
    // Can't use event_tx here (we're in content(), not a mutation).
    // Instead, store pending shape changes for drain_events().
    self.pending_cursor_shape = Some((self.last_cursor_shape, cursor.shape));
    self.last_cursor_shape = cursor.shape;
}
```

**[리뷰 반영] 이벤트 발행 방식 확정 (아키텍처 CONCERN fix)**:

`content()` 메서드에서 상태 변경은 순수성을 위반하므로, `drain_events()` 직후 shape를 비교하는 방식을 채택한다:

```rust
// In crux-terminal/src/terminal.rs
pub fn drain_events(&mut self) -> Vec<TerminalEvent> {
    let mut events: Vec<_> = self.event_rx.try_iter().collect();

    // Check cursor shape change after processing PTY output
    let current_shape = self.term.lock().cursor_style().shape;
    if current_shape != self.last_cursor_shape {
        events.push(TerminalEvent::CursorShapeChanged {
            old_shape: self.last_cursor_shape,
            new_shape: current_shape,
        });
        self.last_cursor_shape = current_shape;
    }

    events
}
```

이 방식은 기존 이벤트 아키텍처와 일관성을 유지하며, `content()`의 순수성을 보존한다.

#### 3.E.2 TIS FFI for input source switching

**New file**: `crates/crux-terminal-view/src/ime_switch.rs`

Use Carbon framework's `TISSelectInputSource` via raw FFI (not objc2 — this is C API):

```rust
#![cfg(target_os = "macos")]

use std::ffi::c_void;

// Carbon Text Input Source Services FFI
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> *mut c_void;
    fn TISSelectInputSource(source: *mut c_void) -> i32;
    fn TISCopyInputSourceForLanguage(language: *const c_void) -> *mut c_void;
    fn TISGetInputSourceProperty(source: *const c_void, key: *const c_void) -> *const c_void;
    fn TISCreateInputSourceList(
        properties: *const c_void,
        include_all: bool,
    ) -> *const c_void;
}

// Link against Carbon framework
#[link(name = "Carbon", kind = "framework")]
extern "C" {}

/// Switch to the ASCII input source (e.g., US keyboard).
pub fn switch_to_ascii() {
    // Use the macism approach: create a CGEvent to force switch.
    // TISSelectInputSource alone is unreliable for CJKV input methods.
    unsafe {
        let props = core_foundation::dictionary::CFDictionary::from_pairs(&[(
            kTISPropertyInputSourceType,
            kTISTypeKeyboardLayout,
        )]);
        let sources = TISCreateInputSourceList(props.as_concrete_TypeRef() as _, false);
        // Find "com.apple.keylayout.ABC" or first ASCII-capable source.
        // Select it via TISSelectInputSource.
    }
}

/// Get the current input source identifier.
pub fn current_input_source() -> Option<String> {
    unsafe {
        let source = TISCopyCurrentKeyboardInputSource();
        if source.is_null() { return None; }
        let id_key = kTISPropertyInputSourceID;
        let id_ref = TISGetInputSourceProperty(source, id_key as _);
        // Convert CFString to Rust String
        // CFRelease(source)
    }
}
```

**Note**: The `macism` workaround is important for reliability. `TISSelectInputSource` alone may fail silently for CJKV input methods. The workaround posts a `CGEvent` key event after selecting the source to force the system to update.

#### 3.E.3 Wire into CruxTerminalView

**`crates/crux-terminal-view/src/view.rs`**:

Add fields:
```rust
/// Whether Vim IME auto-switch is enabled.
vim_ime_switch: bool,
/// The input source that was active before switching to ASCII.
saved_input_source: Option<String>,
```

In `process_events()`, handle `CursorShapeChanged`:
```rust
TerminalEvent::CursorShapeChanged { old_shape, new_shape } => {
    if self.vim_ime_switch {
        let entering_normal = matches!(new_shape, CursorShape::Block)
            && !matches!(old_shape, CursorShape::Block);
        if entering_normal {
            // Save current input source and switch to ASCII.
            self.saved_input_source = ime_switch::current_input_source();
            ime_switch::switch_to_ascii();
        }
        // Note: We intentionally do NOT restore IME on entering insert mode.
        // The user will activate their preferred IME manually.
    }
}
```

#### 3.E.4 Configuration

For now, `vim_ime_switch` defaults to `false`. It can be toggled via a future config system (Phase 5) or via IPC (`crux:ime/set-input-source`).

### Test Strategy

1. **Manual test with nvim**: Open nvim in Crux, switch to Korean input, type Korean in Insert mode, press Escape, verify input switches to ASCII.
2. **Unit test**: Mock cursor shape transitions, verify events are emitted.
3. **Manual test**: Verify no switch happens when `vim_ime_switch` is disabled.

---

## 3.F IME Crash Resilience

**Priority**: Low
**Effort**: ~2 hours
**Dependencies**: None

### Design

Add a 100ms timeout around IME processing to prevent hangs. On macOS, the IME architecture uses Mach IPC which prevents hard deadlocks (unlike Linux X11/IBus), so this is defense-in-depth rather than a critical fix.

### Implementation

The EntityInputHandler methods (`replace_text_in_range`, `replace_and_mark_text_in_range`) are called synchronously by GPUI from the main thread. We cannot add async timeouts to these.

Instead, add defensive checks:

1. **Input validation**: In `replace_and_mark_text_in_range`, cap marked text length (reject preedit > 64 chars).
2. **State reset on anomaly**: If `marked_text` has been set for > 5 seconds without commit, force-commit it.

**File**: `crates/crux-terminal-view/src/view.rs`

```rust
// In CruxTerminalView struct:
marked_text_timestamp: Option<Instant>,

// In replace_and_mark_text_in_range:
self.marked_text_timestamp = if self.marked_text.is_some() {
    Some(Instant::now())
} else {
    None
};

// In process_events or render, check for stale composition:
if let Some(ts) = self.marked_text_timestamp {
    if ts.elapsed() > Duration::from_secs(5) {
        // Force-commit stale composition.
        if let Some(text) = self.marked_text.take() {
            let normalized: String = text.nfc().collect();
            self.terminal.write_to_pty(normalized.as_bytes());
        }
        self.marked_text_selected_range = None;
        self.marked_text_timestamp = None;
    }
}
```

### Test Strategy

1. **Manual test**: Type Korean, wait 5+ seconds mid-composition, verify text is force-committed.

---

## Implementation Order

```
1. 3.A — Wide char cursor fix         (standalone, quick win, ship immediately)
2. 3.B — Rich clipboard               (core feature, enables 3.C and 3.D)
3. 3.C — Drag & drop                  (depends on 3.B temp file logic)
4. 3.D — IPC protocol extensions      (depends on 3.B clipboard wiring)
5. 3.E — Vim IME auto-switch          (standalone, can parallelize with 3.C/3.D)
6. 3.F — IME crash resilience         (low priority, do last)
```

Items 3.E can be worked on in parallel with 3.C/3.D since they touch different files.

---

## New Dependencies Summary

| Crate | Version | Added To | Purpose |
|---|---|---|---|
| `image` | `0.25` | crux-clipboard | TIFF-to-PNG conversion |
| `url` | `2` | crux-clipboard | File URL parsing |
| `shell-escape` | `0.1` | crux-terminal-view | Escape file paths for shell |
| `core-foundation` | `0.10` | crux-terminal-view | TIS FFI CoreFoundation wrappers |
| `objc2-foundation` | `0.2` | crux-terminal-view | `MainThreadMarker` for clipboard access |

**Carbon framework**: Linked via `#[link(name = "Carbon")]` in ime_switch.rs — no crate needed.

---

## File Change Summary

| File | Changes |
|---|---|
| `crates/crux-terminal-view/src/element.rs` | 3.A: Wide char cursor, 3.C: Drop indicator |
| `crates/crux-terminal-view/src/view.rs` | 3.B: Rich paste, 3.C: Drop handlers, 3.E: Vim switch, 3.F: Timeout |
| `crates/crux-terminal-view/src/ime_switch.rs` | **NEW** — 3.E: TIS FFI bindings |
| `crates/crux-terminal-view/Cargo.toml` | 3.B: Add crux-clipboard, objc2-foundation, shell-escape |
| `crates/crux-clipboard/src/lib.rs` | 3.B: TIFF→PNG, temp save, file URL reading |
| `crates/crux-clipboard/Cargo.toml` | 3.B: Add image, url |
| `crates/crux-terminal/src/event.rs` | 3.B: ClipboardSet event, 3.E: CursorShapeChanged event |
| `crates/crux-terminal/src/terminal.rs` | 3.E: Track cursor shape transitions |
| `crates/crux-terminal/src/pty.rs` | 3.B: OSC 52 scanning |
| `crates/crux-protocol/src/lib.rs` | 3.D: All new protocol types + method constants |
| `crates/crux-ipc/src/command.rs` | 3.D: New IpcCommand variants |
| `crates/crux-ipc/src/handler.rs` | 3.D: Dispatch for new methods |
| `crates/crux-app/src/app.rs` | 3.D: Handle new IPC commands |
| `crates/crux-app/Cargo.toml` | 3.B: Add crux-clipboard dependency |
| `crates/crux-mcp/src/tools/` | 3.D: New MCP tools for clipboard/IME |
| `PLAN.md` | Mark 3.1, 3.2, 3.3 items as done |
