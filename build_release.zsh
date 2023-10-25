#!/bin/zsh

rm -rf target/wheels/*.whl
cargo build --release
cd shower4j && gradle build -x test && cd ..
cd shower-py-wrapper && source bin/activate && maturin build --release && deactivate && cd ..
whl_file_name=$(ls target/wheels/shower4py-0.1.0-*)
cd pyshower && source bin/activate && pip install ../$whl_file_name --force-reinstall && deactivate && cd ..

