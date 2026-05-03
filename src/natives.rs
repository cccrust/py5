use crate::eval::{eval_expr, Runtime};
use crate::lexer::{lex_source, TokenKind};
use crate::parser::Parser;
use crate::value::{py_err, py_err_val, Env, PyValue};
use std::cell::RefCell;
use std::env;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use std::process::Command;

pub(crate) fn get_site_packages() -> Vec<String> {
    if let Ok(output) = Command::new("python3")
        .args(["-c", "import site; print(';'.join(site.getsitepackages()))"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8(output.stdout).unwrap_or_default();
            return s
                .trim()
                .split(';')
                .filter(|p| !p.is_empty())
                .map(|p| p.to_string())
                .collect();
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let python_version = std::process::Command::new("python3")
        .args(["--version"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            let v = s.trim().split(' ').last().unwrap_or("3.11");
            Some(v.to_string())
        })
        .unwrap_or_else(|| "3.11".to_string());

    let mut paths = vec![
        format!("{}/.local/lib/python{}/site-packages", home, python_version),
        format!("/usr/local/lib/python{}/site-packages", python_version),
        format!("/usr/lib/python{}/site-packages", python_version),
    ];
    paths.retain(|p| std::path::Path::new(p).exists());
    paths
}

pub(crate) fn load_native_module(name: &str) -> Option<PyValue> {
    match name {
        "math" => Some(load_math()),
        "time" => Some(load_time()),
        "os" => Some(load_os()),
        "sys" => Some(load_sys()),
        "json" => Some(load_json()),
        _ => None,
    }
}

fn load_math() -> PyValue {
    let env = Env::new(None);
    env.borrow_mut()
        .set("pi", PyValue::Float(std::f64::consts::PI));
    env.borrow_mut().set(
        "sqrt",
        PyValue::Builtin(
            "sqrt".into(),
            Rc::new(|_, a, _| {
                if a.is_empty() {
                    return py_err("TypeError", "sqrt() takes exactly 1 argument");
                }
                Ok(PyValue::Float(a[0].as_num()?.sqrt()))
            }),
        ),
    );
    PyValue::Module("math".into(), env)
}

fn load_time() -> PyValue {
    let env = Env::new(None);
    env.borrow_mut().set(
        "time",
        PyValue::Builtin(
            "time".into(),
            Rc::new(|_, _, _| {
                let start = SystemTime::now();
                let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap();
                Ok(PyValue::Float(since_the_epoch.as_secs_f64()))
            }),
        ),
    );

    env.borrow_mut().set(
        "sleep",
        PyValue::Builtin(
            "sleep".into(),
            Rc::new(|_, a, _| {
                if a.is_empty() {
                    return py_err("TypeError", "sleep() takes exactly 1 argument");
                }
                let secs = a[0].as_num()?;
                sleep(Duration::from_secs_f64(secs));
                Ok(PyValue::None)
            }),
        ),
    );
    PyValue::Module("time".into(), env)
}

fn load_os() -> PyValue {
    let env = Env::new(None);
    env.borrow_mut().set(
        "getenv",
        PyValue::Builtin(
            "getenv".into(),
            Rc::new(|_, a, _| {
                if a.is_empty() {
                    return py_err("TypeError", "getenv() takes at least 1 argument");
                }
                let key = if let PyValue::Str(s) = &a[0] {
                    s
                } else {
                    return py_err("TypeError", "key must be string");
                };
                match env::var(key) {
                    Ok(val) => Ok(PyValue::Str(val)),
                    Err(_) => {
                        if a.len() > 1 {
                            Ok(a[1].clone())
                        } else {
                            Ok(PyValue::None)
                        }
                    }
                }
            }),
        ),
    );

    env.borrow_mut().set(
        "system",
        PyValue::Builtin(
            "system".into(),
            Rc::new(|_, a, _| {
                if a.is_empty() {
                    return py_err("TypeError", "system() takes exactly 1 argument");
                }
                let cmd_str = if let PyValue::Str(s) = &a[0] {
                    s
                } else {
                    return py_err("TypeError", "command must be string");
                };
                let status = Command::new("sh")
                    .arg("-c")
                    .arg(cmd_str)
                    .status()
                    .map_err(|e| py_err_val("OSError", &e.to_string()))?;
                Ok(PyValue::Int(status.code().unwrap_or(1) as i64))
            }),
        ),
    );
    PyValue::Module("os".into(), env)
}

fn load_sys() -> PyValue {
    let env = Env::new(None);
    let args: Vec<PyValue> = env::args().skip(1).map(PyValue::Str).collect();
    env.borrow_mut()
        .set("argv", PyValue::List(Rc::new(RefCell::new(args))));

    let mut paths = vec![PyValue::Str(".".to_string())];

    for sp in get_site_packages() {
        paths.push(PyValue::Str(sp));
    }

    if let Ok(pythonpath) = env::var("PYTHONPATH") {
        let separator = if cfg!(windows) { ";" } else { ":" };
        for p in pythonpath.split(separator) {
            if !p.is_empty() {
                paths.push(PyValue::Str(p.to_string()));
            }
        }
    }
    env.borrow_mut()
        .set("path", PyValue::List(Rc::new(RefCell::new(paths))));

    env.borrow_mut().set(
        "exit",
        PyValue::Builtin(
            "exit".into(),
            Rc::new(|_, a, _| {
                let code = if a.is_empty() {
                    0
                } else {
                    a[0].as_num()? as i32
                };
                std::process::exit(code);
            }),
        ),
    );

    env.borrow_mut().set(
        "modules",
        PyValue::Builtin(
            "modules".into(),
            Rc::new(|_, _, _| {
                py_err("NotImplementedError", "sys.modules not yet implemented")
            }),
        ),
    );

    env.borrow_mut().set(
        "version",
        PyValue::Str("3.11.0 (py5 implementation)".to_string()),
    );

    PyValue::Module("sys".into(), env)
}

fn load_json() -> PyValue {
    let env = Env::new(None);

    env.borrow_mut().set(
        "loads",
        PyValue::Builtin(
            "loads".into(),
            Rc::new(|rt, a, _| {
                if a.is_empty() {
                    return py_err("TypeError", "loads() takes exactly 1 argument");
                }
                let json_str = if let PyValue::Str(s) = &a[0] {
                    s
                } else {
                    return py_err("TypeError", "expected string");
                };

                let mut tokens = lex_source(json_str).map_err(|e| py_err_val("ValueError", &e))?;
                for t in &mut tokens {
                    if let TokenKind::Name(n) = &t.kind {
                        match n.as_str() {
                            "true" => t.kind = TokenKind::TrueVal,
                            "false" => t.kind = TokenKind::FalseVal,
                            "null" => t.kind = TokenKind::NoneVal,
                            _ => {}
                        }
                    }
                }

                let mut p = Parser::new(&tokens, "<json>");
                let ast = p
                    .parse_expr()
                    .map_err(|e| py_err_val("ValueError", &format!("Invalid JSON: {}", e)))?;
                eval_expr(rt, &Env::new(None), &ast)
            }),
        ),
    );

    env.borrow_mut().set(
        "dumps",
        PyValue::Builtin(
            "dumps".into(),
            Rc::new(|rt, a, _| {
                if a.is_empty() {
                    return py_err("TypeError", "dumps() takes exactly 1 argument");
                }

                fn dump_val(rt: &mut Runtime, v: &PyValue) -> Result<String, PyValue> {
                    match v {
                        PyValue::None => Ok("null".to_string()),
                        PyValue::Bool(b) => Ok(if *b {
                            "true".to_string()
                        } else {
                            "false".to_string()
                        }),
                        PyValue::Int(i) => Ok(i.to_string()),
                        PyValue::Float(f) => Ok(f.to_string()),
                        PyValue::Str(s) => Ok(format!("\"{}\"", s.replace('"', "\\\""))),
                        PyValue::List(l) => {
                            let mut items = Vec::new();
                            for item in l.borrow().iter() {
                                items.push(dump_val(rt, item)?);
                            }
                            Ok(format!("[{}]", items.join(", ")))
                        }
                        PyValue::Tuple(t) => {
                            let mut items = Vec::new();
                            for item in t.iter() {
                                items.push(dump_val(rt, item)?);
                            }
                            Ok(format!("[{}]", items.join(", ")))
                        }
                        PyValue::Dict(d) => {
                            let mut items = Vec::new();
                            for (k, val) in d.borrow().iter() {
                                items.push(format!("\"{}\": {}", k, dump_val(rt, val)?));
                            }
                            Ok(format!("{{{}}}", items.join(", ")))
                        }
                        _ => py_err("TypeError", "Object of this type is not JSON serializable"),
                    }
                }
                Ok(PyValue::Str(dump_val(rt, &a[0])?))
            }),
        ),
    );

    PyValue::Module("json".into(), env)
}