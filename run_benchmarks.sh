#!/bin/bash
# RustCracker Benchmark Runner
# This script helps you run benchmarks with optimal settings for reproducible results

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}======================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}======================================${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

# Check environment
check_environment() {
    print_header "Checking Benchmark Environment"
    
    # Check if on AC power (for laptops)
    if command -v upower &> /dev/null; then
        if upower -i /org/freedesktop/UPower/devices/battery_BAT0 2>/dev/null | grep -q "state.*discharging"; then
            print_warning "Running on battery power. For best results, plug into AC power."
        else
            print_success "Running on AC power"
        fi
    fi
    
    # Check CPU governor
    if [ -f /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor ]; then
        governor=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor)
        if [ "$governor" != "performance" ]; then
            print_warning "CPU governor is set to '$governor'. Consider setting to 'performance':"
            echo "  sudo cpupower frequency-set -g performance"
        else
            print_success "CPU governor set to performance"
        fi
    fi
    
    # Check for GPU
    if command -v vulkaninfo &> /dev/null; then
        print_success "Vulkan support detected"
        gpu_name=$(vulkaninfo --summary 2>/dev/null | grep "GPU id" | head -1 | cut -d':' -f2 | xargs)
        if [ -n "$gpu_name" ]; then
            echo "  GPU: $gpu_name"
        fi
    else
        print_warning "vulkaninfo not found. Install vulkan-tools to verify GPU support"
    fi
    
    echo ""
}

# Display usage
usage() {
    cat << EOF
RustCracker Benchmark Runner

Usage: $0 [OPTIONS] [BENCHMARK_NAME]

OPTIONS:
    -a, --all           Run all benchmarks
    -q, --quick         Run quick benchmarks (fewer samples)
    -t, --timing        Run GPU timing benchmarks only
    -f, --full          Run full comprehensive benchmarks
    -h, --help          Show this help message
    --baseline NAME     Save results as baseline for comparison
    --compare NAME      Compare current results with saved baseline
    --check-env         Only check environment and exit

BENCHMARK_NAME:
    If specified, runs only the named benchmark group (e.g., "File I/O", "GPU Throughput")

Examples:
    $0 --all                          # Run all benchmarks
    $0 --timing                       # Run only GPU timing benchmarks
    $0 --quick --all                  # Quick run of all benchmarks
    $0 --baseline v1.0                # Run benchmarks and save as v1.0 baseline
    $0 --compare v1.0                 # Compare with v1.0 baseline
    $0 "GPU Throughput"               # Run only GPU Throughput benchmarks
    $0 --check-env                    # Check if environment is optimal

EOF
}

# Run specific benchmark
run_benchmark() {
    local bench_name=$1
    local extra_args=$2
    
    print_header "Running: $bench_name"
    cargo bench --bench "$bench_name" -- $extra_args
    print_success "Completed: $bench_name"
    echo ""
}

# Main script
main() {
    local run_all=false
    local run_timing=false
    local run_full=false
    local quick_mode=""
    local baseline=""
    local compare=""
    local check_env_only=false
    local specific_benchmark=""
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -a|--all)
                run_all=true
                shift
                ;;
            -q|--quick)
                quick_mode="--quick"
                shift
                ;;
            -t|--timing)
                run_timing=true
                shift
                ;;
            -f|--full)
                run_full=true
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            --baseline)
                baseline="$2"
                shift 2
                ;;
            --compare)
                compare="$2"
                shift 2
                ;;
            --check-env)
                check_env_only=true
                shift
                ;;
            *)
                specific_benchmark="$1"
                shift
                ;;
        esac
    done
    
    # Check environment
    check_environment
    
    if [ "$check_env_only" = true ]; then
        exit 0
    fi
    
    # Build extra arguments
    local extra_args=""
    if [ -n "$baseline" ]; then
        extra_args="--save-baseline $baseline"
        print_header "Saving results as baseline: $baseline"
    elif [ -n "$compare" ]; then
        extra_args="--baseline $compare"
        print_header "Comparing with baseline: $compare"
    fi
    
    if [ -n "$quick_mode" ]; then
        print_warning "Running in quick mode (fewer samples)"
    fi
    
    # Determine what to run
    if [ "$run_all" = true ] || [ "$run_full" = true ]; then
        print_header "Running Complete Benchmark Suite"
        run_benchmark "cracker_benchmark" "$extra_args"
        run_benchmark "gpu_timing_benchmark" "$extra_args"
        
        print_success "All benchmarks completed!"
        echo ""
        echo "Results saved to: target/criterion/"
        echo "Open target/criterion/report/index.html in a browser to view detailed results"
        
    elif [ "$run_timing" = true ]; then
        run_benchmark "gpu_timing_benchmark" "$extra_args"
        
    elif [ -n "$specific_benchmark" ]; then
        print_header "Running specific benchmark group: $specific_benchmark"
        cargo bench --bench cracker_benchmark -- "$specific_benchmark" $extra_args
        
    else
        print_error "No benchmark specified. Use --all, --timing, or specify a benchmark name."
        echo ""
        usage
        exit 1
    fi
    
    # Show results location
    echo ""
    print_header "Benchmark Complete"
    echo "View detailed HTML reports:"
    echo "  file://$(pwd)/target/criterion/report/index.html"
    echo ""
    echo "To compare with a baseline in the future, run:"
    echo "  $0 --compare $(date +%Y%m%d)"
}

# Run main
main "$@"
