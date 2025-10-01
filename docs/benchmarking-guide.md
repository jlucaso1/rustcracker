# Fair Performance Comparison: Rustcracker vs Hashcat

## Overview

This guide explains how to properly benchmark rustcracker against hashcat to ensure a fair, accurate comparison. Both tools crack MD5 password hashes using wordlists, but they have different optimization strategies and hardware requirements.

## Prerequisites

### System Requirements
- **GPU**: Dedicated or integrated GPU with Vulkan/OpenCL support
- **RAM**: At least 4GB free for large wordlists
- **Storage**: Sufficient space for test wordlists (10M words ≈ 100MB)

### Software Installation

#### Install Rustcracker
```bash
git clone https://github.com/jlucaso1/rustcracker.git
cd rustcracker
cargo build --release
```

#### Install Hashcat
```bash
# Ubuntu/Debian
sudo apt install hashcat

# Arch Linux
sudo pacman -S hashcat

# macOS
brew install hashcat

# Or download from: https://hashcat.net/hashcat/
```

#### Install GPU Drivers

**For NVIDIA GPUs:**
```bash
# Ubuntu/Debian
sudo apt install nvidia-driver-XXX nvidia-cuda-toolkit

# Verify installation
nvidia-smi
```

**For AMD GPUs:**
```bash
# Ubuntu/Debian
sudo apt install rocm-opencl-runtime

# Verify installation
clinfo
```

**For Intel GPUs:**
```bash
# Ubuntu/Debian
sudo apt install intel-opencl-icd

# Or use compute-runtime
# https://github.com/intel/compute-runtime
```

#### Install Wordlist Generator
```bash
sudo apt install pwgen  # For generating test wordlists
```

## Benchmark Setup

### 1. Generate a Test Wordlist

Create a standardized wordlist for consistent testing:

```bash
# 10 million passwords (recommended for comprehensive testing)
pwgen -s 8 10000000 > /tmp/benchmark_wordlist_10m.txt

# 1 million passwords (for quick tests)
pwgen -s 8 1000000 > /tmp/benchmark_wordlist_1m.txt

# 100k passwords (for rapid iteration)
pwgen -s 8 100000 > /tmp/benchmark_wordlist_100k.txt
```

**Why pwgen?**
- Generates random, realistic passwords
- Consistent format across tests
- No duplicates (important for fair comparison)
- Configurable length and count

### 2. Generate Test Hash

Create an MD5 hash that's **NOT** in the wordlist (for full-scan testing):

```bash
# Generate a hash for "testpassword123"
echo -n "testpassword123" | md5sum
# Output: 1f3e7f84e917c603131343f8e5b61510

# Or use a hash that's definitely not in pwgen output
TEST_HASH="1f3e7f84e917c603131343f8e5b61510"
```

**Alternative**: Create a hash from the wordlist for early-exit testing:

```bash
# Get a password from middle of wordlist
PASSWORD=$(sed -n '5000000p' /tmp/benchmark_wordlist_10m.txt)
echo -n "$PASSWORD" | md5sum
```

## Running Benchmarks

### Important: System Preparation

Before running benchmarks, prepare your system for consistent results:

```bash
# 1. Close unnecessary applications (especially GPU-heavy ones)
#    - Web browsers with video/WebGL
#    - Video players
#    - Other GPU-intensive applications

# 2. Ensure AC power (for laptops)
#    Battery mode may throttle performance

# 3. Set CPU governor to performance (Linux)
sudo cpupower frequency-set -g performance

# 4. Clear system caches
sudo sync
sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'

# 5. Let GPU cool down between runs (wait 30-60 seconds)
```

### Benchmarking Rustcracker

```bash
cd /path/to/rustcracker

# Single run with timing
time ./target/release/rustcracker \
  /tmp/benchmark_wordlist_10m.txt \
  1f3e7f84e917c603131343f8e5b61510

# Multiple runs for average (recommended)
for i in {1..5}; do
  echo "Run $i:"
  sleep 30  # Cool down between runs
  time ./target/release/rustcracker \
    /tmp/benchmark_wordlist_10m.txt \
    1f3e7f84e917c603131343f8e5b61510
done
```

**Expected Output:**
```
Loading wordlist from /tmp/benchmark_wordlist_10m.txt...
Loaded 10000000 passwords
Initializing GPU...
Using GPU: Intel(R) Graphics (ADL GT2)
Cracking hash 1f3e7f84e917c603131343f8e5b61510...
✗ Hash not found in wordlist

./target/release/rustcracker /tmp/benchmark_wordlist_10m.txt ...  
0.20s user 0.07s system 2% cpu 9.908 total
```

