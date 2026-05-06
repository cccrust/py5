# py5

A minimalist Python interpreter written in Rust.

## Features

- Basic syntax: variables, expressions, conditionals, loops, functions
- Data structures: lists, dicts, tuples
- Object-oriented: classes, `__init__`, self, bound methods, inheritance
- Exception handling: `try`/`except`/`raise`
- Standard library: `sys`, `os`, `time`, `json`, `math`
- Module system: import/from import (via PYTHONPATH)
- Others: f-strings, decorators, generators

## Build and Run

```bash
cargo build --release
cargo run -- py/basic.py
```

Note: The `--` separator is required to pass arguments to the script.

## Testing

```bash
./test.sh
```

Individual test files:
```bash
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
```

Module import tests:
```bash
PYTHONPATH=./py/import cargo run -- py/import/main_import.py
PYTHONPATH=./py/pkg cargo run -- py/pkg/main_pkg.py
```

## Architecture

```
src/main.rs      → entry point
src/lib.rs      → run_file(): lex → parse → eval
src/lexer.rs   → tokenizer
src/parser.rs  → AST parser
src/eval.rs    → evaluator
src/value.rs   → values and environment
src/natives.rs  → built-in modules
py/            → Python test scripts
```

## Dependencies

No external crate dependencies (`[dependencies]` is empty).