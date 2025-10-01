#!/bin/bash
# Quick Start Guide for RustCracker Benchmarks
# Run this script to see a quick demo of the benchmark capabilities

echo "╔════════════════════════════════════════════════════════╗"
echo "║  RustCracker Benchmark Suite - Quick Start Guide      ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""

# Function to print colored text
print_step() {
    echo -e "\033[1;34m▶ $1\033[0m"
}

print_info() {
    echo -e "\033[0;36m  $1\033[0m"
}

print_success() {
    echo -e "\033[0;32m  ✓ $1\033[0m"
}

print_step "1. Environment Check"
print_info "First, let's verify your environment is optimized for benchmarking..."
echo ""
./run_benchmarks.sh --check-env
echo ""

read -p "Press Enter to continue..."
echo ""

print_step "2. Available Benchmark Modes"
echo ""
print_info "You can run benchmarks in several ways:"
echo ""
echo "   ./run_benchmarks.sh --all             # Full comprehensive suite"
echo "   ./run_benchmarks.sh --quick --all     # Quick run (fewer samples)"
echo "   ./run_benchmarks.sh --timing          # GPU timing only"
echo "   ./run_benchmarks.sh --check-env       # Environment check only"
echo ""

print_step "3. What Gets Benchmarked"
echo ""
print_info "The suite includes:"
echo "   • File I/O Performance - Wordlist loading (1K to 1M words)"
echo "   • Preprocessing - Data preparation overhead"
echo "   • Batch Preparation - Message encoding"
echo "   • GPU Throughput - Raw hashing performance"
echo "   • End-to-End - Complete cracking scenarios"
echo "   • Variable Lengths - Password length impact"
echo "   • Pure GPU Timing - GPU-only execution (if supported)"
echo ""

print_step "4. Understanding Results"
echo ""
print_info "After benchmarking, you'll get:"
echo "   • HTML report at: target/criterion/report/index.html"
echo "   • Terminal output with timing statistics"
echo "   • Performance comparisons with previous runs"
echo "   • GPU throughput in MH/s (mega hashes/second)"
echo ""

print_step "5. Baseline Comparison"
echo ""
print_info "Track performance over time:"
echo "   1. Save baseline:  ./run_benchmarks.sh --all --baseline v1.0"
echo "   2. Make changes:   (edit code, optimize, etc.)"
echo "   3. Compare:        ./run_benchmarks.sh --all --compare v1.0"
echo ""

print_step "6. Quick Start Commands"
echo ""
echo "   # Option A: Quick benchmark (recommended for first run)"
echo "   ./run_benchmarks.sh --quick --all"
echo ""
echo "   # Option B: Full benchmark (takes longer, more accurate)"
echo "   ./run_benchmarks.sh --all"
echo ""
echo "   # Option C: GPU timing only (fast, GPU-focused)"
echo "   ./run_benchmarks.sh --timing"
echo ""

print_step "7. Viewing Results"
echo ""
print_info "After benchmarking completes:"
echo "   firefox target/criterion/report/index.html"
echo "   # or"
echo "   xdg-open target/criterion/report/index.html"
echo ""

echo "╔════════════════════════════════════════════════════════╗"
echo "║  Ready to Benchmark!                                   ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""

# Ask if user wants to run benchmarks now
read -p "Would you like to run a quick benchmark now? (y/N) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo ""
    print_success "Starting quick benchmark..."
    echo ""
    ./run_benchmarks.sh --quick --all
    
    echo ""
    echo "╔════════════════════════════════════════════════════════╗"
    echo "║  Benchmark Complete!                                   ║"
    echo "╚════════════════════════════════════════════════════════╝"
    echo ""
    print_info "View detailed results:"
    echo "   firefox target/criterion/report/index.html"
    echo ""
else
    echo ""
    print_info "No problem! When you're ready, run:"
    echo "   ./run_benchmarks.sh --quick --all"
    echo ""
    print_info "For more details, see:"
    echo "   • BENCHMARK_SUMMARY.md - Quick reference"
    echo "   • benches/README.md - Comprehensive guide"
    echo "   • README.md - Main documentation"
    echo ""
fi

print_success "For questions or issues, see benches/README.md"
echo ""
