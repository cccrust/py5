#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Neg,
    Not,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub(crate) enum Expr {
    NoneVal,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    FString(String),
    Name(String),
    List(Vec<Expr>),
    Dict(Vec<(Expr, Expr)>),
    Tuple(Vec<Expr>),
    ListComp(Box<Expr>, Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Lambda(Vec<String>, Box<Expr>),
    BinOp(Op, Box<Expr>, Box<Expr>),
    UnaryOp(Op, Box<Expr>),
    Compare(Op, Box<Expr>, Box<Expr>),
    Logical(LogicOp, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>, Vec<(String, Expr)>),
    Attribute(Box<Expr>, String),
    Subscript(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Expr(Expr),
    Assign(Expr, Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    For(Expr, Expr, Vec<Stmt>),
    FunctionDef(
        String,
        Vec<(String, Option<Expr>)>,
        Option<String>,
        Option<String>,
        Vec<Stmt>,
    ),
    ClassDef(String, Option<Expr>, Vec<Stmt>),
    Try(Vec<Stmt>, Vec<(Vec<String>, Option<String>, Vec<Stmt>)>),
    Raise(Expr),
    Import(String),
    FromImport(String, Vec<String>, usize),
    Return(Option<Expr>),
    Break,
    Continue,
    Pass,
}