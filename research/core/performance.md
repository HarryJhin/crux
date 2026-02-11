---
title: "Performance Benchmarks and Optimization"
description: "Input latency targets, VT parser throughput, 120fps Metal rendering, memory management, GPU profiling, optimization patterns from Alacritty/Ghostty/WezTerm"
date: 2026-02-12
phase: [1]
topics: [performance, latency, throughput, metal, profiling]
status: final
related:
  - terminal-architecture.md
  - ../gpui/framework.md
---

# Performance Benchmarks and Optimization

> 작성일: 2026-02-12
> 목적: Crux 터미널의 성능 목표 설정, 벤치마크 기준, 프로파일링 도구, 최적화 패턴 분석

---

## 목차

1. [성능 목표](#1-성능-목표)
2. [Input Latency](#2-input-latency)
3. [VT Parser Throughput](#3-vt-parser-throughput)
4. [Rendering Performance](#4-rendering-performance)
5. [Memory Management](#5-memory-management)
6. [GPU Memory](#6-gpu-memory)
7. [Profiling Tools](#7-profiling-tools)
8. [Optimization Patterns from Other Terminals](#8-optimization-patterns-from-other-terminals)
9. [Crux Implementation Recommendations](#9-crux-implementation-recommendations)

---

## 1. 성능 목표

### Target Metrics

| Metric | Target | Why |
|--------|--------|-----|
| Keystroke-to-display latency (median) | < 8ms | Below human perception threshold (~10ms) |
| Keystroke-to-display latency (p99.9) | < 30ms | No perceptible lag even in worst case |
| VT parser throughput | > 500 MB/s | Handle `cat large_file` without delay |
| Frame render time | < 6ms | Leave headroom for 120fps (8.3ms budget) |
| Memory (RSS) at startup | < 50 MB | Comparable to Alacritty |
| Memory (RSS) with 10K scrollback | < 100 MB | Reasonable for long sessions |
| Memory per additional tab | < 15 MB | Efficient multi-tab usage |
| Time to first frame | < 300ms | Fast cold start |

### How These Compare

| Terminal | Median Latency | Throughput | Memory (idle) |
|----------|---------------|------------|---------------|
| Alacritty | ~5ms | ~900 MB/s | ~30 MB |
| Kitty | ~6ms | ~600 MB/s | ~40 MB |
| Ghostty | ~3ms | ~7 GB/s (SIMD) | ~35 MB |
| WezTerm | ~8ms | ~400 MB/s | ~60 MB |
| iTerm2 | ~15ms | ~200 MB/s | ~100 MB |
| **Crux target** | **< 8ms** | **> 500 MB/s** | **< 50 MB** |

Sources: [typometer benchmarks](https://github.com/pavelfatin/typometer), [terminal throughput tests](https://github.com/alacritty/vtebench)

---

## 2. Input Latency

### Latency Budget Breakdown

```
User presses key
    │  ~1ms     USB/Bluetooth HID polling
    ▼
macOS input event
    │  ~0.5ms   NSEvent dispatch, GPUI event loop
    ▼
GPUI KeyDown handler
    │  ~0.1ms   Key → escape sequence encoding
    ▼
PTY write
    │  ~0.1ms   Write escape sequence to PTY fd
    ▼
Shell/application processes
    │  ~1-5ms   Application-dependent (echo, redraw)
    ▼
PTY read
    │  ~0.1ms   Read response from PTY
    ▼
VT parser
    │  ~0.05ms  Parse escape sequences, update grid
    ▼
Damage tracking
    │  ~0.01ms  Identify changed cells
    ▼
Frame render
    │  ~2ms     GPU rasterization + display
    ▼
Display refresh
    │  ~0-8ms   Wait for next VSync (120Hz = 8.3ms)
    ▼
Photon leaves screen  → Total: ~5-16ms typical
```

### Key Optimization Points

1. **Minimize event loop hops**: Direct path from key event to PTY write
2. **Batch PTY reads**: Don't render every byte — accumulate for up to 4ms
3. **Async PTY I/O**: Non-blocking reads on a dedicated thread
4. **VSync-aligned rendering**: Don't render faster than display refresh rate

### Measuring Latency

```bash
# typometer: Visual latency measurement
# Requires screen recording at high frame rate
# Measures time from key LED change to character appearance
brew install typometer

# Manual measurement with timestamps
# In Crux, add tracing spans:
```

```rust
use tracing::{instrument, info_span};

#[instrument(skip_all)]
fn handle_key_event(&mut self, event: &KeyDownEvent) {
    let _span = info_span!("key_to_display").entered();
    // ... key processing ...
}
```

---

## 3. VT Parser Throughput

### Why Throughput Matters

Users run `cat large_file.log`, `grep -r pattern .`, or build systems that produce megabytes of output. The VT parser must handle this without lag.

### alacritty_terminal Performance

Since Crux uses `alacritty_terminal`, we inherit its parser performance:

- **Alacritty's VT parser**: ~900 MB/s for ASCII-heavy content
- **UTF-8 overhead**: ~30% slower for CJK-heavy content (multi-byte sequences)
- **SGR overhead**: ~50% slower for heavily colored output (many escape sequences)

### Throughput Benchmark

```rust
use criterion::{criterion_group, criterion_main, Criterion, Throughput};

fn bench_throughput(c: &mut Criterion) {
    // Generate test data
    let ascii_data = "A".repeat(80).as_bytes().to_vec();
    let ascii_block: Vec<u8> = std::iter::repeat(ascii_data)
        .take(1000)
        .flat_map(|line| line.into_iter().chain(std::iter::once(b'\n')))
        .collect();

    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Bytes(ascii_block.len() as u64));

    group.bench_function("ascii", |b| {
        b.iter(|| {
            let mut term = create_bench_term(80, 24);
            term.input(&ascii_block);
        })
    });

    group.finish();
}
```

### vtebench

The [vtebench](https://github.com/alacritty/vtebench) tool generates standardized terminal benchmark data:

```bash
git clone https://github.com/alacritty/vtebench
cd vtebench
cargo run --release -- -w 80 -h 24 > benchmark.vte

# Feed to Crux's test harness
cat benchmark.vte | crux-bench
```

### Ghostty's SIMD Optimization

Ghostty achieves 7.3x speedup on ASCII content using SIMD (Single Instruction, Multiple Data):

- **AVX2/NEON**: Process 32 bytes at once for ASCII detection
- **Fast path**: If a 32-byte chunk is all ASCII (bytes < 0x80), skip UTF-8 decode entirely
- **Fallback**: For chunks containing non-ASCII or escape characters, fall back to byte-by-byte parsing

```rust
// Conceptual SIMD fast path (simplified)
fn find_next_escape(data: &[u8]) -> Option<usize> {
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::aarch64::*;
        let escape = vdupq_n_u8(0x1B);
        // Process 16 bytes at a time with NEON
        for chunk in data.chunks(16) {
            let v = vld1q_u8(chunk.as_ptr());
            let cmp = vceqq_u8(v, escape);
            let mask = vmaxvq_u8(cmp);
            if mask != 0 {
                // Found ESC in this chunk — find exact position
                return Some(/* position */);
            }
        }
    }
    None
}
```

**Relevance for Crux**: Since we use `alacritty_terminal`'s parser, SIMD optimization would require forking or wrapping the parser. Consider this only if profiling shows the parser is the bottleneck (unlikely — rendering usually dominates).

---

## 4. Rendering Performance

### GPUI's Metal Pipeline

GPUI handles Metal rendering internally:

```
Terminal Grid State
    │
    ▼
CruxTerminalElement::paint()    ← Crux code
    │
    ▼
GPUI Scene Construction         ← GPUI framework
    │  - Text glyphs → glyph atlas lookups
    │  - Rectangles → quad batches
    │  - Box drawing → custom geometry
    ▼
Metal Command Buffer            ← GPUI/Metal
    │  - Draw calls batched
    │  - GPU submission
    ▼
CVDisplayLink (120 Hz)          ← macOS
    │  - VSync-aligned present
    ▼
Display
```

### 120fps Rendering

Modern Macs have ProMotion displays (120Hz). GPUI uses CVDisplayLink for VSync:

- **Frame budget**: 8.33ms at 120Hz
- **Target render time**: < 6ms (leaves 2ms headroom)
- **Adaptive refresh**: macOS dynamically adjusts refresh rate; GPUI follows

### Damage Tracking

`alacritty_terminal` provides `TermDamage` — a record of which cells changed since the last render:

```rust
use alacritty_terminal::term::TermDamage;

fn render_frame(&mut self, term: &mut Term) {
    let damage = term.damage();

    match damage {
        TermDamage::Full => {
            // Full redraw (resize, scroll, etc.)
            self.render_all_cells(term);
        }
        TermDamage::Partial(damaged_lines) => {
            // Only redraw changed lines
            for line in damaged_lines {
                self.render_line(term, line);
            }
        }
    }

    term.reset_damage();
}
```

**Key insight**: GPUI redraws the entire scene each frame (immediate mode). But damage tracking still helps by reducing the work in `paint()` — we only need to update the GPUI elements for changed cells.

### Event Batching

Don't render every PTY read. Batch events within a time window:

```rust
const MAX_BATCH_SIZE: usize = 100;
const BATCH_TIMEOUT: Duration = Duration::from_millis(4);

fn pty_read_loop(term: Arc<Mutex<Term>>, cx: AsyncAppContext) {
    let mut buffer = Vec::with_capacity(65536);
    let mut last_flush = Instant::now();
    let mut event_count = 0;

    loop {
        match pty.read(&mut buf) {
            Ok(n) => {
                buffer.extend_from_slice(&buf[..n]);
                event_count += 1;

                let should_flush =
                    event_count >= MAX_BATCH_SIZE ||
                    last_flush.elapsed() >= BATCH_TIMEOUT ||
                    buffer.len() >= 65536;

                if should_flush {
                    let mut term = term.lock();
                    term.input(&buffer);
                    buffer.clear();
                    event_count = 0;
                    last_flush = Instant::now();
                    cx.notify();  // Trigger GPUI repaint
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Flush remaining buffer
                if !buffer.is_empty() {
                    let mut term = term.lock();
                    term.input(&buffer);
                    buffer.clear();
                    cx.notify();
                }
                // Wait for more data
                poll_pty(&pty);
            }
            Err(_) => break,
        }
    }
}
```

This is the Zed pattern: max 100 events or 4ms, whichever comes first.

---

## 5. Memory Management

### Memory Budget

| Component | Budget | Strategy |
|-----------|--------|----------|
| Visible grid (80x24) | ~15 KB | Inline in `alacritty_terminal::Grid` |
| Scrollback (10K lines) | ~6 MB | `Grid` stores as `Vec<Row>` |
| Scrollback (100K lines) | ~60 MB | Consider ring buffer |
| Glyph atlas | ~4 MB | GPU texture, GPUI-managed |
| PTY buffers | ~128 KB | Read/write ring buffers |
| Per-tab overhead | ~7 MB | Grid + PTY + state |

### alacritty_terminal Memory Layout

```rust
// Each cell in alacritty_terminal:
pub struct Cell {
    pub c: char,           // 4 bytes
    pub fg: Color,         // 4 bytes (indexed or RGB)
    pub bg: Color,         // 4 bytes
    pub flags: Flags,      // 2 bytes (bold, italic, underline, etc.)
    pub extra: Option<Box<CellExtra>>,  // 8 bytes (hyperlink, rarely used)
}
// Total: ~22 bytes per cell (+ alignment → 24 bytes)

// For 80 columns × 10,000 scrollback lines:
// 80 × 10,000 × 24 bytes = ~18.3 MB
```

### WezTerm's Clustered Storage

WezTerm uses a "clustered" storage format that achieves **40x memory reduction** for scrollback:

Instead of storing per-cell attributes, WezTerm stores runs of cells with the same attributes:

```
Traditional:  [Cell{c='H', fg=white, bg=black}, Cell{c='e', fg=white, bg=black}, ...]
Clustered:    [Cluster{text="Hello", fg=white, bg=black, start=0, len=5}]
```

For typical terminal output (long runs of same-colored text), this dramatically reduces memory.

**Relevance for Crux**: `alacritty_terminal` does not use clustered storage. If memory becomes a concern for large scrollback, consider:
1. Limiting scrollback (configurable, default 10K)
2. Compressing old scrollback lines
3. Spilling scrollback to disk (mmap)

### Memory Profiling

```bash
# dhat-rs: Heap profiling
# Add to Cargo.toml: dhat = "0.3"
DHAT_ENABLED=1 cargo run --release

# Activity Monitor / vmmap
vmmap $(pgrep crux) | head -50

# Instruments: Allocations template
open -a Instruments  # Choose "Allocations" template
```

---

## 6. GPU Memory

### GPUI's GPU Memory Usage

GPUI manages GPU resources internally:

| Resource | Size | Notes |
|----------|------|-------|
| Glyph atlas | 2-8 MB | Grows with unique glyph count |
| Scene buffer | ~1 MB | Per-frame draw commands |
| Framebuffer | ~16 MB | 2560x1600 × 4 bytes × 2 (double buffer) |
| Metal pipeline state | ~100 KB | Cached shader states |

### GPU Memory Optimization

1. **Glyph atlas management**: GPUI handles this; limit unique font/size combinations
2. **Minimize unique colors**: Group similar colors if atlas pressure is high
3. **Frame buffer**: Cannot optimize (display resolution determines size)

### Metal Memory Debugging

```bash
# Enable Metal validation layer
export MTL_DEBUG_LAYER=1

# Use Metal System Trace in Instruments
# Tracks GPU memory allocations, command buffer timing, shader occupancy
```

---

## 7. Profiling Tools

### Complete Profiling Stack

| Tool | What It Measures | When to Use |
|------|-----------------|-------------|
| **criterion** | Micro-benchmarks (ns/μs) | Parser throughput, grid operations |
| **cargo flamegraph** | CPU time distribution | Overall CPU hotspots |
| **tracing + tracy** | Frame-level timing | Per-frame breakdown |
| **Metal System Trace** | GPU timing, occupancy | Render pipeline bottlenecks |
| **dhat-rs** | Heap allocations | Memory allocation patterns |
| **Activity Monitor** | RSS, CPU % | Quick sanity check |
| **vmmap** | Virtual memory map | Memory region analysis |
| **Instruments** | Everything | Deep dive (Allocations, Time Profiler, Metal) |

### criterion (Micro-Benchmarks)

```toml
# Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "vt_parser"
harness = false
```

```bash
cargo bench -- vt_parser
# Opens HTML report with statistical analysis
```

### cargo flamegraph (CPU Profiling)

```bash
cargo install flamegraph
cargo flamegraph --root -p crux-app
# Produces flamegraph.svg — open in browser
```

### tracing + tracy (Frame Profiling)

```rust
// Add tracing spans to critical paths
use tracing::instrument;

#[instrument(skip_all)]
fn paint(&mut self, bounds: Bounds<Pixels>, cx: &mut WindowContext) {
    let _render = tracing::info_span!("terminal_render").entered();
    // ... rendering code ...
}
```

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = "0.3"
tracing-tracy = "0.11"  # Tracy profiler integration
```

```bash
# Run with Tracy connection
TRACY_ENABLE=1 cargo run --release
# Then open Tracy profiler and connect
```

### Metal System Trace (GPU Profiling)

1. Open **Instruments.app**
2. Choose **Metal System Trace** template
3. Record while running Crux
4. Analyze: GPU utilization, command buffer timing, shader occupancy

Key metrics to watch:
- **GPU idle time**: Should be >50% at 120fps (we're not GPU-bound)
- **Command buffer duration**: Should be <4ms
- **Glyph atlas usage**: Watch for atlas rebuilds

---

## 8. Optimization Patterns from Other Terminals

### Alacritty

| Pattern | Description |
|---------|-------------|
| **Damage tracking** | `TermDamage` tracks changed lines/cells |
| **Grid compaction** | Removes trailing whitespace from scrollback |
| **Batch PTY reads** | Processes all available bytes before rendering |
| **Texture atlas** | Single GPU texture for all glyphs |

### Ghostty

| Pattern | Description |
|---------|-------------|
| **SIMD parser** | 7.3x ASCII throughput via NEON/AVX2 |
| **Custom font renderer** | Bypasses Core Text for ASCII fast path |
| **Zero-copy PTY** | mmap-based PTY buffer |
| **Arena allocator** | Per-frame arena for transient allocations |

### WezTerm

| Pattern | Description |
|---------|-------------|
| **Clustered storage** | 40x scrollback memory reduction |
| **Deferred rendering** | Only renders visible + small margin |
| **Font cache** | LRU cache for shaped glyph runs |
| **Async everything** | Fully async I/O with tokio |

### Patterns Applicable to Crux

| Pattern | Source | Applicability | Effort |
|---------|--------|---------------|--------|
| Damage tracking | Alacritty | Already available via `alacritty_terminal` | Free |
| Batch PTY reads | Alacritty/Zed | Must implement in event loop | Low |
| Event batching (100/4ms) | Zed | Must implement | Low |
| Glyph atlas | GPUI | Already provided by GPUI | Free |
| SIMD parser | Ghostty | Only if parser is bottleneck | High |
| Clustered storage | WezTerm | Only if memory is concern | High |
| Arena allocator | Ghostty | Per-frame allocations | Medium |

---

## 9. Crux Implementation Recommendations

### Phase 1 — Performance Foundation

1. **Benchmark suite**: Set up criterion for VT parser and grid operations
2. **Event batching**: Implement 100-event / 4ms batch window (Zed pattern)
3. **Damage tracking**: Use `alacritty_terminal::TermDamage` to minimize paint work
4. **PTY thread**: Dedicated thread for PTY I/O, non-blocking reads
5. **Tracing spans**: Add `tracing` instrumentation to critical paths

### Phase 1+ — Measurement

6. **Latency measurement**: Integrate typometer-style measurement
7. **Throughput benchmark**: Run vtebench, compare against targets
8. **Memory baseline**: Track RSS at startup, with scrollback, per-tab
9. **CI benchmarks**: criterion in CI with regression detection

### Phase 2+ — Optimization

10. **Profile before optimizing**: Use flamegraph/tracy to find actual bottlenecks
11. **SIMD fast path**: Consider for ASCII detection if parser is bottleneck
12. **Scrollback compression**: Implement if memory exceeds targets
13. **GPU profiling**: Metal System Trace for render pipeline analysis

### Anti-Patterns to Avoid

| Anti-Pattern | Why | Alternative |
|-------------|------|-------------|
| Render every PTY byte | Wastes GPU cycles | Batch events |
| Full redraw every frame | Ignores damage tracking | Use TermDamage |
| Blocking PTY reads on main thread | Freezes UI | Dedicated I/O thread |
| Premature SIMD | Complex, fragile | Profile first |
| Unbounded scrollback | Memory explosion | Default limit + config |

---

## Sources

- [typometer](https://github.com/pavelfatin/typometer) — Visual keystroke latency measurement
- [vtebench](https://github.com/alacritty/vtebench) — Terminal throughput benchmark generator
- [Alacritty Performance](https://jwilm.io/blog/alacritty-lands-scrollback/) — Alacritty performance architecture
- [Ghostty SIMD Devlog](https://mitchellh.com/writing/ghostty-devlog-004) — SIMD terminal parsing
- [WezTerm Clustered Storage](https://wezfurlong.org/wezterm/internals.html) — Memory-efficient scrollback
- [criterion.rs](https://bheisler.github.io/criterion.rs/book/) — Statistics-driven Rust benchmarks
- [cargo flamegraph](https://github.com/flamegraph-rs/flamegraph) — CPU profiling
- [Tracy Profiler](https://github.com/wolfpld/tracy) — Frame profiler with Rust support
- [Metal Best Practices Guide](https://developer.apple.com/library/archive/documentation/3DDrawing/Conceptual/MTLBestPracticesGuide/) — Apple GPU optimization guide
