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

impl fmt::Debug for PyValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PyValue::None => write!(f, "PyValue::None"),
            PyValue::Bool(b) => write!(f, "PyValue::Bool({})", b),
            PyValue::Int(i) => write!(f, "PyValue::Int({})", i),
            PyValue::Float(fl) => write!(f, "PyValue::Float({})", fl),
            PyValue::Str(s) => write!(f, "PyValue::Str({:?})", s),
            PyValue::Tuple(items) => {
                write!(f, "PyValue::Tuple(len={})", items.len())
            }
            PyValue::List(l) => {
                write!(f, "PyValue::List(len={})", l.borrow().len())
            }
            PyValue::Dict(d) => {
                write!(f, "PyValue::Dict(len={})", d.borrow().len())
            }
            PyValue::Function { name, .. } => write!(f, "PyValue::Function({:?})", name),
            PyValue::Builtin(name, _) => write!(f, "PyValue::Builtin({:?})", name),
            PyValue::Method(_, name) => write!(f, "PyValue::Method(..., {:?})", name),
            PyValue::Class { name, .. } => write!(f, "PyValue::Class({:?})", name),
            PyValue::Instance { .. } => write!(f, "PyValue::Instance(...)"),
            PyValue::BoundMethod { .. } => write!(f, "PyValue::BoundMethod(...)"),
            PyValue::Exception(t, _) => write!(f, "PyValue::Exception({:?}, ...)", t),
            PyValue::Module(name, _) => write!(f, "PyValue::Module({:?})", name),
            PyValue::File(_) => write!(f, "PyValue::File(...)"),
        }
    }
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

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let keys: Vec<&String> = self.vars.keys().collect();
        write!(f, "Env {{ vars: {:?}, parent: {:?} }}", keys, self.parent.is_some())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn env_with_vars(vars: Vec<(&str, PyValue)>) -> Rc<RefCell<Env>> {
        let env = Env::new(None);
        for (name, val) in vars {
            env.borrow_mut().set(name, val);
        }
        env
    }

    #[test]
    fn test_py_value_partial_eq() {
        assert_eq!(PyValue::None, PyValue::None);
        assert_eq!(PyValue::Bool(true), PyValue::Bool(true));
        assert_eq!(PyValue::Bool(false), PyValue::Bool(false));
        assert_eq!(PyValue::Int(42), PyValue::Int(42));
        assert_eq!(PyValue::Float(3.14), PyValue::Float(3.14));
        assert_eq!(PyValue::Str("hello".into()), PyValue::Str("hello".into()));
    }

    #[test]
    fn test_py_value_partial_eq_different_types() {
        assert_ne!(PyValue::Int(42), PyValue::Int(43));
        assert_ne!(PyValue::Int(42), PyValue::Float(42.0));
        assert_ne!(PyValue::Str("42".into()), PyValue::Int(42));
    }

    #[test]
    fn test_py_value_display() {
        assert_eq!(PyValue::None.to_string(), "None");
        assert_eq!(PyValue::Bool(true).to_string(), "True");
        assert_eq!(PyValue::Bool(false).to_string(), "False");
        assert_eq!(PyValue::Int(42).to_string(), "42");
        assert_eq!(PyValue::Float(3.14).to_string(), "3.14");
        assert_eq!(PyValue::Str("hello".into()).to_string(), "hello");
    }

    #[test]
    fn test_py_value_display_list() {
        let list = PyValue::List(Rc::new(RefCell::new(vec![
            PyValue::Int(1),
            PyValue::Int(2),
            PyValue::Str("three".into()),
        ])));
        assert_eq!(list.to_string(), "[1, 2, 'three']");
    }

    #[test]
    fn test_py_value_display_dict() {
        let dict = PyValue::Dict(Rc::new(RefCell::new(HashMap::from([
            ("a".to_string(), PyValue::Int(1)),
            ("b".to_string(), PyValue::Int(2)),
        ]))));
        let result = dict.to_string();
        assert!(result.contains("'a': 1") && result.contains("'b': 2"), "Dict display should contain both entries, got: {}", result);
    }

    #[test]
    fn test_py_value_display_tuple() {
        assert_eq!(PyValue::Tuple(vec![PyValue::Int(1)]).to_string(), "(1,)");
        assert_eq!(PyValue::Tuple(vec![PyValue::Int(1), PyValue::Int(2)]).to_string(), "(1, 2)");
    }

    #[test]
    fn test_py_value_is_truthy() {
        assert!(!PyValue::None.is_truthy());
        assert!(PyValue::Bool(true).is_truthy());
        assert!(!PyValue::Bool(false).is_truthy());
        assert!(PyValue::Int(1).is_truthy());
        assert!(!PyValue::Int(0).is_truthy());
        assert!(PyValue::Float(1.0).is_truthy());
        assert!(!PyValue::Float(0.0).is_truthy());
        assert!(PyValue::Str("hello".into()).is_truthy());
        assert!(!PyValue::Str("".into()).is_truthy());
    }

    #[test]
    fn test_py_value_is_truthy_tuple() {
        assert!(!PyValue::Tuple(vec![]).is_truthy());
        assert!(PyValue::Tuple(vec![PyValue::Int(1)]).is_truthy());
    }

    #[test]
    fn test_py_value_is_truthy_list() {
        assert!(!PyValue::List(Rc::new(RefCell::new(vec![]))).is_truthy());
        assert!(PyValue::List(Rc::new(RefCell::new(vec![PyValue::Int(1)]))).is_truthy());
    }

    #[test]
    fn test_py_value_is_truthy_dict() {
        assert!(!PyValue::Dict(Rc::new(RefCell::new(HashMap::new()))).is_truthy());
        let mut m = HashMap::new();
        m.insert("a".to_string(), PyValue::Int(1));
        assert!(PyValue::Dict(Rc::new(RefCell::new(m))).is_truthy());
    }

    #[test]
    fn test_py_value_as_num() {
        assert_eq!(PyValue::Int(42).as_num().unwrap(), 42.0);
        assert_eq!(PyValue::Float(3.14).as_num().unwrap(), 3.14);
        assert_eq!(PyValue::Bool(true).as_num().unwrap(), 1.0);
        assert_eq!(PyValue::Bool(false).as_num().unwrap(), 0.0);
    }

    #[test]
    fn test_py_value_as_num_error() {
        let result = PyValue::Str("hello".into()).as_num();
        assert!(result.is_err());
    }

    #[test]
    fn test_py_value_as_key() {
        assert_eq!(PyValue::Str("hello".into()).as_key().unwrap(), "hello");
        assert_eq!(PyValue::Int(42).as_key().unwrap(), "42");
    }

    #[test]
    fn test_py_value_as_key_error() {
        let result = PyValue::List(Rc::new(RefCell::new(vec![]))).as_key();
        assert!(result.is_err());
    }

    #[test]
    fn test_env_new() {
        let env = Env::new(None);
        assert!(env.borrow().vars.is_empty());
        assert!(env.borrow().parent.is_none());
    }

    #[test]
    fn test_env_new_with_parent() {
        let parent = Env::new(None);
        let child = Env::new(Some(parent.clone()));
        assert!(child.borrow().parent.is_some());
    }

    #[test]
    fn test_env_set_and_get() {
        let env = env_with_vars(vec![
            ("x", PyValue::Int(42)),
            ("name", PyValue::Str("Alice".into())),
        ]);
        assert_eq!(env.borrow().get("x").unwrap(), PyValue::Int(42));
        assert_eq!(env.borrow().get("name").unwrap(), PyValue::Str("Alice".into()));
    }

    #[test]
    fn test_env_get_missing() {
        let env = env_with_vars(vec![]);
        let result = env.borrow().get("undefined");
        assert!(result.is_err());
        if let Err(PyValue::Exception(t, _)) = result {
            assert_eq!(t, "NameError");
        }
    }

    #[test]
    fn test_env_get_opt() {
        let env = env_with_vars(vec![("x", PyValue::Int(1))]);
        assert_eq!(env.borrow().get_opt("x"), Some(PyValue::Int(1)));
        assert_eq!(env.borrow().get_opt("y"), None);
    }

    #[test]
    fn test_env_assign() {
        let env = env_with_vars(vec![("x", PyValue::Int(1))]);
        env.borrow_mut().assign("x", PyValue::Int(2));
        assert_eq!(env.borrow().get("x").unwrap(), PyValue::Int(2));
    }

    #[test]
    fn test_env_assign_new_var() {
        let env = env_with_vars(vec![]);
        env.borrow_mut().assign("new_var", PyValue::Int(99));
        assert_eq!(env.borrow().get("new_var").unwrap(), PyValue::Int(99));
    }

    #[test]
    fn test_env_parent_lookup() {
        let parent = env_with_vars(vec![("parent_var", PyValue::Int(100))]);
        let child = Env::new(Some(parent.clone()));
        assert_eq!(child.borrow().get("parent_var").unwrap(), PyValue::Int(100));
    }

    #[test]
    fn test_env_child_overrides_parent() {
        let parent = env_with_vars(vec![("x", PyValue::Int(1))]);
        let child = Env::new(Some(parent));
        child.borrow_mut().set("x", PyValue::Int(2));
        assert_eq!(child.borrow().get("x").unwrap(), PyValue::Int(2));
    }

    #[test]
    fn test_env_assign_searches_parent() {
        let parent = env_with_vars(vec![("x", PyValue::Int(1))]);
        let child = Env::new(Some(parent.clone()));
        child.borrow_mut().assign("x", PyValue::Int(2));
        assert_eq!(child.borrow().get("x").unwrap(), PyValue::Int(2));
        assert_eq!(parent.borrow().get("x").unwrap(), PyValue::Int(2));
    }

    #[test]
    fn test_py_err() {
        let result: Result<i32, PyValue> = py_err("ValueError", "test message");
        assert!(result.is_err());
        if let Err(PyValue::Exception(t, msg)) = result {
            assert_eq!(t, "ValueError");
            if let PyValue::Str(s) = *msg {
                assert_eq!(s, "test message");
            }
        }
    }

    #[test]
    fn test_py_err_val() {
        let err = py_err_val("TypeError", "invalid type");
        assert!(matches!(err, PyValue::Exception(_, _)));
        if let PyValue::Exception(t, msg) = err {
            assert_eq!(t, "TypeError");
            if let PyValue::Str(s) = *msg {
                assert_eq!(s, "invalid type");
            }
        }
    }

    #[test]
    fn test_get_class_method() {
        let mut methods = HashMap::new();
        methods.insert("greet".into(), PyValue::Str("Hello".into()));
        let class = PyValue::Class {
            name: "Person".into(),
            base: None,
            methods: Rc::new(methods),
        };
        let method = get_class_method(&class, "greet");
        assert!(method.is_some());
    }

    #[test]
    fn test_get_class_method_missing() {
        let class = PyValue::Class {
            name: "Foo".into(),
            base: None,
            methods: Rc::new(HashMap::new()),
        };
        let method = get_class_method(&class, "missing");
        assert!(method.is_none());
    }

    #[test]
    fn test_get_class_method_inherited() {
        let mut base_methods = HashMap::new();
        base_methods.insert("greet".into(), PyValue::Str("Hello".into()));
        let base = PyValue::Class {
            name: "Base".into(),
            base: None,
            methods: Rc::new(base_methods),
        };
        let child = PyValue::Class {
            name: "Child".into(),
            base: Some(Box::new(base)),
            methods: Rc::new(HashMap::new()),
        };
        let method = get_class_method(&child, "greet");
        assert!(method.is_some());
    }

    #[test]
    fn test_class_display() {
        let class = PyValue::Class {
            name: "MyClass".into(),
            base: None,
            methods: Rc::new(HashMap::new()),
        };
        assert_eq!(class.to_string(), "<class 'MyClass'>");
    }

    #[test]
    fn test_instance_display() {
        let class = PyValue::Class {
            name: "MyClass".into(),
            base: None,
            methods: Rc::new(HashMap::new()),
        };
        let instance = PyValue::Instance {
            class_val: Box::new(class),
            attrs: Rc::new(RefCell::new(HashMap::new())),
        };
        assert_eq!(instance.to_string(), "<MyClass object>");
    }

    #[test]
    fn test_function_display() {
        let func = PyValue::Function {
            name: "my_func".into(),
            params: vec![],
            defaults: HashMap::new(),
            vararg: None,
            kwarg: None,
            body: Rc::new(vec![]),
            closure: Env::new(None),
        };
        assert_eq!(func.to_string(), "<function my_func>");
    }

    #[test]
    fn test_module_display() {
        let module = PyValue::Module(
            "my_module".into(),
            Env::new(None),
        );
        assert_eq!(module.to_string(), "<module 'my_module'>");
    }

    #[test]
    fn test_builtin_display() {
        let builtin = PyValue::Builtin(
            "my_builtin".into(),
            Rc::new(|_, _, _| Ok(PyValue::None)),
        );
        assert_eq!(builtin.to_string(), "<built-in function my_builtin>");
    }

    #[test]
    fn test_exception_display() {
        let exc = PyValue::Exception(
            "ValueError".into(),
            Box::new(PyValue::Str("test".into())),
        );
        assert_eq!(exc.to_string(), "ValueError(test)");
    }

    #[test]
    fn test_bound_method_display() {
        let bm = PyValue::BoundMethod {
            receiver: Box::new(PyValue::None),
            func: Box::new(PyValue::None),
        };
        assert_eq!(bm.to_string(), "<bound method>");
    }
}