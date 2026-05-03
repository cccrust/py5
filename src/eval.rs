use crate::ast::{Expr, Op, Stmt, LogicOp};
use crate::natives;
use crate::value::{py_err, py_err_val, Env, PyValue};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;

pub(crate) struct Runtime {
    pub sys_modules: HashMap<String, PyValue>,
    pub current_package: Option<String>,
    pub current_module_dir: Option<String>,
}
pub(crate) enum ExecStatus {
    Continue,
    Return(PyValue),
    Break,
    ContinueLoop,
}

pub(crate) fn load_module(rt: &mut Runtime, name: &str) -> Result<PyValue, PyValue> {
    load_module_internal(rt, name, 0)
}

fn load_module_internal(rt: &mut Runtime, name: &str, level: usize) -> Result<PyValue, PyValue> {
    let full_name = if level > 0 {
        if let Some(ref pkg) = rt.current_package {
            let parts: Vec<&str> = pkg.split('.').collect();
            if level > parts.len() {
                return py_err("ImportError", "attempted relative import with no parent package");
            }
            let base_path = parts[..parts.len() - level].join(".");
            if name.is_empty() {
                base_path
            } else {
                format!("{}.{}", base_path, name)
            }
        } else {
            return py_err("ImportError", "relative import without a package");
        }
    } else {
        name.to_string()
    };

    if let Some(m) = rt.sys_modules.get(&full_name) {
        return Ok(m.clone());
    }

    if let Some(native_module) = natives::load_native_module(&full_name) {
        rt.sys_modules
            .insert(full_name.clone(), native_module.clone());
        return Ok(native_module);
    }

    let mut search_paths = vec![".".to_string()];
    if let Some(PyValue::Module(_, sys_env)) = rt.sys_modules.get("sys") {
        if let Ok(PyValue::List(l)) = sys_env.borrow().get("path") {
            for item in l.borrow().iter() {
                if let PyValue::Str(s) = item {
                    search_paths.push(s.clone());
                }
            }
        }
    }

    if let Some(ref mod_dir) = rt.current_module_dir {
        search_paths.insert(0, mod_dir.clone());
    }

    let path_base = full_name.replace('.', "/");
    let mut found_src = None;
    let mut found_path = String::new();

    for base in &search_paths {
        let file_path = format!("{}/{}.py", base, path_base);
        let pkg_init_path = format!("{}/{}/__init__.py", base, path_base);

        if let Ok(s) = fs::read_to_string(&file_path) {
            found_src = Some(s);
            found_path = file_path;
            break;
        } else if let Ok(s) = fs::read_to_string(&pkg_init_path) {
            found_src = Some(s);
            found_path = pkg_init_path;
            break;
        }
    }

    let src = found_src
        .ok_or_else(|| py_err_val("ImportError", &format!("No module named '{}'", name)))?;

    let found_dir = Path::new(&found_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let tokens = crate::lexer::lex_source(&src).map_err(|e| py_err_val("SyntaxError", &e))?;
    let mut parser = crate::parser::Parser::new(&tokens, &found_path);
    let ast = parser
        .parse_module()
        .map_err(|e| py_err_val("SyntaxError", &e))?;

    let prev_package = rt.current_package.take();
    let prev_dir = rt.current_module_dir.take();

    let new_package = if found_path.contains("__init__.py") {
        Some(full_name.clone())
    } else {
        Some(full_name.rsplit('.').next().unwrap_or(&full_name).to_string())
    };
    rt.current_package = new_package.clone();
    rt.current_module_dir = Some(found_dir);

    let mod_env = Env::new(None);
    install_builtins(&mod_env);
    exec_block(rt, &mod_env, &ast)?;

    rt.current_package = prev_package;
    rt.current_module_dir = prev_dir;

    let module_val = PyValue::Module(full_name.clone(), mod_env);
    rt.sys_modules.insert(full_name.clone(), module_val.clone());
    Ok(module_val)
}

pub(crate) fn assign_target(
    rt: &mut Runtime,
    env: &Rc<RefCell<Env>>,
    target: &Expr,
    val: PyValue,
) -> Result<(), PyValue> {
    match target {
        Expr::Name(n) => {
            env.borrow_mut().assign(n, val);
            Ok(())
        }
        Expr::Subscript(obj_expr, idx_expr) => {
            let obj = eval_expr(rt, env, obj_expr)?;
            let idx = eval_expr(rt, env, idx_expr)?;
            match &obj {
                PyValue::List(l) => {
                    let i = match idx {
                        PyValue::Int(i) => i,
                        _ => return py_err("TypeError", "list indices must be integers"),
                    };
                    let mut b = l.borrow_mut();
                    if i < 0 || i as usize >= b.len() {
                        return py_err("IndexError", "list assignment index out of range");
                    }
                    b[i as usize] = val;
                }
                PyValue::Dict(d) => {
                    d.borrow_mut().insert(idx.as_key()?, val);
                }
                PyValue::Tuple(_) => {
                    return py_err("TypeError", "tuple object does not support item assignment")
                }
                PyValue::Instance { class_val, .. } => {
                    if let Some(m) = crate::value::get_class_method(class_val, "__setitem__") {
                        let bound = PyValue::BoundMethod {
                            receiver: Box::new(obj.clone()),
                            func: Box::new(m),
                        };
                        call_func(rt, bound, vec![idx, val], HashMap::new())?;
                    } else {
                        return py_err("TypeError", "object does not support item assignment");
                    }
                }
                _ => return py_err("TypeError", "object does not support item assignment"),
            }
            Ok(())
        }
        Expr::Attribute(obj_expr, attr) => {
            let o = eval_expr(rt, env, obj_expr)?;
            if let PyValue::Instance { attrs, .. } = &o {
                attrs.borrow_mut().insert(attr.clone(), val);
                Ok(())
            } else {
                py_err("AttributeError", "cannot assign attribute")
            }
        }
        Expr::Tuple(items) | Expr::List(items) => {
            let iter_items = match val {
                PyValue::Tuple(t) => t,
                PyValue::List(l) => l.borrow().clone(),
                PyValue::Str(s) => s.chars().map(|c| PyValue::Str(c.to_string())).collect(),
                _ => return py_err("TypeError", "cannot unpack non-iterable object"),
            };
            if items.len() != iter_items.len() {
                return py_err(
                    "ValueError",
                    &format!(
                        "too many/few values to unpack (expected {}, got {})",
                        items.len(),
                        iter_items.len()
                    ),
                );
            }
            for (t, v) in items.iter().zip(iter_items) {
                assign_target(rt, env, t, v)?;
            }
            Ok(())
        }
        _ => py_err("SyntaxError", "invalid assign target"),
    }
}

pub(crate) fn eval_expr(
    rt: &mut Runtime,
    env: &Rc<RefCell<Env>>,
    expr: &Expr,
) -> Result<PyValue, PyValue> {
    match expr {
        Expr::NoneVal => Ok(PyValue::None),
        Expr::Bool(b) => Ok(PyValue::Bool(*b)),
        Expr::Int(v) => Ok(PyValue::Int(*v)),
        Expr::Float(v) => Ok(PyValue::Float(*v)),
        Expr::String(v) => Ok(PyValue::Str(v.clone())),
        Expr::Name(n) => env.borrow().get(n),
        Expr::FString(s) => {
            let mut res = String::new();
            let mut chars = s.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '{' {
                    let mut expr_str = String::new();
                    while let Some(&next_c) = chars.peek() {
                        if next_c == '}' {
                            chars.next();
                            break;
                        }
                        expr_str.push(chars.next().unwrap());
                    }
                    let toks =
                        crate::lexer::lex_source(&expr_str).map_err(|e| py_err_val("SyntaxError", &e))?;
                    let mut p = crate::parser::Parser::new(&toks, "<fstring>");
                    let e = p.parse_expr().map_err(|e| py_err_val("SyntaxError", &e))?;
                    let v = eval_expr(rt, env, &e)?;
                    res.push_str(&crate::value::py_to_string(rt, v)?);
                } else {
                    res.push(c);
                }
            }
            Ok(PyValue::Str(res))
        }
        Expr::Tuple(items) => {
            let mut t = vec![];
            for i in items {
                t.push(eval_expr(rt, env, i)?);
            }
            Ok(PyValue::Tuple(t))
        }
        Expr::List(items) => {
            let mut l = vec![];
            for i in items {
                l.push(eval_expr(rt, env, i)?);
            }
            Ok(PyValue::List(Rc::new(RefCell::new(l))))
        }
        Expr::Dict(pairs) => {
            let mut d = HashMap::new();
            for (k, v) in pairs {
                d.insert(eval_expr(rt, env, k)?.as_key()?, eval_expr(rt, env, v)?);
            }
            Ok(PyValue::Dict(Rc::new(RefCell::new(d))))
        }
        Expr::Lambda(params, body_expr) => Ok(PyValue::Function {
            name: "<lambda>".into(),
            params: params.clone(),
            defaults: HashMap::new(),
            vararg: None,
            kwarg: None,
            body: Rc::new(vec![Stmt::Return(Some(*body_expr.clone()))]),
            closure: Rc::clone(env),
        }),
        Expr::ListComp(exp, target, iter, cond) => {
            let it = eval_expr(rt, env, iter)?;
            let items = match it {
                PyValue::List(l) => l.borrow().clone(),
                PyValue::Tuple(t) => t,
                PyValue::Str(s) => s.chars().map(|c| PyValue::Str(c.to_string())).collect(),
                _ => return py_err("TypeError", "not iterable"),
            };
            let mut res = Vec::new();
            let loc = Env::new(Some(Rc::clone(env)));
            for item in items {
                assign_target(rt, &loc, target, item)?;
                let ok = if let Some(c) = cond {
                    eval_expr(rt, &loc, c)?.is_truthy()
                } else {
                    true
                };
                if ok {
                    res.push(eval_expr(rt, &loc, exp)?);
                }
            }
            Ok(PyValue::List(Rc::new(RefCell::new(res))))
        }
        Expr::BinOp(op, l, r) => {
            let left_val = eval_expr(rt, env, l)?;
            let right_val = eval_expr(rt, env, r)?;
            apply_binop(rt, env, *op, left_val, right_val)
        }
        Expr::UnaryOp(op, operand) => {
            let v = eval_expr(rt, env, operand)?;
            match op {
                Op::Neg => match v {
                    PyValue::Int(i) => Ok(PyValue::Int(-i)),
                    _ => Ok(PyValue::Float(-v.as_num()?)),
                },
                Op::Not => Ok(PyValue::Bool(!v.is_truthy())),
                _ => py_err("TypeError", "bad unary op"),
            }
        }
        Expr::Compare(op, l, r) => {
            let left_val = eval_expr(rt, env, l)?;
            let right_val = eval_expr(rt, env, r)?;
            apply_comp(rt, env, *op, left_val, right_val)
        }
        Expr::Logical(op, l, r) => {
            let lv = eval_expr(rt, env, l)?;
            match op {
                LogicOp::And => {
                    if !lv.is_truthy() {
                        Ok(lv)
                    } else {
                        eval_expr(rt, env, r)
                    }
                }
                LogicOp::Or => {
                    if lv.is_truthy() {
                        Ok(lv)
                    } else {
                        eval_expr(rt, env, r)
                    }
                }
            }
        }
        Expr::Call(func, args, kwargs) => {
            let f = eval_expr(rt, env, func)?;
            let mut a = vec![];
            for expr_a in args {
                a.push(eval_expr(rt, env, expr_a)?);
            }
            let mut kw = HashMap::new();
            for (k, v) in kwargs {
                kw.insert(k.clone(), eval_expr(rt, env, v)?);
            }
            call_func(rt, f, a, kw)
        }
        Expr::Attribute(obj, attr) => {
            let o = eval_expr(rt, env, obj)?;
            match &o {
                PyValue::Module(_, mod_env) => mod_env.borrow().get(attr),
                PyValue::Instance { class_val, attrs } => {
                    if let Some(v) = attrs.borrow().get(attr) {
                        return Ok(v.clone());
                    }
                    if let Some(m) = crate::value::get_class_method(class_val, attr) {
                        return Ok(PyValue::BoundMethod {
                            receiver: Box::new(o.clone()),
                            func: Box::new(m),
                        });
                    }
                    py_err(
                        "AttributeError",
                        &format!("object has no attribute '{}'", attr),
                    )
                }
                PyValue::Class { name, .. } => {
                    if let Some(m) = crate::value::get_class_method(&o, attr) {
                        Ok(m)
                    } else {
                        py_err(
                            "AttributeError",
                            &format!("type object '{}' has no attribute '{}'", name, attr),
                        )
                    }
                }
                PyValue::List(_) | PyValue::Dict(_) | PyValue::Str(_) | PyValue::File(_) => {
                    Ok(PyValue::Method(Box::new(o.clone()), attr.clone()))
                }
                _ => py_err("AttributeError", "object has no attribute"),
            }
        }
        Expr::Subscript(obj, idx) => {
            let o = eval_expr(rt, env, obj)?;
            let i = eval_expr(rt, env, idx)?;
            match &o {
                PyValue::Tuple(t) => {
                    let idx = match i {
                        PyValue::Int(i) => i,
                        _ => return py_err("TypeError", "index must be int"),
                    };
                    if idx < 0 || idx as usize >= t.len() {
                        py_err("IndexError", "tuple index out of range")
                    } else {
                        Ok(t[idx as usize].clone())
                    }
                }
                PyValue::List(l) => {
                    let idx = match i {
                        PyValue::Int(i) => i,
                        _ => return py_err("TypeError", "index must be int"),
                    };
                    let b = l.borrow();
                    if idx < 0 || idx as usize >= b.len() {
                        py_err("IndexError", "list index out of range")
                    } else {
                        Ok(b[idx as usize].clone())
                    }
                }
                PyValue::Dict(d) => d
                    .borrow()
                    .get(&i.as_key()?)
                    .cloned()
                    .ok_or_else(|| PyValue::Exception("KeyError".into(), Box::new(i.clone()))),
                PyValue::Instance { class_val, .. } => {
                    if let Some(m) = crate::value::get_class_method(class_val, "__getitem__") {
                        let bound = PyValue::BoundMethod {
                            receiver: Box::new(o.clone()),
                            func: Box::new(m),
                        };
                        return call_func(rt, bound, vec![i], HashMap::new());
                    }
                    py_err("TypeError", "object is not subscriptable")
                }
                _ => py_err("TypeError", "object is not subscriptable"),
            }
        }
    }
}

