---
title: "VT Conformance Testing Strategy"
description: "Testing pyramid for terminal emulators: unit tests, snapshot testing with insta, Alacritty ref tests, esctest2, fuzzing with cargo-fuzz, CI pipeline, code coverage, performance regression"
date: 2026-02-12
phase: [1]
topics: [testing, vttest, esctest, fuzzing, ci]
status: final
related:
  - terminal-emulation.md
  - terminal-architecture.md
---

# VT Conformance Testing Strategy

> 작성일: 2026-02-12
> 목적: Crux 터미널 에뮬레이터의 테스팅 전략 — 단위 테스트부터 VT 적합성 테스트, 퍼징, CI 파이프라인까지

---

## 목차

1. [테스팅 피라미드 개요](#1-테스팅-피라미드-개요)
2. [Unit Tests — VT Parser](#2-unit-tests--vt-parser)
3. [Snapshot Tests — insta](#3-snapshot-tests--insta)
4. [Reference Tests — Alacritty Style](#4-reference-tests--alacritty-style)
5. [Integration Tests — expectrl](#5-integration-tests--expectrl)
6. [Conformance Tests — vttest and esctest2](#6-conformance-tests--vttest-and-esctest2)
7. [Fuzzing — cargo-fuzz](#7-fuzzing--cargo-fuzz)
8. [CI Pipeline — GitHub Actions](#8-ci-pipeline--github-actions)
9. [Code Coverage — cargo-llvm-cov](#9-code-coverage--cargo-llvm-cov)
10. [Performance Regression — criterion](#10-performance-regression--criterion)
11. [Key Test Scenarios](#11-key-test-scenarios)
12. [Crux Implementation Recommendations](#12-crux-implementation-recommendations)

---

## 1. 테스팅 피라미드 개요

```
                    ╱╲
                   ╱  ╲         Fuzzing (cargo-fuzz)
                  ╱    ╲        → Random input, crash discovery
                 ╱──────╲
                ╱        ╲      Conformance (esctest2, vttest)
               ╱          ╲    → Standard compliance validation
              ╱────────────╲
             ╱              ╲   Integration (expectrl)
            ╱                ╲  → Real shell + real PTY
           ╱──────────────────╲
          ╱                    ╲  Ref Tests (Alacritty-style)
         ╱                      ╲ → Recording-based regression
        ╱────────────────────────╲
       ╱                          ╲  Snapshot (insta)
      ╱                            ╲ → Grid state assertions
     ╱──────────────────────────────╲
    ╱                                ╲  Unit Tests
   ╱                                  ╲ → Parser, grid, cell operations
  ╱════════════════════════════════════╲
```

| Layer | Tool | Speed | Coverage | Runs In CI |
|-------|------|-------|----------|------------|
| Unit | `cargo test` | ~ms | Individual functions | Yes (every PR) |
| Snapshot | `insta` | ~ms | Grid state after sequences | Yes (every PR) |
| Ref Tests | Alacritty recordings | ~10ms | Full VT processing | Yes (every PR) |
| Integration | `expectrl` | ~100ms | PTY + shell interaction | Yes (gated) |
| Conformance | `esctest2` | ~seconds | VT100/xterm standard | Nightly/weekly |
| Fuzzing | `cargo-fuzz` | hours | Crash discovery | Nightly/manual |

---

## 2. Unit Tests — VT Parser

### What to Test

Since Crux uses `alacritty_terminal`, the VT parser itself is already well-tested upstream. Crux unit tests focus on:

- **Event handler callbacks**: Verify that Crux's `EventListener` implementation correctly processes events from `alacritty_terminal`
- **Grid manipulation helpers**: Any custom grid operations beyond what `alacritty_terminal` provides
- **Escape sequence generation**: Mouse reports, key encoding, response sequences
- **Configuration parsing**: TOML deserialization, validation, defaults

### Example

```rust
#[cfg(test)]
mod tests {
    use alacritty_terminal::term::Term;
    use alacritty_terminal::event::EventListener;

    #[test]
    fn test_cursor_movement() {
        let mut term = create_test_term(80, 24);
        // Feed escape sequence: move cursor to row 5, col 10
        term.input(b"\x1b[5;10H");
        let cursor = term.grid().cursor.point;
        assert_eq!(cursor.line.0, 4);   // 0-indexed
        assert_eq!(cursor.column.0, 9); // 0-indexed
    }

    #[test]
    fn test_sgr_mouse_report() {
        let report = sgr_mouse_report(
            AlacPoint::new(Line(4), Column(9)),
            0,     // left button
            true,  // pressed
        );
        assert_eq!(report, "\x1b[<0;10;5M");
    }

    #[test]
    fn test_cjk_wide_char_width() {
        let mut term = create_test_term(80, 24);
        term.input("한글".as_bytes());
        // Korean characters are 2 cells wide
        let cursor = term.grid().cursor.point;
        assert_eq!(cursor.column.0, 4); // 2 chars × 2 cells
    }
}
```

---

## 3. Snapshot Tests — insta

### Overview

The `insta` crate provides snapshot testing: assert against a stored "snapshot" of the expected output. When the output changes, `insta` shows a diff and lets you interactively accept or reject.

### Grid State Snapshots

```rust
use insta::assert_snapshot;

#[test]
fn test_colors_256() {
    let mut term = create_test_term(80, 24);
    // Set all 256 colors
    for i in 0..256 {
        term.input(format!("\x1b[38;5;{i}m{i:3} ").as_bytes());
    }
    assert_snapshot!(grid_to_string(&term));
}

#[test]
fn test_alternate_screen() {
    let mut term = create_test_term(80, 24);
    term.input(b"Main screen content\x1b[?1049h");  // Switch to alt screen
    term.input(b"Alt screen content\x1b[?1049l");   // Switch back
    assert_snapshot!(grid_to_string(&term));
}

fn grid_to_string(term: &Term<impl EventListener>) -> String {
    let mut output = String::new();
    let grid = term.grid();
    for line in grid.display_iter() {
        for cell in line {
            output.push(cell.c);
        }
        output.push('\n');
    }
    output
}
```

### Why insta?

- **Review workflow**: `cargo insta review` shows diffs interactively
- **CI-friendly**: `cargo insta test` fails on snapshot mismatch
- **Redaction**: Can redact dynamic values (timestamps, etc.)
- **Multiple formats**: String, YAML, JSON, CSV snapshots

**Crate**: `insta = "1.39"` with `glob` feature for bulk snapshot tests

---

## 4. Reference Tests — Alacritty Style

### Overview

Alacritty's ref tests are recording-based: a recorded byte stream is fed to the terminal, and the resulting grid state is compared against a stored reference. This is directly applicable to Crux since we use `alacritty_terminal`.

### Format

Each ref test consists of:
1. **Input file** (`*.in`): Raw bytes to feed to the terminal
2. **Grid file** (`*.grid`): Expected grid state (serialized)
3. **Config file** (`*.config`, optional): Terminal configuration overrides

### Leveraging Upstream Tests

```rust
// Can directly reuse Alacritty's ref tests since we use the same Term
#[test]
fn ref_test_vi_mode() {
    let input = include_bytes!("refs/vi_mode.in");
    let expected = include_str!("refs/vi_mode.grid");

    let mut term = create_test_term(80, 24);
    term.input(input);

    assert_eq!(grid_to_string(&term), expected);
}
```

### Creating New Ref Tests

```bash
# Record a terminal session
script -q /dev/null | tee ref_test.in
# ... interact with terminal ...
# The raw bytes are captured in ref_test.in

# Or use a more controlled approach:
printf '\e[38;5;196mRed\e[0m Normal' > color_test.in
```

### Organization

```
crates/crux-terminal/tests/
├── refs/
│   ├── basic/
│   │   ├── cursor_movement.in
│   │   ├── cursor_movement.grid
│   │   ├── line_wrapping.in
│   │   └── line_wrapping.grid
│   ├── colors/
│   │   ├── 256_colors.in
│   │   └── 256_colors.grid
│   ├── unicode/
│   │   ├── cjk_wide.in
│   │   └── cjk_wide.grid
│   └── modes/
│       ├── alt_screen.in
│       └── alt_screen.grid
```

---

## 5. Integration Tests — expectrl

### Overview

Integration tests use real shell processes over real PTYs. The `expectrl` crate (Rust port of Expect) provides pattern-based interaction:

```rust
use expectrl::{Session, Regex};

#[test]
fn test_shell_prompt() {
    let mut session = Session::spawn("bash --norc --noprofile").unwrap();
    session.set_expect_timeout(Some(Duration::from_secs(5)));

    // Wait for prompt
    session.expect(Regex(r"\$")).unwrap();

    // Send command
    session.send_line("echo hello").unwrap();

    // Verify output
    session.expect("hello").unwrap();
}

#[test]
fn test_osc7_cwd_reporting() {
    let mut session = Session::spawn("bash --norc --noprofile").unwrap();

    // Set up OSC 7 in the session
    session.send_line(
        r#"PROMPT_COMMAND='printf "\e]7;file://$(hostname)$(pwd)\e\\"'"#
    ).unwrap();

    // Change directory
    session.send_line("cd /tmp").unwrap();

    // Verify OSC 7 was emitted
    session.expect(Regex(r"\x1b\]7;file://.*?/tmp")).unwrap();
}
```

### What to Integration Test

| Test | What It Validates |
|------|-------------------|
| Shell launch + prompt | PTY creation, shell startup, basic I/O |
| Command execution | stdin → shell → stdout → terminal |
| Ctrl+C handling | Signal delivery through PTY |
| Window resize | SIGWINCH propagation |
| Tab completion | Shell integration with terminal I/O timing |
| Mouse reporting | Full roundtrip: GPUI event → escape sequence → application |

### Crate: `expectrl = "0.7"`

---

## 6. Conformance Tests — vttest and esctest2

### vttest

The classic VT100/VT220 conformance test suite by Thomas Dickey.

```bash
brew install vttest
vttest  # Interactive — requires manual observation
```

**Limitations**:
- Interactive only (requires human to judge pass/fail)
- Cannot be automated in CI
- Still valuable for visual verification during development

**Key vttest Screens**:
1. Character sets
2. Cursor movement
3. Screen features (scrolling regions, insert/delete)
4. VT52 mode
5. Double-size characters
6. Keyboard
7. Colors

### esctest2

Automated VT conformance tests. Originally created by George Nachman (iTerm2 author).

```bash
# Clone and run
git clone https://github.com/gnachman/esctest2
cd esctest2
python3 -m pytest tests/ --terminal crux
```

**Features**:
- Fully automated (pass/fail per test case)
- Tests specific escape sequences individually
- Can connect to any terminal via PTY
- Reports which sequences pass/fail

**Key Test Categories**:
| Category | Tests |
|----------|-------|
| CSI | Cursor movement, erase, scroll, insert/delete |
| DCS | DECRQSS, XTGETTCAP |
| OSC | Title, colors, hyperlinks |
| DEC modes | DECSET/DECRST for all private modes |

### Running in CI

```yaml
# esctest2 can run headlessly against the terminal's VT parser
- name: VT Conformance Tests
  run: |
    python3 -m pytest tests/ \
      --terminal crux-test-harness \
      --expected-failures expected_failures.txt \
      --junit-xml results.xml
```

Maintain an `expected_failures.txt` file tracking known failures. The goal is to decrease this list over time.

---

## 7. Fuzzing — cargo-fuzz

### Overview

Fuzzing the VT parser with random input to discover crashes, panics, and undefined behavior.

### Setup

```bash
cargo install cargo-fuzz
```

### Fuzz Target

```rust
// crates/crux-terminal/fuzz/fuzz_targets/vt_parser.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use alacritty_terminal::term::Term;

fuzz_target!(|data: &[u8]| {
    let mut term = create_fuzz_term(80, 24);
    // Feed arbitrary bytes — should never panic
    term.input(data);
    // Verify invariants
    let grid = term.grid();
    assert!(grid.cursor.point.line.0 < 24);
    assert!(grid.cursor.point.column.0 < 80);
});
```

### Corpus Seeds

Provide meaningful initial inputs to guide the fuzzer:

```
crates/crux-terminal/fuzz/corpus/vt_parser/
├── basic_text           # "Hello, World!\n"
├── csi_cursor           # "\x1b[10;20H"
├── sgr_color            # "\x1b[38;2;255;128;0m"
├── osc_title            # "\x1b]0;Title\x07"
├── alt_screen           # "\x1b[?1049h\x1b[?1049l"
├── utf8_cjk             # "한글テスト中文"
├── malformed_escape     # "\x1b[\x1b[\x1b["
├── max_params           # "\x1b[1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16m"
└── long_osc             # "\x1b]0;" + 100000 × 'A' + "\x07"
```

### Structured Fuzzing

For smarter fuzzing, define a structure for the fuzzer to generate valid-ish escape sequences:

```rust
use arbitrary::Arbitrary;

#[derive(Arbitrary, Debug)]
enum FuzzSequence {
    Text(String),
    Csi { params: Vec<u16>, intermediate: Option<u8>, final_byte: u8 },
    Osc { number: u16, data: String },
    Escape { byte: u8 },
    ControlChar(u8),
}
```

### Running

```bash
# Run locally
cargo fuzz run vt_parser -- -max_len=4096 -timeout=10

# Run for a fixed duration
cargo fuzz run vt_parser -- -max_total_time=3600  # 1 hour

# Check coverage
cargo fuzz coverage vt_parser
```

---

## 8. CI Pipeline — GitHub Actions

### Pipeline Architecture

```
PR Push → [Check] → [Test] → [Build] → [Integration] → [Nightly-only]
            │          │         │           │               │
            ▼          ▼         ▼           ▼               ▼
        cargo fmt   unit     macOS-14    expectrl      esctest2
        cargo       tests    build       tests         fuzzing
        clippy      insta                              coverage
                    ref
```

### Workflow Configuration

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -D warnings

jobs:
  check:
    name: Check & Lint
    runs-on: macos-14  # Apple Silicon
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --check
      - run: cargo clippy --workspace -- -D warnings

  test:
    name: Tests
    runs-on: macos-14
    needs: check
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace
      # Snapshot review check
      - run: cargo insta test --workspace

  build:
    name: Build
    runs-on: macos-14
    needs: check
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      # Verify full Xcode is available (Metal shaders)
      - run: xcrun -sdk macosx metal --version
      - run: cargo build --release -p crux-app
      - uses: actions/upload-artifact@v4
        with:
          name: crux-release
          path: target/release/crux

  integration:
    name: Integration Tests
    runs-on: macos-14
    needs: build
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace --features integration-tests
    # Note: These tests spawn real PTY sessions

  nightly:
    name: Nightly (Fuzz + Conformance + Coverage)
    if: github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - name: Fuzz for 30 minutes
        run: |
          cargo install cargo-fuzz
          timeout 1800 cargo fuzz run vt_parser || true
      - name: Code Coverage
        run: |
          cargo install cargo-llvm-cov
          cargo llvm-cov --workspace --lcov --output-path lcov.info
      - uses: codecov/codecov-action@v4
        with:
          files: lcov.info
```

### macOS CI Notes

| Item | Detail |
|------|--------|
| Runner | `macos-14` (M1, Apple Silicon) |
| Xcode | Pre-installed on GitHub-hosted macOS runners |
| Metal GPU | **NOT available** on GitHub-hosted runners (no GPU passthrough) |
| Workaround | GPU rendering tests must be gated behind a feature flag or run locally |
| Cache | `Swatinem/rust-cache@v2` for Cargo target directory |

### GPU Testing

Since Metal GPU is not available in CI, split tests:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_cell_grid_logic() {
        // This test runs in CI — no GPU needed
    }

    #[test]
    #[cfg(feature = "gpu-tests")]
    fn test_metal_rendering() {
        // This test only runs locally with: cargo test --features gpu-tests
    }
}
```

---

## 9. Code Coverage — cargo-llvm-cov

### Overview

`cargo-llvm-cov` is the recommended coverage tool for macOS. It uses LLVM's instrumentation, which is the most accurate on Apple platforms.

### Usage

```bash
# Install
cargo install cargo-llvm-cov

# Run with HTML report
cargo llvm-cov --workspace --html --open

# Run with LCOV output (for CI upload)
cargo llvm-cov --workspace --lcov --output-path lcov.info

# Show summary
cargo llvm-cov --workspace --summary-only
```

### Coverage Targets

| Crate | Target | Priority |
|-------|--------|----------|
| `crux-terminal` | 80%+ | VT event handling, grid ops |
| `crux-protocol` | 90%+ | Serialization, validation |
| `crux-ipc` | 70%+ | JSON-RPC message handling |
| `crux-clipboard` | 60%+ | Hard to test without GUI |
| `crux-terminal-view` | 50%+ | Rendering logic (GPU-dependent) |
| `crux-app` | 40%+ | Window management (GUI-dependent) |

### Alternative: tarpaulin

`cargo-tarpaulin` is Linux-only and does not work on macOS. Do not use.

---

## 10. Performance Regression — criterion

### Overview

Use `criterion` for micro-benchmarks to catch performance regressions:

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_vt_parser(c: &mut Criterion) {
    let input_ascii = "Hello, World!\r\n".repeat(1000);
    let input_cjk = "한글テスト中文🎉\r\n".repeat(1000);
    let input_sgr = "\x1b[38;2;255;128;0mColor\x1b[0m\r\n".repeat(1000);

    let mut group = c.benchmark_group("vt_parser");

    group.bench_function("ascii", |b| {
        b.iter(|| {
            let mut term = create_bench_term(80, 24);
            term.input(input_ascii.as_bytes());
        })
    });

    group.bench_function("cjk", |b| {
        b.iter(|| {
            let mut term = create_bench_term(80, 24);
            term.input(input_cjk.as_bytes());
        })
    });

    group.bench_function("sgr_color", |b| {
        b.iter(|| {
            let mut term = create_bench_term(80, 24);
            term.input(input_sgr.as_bytes());
        })
    });

    group.finish();
}

fn bench_scrollback(c: &mut Criterion) {
    c.bench_function("scroll_10k_lines", |b| {
        b.iter(|| {
            let mut term = create_bench_term(80, 24);
            let lines = "A".repeat(80) + "\n";
            let input = lines.repeat(10_000);
            term.input(input.as_bytes());
        })
    });
}

criterion_group!(benches, bench_vt_parser, bench_scrollback);
criterion_main!(benches);
```

### CI Integration

```yaml
- name: Benchmark
  run: |
    cargo bench --workspace -- --output-format bencher \
      | tee benchmark_results.txt
- uses: benchmark-action/github-action-benchmark@v1
  with:
    tool: cargo
    output-file-path: benchmark_results.txt
    alert-threshold: "120%"  # Fail if 20% slower
    comment-on-alert: true
```

---

## 11. Key Test Scenarios

### Must-Test VT Features

| Feature | Test Approach | Priority |
|---------|--------------|----------|
| 256 colors (SGR 38;5) | Snapshot: render all 256 colors | P0 |
| True color (SGR 38;2) | Snapshot: gradient rendering | P0 |
| CJK wide characters | Ref test: Korean/Japanese/Chinese text | P0 |
| Line wrapping | Ref test: text at column boundary | P0 |
| Alternate screen | Ref test: `\e[?1049h` / `\e[?1049l` | P0 |
| Scroll regions | Ref test: `\e[5;20r` + scroll | P0 |
| Mouse modes | Integration: vim `set mouse=a` | P1 |
| Bracketed paste | Integration: paste into shell | P1 |
| Synchronized output | Ref test: `\e[?2026h` / `\e[?2026l` | P1 |
| OSC 7 CWD | Integration: cd + verify CWD | P1 |
| OSC 8 hyperlinks | Snapshot: link rendering | P2 |
| Sixel graphics | Snapshot: simple image | P2 |
| Tab stops | Ref test: HTS, CHT, TBC | P2 |
| Character sets | Ref test: G0/G1/G2/G3 designate | P2 |

### Edge Cases to Fuzz

- Maximum parameter count in CSI sequences
- Zero-width characters (combining marks, ZWJ)
- Overlong UTF-8 sequences
- Truncated escape sequences at buffer boundaries
- Rapid mode switching (alternate screen toggle loops)
- Very long OSC strings (title strings > 4KB)

---

## 12. Crux Implementation Recommendations

### Phase 1 — Foundation

1. Set up `cargo test --workspace` with basic unit tests
2. Add `insta` for grid state snapshot tests
3. Port key Alacritty ref tests to verify `alacritty_terminal` integration
4. Set up GitHub Actions CI with check → test → build pipeline

### Phase 1+ — Expansion

5. Add `criterion` benchmarks for VT parser throughput
6. Add `expectrl` integration tests for shell interaction
7. Set up `cargo-fuzz` with VT parser fuzz target
8. Add `cargo-llvm-cov` coverage reporting

### Phase 2+ — Maturity

9. Run `esctest2` conformance suite, track expected failures
10. Add `vttest` to manual QA checklist
11. Set up nightly CI job for fuzzing + coverage + conformance
12. Add benchmark comparison in PRs (github-action-benchmark)

### Test Organization

```
crates/
├── crux-terminal/
│   ├── src/
│   ├── tests/
│   │   ├── refs/          # Alacritty-style ref tests
│   │   ├── snapshots/     # insta snapshots (auto-generated)
│   │   ├── unit/          # Unit test modules
│   │   └── integration/   # expectrl tests (feature-gated)
│   ├── benches/
│   │   └── vt_parser.rs   # criterion benchmarks
│   └── fuzz/
│       ├── corpus/
│       └── fuzz_targets/
│           └── vt_parser.rs
```

---

## Sources

- [Alacritty Ref Tests](https://github.com/alacritty/alacritty/tree/master/alacritty_terminal/tests) — Recording-based test format
- [insta documentation](https://insta.rs/) — Snapshot testing for Rust
- [esctest2](https://github.com/gnachman/esctest2) — Automated VT conformance tests
- [vttest](https://invisible-island.net/vttest/) — Classic VT100 conformance suite
- [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) — Rust fuzzing book
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) — LLVM-based code coverage
- [criterion.rs](https://bheisler.github.io/criterion.rs/book/) — Statistics-driven benchmarking
- [expectrl](https://docs.rs/expectrl/latest/expectrl/) — Rust Expect library
- [github-action-benchmark](https://github.com/benchmark-action/github-action-benchmark) — PR benchmark comparison
