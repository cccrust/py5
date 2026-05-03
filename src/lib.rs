pub mod ast;
pub mod eval;
pub mod lexer;
pub mod natives;
pub mod parser;
pub mod value;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;

pub fn run_file(path: &str) {
    let src = fs::read_to_string(path).unwrap_or_else(|_| {
        eprintln!("cannot open {}", path);
        process::exit(1);
    });

    let globals = value::Env::new(None);
    eval::install_builtins(&globals);
    let mut rt = eval::Runtime {
        sys_modules: HashMap::new(),
        current_package: None,
        current_module_dir: None,
    };

    if let Some(sys_mod) = natives::load_native_module("sys") {
        rt.sys_modules.insert("sys".to_string(), sys_mod);
    }

    let tokens = lexer::lex_source(&src).unwrap_or_else(|e| {
        eprintln!("SyntaxError: {}", e);
        process::exit(1);
    });
    let mut parser = parser::Parser::new(&tokens, path);
    let module = parser.parse_module().unwrap_or_else(|e| {
        eprintln!("SyntaxError: {}", e);
        process::exit(1);
    });

    if let Err(exc) = eval::exec_block(&mut rt, &globals, &module) {
        eprintln!(
            "Traceback (most recent call last):\n  {}",
            value::py_to_string(&mut rt, exc).unwrap_or_default()
        );
        process::exit(1);
    }
}

pub fn run_args() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ./py5 <script.py> [args...]");
        process::exit(1);
    }
    run_file(&args[1]);
}