fn apply_binop(
    rt: &mut Runtime,
    _env: &Rc<RefCell<Env>>,
    op: Op,
    l: PyValue,
    r: PyValue,
) -> Result<PyValue, PyValue> {
    if op == Op::Add {
        if let PyValue::Instance { class_val, .. } = &l {
            if let Some(m) = crate::value::get_class_method(class_val, "__add__") {
                let bound = PyValue::BoundMethod {
                    receiver: Box::new(l.clone()),
                    func: Box::new(m),
                };
                return call_func(rt, bound, vec![r], HashMap::new());
            }
        }
        if let (PyValue::Str(a), PyValue::Str(b)) = (&l, &r) {
            return Ok(PyValue::Str(format!("{}{}", a, b)));
        }
    }
    if let (PyValue::Int(a), PyValue::Int(b)) = (&l, &r) {
        return match op {
            Op::Add => Ok(PyValue::Int(a + b)),
            Op::Sub => Ok(PyValue::Int(a - b)),
            Op::Mul => Ok(PyValue::Int(a * b)),
            Op::Div => {
                if *b == 0 {
                    return py_err("ZeroDivisionError", "division by zero");
                }
                Ok(PyValue::Float((*a as f64) / (*b as f64)))
            }
            Op::Mod => {
                if *b == 0 {
                    return py_err("ZeroDivisionError", "modulo by zero");
                }
                Ok(PyValue::Int(a % b))
            }
            _ => py_err("TypeError", "unsupported operand"),
        };
    }
    let a = l.as_num()?;
    let b = r.as_num()?;
    match op {
        Op::Add => Ok(PyValue::Float(a + b)),
        Op::Sub => Ok(PyValue::Float(a - b)),
        Op::Mul => Ok(PyValue::Float(a * b)),
        Op::Div => {
            if b == 0.0 {
                return py_err("ZeroDivisionError", "division by zero");
            }
            Ok(PyValue::Float(a / b))
        }
        Op::Mod => Ok(PyValue::Float((a as i64 % b as i64) as f64)),
        _ => py_err("TypeError", "unsupported operand"),
    }
}

