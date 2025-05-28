#!/bin/bash

# Build core
RUSTFLAGS='-lLLVM-14' cargo build --release
#RUSTFLAGS='-Zub-checks=no' cargo build
cargo build

