use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustcracker::{GpuCracker, BATCH_SIZE};

mod benchmark_utils;
use benchmark_utils::*;

/// Benchmark: Pure GPU execution time using timestamp queries
fn bench_pure_gpu_timing(c: &mut Criterion) {
    let mut group = c.benchmark_group("Pure GPU Timing");
    group.sample_size(20);

    let cracker = pollster::block_on(GpuCracker::new()).expect("Failed to initialize GPU");

    if !cracker.supports_timestamps() {
        println!(
            "Warning: GPU timestamp queries not supported, skipping pure GPU timing benchmark"
        );
        return;
    }

    let target_hash = md5_hash("benchmark_target");

    // Benchmark with full batch
    let wordlist = generate_wordlist(BATCH_SIZE, "timing");
    let wordlist_refs: Vec<&str> = wordlist.iter().map(|s| s.as_str()).collect();

    group.bench_function("full_batch_gpu_only", |b| {
        b.iter(|| {
            let (_result, gpu_time) = cracker
                .process_batch_with_timing(black_box(&wordlist_refs), black_box(&target_hash));

            if let Some(time_ns) = gpu_time {
                // Calculate hashes per second
                let time_s = time_ns as f64 / 1_000_000_000.0;
                let hashes_per_sec = BATCH_SIZE as f64 / time_s;

                // This is for informational purposes in the benchmark output
                black_box(hashes_per_sec);
            }
        })
    });

    group.finish();
}

/// Custom benchmark runner that reports GPU hashing rate
#[allow(dead_code)]
fn bench_gpu_hashing_rate(_c: &mut Criterion) {
    let cracker = pollster::block_on(GpuCracker::new()).expect("Failed to initialize GPU");

    if !cracker.supports_timestamps() {
        println!("Warning: GPU timestamp queries not supported");
        println!("To enable accurate GPU benchmarking, ensure your GPU supports timestamp queries");
        return;
    }

    let target_hash = md5_hash("rate_test");
    let wordlist = generate_wordlist(BATCH_SIZE, "rate");
    let wordlist_refs: Vec<&str> = wordlist.iter().map(|s| s.as_str()).collect();

    // Warm-up run
    for _ in 0..3 {
        cracker.process_batch_with_timing(&wordlist_refs, &target_hash);
    }

    // Measure multiple runs
    let mut total_time_ns = 0u64;
    let num_runs = 10;

    for _ in 0..num_runs {
        let (_result, gpu_time) = cracker.process_batch_with_timing(&wordlist_refs, &target_hash);
        if let Some(time_ns) = gpu_time {
            total_time_ns += time_ns;
        }
    }

    let avg_time_ns = total_time_ns / num_runs;
    let avg_time_s = avg_time_ns as f64 / 1_000_000_000.0;
    let hashes_per_sec = BATCH_SIZE as f64 / avg_time_s;
    let mega_hashes_per_sec = hashes_per_sec / 1_000_000.0;

    println!("\n========================================");
    println!("GPU MD5 Hashing Performance");
    println!("========================================");
    println!("Batch size:       {BATCH_SIZE} hashes");
    println!(
        "Average GPU time: {:.3} ms",
        avg_time_ns as f64 / 1_000_000.0
    );
    println!("Throughput:       {mega_hashes_per_sec:.2} MH/s (million hashes/sec)");
    println!(
        "Throughput:       {:.2} GH/s (billion hashes/sec)",
        mega_hashes_per_sec / 1000.0
    );
    println!("========================================\n");
}

criterion_group!(gpu_timing_benches, bench_pure_gpu_timing,);

criterion_main!(gpu_timing_benches);