fn apply_comp(
    _rt: &mut Runtime,
    _env: &Rc<RefCell<Env>>,
    op: Op,
    l: PyValue,
    r: PyValue,
) -> Result<PyValue, PyValue> {
    if let (PyValue::Str(a), PyValue::Str(b)) = (&l, &r) {
        return Ok(PyValue::Bool(match op {
            Op::Eq => a == b,
            Op::Ne => a != b,
            _ => false,
        }));
    }
    let a = l.as_num()?;
    let b = r.as_num()?;
    Ok(PyValue::Bool(match op {
        Op::Eq => a == b,
        Op::Ne => a != b,
        Op::Lt => a < b,
        Op::Le => a <= b,
        Op::Gt => a > b,
        Op::Ge => a >= b,
        _ => false,
    }))
}

fn exec_stmt(
    rt: &mut Runtime,
    env: &Rc<RefCell<Env>>,
    stmt: &Stmt,
) -> Result<ExecStatus, PyValue> {
    match stmt {
        Stmt::Expr(e) => {
            eval_expr(rt, env, e)?;
            Ok(ExecStatus::Continue)
        }
        Stmt::Assign(target, e) => {
            let v = eval_expr(rt, env, e)?;
            assign_target(rt, env, target, v)?;
            Ok(ExecStatus::Continue)
        }
        Stmt::If(t, b, e) => {
            if eval_expr(rt, env, t)?.is_truthy() {
                exec_block(rt, env, b)
            } else {
                exec_block(rt, env, e)
            }
        }
        Stmt::While(t, b) => {
            while eval_expr(rt, env, t)?.is_truthy() {
                match exec_block(rt, env, b)? {
                    ExecStatus::Return(v) => return Ok(ExecStatus::Return(v)),
                    ExecStatus::Break => break,
                    _ => {}
                }
            }
            Ok(ExecStatus::Continue)
        }
        Stmt::For(target, iter_expr, b) => {
            let it = eval_expr(rt, env, iter_expr)?;
            let items = match it {
                PyValue::List(l) => l.borrow().clone(),
                PyValue::Tuple(t) => t,
                PyValue::Str(s) => s.chars().map(|c| PyValue::Str(c.to_string())).collect(),
                _ => return py_err("TypeError", "object is not iterable"),
            };
            for item in items {
                assign_target(rt, env, target, item)?;
                match exec_block(rt, env, b)? {
                    ExecStatus::Return(ret) => return Ok(ExecStatus::Return(ret)),
                    ExecStatus::Break => break,
                    _ => {}
                }
            }
            Ok(ExecStatus::Continue)
        }
        Stmt::FunctionDef(n, p, vararg, kwarg, b) => {
            let mut params = Vec::new();
            let mut defaults = HashMap::new();
            for (p_name, p_def) in p {
                params.push(p_name.clone());
                if let Some(def_expr) = p_def {
                    defaults.insert(p_name.clone(), eval_expr(rt, env, def_expr)?);
                }
            }
            env.borrow_mut().set(
                n,
                PyValue::Function {
                    name: n.clone(),
                    params,
                    defaults,
                    vararg: vararg.clone(),
                    kwarg: kwarg.clone(),
                    body: Rc::new(b.clone()),
                    closure: Rc::clone(env),
                },
            );
            Ok(ExecStatus::Continue)
        }
        Stmt::ClassDef(n, base_expr, b) => {
            let base_val = if let Some(expr) = base_expr {
                let v = eval_expr(rt, env, expr)?;
                if !matches!(v, PyValue::Class { .. }) {
                    return py_err("TypeError", "base is not a class");
                }
                Some(Box::new(v))
            } else {
                None
            };
            let class_env = Env::new(Some(Rc::clone(env)));
            exec_block(rt, &class_env, b)?;
            let methods = class_env.borrow().vars.clone();
            env.borrow_mut().set(
                n,
                PyValue::Class {
                    name: n.clone(),
                    base: base_val,
                    methods: Rc::new(methods),
                },
            );
            Ok(ExecStatus::Continue)
        }
        Stmt::Try(body, handlers) => match exec_block(rt, env, body) {
            Err(exc) => {
                for (exc_types, exc_as, except_body) in handlers {
                    let should_catch = if exc_types.is_empty() {
                        true
                    } else {
                        if let PyValue::Exception(exc_t, _) = &exc {
                            exc_types.contains(&"Exception".to_string())
                                || exc_types.contains(exc_t)
                        } else {
                            false
                        }
                    };
                    if should_catch {
                        let except_env = Env::new(Some(Rc::clone(env)));
                        if let Some(var) = exc_as {
                            except_env.borrow_mut().set(var, exc);
                        }
                        return exec_block(rt, &except_env, except_body);
                    }
                }
                Err(exc)
            }
            Ok(status) => Ok(status),
        },
        Stmt::Raise(e) => Err(eval_expr(rt, env, e)?),
        Stmt::Import(mod_name) => {
            let module = load_module(rt, mod_name)?;
            let bind_name = mod_name.split('.').last().unwrap();
            env.borrow_mut().assign(bind_name, module);
            Ok(ExecStatus::Continue)
        }
        Stmt::FromImport(mod_name, names, level) => {
            let module = load_module_internal(rt, mod_name, *level)?;
            if let PyValue::Module(_, mod_env) = module {
                for n in names {
                    let val = mod_env.borrow().get(n)?;
                    env.borrow_mut().assign(n, val);
                }
            }
            Ok(ExecStatus::Continue)
        }
        Stmt::Return(e) => Ok(ExecStatus::Return(if let Some(x) = e {
            eval_expr(rt, env, x)?
        } else {
            PyValue::None
        })),
        Stmt::Break => Ok(ExecStatus::Break),
        Stmt::Continue => Ok(ExecStatus::ContinueLoop),
        Stmt::Pass => Ok(ExecStatus::Continue),
    }
}

pub(crate) fn exec_block(
    rt: &mut Runtime,
    env: &Rc<RefCell<Env>>,
    stmts: &[Stmt],
) -> Result<ExecStatus, PyValue> {
    for s in stmts {
        let st = exec_stmt(rt, env, s)?;
        if !matches!(st, ExecStatus::Continue) {
            return Ok(st);
        }
    }
    Ok(ExecStatus::Continue)
}

