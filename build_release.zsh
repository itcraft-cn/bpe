#!/bin/zsh

rm -rf target/wheels/*.whl
cargo build --release
cd bambootube4j && gradle build -x test && cd ..
cd bambootube-py-wrapper && source bin/activate && maturin build --release && deactivate && cd ..
whl_file_name=$(ls target/wheels/bambootube4py-0.1.0-*)
cd pybambootube && source bin/activate && pip install ../$whl_file_name --force-reinstall && deactivate && cd ..

