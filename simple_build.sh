#!/bin/bash

# Build core
#RUSTFLAGS='-lLLVM-14' cargo build --release
RUSTFLAGS='-lLLVM-14 -Zub-checks=no' cargo build
#RUSTFLAGS='-Zub-checks=no' cargo build
#cargo build