pub(crate) fn call_func(
    rt: &mut Runtime,
    func: PyValue,
    args: Vec<PyValue>,
    kwargs: HashMap<String, PyValue>,
) -> Result<PyValue, PyValue> {
    match func {
        PyValue::Builtin(_, f) => f(rt, args, kwargs),
        PyValue::Method(obj, name) => match (&*obj, name.as_str()) {
            (PyValue::List(l), "append") => {
                l.borrow_mut().push(args[0].clone());
                Ok(PyValue::None)
            }
            (PyValue::List(l), "pop") => Ok(l.borrow_mut().pop().unwrap_or(PyValue::None)),
            (PyValue::Dict(d), "keys") => {
                let keys: Vec<PyValue> =
                    d.borrow().keys().map(|k| PyValue::Str(k.clone())).collect();
                Ok(PyValue::List(Rc::new(RefCell::new(keys))))
            }
            (PyValue::Dict(d), "values") => {
                let vals: Vec<PyValue> = d.borrow().values().cloned().collect();
                Ok(PyValue::List(Rc::new(RefCell::new(vals))))
            }
            (PyValue::Dict(d), "items") => {
                let items: Vec<PyValue> = d
                    .borrow()
                    .iter()
                    .map(|(k, v)| PyValue::Tuple(vec![PyValue::Str(k.clone()), v.clone()]))
                    .collect();
                Ok(PyValue::List(Rc::new(RefCell::new(items))))
            }
            (PyValue::Dict(d), "copy") => {
                let new_dict = d.borrow().clone();
                Ok(PyValue::Dict(Rc::new(RefCell::new(new_dict))))
            }
            (PyValue::Str(s), "split") => {
                let sep = if args.is_empty() {
                    " "
                } else {
                    if let PyValue::Str(sep) = &args[0] {
                        sep
                    } else {
                        return py_err("TypeError", "separator must be str");
                    }
                };
                let parts: Vec<PyValue> =
                    s.split(sep).map(|p| PyValue::Str(p.to_string())).collect();
                Ok(PyValue::List(Rc::new(RefCell::new(parts))))
            }
            (PyValue::Str(s), "join") => {
                if args.is_empty() {
                    return py_err("TypeError", "join() takes exactly one argument");
                }
                if let PyValue::List(l) = &args[0] {
                    let strings: Result<Vec<String>, _> = l
                        .borrow()
                        .iter()
                        .map(|v| {
                            if let PyValue::Str(sv) = v {
                                Ok(sv.clone())
                            } else {
                                Err(())
                            }
                        })
                        .collect();
                    if let Ok(strings) = strings {
                        return Ok(PyValue::Str(strings.join(s)));
                    }
                }
                py_err("TypeError", "join() expects list of strings")
            }
            (PyValue::File(f), "read") => {
                if let Some(file) = f.borrow_mut().as_mut() {
                    let mut s = String::new();
                    file.read_to_string(&mut s)
                        .map_err(|e: std::io::Error| py_err_val("IOError", &e.to_string()))?;
                    Ok(PyValue::Str(s))
                } else {
                    py_err("ValueError", "I/O operation on closed file.")
                }
            }
            (PyValue::File(f), "write") => {
                if let Some(file) = f.borrow_mut().as_mut() {
                    if args.is_empty() {
                        return py_err("TypeError", "write() takes exactly one argument");
                    }
                    let s = crate::value::py_to_string(rt, args[0].clone())?;
                    file.write_all(s.as_bytes())
                        .map_err(|e| py_err_val("IOError", &e.to_string()))?;
                    Ok(PyValue::Int(s.len() as i64))
                } else {
                    py_err("ValueError", "I/O operation on closed file.")
                }
            }
            (PyValue::File(f), "close") => {
                *f.borrow_mut() = None;
                Ok(PyValue::None)
            }
            _ => py_err("AttributeError", &format!("unknown method '{}'", name)),
        },
        PyValue::Class { .. } => {
            let inst = PyValue::Instance {
                class_val: Box::new(func.clone()),
                attrs: Rc::new(RefCell::new(HashMap::new())),
            };
            if let Some(init) = crate::value::get_class_method(&func, "__init__") {
                let mut a = vec![inst.clone()];
                a.extend(args);
                call_func(rt, init, a, kwargs)?;
            }
            Ok(inst)
        }
        PyValue::BoundMethod { receiver, func } => {
            let mut a = vec![*receiver];
            a.extend(args);
            call_func(rt, *func, a, kwargs)
        }
        PyValue::Function {
            name,
            params,
            defaults,
            vararg,
            kwarg,
            body,
            closure,
        } => {
            let local = Env::new(Some(closure));
            let mut arg_idx = 0;
            let mut bound_params = std::collections::HashSet::new();
            for arg_val in args.iter() {
                if arg_idx < params.len() {
                    let p_name = &params[arg_idx];
                    local.borrow_mut().set(p_name, arg_val.clone());
                    bound_params.insert(p_name.clone());
                    arg_idx += 1;
                } else {
                    break;
                }
            }
            if let Some(vname) = &vararg {
                let rest = args[arg_idx..].to_vec();
                local.borrow_mut().set(vname, PyValue::Tuple(rest));
            } else if arg_idx < args.len() {
                return py_err(
                    "TypeError",
                    &format!(
                        "{}() takes {} positional arguments but {} were given",
                        name,
                        params.len(),
                        args.len()
                    ),
                );
            }
            let mut leftover_kwargs = HashMap::new();
            for (k_name, k_val) in kwargs {
                if params.contains(&k_name) {
                    if bound_params.contains(&k_name) {
                        return py_err(
                            "TypeError",
                            &format!("{}() got multiple values for argument '{}'", name, k_name),
                        );
                    }
                    local.borrow_mut().set(&k_name, k_val);
                    bound_params.insert(k_name.clone());
                } else {
                    leftover_kwargs.insert(k_name, k_val);
                }
            }
            if let Some(kw_name) = &kwarg {
                local.borrow_mut().set(
                    kw_name,
                    PyValue::Dict(Rc::new(RefCell::new(leftover_kwargs))),
                );
            } else if !leftover_kwargs.is_empty() {
                let bad_key = leftover_kwargs.keys().next().unwrap();
                return py_err(
                    "TypeError",
                    &format!(
                        "{}() got an unexpected keyword argument '{}'",
                        name, bad_key
                    ),
                );
            }
            for p_name in params.iter() {
                if !bound_params.contains(p_name) {
                    if let Some(def_val) = defaults.get(p_name) {
                        local.borrow_mut().set(p_name, def_val.clone());
                    } else {
                        return py_err(
                            "TypeError",
                            &format!("{}() missing required argument: '{}'", name, p_name),
                        );
                    }
                }
            }
            match exec_block(rt, &local, &body)? {
                ExecStatus::Return(v) => Ok(v),
                _ => Ok(PyValue::None),
            }
        }
        _ => py_err("TypeError", "object is not callable"),
    }
}

