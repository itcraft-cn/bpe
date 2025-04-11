#!/bin/bash

# Build core
RUSTFLAGS='-lLLVM-14' cargo build --release
cargo build

