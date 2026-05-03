# py5

A minimalist Python interpreter written in Rust.

## Build and Run

```bash
cargo build --release  # or cargo build
cargo run -- py/basic.py
```

Note: The `--` separator is required to pass arguments to the script.

## Testing

Run all tests via the provided script:

```bash
./test.sh
```

Individual test files: `py/basic.py`, `py/oop.py`, `py/magic.py`, `py/io.py`, `py/inherit.py`, `py/args.py`, `py/unpack.py`, `py/adv_oop.py`, `py/modern.py`, `py/test_stdlib.py`, `py/test_path.py`, `py/typed_annotation.py`

Import tests require `PYTHONPATH`:
```bash
PYTHONPATH=./py/import cargo run -- py/import/main_import.py
PYTHONPATH=./py/pkg cargo run -- py/pkg/main_pkg.py
```

## Architecture

- `src/main.rs` → entry point, calls `py5::run_args()`
- `src/lib.rs` → `run_file()` orchestrates: lex → parse → eval
- `src/lexer.rs` → tokenizer (TokenKind enum)
- `src/parser.rs` → AST builder (Expr/Stmt from ast.rs)
- `src/eval.rs` → interpreter runtime
- `src/value.rs` → Python value types (PyObject, Env)
- `src/natives.rs` → built-in native modules (sys, etc.)
- `py/` → Python test scripts (not stdlib, used as test cases)

## Notes

- No external crate dependencies (empty `[dependencies]` in Cargo.toml)
- The interpreter handles: lexing, parsing, evaluation, basic stdlib (print, range, etc.)
- Python import system is simulated via PYTHONPATH env var