set -x
cargo run -- py/basic.py
cargo run -- py/oop.py
cargo run -- py/magic.py
cargo run -- py/io.py
cargo run -- py/inherit.py
cargo run -- py/args.py
cargo run -- py/unpack.py
cargo run -- py/adv_oop.py
cargo run -- py/modern.py
cargo run -- py/test_stdlib.py
cargo run -- py/test_path.py
cargo run -- py/typed_annotation.py

PYTHONPATH=./py/import cargo run -- py/import/main_import.py
PYTHONPATH=./py/pkg cargo run -- py/pkg/main_pkg.py