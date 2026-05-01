set -x
cargo run py/basic.py
cargo run py/oop.py
cargo run py/magic.py
# cargo run main_import.py
# cargo run main_pkg.py
cargo run py/io.py
cargo run py/inherit.py
#cargo run py/decorator.py
cargo run py/args.py
cargo run py/unpack.py
cargo run py/adv_oop.py
cargo run py/modern.py
cargo run py/test_stdlib.py
cargo run py/test_path.py

PYTHONPATH=./py/ cargo run py/main_import.py
PYTHONPATH=./py/ cargo run py/main_pkg.py
