#!/bin/zsh

rm -rf target/wheels/*.whl

# Build core
cargo build

# Build java
cd bambootube4j
gradle build -x test
cd ..

# Build python
cd bambootube-py-wrapper
source bin/activate
maturin build
deactivate
cd ..

# Build python wrapper
whl_file_name=$(ls target/wheels/bambootube4py-*)
cd pybambootube
source bin/activate
pip install ../$whl_file_name --force-reinstall
deactivate
cd ..