### Benchmarking Hashcat

#### Option 1: CPU Mode (Most Compatible)

```bash
# Test with CPU only
hashcat -m 0 -D 2 --force -O \
  1f3e7f84e917c603131343f8e5b61510 \
  /tmp/benchmark_wordlist_10m.txt

# With timing
time hashcat -m 0 -D 2 --force -O \
  1f3e7f84e917c603131343f8e5b61510 \
  /tmp/benchmark_wordlist_10m.txt
```

#### Option 2: GPU Mode (If Available)

```bash
# List available devices
hashcat -I

# Test with GPU (device 1, adjust based on -I output)
hashcat -m 0 -D 1 --force -O \
  1f3e7f84e917c603131343f8e5b61510 \
  /tmp/benchmark_wordlist_10m.txt

# With timing
time hashcat -m 0 -D 1 --force -O \
  1f3e7f84e917c603131343f8e5b61510 \
  /tmp/benchmark_wordlist_10m.txt
```

**Hashcat Options Explained:**
- `-m 0`: MD5 hash mode
- `-D 1`: Use OpenCL GPU devices
- `-D 2`: Use CPU devices
- `--force`: Ignore warnings (useful for testing)
- `-O`: Enable optimized kernels (fair comparison with rustcracker optimizations)

**Expected Output:**
```
Session..........: hashcat
Status...........: Exhausted
Hash.Mode........: 0 (MD5)
Hash.Target......: 1f3e7f84e917c603131343f8e5b61510
Time.Started.....: Mon Oct  1 12:00:00 2025
Time.Estimated...: Mon Oct  1 12:00:01 2025
Kernel.Feature...: Optimized Kernel
Guess.Base.......: File (/tmp/benchmark_wordlist_10m.txt)
Speed.#1.........:  11.98 MH/s
Recovered........: 0/1 (0.00%) Digests
Progress.........: 10000000/10000000 (100.00%)
```

## Fair Comparison Criteria

### ✅ Required for Fair Comparison

1. **Same Wordlist**: Both tools must use the exact same wordlist file
2. **Same Hash**: Both tools must crack the same MD5 hash
3. **Full Scan**: Use a hash NOT in the wordlist to measure full performance
4. **No GPU Contention**: Close other GPU-using applications
5. **Stable Power**: Use AC power (laptops), performance CPU governor
6. **Multiple Runs**: Average 3-5 runs to account for variance
7. **Cool Down**: Wait 30-60 seconds between runs for thermal stability

### ✅ Acceptable Variables

1. **Different Devices**: Comparing CPU vs GPU is acceptable (note in results)
2. **Different Batch Sizes**: Each tool uses its optimal batch size
3. **Different APIs**: Rustcracker (Vulkan/wgpu) vs Hashcat (OpenCL/CUDA)

### ❌ Unfair Comparisons to Avoid

1. **Different Wordlists**: Don't compare 1M words vs 10M words
2. **Early Exit**: Don't use a hash at position 100 vs position 9,999,999
3. **Background Load**: Don't run with YouTube/games/video rendering active
4. **Throttled State**: Don't compare with one tool running while GPU is hot
5. **Different Algorithms**: Don't compare MD5 vs SHA-256
6. **Different Optimizations**: Note if comparing `-O` (optimized) vs non-optimized

## Understanding the Results

### Metrics to Record

For each benchmark run, record:

```
Tool: [rustcracker / hashcat]
Device: [GPU model / CPU model]
API: [Vulkan / OpenCL / CUDA / CPU]
Wordlist Size: [10M / 1M / 100k]
Test Hash: [hash value]
Hash Position: [not found / middle / start / end]

Time (total): [seconds]
Throughput: [MH/s]
CPU Time: [user + system seconds]
CPU Usage: [%]

System:
- GPU Driver: [version]
- Temperature: [°C]
- Power Mode: [AC / Battery]
- CPU Governor: [performance / powersave]
```

### Calculating Throughput

```bash
# Throughput = Total passwords / Time in seconds
# Example: 10,000,000 passwords / 9.908 seconds = 1,009,708 H/s = 1.01 MH/s

# From rustcracker timing
PASSWORDS=10000000
TIME=9.908
echo "scale=2; $PASSWORDS / $TIME / 1000000" | bc
# Output: 1.01 MH/s

# Hashcat reports throughput directly
```

### Performance Ratio

```bash
# Calculate how many times faster one tool is
HASHCAT_MHS=11.98
RUSTCRACKER_MHS=1.01
echo "scale=2; $HASHCAT_MHS / $RUSTCRACKER_MHS" | bc
# Output: 11.86x (hashcat is 11.86 times faster)
```

