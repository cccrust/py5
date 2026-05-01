use crate::ast::Stmt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;

use std::rc::Rc;

#[derive(Clone)]
pub(crate) enum PyValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Tuple(Vec<PyValue>),
    List(Rc<RefCell<Vec<PyValue>>>),
    Dict(Rc<RefCell<HashMap<String, PyValue>>>),
    Function {
        name: String,
        params: Vec<String>,
        defaults: HashMap<String, PyValue>,
        vararg: Option<String>,
        kwarg: Option<String>,
        body: Rc<Vec<Stmt>>,
        closure: Rc<RefCell<Env>>,
    },
    Builtin(
        String,
        Rc<
            dyn Fn(
                &mut crate::eval::Runtime,
                Vec<PyValue>,
                HashMap<String, PyValue>,
            ) -> Result<PyValue, PyValue>,
        >,
    ),
    Method(Box<PyValue>, String),
    Class {
        name: String,
        base: Option<Box<PyValue>>,
        methods: Rc<HashMap<String, PyValue>>,
    },
    Instance {
        class_val: Box<PyValue>,
        attrs: Rc<RefCell<HashMap<String, PyValue>>>,
    },
    BoundMethod {
        receiver: Box<PyValue>,
        func: Box<PyValue>,
    },
    Exception(String, Box<PyValue>),
    Module(String, Rc<RefCell<Env>>),
    File(Rc<RefCell<Option<File>>>),
}

