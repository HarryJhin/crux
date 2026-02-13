//! Criterion benchmarks for crux-protocol hot paths.
//!
//! Run with: `cargo bench -p crux-protocol`
//! Quick compile check: `cargo bench -p crux-protocol -- --test`

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

use crux_protocol::{decode_frame, encode_frame, JsonRpcId, JsonRpcRequest, JsonRpcResponse};

// ---------------------------------------------------------------------------
// Frame encode/decode benchmarks
// ---------------------------------------------------------------------------

/// Build a small IPC message (~100 bytes) typical of a simple RPC call.
fn make_small_message() -> Vec<u8> {
    let req = JsonRpcRequest::new(JsonRpcId::Number(1), "crux:pane/list", None);
    serde_json::to_vec(&req).unwrap()
}

/// Build a medium IPC message (~1 KB) typical of a get-text response.
fn make_medium_message() -> Vec<u8> {
    let lines: Vec<String> = (0..20)
        .map(|i| format!("line {i}: drwxr-xr-x  12 user staff  384 Jan  1 12:00 Documents"))
        .collect();
    let resp = JsonRpcResponse::success(
        JsonRpcId::Number(42),
        serde_json::json!({
            "lines": lines,
            "first_line": 0,
            "cursor_row": 10,
            "cursor_col": 0,
        }),
    );
    serde_json::to_vec(&resp).unwrap()
}

/// Build a large IPC message (~16 KB) simulating a full terminal snapshot.
fn make_large_message() -> Vec<u8> {
    let lines: Vec<String> = (0..200)
        .map(|i| format!("line {i:>4}: {}", "x".repeat(70)))
        .collect();
    let resp = JsonRpcResponse::success(
        JsonRpcId::Number(100),
        serde_json::json!({
            "lines": lines,
            "rows": 200,
            "cols": 80,
            "cursor_row": 100,
            "cursor_col": 0,
            "cursor_shape": "block",
            "display_offset": 0,
            "has_selection": false,
            "title": "bash",
            "cwd": "/Users/jjh/Projects/crux",
        }),
    );
    serde_json::to_vec(&resp).unwrap()
}

fn bench_frame_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_encode");

    let small = make_small_message();
    group.throughput(Throughput::Bytes(small.len() as u64));
    group.bench_function("small_100b", |b| {
        b.iter(|| encode_frame(black_box(&small)));
    });

    let medium = make_medium_message();
    group.bench_function("medium_1kb", |b| {
        b.iter(|| encode_frame(black_box(&medium)));
    });

    let large = make_large_message();
    group.bench_function("large_16kb", |b| {
        b.iter(|| encode_frame(black_box(&large)));
    });

    group.finish();
}

fn bench_frame_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_decode");

    let small_frame = encode_frame(&make_small_message()).unwrap();
    group.throughput(Throughput::Bytes(small_frame.len() as u64));
    group.bench_function("small_100b", |b| {
        b.iter(|| decode_frame(black_box(&small_frame)));
    });

    let medium_frame = encode_frame(&make_medium_message()).unwrap();
    group.bench_function("medium_1kb", |b| {
        b.iter(|| decode_frame(black_box(&medium_frame)));
    });

    let large_frame = encode_frame(&make_large_message()).unwrap();
    group.bench_function("large_16kb", |b| {
        b.iter(|| decode_frame(black_box(&large_frame)));
    });

    // Benchmark incomplete frame (should return None quickly).
    group.bench_function("incomplete_header", |b| {
        let partial = &[0x00u8, 0x00];
        b.iter(|| decode_frame(black_box(partial)));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// JSON-RPC serialization/deserialization benchmarks
// ---------------------------------------------------------------------------

fn bench_jsonrpc_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("jsonrpc_serialize");

    // Simple request (no params).
    let simple_req = JsonRpcRequest::new(JsonRpcId::Number(1), "crux:pane/list", None);
    group.bench_function("request_no_params", |b| {
        b.iter(|| serde_json::to_vec(black_box(&simple_req)));
    });

    // Request with params.
    let params_req = JsonRpcRequest::new(
        JsonRpcId::Number(2),
        "crux:pane/send-text",
        Some(serde_json::json!({
            "pane_id": 1,
            "text": "echo hello world\n",
            "bracketed_paste": false,
        })),
    );
    group.bench_function("request_with_params", |b| {
        b.iter(|| serde_json::to_vec(black_box(&params_req)));
    });

    // Success response with result.
    let success_resp = JsonRpcResponse::success(
        JsonRpcId::Number(42),
        serde_json::json!({
            "panes": [
                {"pane_id": 1, "window_id": 1, "tab_id": 1, "title": "bash", "is_active": true},
                {"pane_id": 2, "window_id": 1, "tab_id": 1, "title": "vim", "is_active": false},
            ]
        }),
    );
    group.bench_function("response_success", |b| {
        b.iter(|| serde_json::to_vec(black_box(&success_resp)));
    });

    // Error response.
    let error_resp = JsonRpcResponse::error(JsonRpcId::Number(7), -1001, "pane 99 not found");
    group.bench_function("response_error", |b| {
        b.iter(|| serde_json::to_vec(black_box(&error_resp)));
    });

    group.finish();
}

fn bench_jsonrpc_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("jsonrpc_deserialize");

    // Pre-serialize to JSON bytes for deserialization benchmarks.
    let simple_json = serde_json::to_vec(&JsonRpcRequest::new(
        JsonRpcId::Number(1),
        "crux:pane/list",
        None,
    ))
    .unwrap();
    group.throughput(Throughput::Bytes(simple_json.len() as u64));
    group.bench_function("request_no_params", |b| {
        b.iter(|| serde_json::from_slice::<JsonRpcRequest>(black_box(&simple_json)));
    });

    let params_json = serde_json::to_vec(&JsonRpcRequest::new(
        JsonRpcId::Number(2),
        "crux:pane/send-text",
        Some(serde_json::json!({
            "pane_id": 1,
            "text": "echo hello world\n",
            "bracketed_paste": false,
        })),
    ))
    .unwrap();
    group.bench_function("request_with_params", |b| {
        b.iter(|| serde_json::from_slice::<JsonRpcRequest>(black_box(&params_json)));
    });

    let resp_json = serde_json::to_vec(&JsonRpcResponse::success(
        JsonRpcId::Number(42),
        serde_json::json!({"panes": [{"pane_id": 1, "title": "bash"}]}),
    ))
    .unwrap();
    group.bench_function("response_success", |b| {
        b.iter(|| serde_json::from_slice::<JsonRpcResponse>(black_box(&resp_json)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_frame_encode,
    bench_frame_decode,
    bench_jsonrpc_serialize,
    bench_jsonrpc_deserialize,
);
criterion_main!(benches);
