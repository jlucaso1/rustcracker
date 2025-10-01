use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rustcracker::{GpuCracker, BATCH_SIZE};
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

mod benchmark_utils;
use benchmark_utils::*;

/// Benchmark: File I/O and wordlist loading
fn bench_wordlist_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("File I/O");

    // Test with different wordlist sizes
    for size in [1_000, 10_000, 100_000, 1_000_000].iter() {
        let wordlist = generate_wordlist(*size, "test");

        // Create a temporary file
        let mut temp_file = NamedTempFile::new().unwrap();
        for word in &wordlist {
            writeln!(temp_file, "{word}").unwrap();
        }
        temp_file.flush().unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{size}_words")),
            size,
            |b, _| {
                b.iter(|| {
                    let content = fs::read_to_string(black_box(&path)).unwrap();
                    let wordlist: Vec<&str> = content.lines().collect();
                    black_box(wordlist.len())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Wordlist preprocessing (converting to appropriate format)
fn bench_wordlist_preprocessing(c: &mut Criterion) {
    let mut group = c.benchmark_group("Wordlist Preprocessing");

    for size in [1_000, 10_000, 100_000].iter() {
        let wordlist = generate_wordlist(*size, "prep");
        let wordlist_content = wordlist.join("\n");

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{size}_words")),
            size,
            |b, _| {
                b.iter(|| {
                    let lines: Vec<&str> = black_box(&wordlist_content).lines().collect();

                    // Simulate the preprocessing that happens before GPU submission
                    let mut total_bytes = 0usize;
                    for line in &lines {
                        total_bytes += line.len();
                    }
                    black_box(total_bytes)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Batch preparation overhead
fn bench_batch_preparation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Batch Preparation");

    let wordlist = generate_wordlist(BATCH_SIZE, "batch");
    let wordlist_refs: Vec<&str> = wordlist.iter().map(|s| s.as_str()).collect();

    group.bench_function("prepare_single_batch", |b| {
        b.iter(|| {
            // Simulate the data preparation that happens in process_batch
            let mut message_data_bytes = Vec::new();
            let mut message_lengths = Vec::new();
            let mut message_offsets = Vec::new();
            let mut current_offset = 0u32;

            for msg in black_box(&wordlist_refs) {
                let msg_bytes = msg.as_bytes();
                message_offsets.push(current_offset);
                message_lengths.push(msg_bytes.len() as u32);
                message_data_bytes.extend_from_slice(msg_bytes);
                current_offset += msg_bytes.len() as u32;
            }

            // Convert to u32 array
            let mut message_data_u32 = Vec::new();
            for chunk in message_data_bytes.chunks(4) {
                let mut word = 0u32;
                for (i, &byte) in chunk.iter().enumerate() {
                    word |= (byte as u32) << (i * 8);
                }
                message_data_u32.push(word);
            }

            black_box((message_data_u32, message_lengths, message_offsets))
        })
    });

    group.finish();
}

/// Benchmark: GPU cracking throughput (end-to-end with small batches)
fn bench_gpu_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("GPU Throughput");
    group.sample_size(10); // Reduce sample size for GPU benchmarks (they're slower)

    // Initialize GPU cracker once
    let cracker = pollster::block_on(GpuCracker::new()).expect("Failed to initialize GPU");

    // Generate a target hash (for "password123")
    let target_password = "password123";
    let target_hash = md5_hash(target_password);

    // Test with different batch sizes
    for batch_size in [512, 1024, 2048, 4096].iter() {
        let wordlist = generate_wordlist(*batch_size, "gpu");
        // Put target at the end to force full batch processing
        let mut wordlist_with_target = wordlist.clone();
        wordlist_with_target[batch_size - 1] = target_password.to_string();
        let wordlist_refs: Vec<&str> = wordlist_with_target.iter().map(|s| s.as_str()).collect();

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{batch_size}_hashes")),
            batch_size,
            |b, _| {
                b.iter(|| cracker.process_batch(black_box(&wordlist_refs), black_box(&target_hash)))
            },
        );
    }

    group.finish();
}

/// Benchmark: End-to-end cracking scenarios
fn bench_end_to_end_cracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("End-to-End Cracking");
    group.sample_size(10); // GPU benchmarks are slower

    let cracker = pollster::block_on(GpuCracker::new()).expect("Failed to initialize GPU");

    // Scenario 1: Password at the beginning
    let password_start = "target_password_start";
    let hash_start = md5_hash(password_start);
    let wordlist_start = generate_wordlist_with_target(50_000, password_start, 100);
    let wordlist_start_refs: Vec<&str> = wordlist_start.iter().map(|s| s.as_str()).collect();

    group.bench_function("password_at_start_50k", |b| {
        b.iter(|| cracker.crack(black_box(&hash_start), black_box(&wordlist_start_refs)))
    });

    // Scenario 2: Password in the middle
    let password_middle = "target_password_middle";
    let hash_middle = md5_hash(password_middle);
    let wordlist_middle = generate_wordlist_with_target(50_000, password_middle, 25_000);
    let wordlist_middle_refs: Vec<&str> = wordlist_middle.iter().map(|s| s.as_str()).collect();

    group.bench_function("password_in_middle_50k", |b| {
        b.iter(|| cracker.crack(black_box(&hash_middle), black_box(&wordlist_middle_refs)))
    });

    // Scenario 3: Password at the end
    let password_end = "target_password_end";
    let hash_end = md5_hash(password_end);
    let wordlist_end = generate_wordlist_with_target(50_000, password_end, 49_999);
    let wordlist_end_refs: Vec<&str> = wordlist_end.iter().map(|s| s.as_str()).collect();

    group.bench_function("password_at_end_50k", |b| {
        b.iter(|| cracker.crack(black_box(&hash_end), black_box(&wordlist_end_refs)))
    });

    // Scenario 4: Password not found (worst case)
    let hash_not_found = md5_hash("this_password_does_not_exist_in_wordlist");
    let wordlist_not_found = generate_wordlist(10_000, "notfound");
    let wordlist_not_found_refs: Vec<&str> =
        wordlist_not_found.iter().map(|s| s.as_str()).collect();

    group.bench_function("password_not_found_10k", |b| {
        b.iter(|| {
            cracker.crack(
                black_box(&hash_not_found),
                black_box(&wordlist_not_found_refs),
            )
        })
    });

    group.finish();
}

/// Benchmark: Variable password length impact
fn bench_variable_password_lengths(c: &mut Criterion) {
    let mut group = c.benchmark_group("Variable Password Lengths");
    group.sample_size(10);

    let cracker = pollster::block_on(GpuCracker::new()).expect("Failed to initialize GPU");

    // Test with uniform short passwords
    let short_wordlist: Vec<String> = (0..BATCH_SIZE).map(|i| format!("pwd{i}")).collect();
    let short_refs: Vec<&str> = short_wordlist.iter().map(|s| s.as_str()).collect();
    let target_hash = md5_hash("pwd999");

    group.bench_function("short_passwords_4-7_chars", |b| {
        b.iter(|| cracker.process_batch(black_box(&short_refs), black_box(&target_hash)))
    });

    // Test with uniform long passwords
    let long_wordlist: Vec<String> = (0..BATCH_SIZE)
        .map(|i| format!("this_is_a_very_long_password_number_{i}_for_testing"))
        .collect();
    let long_refs: Vec<&str> = long_wordlist.iter().map(|s| s.as_str()).collect();

    group.bench_function("long_passwords_40-50_chars", |b| {
        b.iter(|| cracker.process_batch(black_box(&long_refs), black_box(&target_hash)))
    });

    // Test with varied lengths
    let varied_wordlist = generate_varied_length_wordlist(BATCH_SIZE);
    let varied_refs: Vec<&str> = varied_wordlist.iter().map(|s| s.as_str()).collect();

    group.bench_function("varied_passwords_4-64_chars", |b| {
        b.iter(|| cracker.process_batch(black_box(&varied_refs), black_box(&target_hash)))
    });

    group.finish();
}

// Configure benchmark groups
criterion_group!(
    benches,
    bench_wordlist_loading,
    bench_wordlist_preprocessing,
    bench_batch_preparation,
    bench_gpu_throughput,
    bench_end_to_end_cracking,
    bench_variable_password_lengths,
);

criterion_main!(benches);