## Common Issues and Solutions

### Issue: Hashcat Skips GPU

```
* Device #1: Skipping hash-mode 0 - not supported by OpenCL driver
```

**Solution**: Use CPU mode instead:
```bash
hashcat -m 0 -D 2 --force -O [hash] [wordlist]
```

### Issue: Rustcracker Can't Find GPU

```
Error: No suitable GPU adapter found
```

**Solution**: Install Vulkan drivers:
```bash
# Ubuntu/Debian
sudo apt install vulkan-tools mesa-vulkan-drivers

# Verify
vulkaninfo
```

### Issue: High Performance Variance

```
Run 1: 9.9s
Run 2: 11.6s
Run 3: 14.7s
```

**Causes and Solutions:**

1. **Thermal Throttling**
   - Wait longer between runs (60+ seconds)
   - Check GPU temperature
   - Improve cooling

2. **Power Management**
   - Ensure AC power for laptops
   - Set CPU governor to performance
   - Disable GPU power management

3. **Background Processes**
   - Close web browsers with video
   - Check `nvidia-smi` or `intel_gpu_top` for other GPU users
   - Run `htop` to check CPU load

4. **Shared Resources (Integrated GPU)**
   - Integrated GPUs share memory bandwidth with CPU
   - Close memory-intensive applications
   - Expect higher variance than dedicated GPUs

### Issue: Out of Memory

```
Error: Failed to allocate GPU buffer
```

**Solution**: Use smaller wordlist or increase GPU memory:
```bash
# Test with smaller wordlist first
pwgen -s 8 1000000 > /tmp/benchmark_wordlist_1m.txt

# Or adjust rustcracker BATCH_SIZE in src/lib.rs (recompile)
```

## Sample Benchmark Results

### Example: Intel i5-12450H with Intel UHD Graphics

**System Configuration:**
- CPU: Intel i5-12450H (12 cores, 4.4GHz boost)
- GPU: Intel UHD Graphics (Integrated, Alder Lake-P GT1)
- RAM: 16GB DDR4
- OS: Ubuntu 22.04
- Drivers: Mesa 23.0, intel-opencl-icd

**Test Setup:**
- Wordlist: 10M passwords (pwgen -s 8)
- Hash: `1f3e7f84e917c603131343f8e5b61510` (not in wordlist)
- Power: AC, performance governor
- Runs: 5 (averaged)

**Results:**

| Tool | Device | API | Time | Throughput | Speed Ratio |
|------|--------|-----|------|------------|-------------|
| **Hashcat** | CPU | Native | 0.8s | 11.98 MH/s | 11.9x (baseline) |
| **Rustcracker** (original) | GPU | Vulkan | 12.83s | 0.78 MH/s | 1.0x |
| **Rustcracker** (optimized) | GPU | Vulkan | 9.91s | 1.01 MH/s | 1.3x |

**Notes:**
- Hashcat GPU mode not available on Intel UHD Graphics
- Rustcracker 22.8% faster after optimizations
- Integrated GPU shows 15-30% performance variance due to thermal/power management
- Hashcat CPU mode outperforms due to mature optimizations

## Benchmark Automation Script

Save this as `benchmark.sh`:

