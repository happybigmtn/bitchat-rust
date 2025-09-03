#!/bin/bash
# Build script for BitCraps WebAssembly integration

set -e

echo "🚀 Building BitCraps for the web..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "❌ wasm-pack is not installed. Installing..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Clean previous build
echo "🧹 Cleaning previous build..."
rm -rf pkg/
rm -rf examples/web/pkg/

# Build the WASM package
echo "🔨 Building WASM package..."
wasm-pack build --target web --out-dir pkg --features "wasm,webrtc,browser" --release

# Copy built package to examples directory
echo "📦 Copying package to web example..."
cp -r pkg/ examples/web/pkg/

# Create a simple HTTP server script for testing
cat > examples/web/serve.py << 'EOF'
#!/usr/bin/env python3
"""Simple HTTP server for testing BitCraps WASM integration"""

import http.server
import socketserver
import os
import sys

# Change to the web directory
os.chdir(os.path.dirname(os.path.abspath(__file__)))

class MyHTTPRequestHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        # Add CORS headers for local development
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type')
        super().end_headers()

    def guess_type(self, path):
        mimetype = super().guess_type(path)
        # Ensure WASM files have correct MIME type
        if path.endswith('.wasm'):
            return 'application/wasm'
        return mimetype

PORT = 8000
print(f"🌐 Starting HTTP server on port {PORT}...")
print(f"📱 Open http://localhost:{PORT} in your browser")
print("Press Ctrl+C to stop")

try:
    with socketserver.TCPServer(("", PORT), MyHTTPRequestHandler) as httpd:
        httpd.serve_forever()
except KeyboardInterrupt:
    print("\n👋 Server stopped")
EOF

chmod +x examples/web/serve.py

# Create a Node.js server alternative
cat > examples/web/serve.js << 'EOF'
#!/usr/bin/env node
/**
 * Simple Node.js HTTP server for BitCraps WASM development
 * Serves static files with proper CORS and MIME type headers
 */

const http = require('http');
const fs = require('fs');
const path = require('path');
const url = require('url');

const PORT = 8000;

// MIME types
const mimeTypes = {
    '.html': 'text/html',
    '.js': 'text/javascript',
    '.css': 'text/css',
    '.wasm': 'application/wasm',
    '.json': 'application/json',
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.gif': 'image/gif',
    '.svg': 'image/svg+xml',
    '.ico': 'image/x-icon'
};

const server = http.createServer((req, res) => {
    // Parse URL
    const parsedUrl = url.parse(req.url);
    let pathname = parsedUrl.pathname;

    // Default to index.html
    if (pathname === '/') {
        pathname = '/index.html';
    }

    const filePath = path.join(__dirname, pathname);
    const ext = path.extname(filePath);

    // Check if file exists
    fs.access(filePath, fs.constants.F_OK, (err) => {
        if (err) {
            res.writeHead(404, { 'Content-Type': 'text/plain' });
            res.end('Not Found');
            return;
        }

        // Set CORS and security headers
        res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
        res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
        res.setHeader('Access-Control-Allow-Origin', '*');
        res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
        res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

        // Set content type
        const contentType = mimeTypes[ext] || 'application/octet-stream';
        res.setHeader('Content-Type', contentType);

        // Stream file
        const stream = fs.createReadStream(filePath);
        stream.pipe(res);
    });
});

server.listen(PORT, () => {
    console.log(`🌐 BitCraps development server running on http://localhost:${PORT}`);
    console.log('📱 Open the URL in your browser to test the WASM integration');
    console.log('Press Ctrl+C to stop');
});

server.on('error', (err) => {
    if (err.code === 'EADDRINUSE') {
        console.error(`❌ Port ${PORT} is already in use`);
        console.log('Try a different port or stop the other server');
    } else {
        console.error('Server error:', err);
    }
});
EOF

chmod +x examples/web/serve.js

# Create package.json for npm dependencies
cat > examples/web/package.json << 'EOF'
{
  "name": "bitcraps-web-example",
  "version": "1.0.0",
  "description": "BitCraps Web Example with WebAssembly",
  "main": "index.html",
  "scripts": {
    "serve": "node serve.js",
    "serve-python": "python3 serve.py",
    "build": "../../build-web.sh"
  },
  "keywords": ["bitcraps", "webassembly", "blockchain", "gaming", "p2p"],
  "author": "BitCraps Team",
  "license": "MIT",
  "devDependencies": {
    "http-server": "^14.1.1"
  }
}
EOF

# Create README for web example
cat > examples/web/README.md << 'EOF'
# BitCraps Web Example

This example demonstrates how to integrate BitCraps with web browsers using WebAssembly.

## Features

- **WebAssembly Integration**: Full BitCraps functionality in the browser
- **WebRTC P2P**: Direct peer-to-peer connections between browsers
- **WASM Plugins**: Load custom game logic as WASM modules
- **Real-time Gaming**: Synchronized multiplayer craps games
- **Modern UI**: Responsive web interface with real-time updates

## Quick Start

1. Build the WASM package:
   ```bash
   ../../build-web.sh
   ```

2. Start the development server:
   ```bash
   # Using Node.js (recommended)
   node serve.js
   
   # Or using Python
   python3 serve.py
   
   # Or using npm
   npm run serve
   ```

3. Open http://localhost:8000 in your browser

## Browser Requirements

