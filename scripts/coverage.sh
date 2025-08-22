#!/bin/bash
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Cinstrument-coverage"

cargo test --all-features
grcov . --binary-path ./target/debug/ -s . -t html --branch --ignore-not-existing -o ./target/coverage/