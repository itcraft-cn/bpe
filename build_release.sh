#!/bin/bash

rm -rf target/wheels/*.whl

# Build core
RUSTFLAGS='-lLLVM-14' cargo build --release

# Build java
cd bambootube4j
mvnd package -Dmaven.test.skip=true
cd ..

# Build python
cd bambootube-py-wrapper
source bin/activate
RUSTFLAGS='-lLLVM-14' maturin build --release
deactivate
cd ..

# Build python wrapper
#whl_file_name=$(ls target/wheels/bambootube4py-*)
#cd pybambootube
#source bin/activate
#pip install ../$whl_file_name --force-reinstall
#deactivate
#cd ..

