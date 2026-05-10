use std::fmt;

/// Posição no código-fonte Lua original (início do token).
/// Usado exclusivamente para mensagens de erro — nunca em runtime AVR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub col:  u32,
}

// ── Operadores ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp { Neg, Not }

// ── Expressões ────────────────────────────────────────────────────────────────

/// Nó de expressão alocado na `Arena<Expr>`.
/// `'a` é o tempo de vida da arena que detém os filhos.
#[derive(Debug)]
pub enum Expr<'a> {
    LitInt(i32, Span),
    /// Emitido pelo parser; análise semântica rejeita em target AVR.
    LitFloat(f32, Span),
    /// Apenas para argumentos de intrínsecas (ex: "OUTPUT").
    LitStr(String, Span),
    LitBool(bool, Span),
    Nil(Span),
    Var(String, Span),
    BinOp {
        op:   BinOp,
        lhs:  &'a Expr<'a>,
        rhs:  &'a Expr<'a>,
        span: Span,
    },
    UnOp {
        op:      UnOp,
        operand: &'a Expr<'a>,
        span:    Span,
    },
    Call {
        name: String,
        args: Vec<&'a Expr<'a>>,
        span: Span,
    },
}

impl<'a> Expr<'a> {
    pub fn span(&self) -> Span {
        match self {
            Expr::LitInt(_, s) | Expr::LitFloat(_, s) | Expr::LitStr(_, s)
            | Expr::LitBool(_, s) | Expr::Nil(s) | Expr::Var(_, s) => *s,
            Expr::BinOp { span, .. }
            | Expr::UnOp { span, .. }
            | Expr::Call { span, .. } => *span,
        }
    }
}

// ── Instruções ────────────────────────────────────────────────────────────────

/// Cláusula `elseif` de um `Stmt::If`.
#[derive(Debug)]
pub struct ElseIf<'a> {
    pub cond: &'a Expr<'a>,
    pub body: Vec<&'a Stmt<'a>>,
}

/// Nó de instrução alocado na `Arena<Stmt>`.
#[derive(Debug)]
pub enum Stmt<'a> {
    Local {
        name: String,
        init: Option<&'a Expr<'a>>,
        span: Span,
    },
    Assign {
        name:  String,
        value: &'a Expr<'a>,
        span:  Span,
    },
    While {
        cond: &'a Expr<'a>,
        body: Vec<&'a Stmt<'a>>,
        span: Span,
    },
    If {
        cond:           &'a Expr<'a>,
        then_body:      Vec<&'a Stmt<'a>>,
        elseif_clauses: Vec<ElseIf<'a>>,
        else_body:      Option<Vec<&'a Stmt<'a>>>,
        span:           Span,
    },
    /// Loop numérico `for var = start, limit [, step] do body end`.
    NumFor {
        var:   String,
        start: &'a Expr<'a>,
        limit: &'a Expr<'a>,
        step:  Option<&'a Expr<'a>>,
        body:  Vec<&'a Stmt<'a>>,
        span:  Span,
    },
    Function {
        name:   String,
        params: Vec<String>,
        body:   Vec<&'a Stmt<'a>>,
        span:   Span,
    },
    Return {
        value: Option<&'a Expr<'a>>,
        span:  Span,
    },
    /// Chamada de função/intrínseca usada como instrução (ex: `delay(500)`).
    Call {
        name: String,
        args: Vec<&'a Expr<'a>>,
        span: Span,
    },
}

impl<'a> Stmt<'a> {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Local    { span, .. }
            | Stmt::Assign   { span, .. }
            | Stmt::While    { span, .. }
            | Stmt::If       { span, .. }
            | Stmt::NumFor   { span, .. }
            | Stmt::Function { span, .. }
            | Stmt::Return   { span, .. }
            | Stmt::Call     { span, .. } => *span,
        }
    }
}

// ── Display dos operadores (usado em mensagens de erro) ───────────────────────

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),  BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),  BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"),  BinOp::Eq  => write!(f, "=="),
            BinOp::Ne  => write!(f, "~="), BinOp::Lt  => write!(f, "<"),
            BinOp::Le  => write!(f, "<="), BinOp::Gt  => write!(f, ">"),
            BinOp::Ge  => write!(f, ">="), BinOp::And => write!(f, "and"),
            BinOp::Or  => write!(f, "or"),
        }
    }
}

impl fmt::Display for UnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnOp::Neg => write!(f, "-"),
            UnOp::Not => write!(f, "not"),
        }
    }
}
