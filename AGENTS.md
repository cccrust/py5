# py5

A minimalist Python interpreter written in Rust.

## Build and Run

```bash
cargo build --release  # or cargo build
cargo run -- py/basic.py
```

## Testing

Run all tests via the provided script:

```bash
./test.sh
```

Key: some tests require `PYTHONPATH` set for import tests:
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