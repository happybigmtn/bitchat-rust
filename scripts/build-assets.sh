#!/bin/bash
# Asset Optimization Pipeline for BitCraps CDN
# Optimizes and builds all assets for global CDN distribution

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ASSETS_DIR="$PROJECT_ROOT/assets"
BUILD_DIR="$PROJECT_ROOT/dist"
CDN_DIR="$BUILD_DIR/cdn"
WASM_DIR="$CDN_DIR/wasm"
STATIC_DIR="$CDN_DIR/static"
TEMP_DIR="$(mktemp -d)"

# Build configuration
export NODE_ENV="${NODE_ENV:-production}"
export RUST_LOG="${RUST_LOG:-warn}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[WARNING] $1${NC}"
}

error() {
    echo -e "${RED}[ERROR] $1${NC}" >&2
}

success() {
    echo -e "${GREEN}[SUCCESS] $1${NC}"
}

# Cleanup function
cleanup() {
    log "Cleaning up temporary files..."
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# Check dependencies
check_dependencies() {
    log "Checking build dependencies..."
    
    local missing_deps=()
    
    # Check for required tools
    command -v wasm-pack >/dev/null || missing_deps+=("wasm-pack")
    command -v cargo >/dev/null || missing_deps+=("cargo")
    command -v node >/dev/null || missing_deps+=("node")
    command -v npm >/dev/null || missing_deps+=("npm")
    command -v convert >/dev/null || missing_deps+=("imagemagick")
    command -v optipng >/dev/null || missing_deps+=("optipng")
    command -v jpegoptim >/dev/null || missing_deps+=("jpegoptim")
    command -v brotli >/dev/null || missing_deps+=("brotli")
    
    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        error "Missing required dependencies: ${missing_deps[*]}"
        error "Please install the missing dependencies and try again."
        exit 1
    fi
    
    success "All dependencies are available"
}

# Setup build directories
setup_directories() {
    log "Setting up build directories..."
    
    # Remove existing build directory
    rm -rf "$BUILD_DIR"
    
    # Create directory structure
    mkdir -p "$CDN_DIR"/{wasm,static/{css,js,images,fonts},api}
    mkdir -p "$TEMP_DIR"/{wasm,assets,compressed}
    
    success "Build directories created"
}

# Build WASM packages
build_wasm() {
    log "Building WebAssembly packages..."
    
    cd "$PROJECT_ROOT"
    
    # Build main WASM package with optimizations
    log "Building main BitCraps WASM package..."
    wasm-pack build --target web --out-dir "$TEMP_DIR/wasm/bitcraps" \
        --release --scope bitcraps -- --features wasm
    
    # Build crypto WASM package separately for better caching
    if [[ -d "$PROJECT_ROOT/crates/crypto" ]]; then
        log "Building crypto WASM package..."
        cd "$PROJECT_ROOT/crates/crypto"
        wasm-pack build --target web --out-dir "$TEMP_DIR/wasm/crypto" \
            --release --scope bitcraps-crypto
        cd "$PROJECT_ROOT"
    fi
    
    # Optimize WASM files
    log "Optimizing WASM files..."
    for wasm_file in $(find "$TEMP_DIR/wasm" -name "*.wasm"); do
        local basename=$(basename "$wasm_file" .wasm)
        local dirname=$(dirname "$wasm_file")
        
        # Use wasm-opt if available for further optimization
        if command -v wasm-opt >/dev/null; then
            log "Optimizing $basename.wasm with wasm-opt..."
            wasm-opt -Oz --enable-simd --enable-bulk-memory \
                "$wasm_file" -o "$dirname/${basename}_opt.wasm"
            mv "$dirname/${basename}_opt.wasm" "$wasm_file"
        fi
        
        # Copy optimized WASM to CDN directory
        cp "$wasm_file" "$WASM_DIR/"
        
        # Generate integrity hash
        local hash=$(sha384sum "$wasm_file" | cut -d' ' -f1)
        echo "sha384-$(echo -n "$hash" | base64 -w 0)" > "$WASM_DIR/$(basename "$wasm_file" .wasm).integrity"
        
        log "WASM file $(basename "$wasm_file") optimized ($(du -h "$wasm_file" | cut -f1))"
    done
    
    # Copy TypeScript bindings
    find "$TEMP_DIR/wasm" -name "*.ts" -o -name "*.js" | while read -r file; do
        cp "$file" "$WASM_DIR/"
    done
    
    success "WASM packages built and optimized"
}

# Process static assets
process_static_assets() {
    log "Processing static assets..."
    
    # Process CSS files
    if [[ -d "$ASSETS_DIR/css" ]]; then
        log "Processing CSS files..."
        for css_file in "$ASSETS_DIR/css"/*.css; do
            [[ -f "$css_file" ]] || continue
            local basename=$(basename "$css_file" .css)
            
            # Minify CSS
            npx clean-css-cli -o "$STATIC_DIR/css/${basename}.min.css" "$css_file"
            
            # Create compressed versions
            gzip -c "$STATIC_DIR/css/${basename}.min.css" > "$STATIC_DIR/css/${basename}.min.css.gz"
            brotli -c "$STATIC_DIR/css/${basename}.min.css" > "$STATIC_DIR/css/${basename}.min.css.br"
            
            log "CSS: $basename.css -> ${basename}.min.css ($(du -h "$STATIC_DIR/css/${basename}.min.css" | cut -f1))"
        done
    fi
    
    # Process JavaScript files
    if [[ -d "$ASSETS_DIR/js" ]]; then
        log "Processing JavaScript files..."
        for js_file in "$ASSETS_DIR/js"/*.js; do
            [[ -f "$js_file" ]] || continue
            local basename=$(basename "$js_file" .js)
            
            # Bundle and minify JavaScript
            npx esbuild "$js_file" \
                --bundle --minify --target=es2020 \
                --outfile="$STATIC_DIR/js/${basename}.min.js"
            
            # Create compressed versions
            gzip -c "$STATIC_DIR/js/${basename}.min.js" > "$STATIC_DIR/js/${basename}.min.js.gz"
            brotli -c "$STATIC_DIR/js/${basename}.min.js" > "$STATIC_DIR/js/${basename}.min.js.br"
            
            log "JS: $basename.js -> ${basename}.min.js ($(du -h "$STATIC_DIR/js/${basename}.min.js" | cut -f1))"
        done
    fi
    
    # Process TypeScript files if any
    if [[ -d "$ASSETS_DIR/ts" ]]; then
        log "Processing TypeScript files..."
        for ts_file in "$ASSETS_DIR/ts"/*.ts; do
            [[ -f "$ts_file" ]] || continue
            local basename=$(basename "$ts_file" .ts)
            
            # Compile and bundle TypeScript
            npx esbuild "$ts_file" \
                --bundle --minify --target=es2020 \
                --outfile="$STATIC_DIR/js/${basename}.min.js"
            
            # Create compressed versions
            gzip -c "$STATIC_DIR/js/${basename}.min.js" > "$STATIC_DIR/js/${basename}.min.js.gz"
            brotli -c "$STATIC_DIR/js/${basename}.min.js" > "$STATIC_DIR/js/${basename}.min.js.br"
            
            log "TS: $basename.ts -> ${basename}.min.js ($(du -h "$STATIC_DIR/js/${basename}.min.js" | cut -f1))"
        done
    fi
    
    success "Static assets processed"
}

# Optimize images
optimize_images() {
    log "Optimizing images..."
    
    if [[ ! -d "$ASSETS_DIR/images" ]]; then
        warn "No images directory found, skipping image optimization"
        return
    fi
    
    # Process PNG images
    find "$ASSETS_DIR/images" -name "*.png" | while read -r img; do
        local basename=$(basename "$img" .png)
        local rel_path=$(realpath --relative-to="$ASSETS_DIR/images" "$(dirname "$img")")
        local output_dir="$STATIC_DIR/images/$rel_path"
        mkdir -p "$output_dir"
        
        # Optimize PNG
        cp "$img" "$output_dir/$basename.png"
        optipng -o7 -quiet "$output_dir/$basename.png"
        
        # Generate WebP version
        convert "$img" -quality 85 "$output_dir/$basename.webp"
        
        # Generate different sizes for responsive images
        for size in 320 640 1024 1920; do
            convert "$img" -resize "${size}x${size}>" -quality 85 "$output_dir/${basename}-${size}w.png"
            convert "$img" -resize "${size}x${size}>" -quality 85 "$output_dir/${basename}-${size}w.webp"
        done
        
        log "PNG: $basename.png optimized with responsive variants"
    done
    
    # Process JPEG images
    find "$ASSETS_DIR/images" -name "*.jpg" -o -name "*.jpeg" | while read -r img; do
        local ext=$(echo "${img##*.}" | tr '[:upper:]' '[:lower:]')
        local basename=$(basename "$img" ".$ext")
        local rel_path=$(realpath --relative-to="$ASSETS_DIR/images" "$(dirname "$img")")
        local output_dir="$STATIC_DIR/images/$rel_path"
        mkdir -p "$output_dir"
        
        # Optimize JPEG
        cp "$img" "$output_dir/$basename.jpg"
        jpegoptim --max=85 --strip-all --quiet "$output_dir/$basename.jpg"
        
        # Generate WebP version
        convert "$img" -quality 85 "$output_dir/$basename.webp"
        
        # Generate different sizes for responsive images
        for size in 320 640 1024 1920; do
            convert "$img" -resize "${size}x${size}>" -quality 85 "$output_dir/${basename}-${size}w.jpg"
            convert "$img" -resize "${size}x${size}>" -quality 85 "$output_dir/${basename}-${size}w.webp"
        done
        
        log "JPEG: $basename.$ext optimized with responsive variants"
    done
    
    # Process SVG images (minify)
    find "$ASSETS_DIR/images" -name "*.svg" | while read -r img; do
        local basename=$(basename "$img" .svg)
        local rel_path=$(realpath --relative-to="$ASSETS_DIR/images" "$(dirname "$img")")
        local output_dir="$STATIC_DIR/images/$rel_path"
        mkdir -p "$output_dir"
        
        # Minify SVG if svgo is available
        if command -v svgo >/dev/null; then
            svgo --input "$img" --output "$output_dir/$basename.svg" --quiet
        else
            cp "$img" "$output_dir/$basename.svg"
        fi
        
        # Create compressed versions
        gzip -c "$output_dir/$basename.svg" > "$output_dir/$basename.svg.gz"
        brotli -c "$output_dir/$basename.svg" > "$output_dir/$basename.svg.br"
        
        log "SVG: $basename.svg optimized"
    done
    
    success "Images optimized"
}

# Process fonts
process_fonts() {
    log "Processing fonts..."
    
    if [[ ! -d "$ASSETS_DIR/fonts" ]]; then
        warn "No fonts directory found, skipping font processing"
        return
    fi
    
    # Copy and compress font files
    find "$ASSETS_DIR/fonts" -name "*.woff2" -o -name "*.woff" -o -name "*.ttf" -o -name "*.otf" | while read -r font; do
        local basename=$(basename "$font")
        cp "$font" "$STATIC_DIR/fonts/"
        
        # Fonts are already compressed, but create gzip versions for older browsers
        gzip -c "$STATIC_DIR/fonts/$basename" > "$STATIC_DIR/fonts/$basename.gz"
        
        log "Font: $basename processed"
    done
    
    success "Fonts processed"
}

# Generate service worker
generate_service_worker() {
    log "Generating service worker..."
    
    cat > "$CDN_DIR/sw.js" << 'EOF'
// BitCraps CDN Service Worker for offline gaming
const CACHE_NAME = 'bitcraps-v1';
const RUNTIME = 'runtime';

// Cache strategy for different content types
const cacheStrategies = {
  wasm: 'CacheFirst',
  static: 'CacheFirst', 
  api: 'NetworkFirst',
  game: 'NetworkFirst'
};

// Assets to cache immediately
const PRECACHE_ASSETS = [
  '/',
  '/static/css/main.min.css',
  '/static/js/main.min.js',
  '/wasm/bitcraps.wasm',
  '/static/images/logo.webp'
];

// Install event - cache essential assets
self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => cache.addAll(PRECACHE_ASSETS))
      .then(() => self.skipWaiting())
  );
});

// Activate event - clean up old caches
self.addEventListener('activate', event => {
  const currentCaches = [CACHE_NAME, RUNTIME];
  event.waitUntil(
    caches.keys()
      .then(cacheNames => {
        return cacheNames.filter(cacheName => 
          !currentCaches.includes(cacheName)
        );
      })
      .then(cachesToDelete => {
        return Promise.all(
          cachesToDelete.map(cacheToDelete => 
            caches.delete(cacheToDelete)
          )
        );
      })
      .then(() => self.clients.claim())
  );
});

// Fetch event - serve from cache or network based on strategy
self.addEventListener('fetch', event => {
  if (event.request.url.startsWith(self.location.origin)) {
    event.respondWith(
      caches.match(event.request).then(cachedResponse => {
        if (cachedResponse) {
          return cachedResponse;
        }

        return caches.open(RUNTIME).then(cache => {
          return fetch(event.request).then(response => {
            // Cache successful responses
            if (response.status === 200) {
              cache.put(event.request, response.clone());
            }
            return response;
          });
        });
      })
    );
  }
});

// Background sync for game state
self.addEventListener('sync', event => {
  if (event.tag === 'game-state-sync') {
    event.waitUntil(syncGameState());
  }
});

async function syncGameState() {
  // Implement game state synchronization when back online
  console.log('Syncing game state...');
}
EOF

    # Minify service worker
    if command -v terser >/dev/null; then
        terser "$CDN_DIR/sw.js" -c -m -o "$CDN_DIR/sw.min.js"
        mv "$CDN_DIR/sw.min.js" "$CDN_DIR/sw.js"
    fi
    
    success "Service worker generated"
}

# Create asset manifest
create_asset_manifest() {
    log "Creating asset manifest..."
    
    cat > "$CDN_DIR/manifest.json" << EOF
{
  "name": "BitCraps",
  "short_name": "BitCraps",
  "description": "Decentralized Gaming Platform",
  "start_url": "/",
  "display": "standalone",
  "theme_color": "#1a1a1a",
  "background_color": "#ffffff",
  "icons": [
    {
      "src": "/static/images/icon-192.webp",
      "sizes": "192x192",
      "type": "image/webp"
    },
    {
      "src": "/static/images/icon-512.webp", 
      "sizes": "512x512",
      "type": "image/webp"
    }
  ]
}
EOF
    
    success "Asset manifest created"
}

# Generate integrity hashes
generate_integrity_hashes() {
    log "Generating integrity hashes..."
    
    local manifest_file="$CDN_DIR/integrity.json"
    echo "{" > "$manifest_file"
    
    local first_entry=true
    find "$CDN_DIR" -type f \( -name "*.js" -o -name "*.css" -o -name "*.wasm" \) | while read -r file; do
        local rel_path=$(realpath --relative-to="$CDN_DIR" "$file")
        local hash=$(sha384sum "$file" | cut -d' ' -f1)
        local integrity="sha384-$(echo -n "$hash" | base64 -w 0)"
        
        if [[ "$first_entry" == "true" ]]; then
            first_entry=false
        else
            echo "," >> "$manifest_file"
        fi
        
        echo -n "  \"$rel_path\": \"$integrity\"" >> "$manifest_file"
    done
    
    echo -e "\n}" >> "$manifest_file"
    
    success "Integrity hashes generated"
}

# Upload to CDN (if configured)
upload_to_cdn() {
    if [[ -z "${CDN_UPLOAD:-}" ]]; then
        log "CDN upload not configured, skipping..."
        return
    fi
    
    log "Uploading assets to CDN..."
    
    case "${CDN_PROVIDER:-s3}" in
        "s3")
            if command -v aws >/dev/null; then
                aws s3 sync "$CDN_DIR/" "s3://${CDN_BUCKET:-bitcraps-cdn}/" \
                    --delete --compress --cache-control "public,max-age=31536000"
                success "Assets uploaded to S3"
            else
                warn "AWS CLI not available, skipping S3 upload"
            fi
            ;;
        "gcs")
            if command -v gsutil >/dev/null; then
                gsutil -m rsync -r -d "$CDN_DIR/" "gs://${CDN_BUCKET:-bitcraps-cdn}/"
                success "Assets uploaded to Google Cloud Storage"
            else
                warn "gsutil not available, skipping GCS upload"
            fi
            ;;
        *)
            warn "Unknown CDN provider: ${CDN_PROVIDER:-s3}"
            ;;
    esac
}

# Generate build report
generate_build_report() {
    log "Generating build report..."
    
    local report_file="$BUILD_DIR/build-report.json"
    local build_time=$(date -Iseconds)
    local total_size=$(du -sb "$CDN_DIR" | cut -f1)
    local compressed_size=$(find "$CDN_DIR" -name "*.gz" -exec du -cb {} + | tail -1 | cut -f1)
    
    cat > "$report_file" << EOF
{
  "buildTime": "$build_time",
  "version": "$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')",
  "totalSize": $total_size,
  "compressedSize": $compressed_size,
  "compressionRatio": $(echo "scale=2; $compressed_size * 100 / $total_size" | bc -l),
  "assets": {
    "wasm": $(find "$WASM_DIR" -name "*.wasm" | wc -l),
    "css": $(find "$STATIC_DIR/css" -name "*.css" | wc -l),
    "js": $(find "$STATIC_DIR/js" -name "*.js" | wc -l),
    "images": $(find "$STATIC_DIR/images" -type f | wc -l),
    "fonts": $(find "$STATIC_DIR/fonts" -type f | wc -l)
  }
}
EOF
    
    success "Build report generated: $report_file"
    log "Total size: $(numfmt --to=iec $total_size)"
    log "Compressed size: $(numfmt --to=iec $compressed_size)"
}

# Main execution
main() {
    log "Starting BitCraps asset optimization pipeline..."
    
    check_dependencies
    setup_directories
    build_wasm
    process_static_assets
    optimize_images
    process_fonts
    generate_service_worker
    create_asset_manifest
    generate_integrity_hashes
    upload_to_cdn
    generate_build_report
    
    success "Asset optimization pipeline completed!"
    log "Build output available at: $BUILD_DIR"
    log "CDN-ready assets available at: $CDN_DIR"
}

# Run main function
main "$@"