```bash
#!/bin/bash

# Benchmark automation script for rustcracker vs hashcat
# Usage: ./benchmark.sh [wordlist_size]

set -e

WORDLIST_SIZE=${1:-10000000}
WORDLIST="/tmp/benchmark_wordlist_${WORDLIST_SIZE}.txt"
TEST_HASH="1f3e7f84e917c603131343f8e5b61510"
RUNS=5
COOLDOWN=30

echo "==================================="
echo "Rustcracker vs Hashcat Benchmark"
echo "==================================="
echo "Wordlist size: $WORDLIST_SIZE"
echo "Test hash: $TEST_HASH"
echo "Runs: $RUNS"
echo ""

# Generate wordlist if it doesn't exist
if [ ! -f "$WORDLIST" ]; then
    echo "Generating wordlist..."
    pwgen -s 8 "$WORDLIST_SIZE" > "$WORDLIST"
    echo "Wordlist created: $WORDLIST"
fi

# Clear caches
echo "Clearing system caches..."
sudo sync
sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'

echo ""
echo "=== Benchmarking Rustcracker ==="
echo ""

RUSTCRACKER_TIMES=()
for i in $(seq 1 $RUNS); do
    echo "Run $i/$RUNS..."
    sleep "$COOLDOWN"
    
    # Run and capture time
    TIME_OUTPUT=$( { time ./target/release/rustcracker "$WORDLIST" "$TEST_HASH" 2>&1 >/dev/null; } 2>&1 )
    TOTAL_TIME=$(echo "$TIME_OUTPUT" | grep "cpu" | awk '{print $7}')
    RUSTCRACKER_TIMES+=("$TOTAL_TIME")
    echo "  Time: ${TOTAL_TIME}s"
done

echo ""
echo "=== Benchmarking Hashcat (CPU) ==="
echo ""

HASHCAT_TIMES=()
for i in $(seq 1 $RUNS); do
    echo "Run $i/$RUNS..."
    sleep "$COOLDOWN"
    
    # Run hashcat and parse output
    HASHCAT_OUTPUT=$(hashcat -m 0 -D 2 --force -O --quiet "$TEST_HASH" "$WORDLIST" 2>&1 || true)
    HASHCAT_SPEED=$(echo "$HASHCAT_OUTPUT" | grep "Speed" | awk '{print $2}')
    echo "  Speed: ${HASHCAT_SPEED}"
    HASHCAT_TIMES+=("$HASHCAT_SPEED")
done

echo ""
echo "==================================="
echo "Results Summary"
echo "==================================="
echo ""
echo "Rustcracker times: ${RUSTCRACKER_TIMES[@]}"
echo "Hashcat speeds: ${HASHCAT_TIMES[@]}"
echo ""
echo "Calculate averages manually or save to file for analysis."
```

Make it executable:
```bash
chmod +x benchmark.sh
```

Run it:
```bash
./benchmark.sh 10000000  # 10M wordlist
./benchmark.sh 1000000   # 1M wordlist
```

## Interpretation Guidelines

### What Success Looks Like

Rustcracker is **successful** if it:

✅ Completes the benchmark without errors  
✅ Produces correct results (finds/doesn't find hash consistently)  
✅ Shows reasonable performance for a safe Rust implementation  
✅ Demonstrates improvement from optimizations  
✅ Works across different GPUs/platforms

Rustcracker is **NOT expected** to:

❌ Match hashcat's raw speed (hashcat has 10+ years of optimization)  
❌ Beat specialized tools on their home turf  
❌ Win on absolute performance metrics

### Understanding the Gap

The performance difference exists because:

1. **Maturity**: Hashcat has 10+ years of hand-tuned optimization
2. **Specialization**: Hand-written assembly for specific GPU architectures
3. **Abstraction**: wgpu/Vulkan adds portability overhead
4. **Memory Safety**: Rust's safety guarantees have some runtime cost
5. **Toolchain**: rust-gpu is newer than mature CUDA/OpenCL compilers

### Value Proposition

Rustcracker's value is in:

- **100% Safe Rust**: Memory safety guarantees
- **Cross-Platform**: Works on Vulkan, Metal, DX12
- **Maintainability**: Clean, modern codebase
- **Educational**: Demonstrates GPGPU in pure Rust
- **Practical**: Still achieves meaningful performance (1+ MH/s)

## Reporting Results

When sharing benchmark results, include:

```markdown
## Benchmark Results

**Hardware:**
- CPU: [model]
- GPU: [model]
- RAM: [amount]

**Software:**
- OS: [version]
- Rustcracker: [commit/version]
- Hashcat: [version]
- GPU Driver: [version]

**Test Configuration:**
- Wordlist: [size] passwords ([source])
- Hash: [MD5 hash] (position: [not found / middle / etc.])
- Runs: [number] (averaged)
- Cooldown: [seconds] between runs

**Results:**

| Tool | Device | Time | Throughput | Notes |
|------|--------|------|------------|-------|
| Rustcracker | [GPU model] | [avg]s | [avg] MH/s | [any notes] |
| Hashcat | [GPU/CPU] | [avg]s | [avg] MH/s | [any notes] |

**Performance Ratio:** [X]x faster (hashcat)

**System State:**
- Power: [AC / Battery]
- CPU Governor: [performance / powersave]
- GPU Temperature: [°C]
- Background Load: [none / minimal / etc.]

**Observations:**
[Any notable patterns, variance, thermal throttling, etc.]
```

## Conclusion

Fair benchmarking requires:
1. Identical test conditions (wordlist, hash, system state)
2. Multiple runs with proper cooldown
3. Understanding of hardware capabilities and limitations
4. Recognition that different tools optimize for different goals

Rustcracker prioritizes **safety, portability, and maintainability** while still achieving practical performance. Hashcat prioritizes **raw speed** through specialized optimization. Both are valid approaches with different trade-offs.

---

**Questions?** Open an issue on the rustcracker GitHub repository.
