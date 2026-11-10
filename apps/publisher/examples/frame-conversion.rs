//! Offscreen conversion-stage benchmark; does not capture, connect or render.
#[path = "../src/video_image.rs"]
#[allow(dead_code)]
mod video_image;

use livekit::webrtc::video_frame::I420Buffer;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

struct CountedAllocator;
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
// Delegates every operation unchanged to System. Counts Rust allocation traffic,
// not C++ decoder allocations, resident pages, GPU memory or total app memory.
unsafe impl GlobalAlloc for CountedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(size as u64, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountedAllocator = CountedAllocator;

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        args.len() == 4,
        "Use width height frames-per-worker workers."
    );
    let width: u32 = args[0].parse()?;
    let height: u32 = args[1].parse()?;
    let count: usize = args[2].parse()?;
    let workers: usize = args[3].parse()?;
    anyhow::ensure!(
        (2..=3840).contains(&width)
            && (2..=2160).contains(&height)
            && (20..=10000).contains(&count)
            && (1..=4).contains(&workers),
        "Invalid workload."
    );
    let ready = Arc::new(Barrier::new(workers + 1));
    let start = Arc::new(Barrier::new(workers + 1));
    let handles: Vec<_> = (0..workers)
        .map(|worker| {
            let ready = ready.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                let mut buffer = I420Buffer::new(width, height);
                let (y, u, v) = buffer.data_mut();
                for (plane, offset) in [(y, 16), (u, 70), (v, 140)] {
                    for (index, byte) in plane.iter_mut().enumerate() {
                        *byte = ((index * 37 + offset + worker * 13) % 256) as u8;
                    }
                }
                for _ in 0..10 {
                    black_box(video_image::to_color_image(buffer.as_ref()));
                }
                let mut samples = Vec::with_capacity(count);
                let mut checksum = 0_u64;
                ready.wait();
                start.wait();
                for _ in 0..count {
                    let now = Instant::now();
                    let image = video_image::to_color_image(black_box(buffer.as_ref()));
                    checksum = checksum.wrapping_add(u32::from_le_bytes(
                        image.pixels[count % image.pixels.len()].to_array(),
                    ) as u64);
                    black_box(&image);
                    drop(image);
                    samples.push(now.elapsed().as_nanos() as u64);
                }
                (samples, checksum)
            })
        })
        .collect();
    ready.wait();
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    let allocated_bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
    let now = Instant::now();
    start.wait();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("conversion worker"))
        .collect();
    let elapsed_ms = now.elapsed().as_secs_f64() * 1000.0;
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations;
    let allocated_bytes = ALLOCATED_BYTES.load(Ordering::Relaxed) - allocated_bytes;
    let checksum: u64 = results.iter().map(|(_, sum)| *sum).sum();
    let mut samples: Vec<_> = results
        .into_iter()
        .flat_map(|(samples, _)| samples)
        .collect();
    samples.sort_unstable();
    let percentile = |p: usize| samples[(samples.len() - 1) * p / 100] as f64 / 1_000_000.0;
    println!(
        "{}",
        serde_json::json!({"width":width,"height":height,"workers":workers,"frames":count * workers,
        "wallMs":elapsed_ms,"p50Ms":percentile(50),"p95Ms":percentile(95),"p99Ms":percentile(99),
        "rustAllocations":allocations,"rustAllocatedBytes":allocated_bytes,"checksum":checksum})
    );
    Ok(())
}
