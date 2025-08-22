# BitCraps 🎲 - Decentralized Craps Casino Protocol

A peer-to-peer Bluetooth mesh casino protocol with CRAP tokens, built in Rust. BitCraps enables trustless, decentralized gaming over local mesh networks without internet connectivity.

## 🎯 Features

### Core Protocol
- **Bluetooth Mesh Networking**: Direct peer-to-peer gaming without internet
- **Noise Protocol Encryption**: End-to-end encrypted communications
- **Forward Secrecy**: Ephemeral key rotation for perfect forward secrecy
- **Proof-of-Relay Mining**: Earn CRAP tokens by relaying messages

### Gaming Features
- **Decentralized Craps**: Classic casino craps with cryptographic fairness
- **CRAP Token Economy**: Native utility token for betting and rewards
- **Treasury Participation**: Automated house betting system
- **Multi-table Support**: Run multiple games simultaneously

## 🚀 Quick Start

```bash
# Clone the repository
git clone https://github.com/happybigmtn/bitchat-rust
cd bitchat-rust

# Build the project
cargo build --release

# Run tests
cargo test

# Start a node
cargo run -- start
```

## 📖 Documentation

The project is organized into weekly development modules (see docs/ folder).

## 🎮 Game Rules

BitCraps implements standard craps rules with automated treasury participation.

## 🏗️ Architecture

```
bitcraps/
├── src/
│   ├── protocol/     # Wire protocol & message types
│   ├── transport/    # Bluetooth & networking
│   ├── mesh/         # Mesh routing & game sessions
│   ├── session/      # Noise protocol & encryption
│   ├── gaming/       # Craps game logic
│   ├── token/        # CRAP token ledger
│   └── ui/           # CLI & TUI interfaces
└── docs/             # Weekly documentation
```

## 📜 License

MIT License

## ⚠️ Disclaimer

BitCraps is an experimental protocol for educational purposes.

---

**Built with 🦀 and ❤️ for decentralized gaming**
