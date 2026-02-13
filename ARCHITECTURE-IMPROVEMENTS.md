# Architecture Improvements

Findings from Rust architectural principles audit (2026-02-13).

## Priority: Critical — All Complete

### C1. Define `Terminal` trait in crux-terminal ✓
- **Location**: `crates/crux-terminal/src/traits.rs`
- **Done**: Extracted 13 methods into `Terminal` trait. `CruxTerminal` implements it. Re-exported via `lib.rs`.

### C2. Define `ClipboardProvider` trait in crux-clipboard ✓
- **Location**: `crates/crux-clipboard/src/lib.rs`
- **Done**: Defined `ClipboardProvider` trait with 6 methods. macOS NSPasteboard impl extracted to `macos.rs`. `ClipboardError` converted to thiserror.

### C3. Define `IpcTransport` trait in crux-ipc ✓
- **Location**: `crates/crux-ipc/src/client.rs`
- **Done**: Defined `IpcTransport` trait. `IpcClient` implements it. MCP server uses `Arc<dyn IpcTransport>`.

### C4. Fix unsafe `unwrap()` in ipc_dispatch.rs ✓
- **Location**: `crates/crux-app/src/ipc_dispatch.rs`
- **Done**: Introduced `resolve_pane()` helper that returns proper JSON-RPC errors for missing/invalid panes.

## Priority: Important — All Complete (except I4)

### I1. Handle Mutex poisoning in IpcClient ✓
- **Done**: Replaced `lock().unwrap()` with `lock().map_err()` in `call_inner()`.

### I2. Add read timeout to IpcClient ✓
- **Done**: Added `set_read_timeout(Some(Duration::from_secs(30)))` in `connect_to()`.

### I3. Remove `term_arc()` from public API ✓
- **Done**: Method was dead code (no callers). Removed entirely.

### I4. Damage-aware cell copy in `content()` — Deferred
- **Location**: `crates/crux-terminal/src/terminal.rs`
- **Reason**: Performance optimization requiring careful benchmarking. Not a correctness issue.

### I5. Replace 9-argument render function with `RenderConfig` ✓
- **Done**: Extracted `RenderConfig` struct. Removed `#[allow(clippy::too_many_arguments)]`.

## Priority: Nice-to-have — All Complete (except N2)

### N1. Convert `FrameError` to thiserror ✓
- **Done**: 13 lines → 3 lines with `#[derive(thiserror::Error)]`.

### N2. Pre-build font variants before cell loop — Deferred
- **Reason**: Requires deeper analysis of GPUI's font shaping pipeline to avoid regressions.

### N3. Comment infallible `unwrap()` calls ✓
- **Done**: Added `// infallible: writing to Vec<u8>` to `input.rs` and `mouse.rs`.

### N4. Add `Default` impl for `DamageState` ✓
- **Done**: `#[derive(Default)]` with `#[default]` on `None` variant.
