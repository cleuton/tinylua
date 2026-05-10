use crate::ast::Span;
use std::fmt;

/// Erro de sintaxe com posição no código Lua original.
/// Formato de exibição: `erro[linha:col]: <mensagem>`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span:    Span,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro[{}:{}]: {}", self.span.line, self.span.col, self.message)
    }
}

impl std::error::Error for ParseError {}
