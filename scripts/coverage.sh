#!/bin/bash

# BitCraps Coverage Analysis Script
# Generates comprehensive test coverage reports using tarpaulin

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
COVERAGE_THRESHOLD=70
OUTPUT_DIR="target/tarpaulin"
HTML_DIR="$OUTPUT_DIR/html"
REPORTS_DIR="reports"

echo -e "${BLUE}🔍 BitCraps Test Coverage Analysis${NC}"
echo "======================================"

# Check if tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}⚠️  Installing cargo-tarpaulin...${NC}"
    cargo install cargo-tarpaulin
fi

# Create output directories
mkdir -p "$OUTPUT_DIR"
mkdir -p "$HTML_DIR"
mkdir -p "$REPORTS_DIR"

# Clean previous coverage data
echo -e "${BLUE}🧹 Cleaning previous coverage data...${NC}"
cargo clean
rm -rf "$OUTPUT_DIR"/*
rm -rf "$REPORTS_DIR"/coverage_*

# Function to run coverage for specific test suite
run_coverage() {
    local test_name=$1
    local test_pattern=$2
    local description=$3
    
    echo -e "${BLUE}🧪 Running coverage for $description...${NC}"
    
    cargo tarpaulin \
        --config tarpaulin.toml \
        --out Html,Lcov,Json \
        --output-dir "$OUTPUT_DIR/$test_name" \
        --test "$test_pattern" \
        --verbose \
        2>&1 | tee "$REPORTS_DIR/coverage_${test_name}.log"
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ Coverage analysis completed for $description${NC}"
    else
        echo -e "${RED}❌ Coverage analysis failed for $description${NC}"
        return 1
    fi
}

# Function to run comprehensive coverage
run_comprehensive_coverage() {
    echo -e "${BLUE}🔬 Running comprehensive coverage analysis...${NC}"
    
    cargo tarpaulin \
        --config tarpaulin.toml \
        --out Html,Lcov,Json,Stdout \
        --output-dir "$OUTPUT_DIR" \
        --all-features \
        --workspace \
        --timeout 300 \
        --verbose \
        2>&1 | tee "$REPORTS_DIR/coverage_comprehensive.log"
    
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo -e "${GREEN}✅ Comprehensive coverage analysis completed${NC}"
    else
        echo -e "${RED}❌ Comprehensive coverage analysis failed${NC}"
        return 1
    fi
    
    return $exit_code
}

# Function to analyze coverage results
analyze_coverage() {
    local coverage_file="$OUTPUT_DIR/tarpaulin-report.json"
    
    if [ ! -f "$coverage_file" ]; then
        echo -e "${RED}❌ Coverage report not found: $coverage_file${NC}"
        return 1
    fi
    
    echo -e "${BLUE}📊 Analyzing coverage results...${NC}"
    
    # Extract coverage percentage using jq if available
    if command -v jq &> /dev/null; then
        local coverage_percent=$(jq -r '.coverage' "$coverage_file" 2>/dev/null || echo "0")
        coverage_percent=${coverage_percent%.*} # Remove decimal part
        
        echo "Overall Coverage: ${coverage_percent}%"
        
        if [ "$coverage_percent" -ge "$COVERAGE_THRESHOLD" ]; then
            echo -e "${GREEN}✅ Coverage meets threshold (${COVERAGE_THRESHOLD}%)${NC}"
            return 0
        else
            echo -e "${RED}❌ Coverage below threshold (${COVERAGE_THRESHOLD}%)${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}⚠️  jq not found, skipping detailed analysis${NC}"
        echo "Install jq for detailed coverage analysis: sudo apt-get install jq"
    fi
}

# Function to generate coverage summary report
generate_summary() {
    local summary_file="$REPORTS_DIR/coverage_summary.md"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    cat > "$summary_file" << EOF
# BitCraps Coverage Report

**Generated:** $timestamp  
**Threshold:** ${COVERAGE_THRESHOLD}%  

## Summary

EOF

    # Add coverage details if JSON report exists
    if [ -f "$OUTPUT_DIR/tarpaulin-report.json" ] && command -v jq &> /dev/null; then
        local coverage_percent=$(jq -r '.coverage' "$OUTPUT_DIR/tarpaulin-report.json" 2>/dev/null || echo "N/A")
        local lines_covered=$(jq -r '.covered' "$OUTPUT_DIR/tarpaulin-report.json" 2>/dev/null || echo "N/A")
        local lines_total=$(jq -r '.coverable' "$OUTPUT_DIR/tarpaulin-report.json" 2>/dev/null || echo "N/A")
        
        cat >> "$summary_file" << EOF
- **Overall Coverage:** ${coverage_percent}%
- **Lines Covered:** ${lines_covered}
- **Total Lines:** ${lines_total}

## Module Coverage

EOF

        # Add module-specific coverage if available
        jq -r '.files[] | "\(.name): \(.coverage)%"' "$OUTPUT_DIR/tarpaulin-report.json" 2>/dev/null | \
        head -20 | \
        sed 's/^/- /' >> "$summary_file" || true
        
        cat >> "$summary_file" << EOF

## Files

For detailed coverage information, see the [HTML report](../target/tarpaulin/html/index.html).

## Recommendations

EOF

        # Add recommendations based on coverage
        local coverage_num=${coverage_percent%.*}
        if [ "$coverage_num" -lt "$COVERAGE_THRESHOLD" ]; then
            cat >> "$summary_file" << EOF
- ❌ **Action Required:** Coverage is below threshold (${COVERAGE_THRESHOLD}%)
- 🎯 **Focus Areas:** Review modules with low coverage
- 📝 **Next Steps:** Add unit tests for uncovered code paths
EOF
        else
            cat >> "$summary_file" << EOF
- ✅ **Status:** Coverage meets quality standards
- 🚀 **Next Steps:** Consider increasing threshold for critical modules
- 🔄 **Maintenance:** Monitor coverage in CI/CD pipeline
EOF
        fi
    fi
    
    echo -e "${GREEN}📋 Coverage summary generated: $summary_file${NC}"
}

# Main execution
main() {
    local run_mode=${1:-"comprehensive"}
    
    case "$run_mode" in
        "comprehensive"|"all")
            echo -e "${BLUE}🎯 Running comprehensive coverage analysis...${NC}"
            
            if run_comprehensive_coverage; then
                analyze_coverage
                generate_summary
                
                echo -e "${GREEN}🎉 Coverage analysis completed successfully!${NC}"
                echo -e "📁 Reports available in: $OUTPUT_DIR"
                echo -e "📊 HTML report: $HTML_DIR/index.html"
                
            else
                echo -e "${RED}💥 Coverage analysis failed!${NC}"
                echo -e "📋 Check logs in: $REPORTS_DIR/"
                exit 1
            fi
            ;;
            
        "unit")
            echo -e "${BLUE}🧪 Running unit test coverage...${NC}"
            run_coverage "unit" "unit_tests" "Unit Tests"
            ;;
            
        "integration")
            echo -e "${BLUE}🔗 Running integration test coverage...${NC}"
            run_coverage "integration" "integration" "Integration Tests"
            ;;
            
        "gaming")
            echo -e "${BLUE}🎲 Running gaming module coverage...${NC}"
            run_coverage "gaming" "gaming" "Gaming Modules"
            ;;
            
        "transport")
            echo -e "${BLUE}📡 Running transport module coverage...${NC}"
            run_coverage "transport" "transport" "Transport Modules"
            ;;
            
        "help"|"-h"|"--help")
            echo "Usage: $0 [MODE]"
            echo ""
            echo "Modes:"
            echo "  comprehensive  Run complete coverage analysis (default)"
            echo "  unit          Run unit test coverage only"
            echo "  integration   Run integration test coverage only"
            echo "  gaming        Run gaming module coverage only"
            echo "  transport     Run transport module coverage only"
            echo "  help          Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                    # Run comprehensive analysis"
            echo "  $0 comprehensive      # Same as above"
            echo "  $0 unit              # Run unit tests only"
            echo "  $0 gaming            # Focus on gaming modules"
            ;;
            
        *)
            echo -e "${RED}❌ Unknown mode: $run_mode${NC}"
            echo "Use '$0 help' for usage information"
            exit 1
            ;;
    esac
}

# Trap to clean up on interrupt
trap 'echo -e "\n${YELLOW}⚠️  Coverage analysis interrupted${NC}"; exit 130' INT

# Run main function with all arguments
main "$@"