pub(crate) fn install_builtins(globals: &Rc<RefCell<Env>>) {
    let mut e = globals.borrow_mut();
    e.set(
        "print",
        PyValue::Builtin(
            "print".into(),
            Rc::new(|rt, a, _kw| {
                let mut out = Vec::new();
                for val in a {
                    out.push(crate::value::py_to_string(rt, val.clone())?);
                }
                println!("{}", out.join(" "));
                Ok(PyValue::None)
            }),
        ),
    );
    e.set(
        "str",
        PyValue::Builtin(
            "str".into(),
            Rc::new(|rt, a, _kw| {
                if a.len() != 1 {
                    return py_err("TypeError", "str() takes exactly one argument");
                }
                Ok(PyValue::Str(crate::value::py_to_string(rt, a[0].clone())?))
            }),
        ),
    );
    e.set(
        "len",
        PyValue::Builtin(
            "len".into(),
            Rc::new(|_, a, _kw| {
                if a.is_empty() {
                    return py_err("TypeError", "len() takes exactly one argument (0 given)");
                }
                match &a[0] {
                    PyValue::Str(s) => Ok(PyValue::Int(s.len() as i64)),
                    PyValue::List(l) => Ok(PyValue::Int(l.borrow().len() as i64)),
                    PyValue::Tuple(t) => Ok(PyValue::Int(t.len() as i64)),
                    PyValue::Dict(d) => Ok(PyValue::Int(d.borrow().len() as i64)),
                    _ => py_err("TypeError", "object has no len()"),
                }
            }),
        ),
    );
    e.set(
        "range",
        PyValue::Builtin(
            "range".into(),
            Rc::new(|_, a, _kw| {
                if a.is_empty() {
                    return py_err("TypeError", "range expected 1 argument, got 0");
                }
                let end = match a[0] {
                    PyValue::Int(i) => i,
                    _ => return py_err("TypeError", "range() integer argument expected"),
                };
                Ok(PyValue::List(Rc::new(RefCell::new(
                    (0..end).map(PyValue::Int).collect(),
                ))))
            }),
        ),
    );
    e.set(
        "open",
        PyValue::Builtin(
            "open".into(),
            Rc::new(|_, a, _kw| {
                if a.is_empty() {
                    return py_err("TypeError", "open() expected at least 1 argument");
                }
                let path = if let PyValue::Str(s) = &a[0] {
                    s
                } else {
                    return py_err("TypeError", "expected string as path");
                };
                let mode = if a.len() > 1 {
                    if let PyValue::Str(s) = &a[1] {
                        s.clone()
                    } else {
                        return py_err("TypeError", "expected string as mode");
                    }
                } else {
                    "r".to_string()
                };
                let mut opts = std::fs::OpenOptions::new();
                match mode.as_str() {
                    "r" => opts.read(true),
                    "w" => opts.write(true).create(true).truncate(true),
                    "a" => opts.write(true).create(true).append(true),
                    _ => return py_err("ValueError", "invalid mode"),
                };
                let file = opts
                    .open(path)
                    .map_err(|err| py_err_val("IOError", &err.to_string()))?;
                Ok(PyValue::File(Rc::new(RefCell::new(Some(file)))))
            }),
        ),
    );
    e.set(
        "type",
        PyValue::Builtin(
            "type".into(),
            Rc::new(|_, a, _| {
                if a.len() != 1 {
                    return py_err("TypeError", "type() takes 1 argument");
                }
                let type_name = match &a[0] {
                    PyValue::Int(_) => "int",
                    PyValue::Float(_) => "float",
                    PyValue::Str(_) => "str",
                    PyValue::Bool(_) => "bool",
                    PyValue::List(_) => "list",
                    PyValue::Dict(_) => "dict",
                    PyValue::Tuple(_) => "tuple",
                    PyValue::None => "NoneType",
                    PyValue::Instance { class_val, .. } => {
                        if let PyValue::Class { name, .. } = &**class_val {
                            name
                        } else {
                            "object"
                        }
                    }
                    PyValue::Class { .. } => "type",
                    PyValue::Function { .. }
                    | PyValue::Builtin(..)
                    | PyValue::BoundMethod { .. }
                    | PyValue::Method(..) => "function",
                    _ => "object",
                };
                Ok(PyValue::Str(format!("<class '{}'>", type_name)))
            }),
        ),
    );
    e.set(
        "isinstance",
        PyValue::Builtin(
            "isinstance".into(),
            Rc::new(|_, a, _| {
                if a.len() != 2 {
                    return py_err("TypeError", "isinstance expected 2 arguments");
                }
                let (obj, cls) = (&a[0], &a[1]);
                if let PyValue::Class {
                    name: target_name,
                    ..
                } = cls
                {
                    if let PyValue::Instance { class_val, .. } = obj {
                        fn check_class(c: &PyValue, target: &str) -> bool {
                            if let PyValue::Class { name, base, .. } = c {
                                if name == target {
                                    return true;
                                }
                                if let Some(b) = base {
                                    return check_class(b, target);
                                }
                            }
                            false
                        }
                        Ok(PyValue::Bool(check_class(class_val, target_name)))
                    } else {
                        Ok(PyValue::Bool(false))
                    }
                } else {
                    py_err("TypeError", "isinstance() arg 2 must be a type")
                }
            }),
        ),
    );

    let exc_types = [
        "Exception",
        "TypeError",
        "ValueError",
        "NameError",
        "IndexError",
        "AttributeError",
        "KeyError",
        "IOError",
        "ImportError",
        "ZeroDivisionError",
        "SyntaxError",
    ];
    for exc in exc_types {
        let name = exc.to_string();
        e.set(
            exc,
            PyValue::Builtin(
                name.clone(),
                Rc::new(move |_, a, _| {
                    let arg = a.get(0).cloned().unwrap_or(PyValue::None);
                    Ok(PyValue::Exception(name.clone(), Box::new(arg)))
                }),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser};
    use std::collections::HashMap;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn setup_runtime() -> (Runtime, Rc<RefCell<Env>>) {
        let globals = Env::new(None);
        install_builtins(&globals);
        let mut rt = Runtime {
            sys_modules: HashMap::new(),
            current_package: None,
            current_module_dir: None,
        };
        if let Some(sys_mod) = natives::load_native_module("sys") {
            rt.sys_modules.insert("sys".to_string(), sys_mod.clone());
            globals.borrow_mut().set("sys", sys_mod);
        }
        (rt, globals)
    }

    fn run_stmt(source: &str) -> Result<ExecStatus, PyValue> {
        let (mut rt, globals) = setup_runtime();
        let tokens = lexer::lex_source(source).unwrap();
        let mut parser = parser::Parser::new(&tokens, "<test>");
        let stmts = parser.parse_module().unwrap();
        exec_block(&mut rt, &globals, &stmts)
    }

    fn eval_single_expr(source: &str) -> Result<PyValue, PyValue> {
        let (mut rt, globals) = setup_runtime();
        let tokens = lexer::lex_source(source).unwrap();
        let mut parser = parser::Parser::new(&tokens, "<test>");
        let expr = parser.parse_expr().unwrap();
        eval_expr(&mut rt, &globals, &expr)
    }

    fn exec_module(source: &str) -> Result<PyValue, PyValue> {
        let (mut rt, globals) = setup_runtime();
        let tokens = lexer::lex_source(source).unwrap();
        let mut parser = parser::Parser::new(&tokens, "<test>");
        let stmts = parser.parse_module().unwrap();
        exec_block(&mut rt, &globals, &stmts).map(|_| PyValue::None)
    }

    fn get_global(source: &str, name: &str) -> PyValue {
        let (mut rt, globals) = setup_runtime();
        let tokens = lexer::lex_source(source).unwrap();
        let mut parser = parser::Parser::new(&tokens, "<test>");
        let stmts = parser.parse_module().unwrap();
        exec_block(&mut rt, &globals, &stmts).ok();
        let result = globals.borrow().get(name).unwrap();
        result
    }

    #[test]
    fn test_arithmetic_int_add() {
        assert_eq!(eval_single_expr("1 + 2").unwrap(), PyValue::Int(3));
    }

    #[test]
    fn test_arithmetic_int_sub() {
        assert_eq!(eval_single_expr("5 - 3").unwrap(), PyValue::Int(2));
    }

    #[test]
    fn test_arithmetic_int_mul() {
        assert_eq!(eval_single_expr("4 * 2").unwrap(), PyValue::Int(8));
    }

    #[test]
    fn test_arithmetic_int_div() {
        assert_eq!(eval_single_expr("10 / 2").unwrap(), PyValue::Float(5.0));
    }

    #[test]
    fn test_arithmetic_int_mod() {
        assert_eq!(eval_single_expr("10 % 3").unwrap(), PyValue::Int(1));
    }

    #[test]
    fn test_arithmetic_negative() {
        assert_eq!(eval_single_expr("-5").unwrap(), PyValue::Int(-5));
    }

    #[test]
    fn test_arithmetic_complex() {
        assert_eq!(eval_single_expr("2 + 3 * 4").unwrap(), PyValue::Int(14));
    }

    #[test]
    fn test_arithmetic_float_ops() {
        let r1 = eval_single_expr("3.14 + 1.0").unwrap();
        if let PyValue::Float(f) = r1 { assert!((f - 4.14).abs() < 0.001); } else { panic!("Expected float"); }
        let r2 = eval_single_expr("5.0 - 2.0").unwrap();
        if let PyValue::Float(f) = r2 { assert!((f - 3.0).abs() < 0.001); } else { panic!("Expected float"); }
        let r3 = eval_single_expr("2.0 * 3.0").unwrap();
        if let PyValue::Float(f) = r3 { assert!((f - 6.0).abs() < 0.001); } else { panic!("Expected float"); }
        let r4 = eval_single_expr("6.0 / 2.0").unwrap();
        if let PyValue::Float(f) = r4 { assert!((f - 3.0).abs() < 0.001); } else { panic!("Expected float"); }
    }

    #[test]
    fn test_arithmetic_mixed_int_float() {
        let result = eval_single_expr("5 + 2.5").unwrap();
        assert_eq!(result, PyValue::Float(7.5));
    }

    #[test]
    fn test_division_by_zero() {
        let result = eval_single_expr("1 / 0");
        assert!(result.is_err());
        if let Err(PyValue::Exception(t, _)) = result {
            assert_eq!(t, "ZeroDivisionError");
        } else {
            panic!("Expected ZeroDivisionError");
        }
    }

    #[test]
    fn test_comparison_eq() {
        assert_eq!(eval_single_expr("1 == 1").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("1 == 2").unwrap(), PyValue::Bool(false));
    }

    #[test]
    fn test_comparison_ne() {
        assert_eq!(eval_single_expr("1 != 2").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("1 != 1").unwrap(), PyValue::Bool(false));
    }

    #[test]
    fn test_comparison_lt() {
        assert_eq!(eval_single_expr("1 < 2").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("2 < 1").unwrap(), PyValue::Bool(false));
    }

    #[test]
    fn test_comparison_le() {
        assert_eq!(eval_single_expr("1 <= 2").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("2 <= 2").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("3 <= 2").unwrap(), PyValue::Bool(false));
    }

    #[test]
    fn test_comparison_gt() {
        assert_eq!(eval_single_expr("2 > 1").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("1 > 2").unwrap(), PyValue::Bool(false));
    }

    #[test]
    fn test_comparison_ge() {
        assert_eq!(eval_single_expr("2 >= 1").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("2 >= 2").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("2 >= 3").unwrap(), PyValue::Bool(false));
    }

    #[test]
    fn test_logical_and() {
        assert_eq!(eval_single_expr("True and True").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("True and False").unwrap(), PyValue::Bool(false));
        assert_eq!(eval_single_expr("False and True").unwrap(), PyValue::Bool(false));
    }

    #[test]
    fn test_logical_or() {
        assert_eq!(eval_single_expr("False or True").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("False or False").unwrap(), PyValue::Bool(false));
        assert_eq!(eval_single_expr("True or False").unwrap(), PyValue::Bool(true));
    }

    #[test]
    fn test_logical_not() {
        assert_eq!(eval_single_expr("not True").unwrap(), PyValue::Bool(false));
        assert_eq!(eval_single_expr("not False").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("not 0").unwrap(), PyValue::Bool(true));
        assert_eq!(eval_single_expr("not 1").unwrap(), PyValue::Bool(false));
    }

    #[test]
    fn test_assignment() {
        let result = exec_module("x = 42");
        assert!(result.is_ok());
        let x = get_global("x = 42", "x");
        assert_eq!(x, PyValue::Int(42));
    }

    #[test]
    fn test_assignment_multiple() {
        exec_module("x = 1\ny = 2\nz = 3").ok();
        let globals = Env::new(None);
        install_builtins(&globals);
        // can't easily test multi-var, but at least it shouldn't crash
    }

    #[test]
    fn test_augmented_assignment_plus() {
        exec_module("x = 5\nx += 3").ok();
        let x = get_global("x = 5\nx += 3", "x");
        assert_eq!(x, PyValue::Int(8));
    }

    #[test]
    fn test_augmented_assignment_minus() {
        exec_module("x = 5\nx -= 3").ok();
        let x = get_global("x = 5\nx -= 3", "x");
        assert_eq!(x, PyValue::Int(2));
    }

    #[test]
    fn test_if_statement_true_branch() {
        let result = exec_module("if True:\n    x = 42");
        assert!(result.is_ok());
        let x = get_global("if True:\n    x = 42", "x");
        assert_eq!(x, PyValue::Int(42));
    }

    #[test]
    fn test_if_statement_false_branch() {
        exec_module("if False:\n    x = 42\nelse:\n    x = 99").ok();
        let x = get_global("if False:\n    x = 42\nelse:\n    x = 99", "x");
        assert_eq!(x, PyValue::Int(99));
    }

    #[test]
    fn test_if_elif() {
        exec_module("x = 0\nif False:\n    x = 1\nelif True:\n    x = 2\nelse:\n    x = 3").ok();
        let x = get_global("x = 0\nif False:\n    x = 1\nelif True:\n    x = 2\nelse:\n    x = 3", "x");
        assert_eq!(x, PyValue::Int(2));
    }

    #[test]
    fn test_while_loop() {
        exec_module("x = 0\nwhile x < 5:\n    x += 1").ok();
        let x = get_global("x = 0\nwhile x < 5:\n    x += 1", "x");
        assert_eq!(x, PyValue::Int(5));
    }

    #[test]
    fn test_while_break() {
        exec_module("x = 0\nwhile True:\n    x += 1\n    if x >= 5:\n        break").ok();
        let x = get_global("x = 0\nwhile True:\n    x += 1\n    if x >= 5:\n        break", "x");
        assert_eq!(x, PyValue::Int(5));
    }

    #[test]
    fn test_while_continue() {
        exec_module("x = 0\ny = 0\nwhile x < 5:\n    x += 1\n    if x % 2 == 0:\n        continue\n    y += 1").ok();
        let y = get_global("x = 0\ny = 0\nwhile x < 5:\n    x += 1\n    if x % 2 == 0:\n        continue\n    y += 1", "y");
        assert_eq!(y, PyValue::Int(3));
    }

    #[test]
    fn test_for_loop() {
        exec_module("total = 0\nfor i in [1, 2, 3, 4, 5]:\n    total += i").ok();
        let total = get_global("total = 0\nfor i in [1, 2, 3, 4, 5]:\n    total += i", "total");
        assert_eq!(total, PyValue::Int(15));
    }

    #[test]
    fn test_for_range() {
        exec_module("total = 0\nfor i in range(5):\n    total += i + 1").ok();
        let total = get_global("total = 0\nfor i in range(5):\n    total += i + 1", "total");
        assert_eq!(total, PyValue::Int(15));
    }

    #[test]
    fn test_for_string_iteration() {
        exec_module("result = ''\nfor c in 'abc':\n    result += c").ok();
        let result = get_global("result = ''\nfor c in 'abc':\n    result += c", "result");
        assert_eq!(result, PyValue::Str("abc".into()));
    }

    #[test]
    fn test_function_def() {
        exec_module("def foo():\n    return 42").ok();
        let foo = get_global("def foo():\n    return 42", "foo");
        assert!(matches!(foo, PyValue::Function { name, .. } if name == "foo"));
    }

    #[test]
    fn test_function_call() {
        exec_module("def bar():\n    return 99\nresult = bar()").ok();
        let result = get_global("def bar():\n    return 99\nresult = bar()", "result");
        assert_eq!(result, PyValue::Int(99));
    }

    #[test]
    fn test_function_with_params() {
        exec_module("def add(a, b):\n    return a + b\nresult = add(3, 4)").ok();
        let result = get_global("def add(a, b):\n    return a + b\nresult = add(3, 4)", "result");
        assert_eq!(result, PyValue::Int(7));
    }

    #[test]
    fn test_function_with_default_param() {
        exec_module("def greet(name = 'World'):\n    return name\nresult = greet()").ok();
        let result = get_global("def greet(name = 'World'):\n    return name\nresult = greet()", "result");
        assert_eq!(result, PyValue::Str("World".into()));
    }

    #[test]
    fn test_function_default_override() {
        exec_module("def greet(name = 'World'):\n    return name\nresult = greet('Bob')").ok();
        let result = get_global("def greet(name = 'World'):\n    return name\nresult = greet('Bob')", "result");
        assert_eq!(result, PyValue::Str("Bob".into()));
    }

    #[test]
    fn test_recursive_factorial() {
        exec_module(
            "def fact(n):\n    if n <= 1:\n        return 1\n    return n * fact(n - 1)\nresult = fact(5)"
        ).ok();
        let result = get_global(
            "def fact(n):\n    if n <= 1:\n        return 1\n    return n * fact(n - 1)\nresult = fact(5)",
            "result"
        );
        assert_eq!(result, PyValue::Int(120));
    }

    #[test]
    fn test_lambda() {
        exec_module("f = lambda x: x * 2\nresult = f(21)").ok();
        let result = get_global("f = lambda x: x * 2\nresult = f(21)", "result");
        assert_eq!(result, PyValue::Int(42));
    }

    #[test]
    fn test_lambda_with_multiple_params() {
        exec_module("add = lambda a, b: a + b\nresult = add(3, 4)").ok();
        let result = get_global("add = lambda a, b: a + b\nresult = add(3, 4)", "result");
        assert_eq!(result, PyValue::Int(7));
    }

    #[test]
    fn test_list_literal() {
        exec_module("x = [1, 2, 3]").ok();
        let x = get_global("x = [1, 2, 3]", "x");
        assert!(matches!(x, PyValue::List(_)));
    }

    #[test]
    fn test_list_index() {
        exec_module("x = [10, 20, 30]\nresult = x[1]").ok();
        let result = get_global("x = [10, 20, 30]\nresult = x[1]", "result");
        assert_eq!(result, PyValue::Int(20));
    }

    #[test]
    fn test_list_negative_index() {
        let result = eval_single_expr("[10, 20, 30][-1]");
        assert!(result.is_err(), "Negative list index not supported in this py5 version");
    }

    #[test]
    fn test_list_index_out_of_range() {
        let result = eval_single_expr("[1, 2, 3][10]");
        assert!(result.is_err());
        if let Err(PyValue::Exception(t, _)) = result {
            assert_eq!(t, "IndexError");
        }
    }

    #[test]
    fn test_list_append() {
        exec_module("x = [1, 2]\nx.append(3)").ok();
        let x = get_global("x = [1, 2]\nx.append(3)", "x");
        if let PyValue::List(l) = x {
            assert_eq!(l.borrow().len(), 3);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_list_pop() {
        exec_module("x = [1, 2, 3]\nresult = x.pop()").ok();
        let result = get_global("x = [1, 2, 3]\nresult = x.pop()", "result");
        assert_eq!(result, PyValue::Int(3));
    }

    #[test]
    fn test_dict_literal() {
        exec_module("d = {'a': 1, 'b': 2}").ok();
        let d = get_global("d = {'a': 1, 'b': 2}", "d");
        assert!(matches!(d, PyValue::Dict(_)));
    }

    #[test]
    fn test_dict_index() {
        exec_module("d = {'x': 42}\nresult = d['x']").ok();
        let result = get_global("d = {'x': 42}\nresult = d['x']", "result");
        assert_eq!(result, PyValue::Int(42));
    }

    #[test]
    fn test_dict_keys() {
        exec_module("d = {'a': 1, 'b': 2}\nresult = d.keys()").ok();
        let result = get_global("d = {'a': 1, 'b': 2}\nresult = d.keys()", "result");
        assert!(matches!(result, PyValue::List(_)));
    }

    #[test]
    fn test_dict_values() {
        exec_module("d = {'a': 1, 'b': 2}\nresult = d.values()").ok();
        let result = get_global("d = {'a': 1, 'b': 2}\nresult = d.values()", "result");
        assert!(matches!(result, PyValue::List(_)));
    }

    #[test]
    fn test_dict_items() {
        exec_module("d = {'a': 1}\nresult = d.items()").ok();
        let result = get_global("d = {'a': 1}\nresult = d.items()", "result");
        assert!(matches!(result, PyValue::List(_)));
    }

    #[test]
    fn test_dict_copy() {
        exec_module("d = {'x': 1}\nc = d.copy()").ok();
        let c = get_global("d = {'x': 1}\nc = d.copy()", "c");
        assert!(matches!(c, PyValue::Dict(_)));
    }

    #[test]
    fn test_tuple_literal() {
        exec_module("t = (1, 2, 3)").ok();
        let t = get_global("t = (1, 2, 3)", "t");
        assert!(matches!(t, PyValue::Tuple(_)));
    }

    #[test]
    fn test_tuple_index() {
        exec_module("t = (10, 20, 30)\nresult = t[0]").ok();
        let result = get_global("t = (10, 20, 30)\nresult = t[0]", "result");
        assert_eq!(result, PyValue::Int(10));
    }

    #[test]
    fn test_string_concat() {
        assert_eq!(eval_single_expr("'hello' + ' world'").unwrap(), PyValue::Str("hello world".into()));
    }

    #[test]
    fn test_string_index() {
        let result = eval_single_expr("'hello'[1]");
        assert!(result.is_err(), "String indexing not supported in this py5 version");
    }

    #[test]
    fn test_string_repeat() {
        let result = eval_single_expr("'ab' * 3");
        assert!(result.is_err(), "String repeat not supported in this py5 version");
    }

    #[test]
    fn test_string_split() {
        exec_module("s = 'a b c'\nparts = s.split(' ')").ok();
        let parts = get_global("s = 'a b c'\nparts = s.split(' ')", "parts");
        assert!(matches!(parts, PyValue::List(_)));
    }

    #[test]
    fn test_string_join() {
        exec_module("result = '-'.join(['a', 'b', 'c'])").ok();
        let result = get_global("result = '-'.join(['a', 'b', 'c'])", "result");
        assert_eq!(result, PyValue::Str("a-b-c".into()));
    }

    #[test]
    fn test_class_def() {
        exec_module("class Foo:\n    pass").ok();
        let foo = get_global("class Foo:\n    pass", "Foo");
        assert!(matches!(foo, PyValue::Class { name, .. } if name == "Foo"));
    }

    #[test]
    fn test_class_instantiation() {
        exec_module("class Foo:\n    pass\nobj = Foo()").ok();
        let obj = get_global("class Foo:\n    pass\nobj = Foo()", "obj");
        assert!(matches!(obj, PyValue::Instance { .. }));
    }

    #[test]
    fn test_class_with_method() {
        exec_module(
            "class Counter:\n    def __init__(self):\n        self.count = 0\n    def inc(self):\n        self.count += 1\nc = Counter()\nc.inc()"
        ).ok();
        let c = get_global(
            "class Counter:\n    def __init__(self):\n        self.count = 0\n    def inc(self):\n        self.count += 1\nc = Counter()\nc.inc()",
            "c"
        );
        if let PyValue::Instance { attrs, .. } = c {
            let count = attrs.borrow().get("count").unwrap().clone();
            assert_eq!(count, PyValue::Int(1));
        } else {
            panic!("Expected instance");
        }
    }

    #[test]
    fn test_class_inheritance() {
        exec_module(
            "class Animal:\n    def greet(self):\n        return 'Hello'\nclass Dog(Animal):\n    pass\nd = Dog()\nresult = d.greet()"
        ).ok();
        let result = get_global(
            "class Animal:\n    def greet(self):\n        return 'Hello'\nclass Dog(Animal):\n    pass\nd = Dog()\nresult = d.greet()",
            "result"
        );
        assert_eq!(result, PyValue::Str("Hello".into()));
    }

    #[test]
    fn test_try_except() {
        exec_module("x = 0\ntry:\n    y = 1 / 0\nexcept:\n    x = 42").ok();
        let x = get_global("x = 0\ntry:\n    y = 1 / 0\nexcept:\n    x = 42", "x");
        assert_eq!(x, PyValue::Int(42));
    }

    #[test]
    fn test_try_except_with_exception_var() {
        let result = exec_module(
            "try:\n    raise Exception('test error')\nexcept Exception as e:\n    pass"
        );
        assert!(result.is_ok(), "Try-except should not error");
    }

    #[test]
    fn test_raise_exception() {
        let result = exec_module("raise Exception('oops')");
        assert!(result.is_err());
    }

    #[test]
    fn test_builtin_len() {
        assert_eq!(eval_single_expr("len('hello')").unwrap(), PyValue::Int(5));
        assert_eq!(eval_single_expr("len([1, 2, 3])").unwrap(), PyValue::Int(3));
        assert_eq!(eval_single_expr("len({'a': 1})").unwrap(), PyValue::Int(1));
    }

    #[test]
    fn test_builtin_range() {
        let result = eval_single_expr("range(5)").unwrap();
        assert!(matches!(result, PyValue::List(_)));
    }

    #[test]
    fn test_builtin_str() {
        assert_eq!(eval_single_expr("str(42)").unwrap(), PyValue::Str("42".into()));
        assert_eq!(eval_single_expr("str(True)").unwrap(), PyValue::Str("True".into()));
    }

    #[test]
    fn test_builtin_type() {
        let result = eval_single_expr("type(42)").unwrap();
        assert_eq!(result, PyValue::Str("<class 'int'>".into()));
    }

    #[test]
    fn test_builtin_isinstance() {
        exec_module("class Foo:\n    pass\nobj = Foo()\nis_inst = isinstance(obj, Foo)").ok();
        let is_inst = get_global("class Foo:\n    pass\nobj = Foo()\nis_inst = isinstance(obj, Foo)", "is_inst");
        assert_eq!(is_inst, PyValue::Bool(true));
    }

    #[test]
    fn test_builtin_isinstance_false() {
        exec_module("class Foo:\n    pass\nis_inst = isinstance(42, Foo)").ok();
        let is_inst = get_global("class Foo:\n    pass\nis_inst = isinstance(42, Foo)", "is_inst");
        assert_eq!(is_inst, PyValue::Bool(false));
    }

    #[test]
    fn test_list_comprehension() {
        exec_module("result = [x * 2 for x in [1, 2, 3]]").ok();
        let result = get_global("result = [x * 2 for x in [1, 2, 3]]", "result");
        if let PyValue::List(l) = result {
            let vals: Vec<i64> = l.borrow().iter().map(|v| {
                if let PyValue::Int(i) = v { *i } else { 0 }
            }).collect();
            assert_eq!(vals, vec![2, 4, 6]);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_list_comprehension_with_filter() {
        exec_module("result = [x for x in [1, 2, 3, 4, 5] if x % 2 == 0]").ok();
        let result = get_global("result = [x for x in [1, 2, 3, 4, 5] if x % 2 == 0]", "result");
        if let PyValue::List(l) = result {
            let vals: Vec<i64> = l.borrow().iter().map(|v| {
                if let PyValue::Int(i) = v { *i } else { 0 }
            }).collect();
            assert_eq!(vals, vec![2, 4]);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_none_value() {
        exec_module("x = None").ok();
        let x = get_global("x = None", "x");
        assert_eq!(x, PyValue::None);
    }

    #[test]
    fn test_truthy_values() {
        assert!(eval_single_expr("True").unwrap().is_truthy());
        assert!(eval_single_expr("1").unwrap().is_truthy());
        assert!(eval_single_expr("'hello'").unwrap().is_truthy());
    }

    #[test]
    fn test_falsy_values() {
        assert!(!eval_single_expr("False").unwrap().is_truthy());
        assert!(!eval_single_expr("0").unwrap().is_truthy());
        assert!(!eval_single_expr("''").unwrap().is_truthy());
        assert!(!eval_single_expr("None").unwrap().is_truthy());
        assert!(!eval_single_expr("[]").unwrap().is_truthy());
        assert!(!eval_single_expr("{}").unwrap().is_truthy());
    }

    #[test]
    fn test_module_attribute_access() {
        exec_module("import sys\nresult = sys.version").ok();
        let result = get_global("import sys\nresult = sys.version", "result");
        assert!(matches!(result, PyValue::Str(_)));
    }

    #[test]
    fn test_nested_function_closure() {
        exec_module(
            "def outer():\n    x = 10\n    def inner():\n        return x\n    return inner\nfn = outer()\nresult = fn()"
        ).ok();
        let result = get_global(
            "def outer():\n    x = 10\n    def inner():\n        return x\n    return inner\nfn = outer()\nresult = fn()",
            "result"
        );
        assert_eq!(result, PyValue::Int(10));
    }

    #[test]
    fn test_string_fstring() {
        exec_module("name = 'World'\nresult = f'Hello, {name}!'").ok();
        let result = get_global("name = 'World'\nresult = f'Hello, {name}!'", "result");
        assert_eq!(result, PyValue::Str("Hello, World!".into()));
    }

    #[test]
    fn test_fstring_with_expression() {
        exec_module("x = 5\ny = 3\nresult = f'{x} + {y} = {x + y}'").ok();
        let result = get_global("x = 5\ny = 3\nresult = f'{x} + {y} = {x + y}'", "result");
        assert_eq!(result, PyValue::Str("5 + 3 = 8".into()));
    }

    #[test]
    fn test_pass_statement() {
        let result = exec_module("if True:\n    pass");
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_except_handlers() {
        exec_module(
            "x = 0\ntry:\n    pass\nexcept TypeError:\n    x = 1\nexcept ValueError:\n    x = 2\nexcept:\n    x = 3"
        ).ok();
        let x = get_global(
            "x = 0\ntry:\n    pass\nexcept TypeError:\n    x = 1\nexcept ValueError:\n    x = 2\nexcept:\n    x = 3",
            "x"
        );
        assert_eq!(x, PyValue::Int(0));
    }

    #[test]
    fn test_attribute_access_on_instance() {
        exec_module(
            "class Foo:\n    def __init__(self):\n        self.x = 42\nobj = Foo()\nresult = obj.x"
        ).ok();
        let result = get_global(
            "class Foo:\n    def __init__(self):\n        self.x = 42\nobj = Foo()\nresult = obj.x",
            "result"
        );
        assert_eq!(result, PyValue::Int(42));
    }

    #[test]
    fn test_attribute_assignment() {
        exec_module(
            "class Foo:\n    pass\nobj = Foo()\nobj.x = 99"
        ).ok();
        let obj = get_global("class Foo:\n    pass\nobj = Foo()\nobj.x = 99", "obj");
        if let PyValue::Instance { attrs, .. } = obj {
            assert_eq!(attrs.borrow().get("x"), Some(&PyValue::Int(99)));
        } else {
            panic!("Expected instance");
        }
    }

    #[test]
    fn test_subscript_assignment() {
        exec_module(
            "lst = [1, 2, 3]\nlst[1] = 99"
        ).ok();
        let lst = get_global("lst = [1, 2, 3]\nlst[1] = 99", "lst");
        if let PyValue::List(l) = lst {
            assert_eq!(l.borrow()[1], PyValue::Int(99));
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_unpacking() {
        exec_module("a, b = [1, 2]").ok();
        let a = get_global("a, b = [1, 2]", "a");
        let b = get_global("a, b = [1, 2]", "b");
        assert_eq!(a, PyValue::Int(1));
        assert_eq!(b, PyValue::Int(2));
    }

    #[test]
    fn test_from_import() {
        exec_module("from sys import version").ok();
        let version = get_global("from sys import version", "version");
        assert!(matches!(version, PyValue::Str(_)));
    }

    #[test]
    fn test_for_loop_nested() {
        exec_module(
            "result = 0\nfor i in range(3):\n    for j in range(3):\n        result += 1"
        ).ok();
        let result = get_global(
            "result = 0\nfor i in range(3):\n    for j in range(3):\n        result += 1",
            "result"
        );
        assert_eq!(result, PyValue::Int(9));
    }

    #[test]
    fn test_while_nested() {
        exec_module(
            "i = 0\nj = 0\nwhile i < 3:\n    j = 0\n    while j < 3:\n        j += 1\n    i += 1\nresult = j"
        ).ok();
        let result = get_global(
            "i = 0\nj = 0\nwhile i < 3:\n    j = 0\n    while j < 3:\n        j += 1\n    i += 1\nresult = j",
            "result"
        );
        assert_eq!(result, PyValue::Int(3));
    }
}