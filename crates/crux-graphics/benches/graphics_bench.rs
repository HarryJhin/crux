//! Criterion benchmarks for crux-graphics hot paths.
//!
//! Run with: `cargo bench -p crux-graphics`
//! Quick compile check: `cargo bench -p crux-graphics -- --test`

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

use crux_graphics::protocol::kitty::parse_kitty_command;
use crux_graphics::types::{ImageData, ImageId, ImagePlacement, PixelFormat};
use crux_graphics::ImageManager;

/// Build a realistic Kitty transmit+display command with a small RGBA payload.
fn make_kitty_transmit_cmd() -> Vec<u8> {
    // 10x10 RGBA image = 400 bytes raw, ~536 bytes base64.
    let raw_pixels = vec![0xAAu8; 400];
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &raw_pixels);
    format!("a=T,f=32,s=10,v=10,i=1;{b64}").into_bytes()
}

/// Build a Kitty delete-all command (minimal payload, pure parsing).
fn make_kitty_delete_cmd() -> Vec<u8> {
    b"a=d,d=a".to_vec()
}

/// Build a Kitty command with many key-value pairs to stress the parser.
fn make_kitty_complex_cmd() -> Vec<u8> {
    b"a=T,f=32,s=200,v=150,i=42,p=7,c=20,r=10,x=5,y=5,w=100,h=75,z=-1,q=2,m=0;AAAA"
        .to_vec()
}

fn bench_kitty_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("kitty_parse");

    let transmit = make_kitty_transmit_cmd();
    group.throughput(Throughput::Bytes(transmit.len() as u64));
    group.bench_function("transmit_display", |b| {
        b.iter(|| parse_kitty_command(black_box(&transmit)));
    });

    let delete = make_kitty_delete_cmd();
    group.bench_function("delete_all", |b| {
        b.iter(|| parse_kitty_command(black_box(&delete)));
    });

    let complex = make_kitty_complex_cmd();
    group.bench_function("complex_params", |b| {
        b.iter(|| parse_kitty_command(black_box(&complex)));
    });

    group.finish();
}

fn bench_image_store_retrieve(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_manager");

    // Benchmark storing a 100x100 BGRA image (40 KB).
    let pixel_data = vec![0u8; 100 * 100 * 4];
    group.bench_function("store_40kb", |b| {
        b.iter(|| {
            let mut mgr = ImageManager::new();
            let img = ImageData::new(pixel_data.clone(), 100, 100, PixelFormat::Bgra);
            mgr.store_image(black_box(ImageId(1)), img).unwrap();
        });
    });

    // Benchmark retrieving an image (LRU counter update).
    group.bench_function("retrieve", |b| {
        let mut mgr = ImageManager::new();
        let img = ImageData::new(vec![0u8; 40_000], 100, 100, PixelFormat::Bgra);
        mgr.store_image(ImageId(1), img).unwrap();
        b.iter(|| {
            mgr.get_image(black_box(ImageId(1))).unwrap();
        });
    });

    // Benchmark store with LRU eviction (quota pressure).
    group.bench_function("store_with_eviction", |b| {
        b.iter(|| {
            // Small quota forces eviction on each new store after the first two.
            let mut mgr = ImageManager::with_quota(80_000);
            for i in 0u32..10 {
                let img = ImageData::new(vec![0u8; 40_000], 100, 100, PixelFormat::Bgra);
                let _ = mgr.store_image(ImageId(i), img);
            }
        });
    });

    group.finish();
}

fn bench_placement_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("placement_query");

    // Setup: 50 images with 2 placements each at various rows.
    let mut mgr = ImageManager::new();
    for i in 1u32..=50 {
        let img = ImageData::new(vec![0u8; 1000], 10, 10, PixelFormat::Bgra);
        mgr.store_image(ImageId(i), img).unwrap();
        for p in 0..2u32 {
            let mut placement = ImagePlacement::new(ImageId(i));
            placement.placement_id = p;
            placement.row = (i as i32) * 2 + p as i32;
            placement.z_index = if p == 0 { -1 } else { 1 };
            mgr.place_image(placement).unwrap();
        }
    }

    // Query a viewport-sized range (24 rows).
    group.bench_function("viewport_24_rows", |b| {
        b.iter(|| {
            mgr.get_placements_in_range(black_box(10), black_box(34));
        });
    });

    // Query the full range (all placements).
    group.bench_function("full_range", |b| {
        b.iter(|| {
            mgr.get_placements_in_range(black_box(0), black_box(200));
        });
    });

    // Query an empty range (no placements).
    group.bench_function("empty_range", |b| {
        b.iter(|| {
            mgr.get_placements_in_range(black_box(500), black_box(600));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_kitty_parse,
    bench_image_store_retrieve,
    bench_placement_query,
);
criterion_main!(benches);
