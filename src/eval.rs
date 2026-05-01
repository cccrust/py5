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