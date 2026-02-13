//! Criterion benchmarks for crux-terminal hot paths.
//!
//! Run with: `cargo bench -p crux-terminal`
//! Quick compile check: `cargo bench -p crux-terminal -- --test`

use std::sync::mpsc;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

use crux_terminal::event::TerminalEvent;
use crux_terminal::osc_scanner::{parse_osc7_uri, scan_osc133, scan_osc7};

/// Build a realistic 4 KB PTY output buffer containing one OSC 7 sequence
/// embedded in normal terminal output.
fn make_osc7_buffer() -> Vec<u8> {
    let mut buf = Vec::with_capacity(4096);
    // Leading terminal output (command prompt + ls output).
    buf.extend_from_slice(b"drwxr-xr-x  12 user staff  384 Jan  1 12:00 Documents\r\n");
    buf.extend_from_slice(b"drwxr-xr-x   8 user staff  256 Jan  1 12:00 Downloads\r\n");
    buf.extend_from_slice(b"-rw-r--r--   1 user staff 1024 Jan  1 12:00 file.txt\r\n");
    // OSC 7 sequence (typical shell integration).
    buf.extend_from_slice(b"\x1b]7;file://MacBook-Pro.local/Users/jjh/Projects/crux\x07");
    // Trailing output to fill buffer.
    while buf.len() < 4096 {
        buf.extend_from_slice(b"-rw-r--r--   1 user staff  512 Jan  1 12:00 padding.txt\r\n");
    }
    buf.truncate(4096);
    buf
}

/// Build a realistic 4 KB PTY output buffer containing a full OSC 133
/// prompt cycle (A, B, C, D markers).
fn make_osc133_buffer() -> Vec<u8> {
    let mut buf = Vec::with_capacity(4096);
    // Prompt start.
    buf.extend_from_slice(b"\x1b]133;A\x07");
    buf.extend_from_slice(b"user@host:~/Projects/crux$ ");
    // Command start.
    buf.extend_from_slice(b"\x1b]133;B\x07");
    buf.extend_from_slice(b"cargo build\r\n");
    // Output start.
    buf.extend_from_slice(b"\x1b]133;C\x07");
    buf.extend_from_slice(b"   Compiling crux-terminal v0.1.0\r\n");
    buf.extend_from_slice(b"    Finished dev target(s) in 2.34s\r\n");
    // Command complete with exit code.
    buf.extend_from_slice(b"\x1b]133;D;0\x07");
    // Fill remainder with typical output.
    while buf.len() < 4096 {
        buf.extend_from_slice(b"   Compiling some-crate v1.0.0\r\n");
    }
    buf.truncate(4096);
    buf
}

/// Build a buffer with NO OSC sequences (pure terminal output) to measure
/// scanner overhead on non-matching data.
fn make_plain_buffer() -> Vec<u8> {
    let line = b"drwxr-xr-x  12 user staff  384 Jan  1 12:00 Documents\r\n";
    let mut buf = Vec::with_capacity(4096);
    while buf.len() < 4096 {
        buf.extend_from_slice(line);
    }
    buf.truncate(4096);
    buf
}

fn bench_osc7_scanner(c: &mut Criterion) {
    let buf = make_osc7_buffer();
    let mut group = c.benchmark_group("osc7_scanner");
    group.throughput(Throughput::Bytes(buf.len() as u64));

    group.bench_function("scan_with_match", |b| {
        b.iter(|| {
            let (tx, _rx) = mpsc::channel::<TerminalEvent>();
            scan_osc7(black_box(&buf), &tx);
        });
    });

    let plain = make_plain_buffer();
    group.bench_function("scan_no_match", |b| {
        b.iter(|| {
            let (tx, _rx) = mpsc::channel::<TerminalEvent>();
            scan_osc7(black_box(&plain), &tx);
        });
    });

    group.finish();
}

fn bench_osc133_scanner(c: &mut Criterion) {
    let buf = make_osc133_buffer();
    let mut group = c.benchmark_group("osc133_scanner");
    group.throughput(Throughput::Bytes(buf.len() as u64));

    group.bench_function("scan_full_cycle", |b| {
        b.iter(|| {
            let (tx, _rx) = mpsc::channel::<TerminalEvent>();
            scan_osc133(black_box(&buf), &tx);
        });
    });

    let plain = make_plain_buffer();
    group.bench_function("scan_no_match", |b| {
        b.iter(|| {
            let (tx, _rx) = mpsc::channel::<TerminalEvent>();
            scan_osc133(black_box(&plain), &tx);
        });
    });

    group.finish();
}

fn bench_parse_osc7_uri(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_osc7_uri");

    let simple = "file://hostname/Users/jjh/Projects/crux";
    group.bench_function("simple_path", |b| {
        b.iter(|| parse_osc7_uri(black_box(simple)));
    });

    let encoded = "file://host/Users/jjh/My%20Documents/some%20path%20with%20spaces";
    group.bench_function("percent_encoded", |b| {
        b.iter(|| parse_osc7_uri(black_box(encoded)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_osc7_scanner,
    bench_osc133_scanner,
    bench_parse_osc7_uri,
);
criterion_main!(benches);
