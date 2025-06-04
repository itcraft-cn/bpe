#!/bin/bash

if [ $# -eq 0 ]; then
    echo without param, as default, simple build, debug
    CHOOSE=0
    elif [ $# -eq 1 ]; then
    CHOOSE=$1
else
    echo param error, quit
    exit 255
fi

if [ $CHOOSE -eq 0 ]; then
    echo simple build, debug
    CARGO_CMD="cargo build"
    FULL_BUILD=0
    RUSTFLAGS='-lLLVM-14 -Zub-checks=no'
elif [ $CHOOSE -eq 1 ]; then
    echo simple build, release
    CARGO_CMD="cargo build --release"
    FULL_BUILD=0
    RUSTFLAGS='-lLLVM-14'
elif [ $CHOOSE -eq 2 ]; then
    echo full build, debug
    CARGO_CMD="cargo build"
    MATURIN_CMD="maturin build"
    FULL_BUILD=1
    RUSTFLAGS='-lLLVM-14 -Zub-checks=no'
elif [ $CHOOSE -eq 3 ]; then
    echo full build, release
    CARGO_CMD="cargo build --release"
    MATURIN_CMD="maturin build --release"
    FULL_BUILD=1
    RUSTFLAGS='-lLLVM-14'
else
    echo param error, quit
    exit 255
fi

rm -rf target/wheels/*.whl

# Build core
RUSTFLAGS=$RUSTFLAGS $CARGO_CMD

if [ $FULL_BUILD -eq 1 ]; then
    # Build java
    cd bambootube4j
    mvnd package -Dmaven.test.skip=true
    cd ..
    
    # Build python wrapper
    cd bambootube-py-wrapper
    source bin/activate
    RUSTFLAGS=$RUSTFLAGS $MATURIN_CMD
    deactivate
    cd ..
    
    # install to python sample
    whl_file_name=$(ls target/wheels/bambootube4py-*)
    cd pysample
    source bin/activate
    pip install ../$whl_file_name --force-reinstall
    deactivate
    cd
fi