- **WebAssembly**: All modern browsers (Chrome 57+, Firefox 52+, Safari 11+)
- **WebRTC**: For P2P functionality (Chrome 23+, Firefox 22+, Safari 11+)
- **ES6 Modules**: For module imports (Chrome 61+, Firefox 60+, Safari 10.1+)

## Development

### Building

The build process uses `wasm-pack` to compile Rust to WebAssembly:

```bash
wasm-pack build --target web --features "wasm,webrtc,browser"
```

### TypeScript Support

TypeScript definitions are available in `../../bitcraps.d.ts` for full type safety:

```typescript
import { BitCrapsWasm, JsGameAction, BrowserConfig } from './pkg/bitcraps_wasm.js';

const config: BrowserConfig = {
  enable_webrtc: true,
  signaling_server: 'wss://signal.bitcraps.io',
  debug_mode: false
};

const bitcraps = new BitCrapsWasm(config);
await bitcraps.initialize();
```

### WebRTC Configuration

Configure STUN/TURN servers for NAT traversal:

```javascript
const config = {
  enable_webrtc: true,
  signaling_server: 'wss://your-signaling-server.com',
  stun_servers: [
    'stun:stun.l.google.com:19302',
    'stun:stun1.l.google.com:19302'
  ],
  max_peers: 8,
  auto_connect: true
};
```

### Plugin Development

Load custom WASM plugins for game logic:

```javascript
// Load plugin from file
const response = await fetch('./plugins/custom-game.wasm');
const wasmBytes = new Uint8Array(await response.arrayBuffer());
await bitcraps.load_plugin('custom-game', wasmBytes);

// Execute plugin function
const action = new JsGameAction('place_bet', playerId);
action.set_amount(100);
action.set_bet_type('pass_line');

const newState = await bitcraps.execute_action(gameId, action);
```

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Web Browser   │    │  Signaling Server │    │   Other Peers   │
│                 │    │                  │    │                 │
│ ┌─────────────┐ │    │                  │    │ ┌─────────────┐ │
│ │ JavaScript  │ │    │                  │    │ │ JavaScript  │ │
│ │     UI      │ │    │                  │    │ │     UI      │ │
│ └─────────────┘ │    │                  │    │ └─────────────┘ │
│ ┌─────────────┐ │    │                  │    │ ┌─────────────┐ │
│ │   BitCraps  │◄┼────┼──────────────────┼────┼►│   BitCraps  │ │
│ │    WASM     │ │    │                  │    │ │    WASM     │ │
│ └─────────────┘ │    │                  │    │ └─────────────┘ │
│ ┌─────────────┐ │    │                  │    │ ┌─────────────┐ │
│ │   WebRTC    │◄┼────┼──────────────────┼────┼►│   WebRTC    │ │
│ │ Data Channel│ │    │                  │    │ │ Data Channel│ │
│ └─────────────┘ │    │                  │    │ └─────────────┘ │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## Security Considerations

- **Sandboxing**: WASM provides memory isolation
- **Content Security Policy**: Configure CSP headers appropriately
- **HTTPS**: Required for WebRTC in production
- **Input Validation**: All user input is validated client and server-side
- **Cryptographic Security**: Uses secure random number generation

## Performance Tips

- **WASM Memory**: Configure appropriate memory limits
- **Plugin Size**: Keep WASM plugins small for faster loading
- **Connection Limits**: Limit concurrent P2P connections
- **Garbage Collection**: WASM runtime manages memory automatically

## Troubleshooting

### Common Issues

1. **WASM Loading Failed**
   - Ensure server serves `.wasm` files with `application/wasm` MIME type
   - Check browser console for CORS errors

2. **WebRTC Connection Failed**
   - Verify STUN/TURN server configuration
   - Check firewall and NAT settings
   - Ensure HTTPS for production use

3. **Module Import Errors**
   - Use ES6 modules, not CommonJS
   - Check file paths in import statements
   - Verify browser supports ES6 modules

### Debug Mode

Enable debug logging for troubleshooting:

```javascript
bitcraps.enable_debug();
```

### Browser Developer Tools

Use browser developer tools to inspect:
- Network requests and WebSocket connections
- WASM module loading and execution
- WebRTC peer connection status
- Console logs and error messages

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test with `npm run serve`
5. Submit a pull request

## License

MIT License - see LICENSE file for details
EOF

echo "✅ Build complete!"
echo ""
echo "🎮 BitCraps Web Integration Ready!"
echo "📁 Files created:"
echo "   - pkg/                     (WASM package)"
echo "   - examples/web/pkg/        (Web example package)"  
echo "   - examples/web/serve.js    (Node.js server)"
echo "   - examples/web/serve.py    (Python server)"
echo "   - examples/web/package.json (NPM config)"
echo "   - examples/web/README.md   (Documentation)"
echo ""
echo "🚀 To test the web integration:"
echo "   cd examples/web"
echo "   node serve.js              (or python3 serve.py)"
echo "   Open http://localhost:8000 in your browser"
echo ""
echo "🌐 The web interface includes:"
echo "   - Real-time peer-to-peer networking via WebRTC"  
echo "   - Full craps game implementation in WASM"
echo "   - Plugin system for custom game logic"
echo "   - Modern responsive UI with live statistics"
echo ""
echo "Happy gaming! 🎲🎲"