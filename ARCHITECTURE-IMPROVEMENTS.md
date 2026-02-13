# Architecture Improvements

Findings from Rust architectural principles audit (2026-02-13).

## Priority: Critical

### C1. Define `Terminal` trait in crux-terminal
- **Location**: `crates/crux-terminal/src/terminal.rs`
- **Problem**: `CruxTerminalView` depends on concrete `CruxTerminal`. No mock possible for unit testing.
- **Action**: Extract `write_to_pty()`, `resize()`, `content()`, `drain_events()`, `cwd()`, `size()`, `scroll_display()`, `selection_to_string()` into a `Terminal` trait. `CruxTerminal` implements it.
- **Impact**: Unlocks mock-based testing of the view layer.

### C2. Define `ClipboardProvider` trait in crux-clipboard
- **Location**: `crates/crux-clipboard/src/lib.rs`
- **Problem**: Static methods on concrete `Clipboard` struct. Cannot mock for tests, cannot swap for Linux.
- **Action**: Define `ClipboardProvider` trait with `read()`, `write_text()`, `write_image()` methods. NSPasteboard impl becomes one implementor.
- **Impact**: Enables testing + future cross-platform support.

### C3. Define `IpcTransport` trait in crux-ipc
- **Location**: `crates/crux-ipc/src/client.rs`
- **Problem**: `CruxMcpServer` depends on concrete `IpcClient`. Cannot test MCP tools without running Crux.
- **Action**: Define `IpcTransport` trait with `fn call(&self, method: &str, params: Value) -> Result<Value>`. `IpcClient` implements it.
- **Impact**: Enables MCP server unit testing.

### C4. Fix unsafe `unwrap()` in ipc_dispatch.rs
- **Location**: `crates/crux-app/src/ipc_dispatch.rs:115,135,153,167`
- **Problem**: `params.pane_id.unwrap()` can crash the app on malformed IPC requests.
- **Action**: Replace with `.ok_or_else(|| anyhow::anyhow!("pane_id required"))` and return JSON-RPC error.
- **Impact**: Prevents app crash from malformed IPC input.

## Priority: Important

### I1. Handle Mutex poisoning in IpcClient
- **Location**: `crates/crux-ipc/src/client.rs:65-66`
- **Problem**: `lock().unwrap()` will panic if a previous thread panicked while holding the lock.
- **Action**: Replace with `lock().map_err()` or switch to `parking_lot::Mutex` (no poisoning).

### I2. Add read timeout to IpcClient
- **Location**: `crates/crux-ipc/src/client.rs:82`
- **Problem**: Blocking `stream.read()` with no timeout. Hangs forever if server stops.
- **Action**: `stream.set_read_timeout(Some(Duration::from_secs(30)))` after connection.

### I3. Remove `term_arc()` from public API
- **Location**: `crates/crux-terminal/src/terminal.rs:243-245`
- **Problem**: Leaks internal `Arc<FairMutex<Term>>`. Undermines `with_term()` encapsulation.
- **Action**: Pass `Arc` directly during construction. Make method `pub(crate)` or remove.

### I4. Damage-aware cell copy in `content()`
- **Location**: `crates/crux-terminal/src/terminal.rs:252-320`
- **Problem**: Copies ALL cells every frame (~46KB at 80x24). Ignores `DamageState::Partial`.
- **Action**: When `DamageState::Partial`, only copy damaged lines. 80-90% allocation reduction.

### I5. Replace 9-argument render function with `RenderConfig`
- **Location**: `crates/crux-terminal-view/src/element.rs:29`
- **Problem**: `render_terminal_canvas()` takes 9 arguments, suppresses clippy lint.
- **Action**: Group into `RenderConfig` struct.

## Priority: Nice-to-have

### N1. Convert `FrameError` to thiserror
- **Location**: `crates/crux-protocol/src/framing.rs:10-25`
- **Action**: Replace manual `impl Display + impl Error` (13 lines) with `#[derive(thiserror::Error)]` (3 lines).

### N2. Pre-build font variants before cell loop
- **Location**: `crates/crux-terminal-view/src/element.rs:186,212,282,368`
- **Action**: Create `font_normal`, `font_bold`, `font_italic`, `font_bold_italic` once per frame. Eliminates `font.clone()` in inner loop.

### N3. Comment infallible `unwrap()` calls
- **Location**: `crates/crux-terminal-view/src/input.rs:173-195`, `mouse.rs:26`
- **Action**: Add `// infallible: writing to Vec<u8>` comment.

### N4. Add `Default` impl for `DamageState`
- **Location**: `crates/crux-terminal/src/terminal.rs:56`
- **Action**: `impl Default for DamageState { fn default() -> Self { Self::None } }`