impl PartialEq for PyValue {
    fn eq(&self, o: &Self) -> bool {
        match (self, o) {
            (PyValue::None, PyValue::None) => true,
            (PyValue::Bool(a), PyValue::Bool(b)) => a == b,
            (PyValue::Int(a), PyValue::Int(b)) => a == b,
            (PyValue::Float(a), PyValue::Float(b)) => a == b,
            (PyValue::Str(a), PyValue::Str(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for PyValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PyValue::None => write!(f, "None"),
            PyValue::Bool(b) => write!(f, "{}", if *b { "True" } else { "False" }),
            PyValue::Int(i) => write!(f, "{}", i),
            PyValue::Float(n) => write!(f, "{}", n),
            PyValue::Str(s) => write!(f, "{}", s),
            PyValue::Tuple(t) => {
                let items: Vec<String> = t
                    .iter()
                    .map(|v| match v {
                        PyValue::Str(s) => format!("'{}'", s),
                        _ => v.to_string(),
                    })
                    .collect();
                if items.len() == 1 {
                    write!(f, "({},)", items[0])
                } else {
                    write!(f, "({})", items.join(", "))
                }
            }
            PyValue::List(l) => {
                let items: Vec<String> = l
                    .borrow()
                    .iter()
                    .map(|v| match v {
                        PyValue::Str(s) => format!("'{}'", s),
                        _ => v.to_string(),
                    })
                    .collect();
                write!(f, "[{}]", items.join(", "))
            }
            PyValue::Dict(d) => {
                let items: Vec<String> = d
                    .borrow()
                    .iter()
                    .map(|(k, v)| format!("'{}': {}", k, v))
                    .collect();
                write!(f, "{{{}}}", items.join(", "))
            }
            PyValue::Class { name, .. } => write!(f, "<class '{}'>", name),
            PyValue::Instance { class_val, .. } => {
                if let PyValue::Class { name, .. } = &**class_val {
                    write!(f, "<{} object>", name)
                } else {
                    write!(f, "<object>")
                }
            }
            PyValue::BoundMethod { .. } => write!(f, "<bound method>"),
            PyValue::Exception(t, a) => write!(f, "{}({})", t, a),
            PyValue::Builtin(name, _) => write!(f, "<built-in function {}>", name),
            PyValue::Function { name, .. } => write!(f, "<function {}>", name),
            PyValue::Module(name, _) => write!(f, "<module '{}'>", name),
            PyValue::File(file) => {
                if file.borrow().is_some() {
                    write!(f, "<open file>")
                } else {
                    write!(f, "<closed file>")
                }
            }
            PyValue::Method(_, name) => write!(f, "<built-in method {}>", name),
        }
    }
}

pub(crate) fn py_err<T>(typ: &str, msg: &str) -> Result<T, PyValue> {
    Err(PyValue::Exception(
        typ.to_string(),
        Box::new(PyValue::Str(msg.to_string())),
    ))
}
pub(crate) fn py_err_val(typ: &str, msg: &str) -> PyValue {
    PyValue::Exception(typ.to_string(), Box::new(PyValue::Str(msg.to_string())))
}
pub(crate) fn get_class_method(class_val: &PyValue, method_name: &str) -> Option<PyValue> {
    if let PyValue::Class { methods, base, .. } = class_val {
        if let Some(m) = methods.get(method_name) {
            return Some(m.clone());
        }
        if let Some(b) = base {
            return get_class_method(b, method_name);
        }
    }
    None
}
pub(crate) fn py_to_string(rt: &mut crate::eval::Runtime, val: PyValue) -> Result<String, PyValue> {
    if let PyValue::Instance { class_val, .. } = &val {
        if let Some(m) = get_class_method(class_val, "__str__") {
            let bound = PyValue::BoundMethod {
                receiver: Box::new(val.clone()),
                func: Box::new(m),
            };
            let res = crate::eval::call_func(rt, bound, vec![], HashMap::new())?;
            if let PyValue::Str(s) = res {
                return Ok(s);
            }
        }
    }
    Ok(val.to_string())
}

impl PyValue {
    pub(crate) fn is_truthy(&self) -> bool {
        match self {
            PyValue::None => false,
            PyValue::Bool(b) => *b,
            PyValue::Int(i) => *i != 0,
            PyValue::Float(f) => *f != 0.0,
            PyValue::Str(s) => !s.is_empty(),
            PyValue::Tuple(t) => !t.is_empty(),
            PyValue::List(l) => !l.borrow().is_empty(),
            PyValue::Dict(d) => !d.borrow().is_empty(),
            _ => true,
        }
    }
    pub(crate) fn as_num(&self) -> Result<f64, PyValue> {
        match self {
            PyValue::Int(i) => Ok(*i as f64),
            PyValue::Float(f) => Ok(*f),
            PyValue::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            _ => py_err("TypeError", "expected number"),
        }
    }
    pub(crate) fn as_key(&self) -> Result<String, PyValue> {
        match self {
            PyValue::Str(s) => Ok(s.clone()),
            PyValue::Int(i) => Ok(i.to_string()),
            _ => py_err("TypeError", "unhashable type"),
        }
    }
}

pub(crate) struct Env {
    parent: Option<Rc<RefCell<Env>>>,
    pub(crate) vars: HashMap<String, PyValue>,
}
impl Env {
    pub(crate) fn new(parent: Option<Rc<RefCell<Env>>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Env {
            parent,
            vars: HashMap::new(),
        }))
    }
    pub(crate) fn set(&mut self, n: &str, v: PyValue) {
        self.vars.insert(n.to_string(), v);
    }
    pub(crate) fn assign(&mut self, n: &str, v: PyValue) {
        if self.vars.contains_key(n) {
            self.vars.insert(n.to_string(), v);
            return;
        }
        if let Some(p) = &self.parent {
            if p.borrow().get_opt(n).is_some() {
                p.borrow_mut().assign(n, v);
                return;
            }
        }
        self.vars.insert(n.to_string(), v);
    }
    pub(crate) fn get_opt(&self, n: &str) -> Option<PyValue> {
        if let Some(v) = self.vars.get(n) {
            Some(v.clone())
        } else if let Some(p) = &self.parent {
            p.borrow().get_opt(n)
        } else {
            None
        }
    }
    pub(crate) fn get(&self, n: &str) -> Result<PyValue, PyValue> {
        self.get_opt(n)
            .ok_or_else(|| py_err_val("NameError", &format!("name '{}' is not defined", n)))
    }
}