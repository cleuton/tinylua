use tinylua_lexer::{Lexer, SpannedToken, Token};
use typed_arena::Arena;

use crate::ast::*;
use crate::error::ParseError;

// ── Parser ────────────────────────────────────────────────────────────────────

struct Parser<'a> {
    tokens: Vec<SpannedToken>,
    pos:    usize,
    exprs:  &'a Arena<Expr<'a>>,
    stmts:  &'a Arena<Stmt<'a>>,
}

impl<'a> Parser<'a> {
    fn new(src: &str, exprs: &'a Arena<Expr<'a>>, stmts: &'a Arena<Stmt<'a>>) -> Self {
        Self { tokens: Lexer::tokenize(src), pos: 0, exprs, stmts }
    }

    // ── primitivas ────────────────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].node
    }

    fn peek_span(&self) -> Span {
        let t = &self.tokens[self.pos];
        Span { line: t.line, col: t.col }
    }

    fn bump(&mut self) {
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<Span, ParseError> {
        if self.peek() == expected {
            let span = self.peek_span();
            self.bump();
            Ok(span)
        } else {
            let found = self.peek().to_string();
            Err(ParseError {
                message: format!("esperado `{expected}`, encontrado `{found}`"),
                span: self.peek_span(),
            })
        }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError { message: msg.into(), span: self.peek_span() }
    }

    fn expect_ident(&mut self, ctx: &str) -> Result<String, ParseError> {
        let tok = self.peek().clone();
        match tok {
            Token::Ident(n) => { self.bump(); Ok(n) }
            other => Err(self.err(format!(
                "esperado identificador {ctx}, encontrado `{other}`"
            ))),
        }
    }

    fn alloc_expr(&self, e: Expr<'a>) -> &'a Expr<'a> {
        self.exprs.alloc(e)
    }

    fn alloc_stmt(&self, s: Stmt<'a>) -> &'a Stmt<'a> {
        self.stmts.alloc(s)
    }

    // ── bloco ─────────────────────────────────────────────────────────────────

    fn parse_block(&mut self, stop: &[Token]) -> Result<Vec<&'a Stmt<'a>>, ParseError> {
        let mut block = Vec::new();
        loop {
            // pula ponto-e-vírgula opcional
            while matches!(self.peek(), Token::Semicolon) {
                self.bump();
            }
            // para ao encontrar EOF ou um dos tokens de parada
            if matches!(self.peek(), Token::Eof) || stop.contains(self.peek()) {
                break;
            }
            block.push(self.parse_stmt()?);
        }
        Ok(block)
    }

    // ── instruções ────────────────────────────────────────────────────────────

    fn parse_stmt(&mut self) -> Result<&'a Stmt<'a>, ParseError> {
        let span = self.peek_span();
        let tok  = self.peek().clone();
        let stmt = match tok {
            Token::KwLocal    => self.parse_local(span)?,
            Token::KwIf       => self.parse_if(span)?,
            Token::KwWhile    => self.parse_while(span)?,
            Token::KwFor      => self.parse_for(span)?,
            Token::KwFunction => self.parse_function(span)?,
            Token::KwReturn   => self.parse_return(span)?,
            Token::Ident(_)   => self.parse_ident_stmt(span)?,
            other => return Err(ParseError {
                message: format!("instrução inválida: `{other}`"),
                span,
            }),
        };
        Ok(self.alloc_stmt(stmt))
    }

    fn parse_local(&mut self, span: Span) -> Result<Stmt<'a>, ParseError> {
        self.bump(); // 'local'
        let name = self.expect_ident("após 'local'")?;
        let init = if matches!(self.peek(), Token::OpEq) {
            self.bump();
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(Stmt::Local { name, init, span })
    }

    fn parse_if(&mut self, span: Span) -> Result<Stmt<'a>, ParseError> {
        self.bump(); // 'if'
        let cond = self.parse_expr()?;
        self.expect(&Token::KwThen)?;
        let then_body = self.parse_block(&[Token::KwElseif, Token::KwElse, Token::KwEnd])?;

        let mut elseif_clauses = Vec::new();
        while matches!(self.peek(), Token::KwElseif) {
            self.bump();
            let ei_cond = self.parse_expr()?;
            self.expect(&Token::KwThen)?;
            let ei_body = self.parse_block(&[Token::KwElseif, Token::KwElse, Token::KwEnd])?;
            elseif_clauses.push(ElseIf { cond: ei_cond, body: ei_body });
        }

        let else_body = if matches!(self.peek(), Token::KwElse) {
            self.bump();
            Some(self.parse_block(&[Token::KwEnd])?)
        } else {
            None
        };
        self.expect(&Token::KwEnd)?;
        Ok(Stmt::If { cond, then_body, elseif_clauses, else_body, span })
    }

    fn parse_while(&mut self, span: Span) -> Result<Stmt<'a>, ParseError> {
        self.bump(); // 'while'
        let cond = self.parse_expr()?;
        self.expect(&Token::KwDo)?;
        let body = self.parse_block(&[Token::KwEnd])?;
        self.expect(&Token::KwEnd)?;
        Ok(Stmt::While { cond, body, span })
    }

    fn parse_for(&mut self, span: Span) -> Result<Stmt<'a>, ParseError> {
        self.bump(); // 'for'
        let var = self.expect_ident("após 'for'")?;
        self.expect(&Token::OpEq)?;
        let start = self.parse_expr()?;
        self.expect(&Token::Comma)?;
        let limit = self.parse_expr()?;
        let step = if matches!(self.peek(), Token::Comma) {
            self.bump();
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(&Token::KwDo)?;
        let body = self.parse_block(&[Token::KwEnd])?;
        self.expect(&Token::KwEnd)?;
        Ok(Stmt::NumFor { var, start, limit, step, body, span })
    }

    fn parse_function(&mut self, span: Span) -> Result<Stmt<'a>, ParseError> {
        self.bump(); // 'function'
        let name = self.expect_ident("após 'function'")?;
        self.expect(&Token::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(&Token::RParen)?;
        let body = self.parse_block(&[Token::KwEnd])?;
        self.expect(&Token::KwEnd)?;
        Ok(Stmt::Function { name, params, body, span })
    }

    fn parse_return(&mut self, span: Span) -> Result<Stmt<'a>, ParseError> {
        self.bump(); // 'return'
        // sem valor se o próximo token termina o bloco
        let no_value = matches!(
            self.peek(),
            Token::KwEnd | Token::KwElse | Token::KwElseif | Token::Semicolon | Token::Eof
        );
        let value = if no_value { None } else { Some(self.parse_expr()?) };
        Ok(Stmt::Return { value, span })
    }

    fn parse_ident_stmt(&mut self, span: Span) -> Result<Stmt<'a>, ParseError> {
        let name = self.expect_ident("no início de instrução")?;
        let next = self.peek().clone();
        match next {
            Token::OpEq => {
                self.bump();
                let value = self.parse_expr()?;
                Ok(Stmt::Assign { name, value, span })
            }
            Token::LParen => {
                let args = self.parse_arg_list()?;
                Ok(Stmt::Call { name, args, span })
            }
            other => Err(ParseError {
                message: format!("esperado '=' ou '(' após '{name}', encontrado `{other}`"),
                span: self.peek_span(),
            }),
        }
    }

    // ── listas ────────────────────────────────────────────────────────────────

    fn parse_param_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut params = Vec::new();
        if matches!(self.peek(), Token::RParen) {
            return Ok(params);
        }
        loop {
            params.push(self.expect_ident("na lista de parâmetros")?);
            if !matches!(self.peek(), Token::Comma) {
                break;
            }
            self.bump();
        }
        Ok(params)
    }

    fn parse_arg_list(&mut self) -> Result<Vec<&'a Expr<'a>>, ParseError> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        if !matches!(self.peek(), Token::RParen) {
            loop {
                args.push(self.parse_expr()?);
                if !matches!(self.peek(), Token::Comma) {
                    break;
                }
                self.bump();
            }
        }
        self.expect(&Token::RParen)?;
        Ok(args)
    }

    // ── expressões (precedência ascendente) ───────────────────────────────────
    //
    // Hierarquia (do mais fraco ao mais forte):
    //   or → and → cmp → add → mul → unary → primary

    fn parse_expr(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let mut lhs = self.parse_and()?;
        while matches!(self.peek(), Token::KwOr) {
            let span = self.peek_span();
            self.bump();
            let rhs = self.parse_and()?;
            lhs = self.alloc_expr(Expr::BinOp { op: BinOp::Or, lhs, rhs, span });
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let mut lhs = self.parse_cmp()?;
        while matches!(self.peek(), Token::KwAnd) {
            let span = self.peek_span();
            self.bump();
            let rhs = self.parse_cmp()?;
            lhs = self.alloc_expr(Expr::BinOp { op: BinOp::And, lhs, rhs, span });
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let mut lhs = self.parse_add()?;
        loop {
            // mapeia token → BinOp sem manter referência ao token
            let op = match self.peek() {
                Token::OpEqEq    => BinOp::Eq,
                Token::OpTildeEq => BinOp::Ne,
                Token::OpLt      => BinOp::Lt,
                Token::OpLtEq    => BinOp::Le,
                Token::OpGt      => BinOp::Gt,
                Token::OpGtEq    => BinOp::Ge,
                _ => break,
            };
            let span = self.peek_span();
            self.bump();
            let rhs = self.parse_add()?;
            lhs = self.alloc_expr(Expr::BinOp { op, lhs, rhs, span });
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Token::OpPlus  => BinOp::Add,
                Token::OpMinus => BinOp::Sub,
                _ => break,
            };
            let span = self.peek_span();
            self.bump();
            let rhs = self.parse_mul()?;
            lhs = self.alloc_expr(Expr::BinOp { op, lhs, rhs, span });
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::OpStar    => BinOp::Mul,
                Token::OpSlash   => BinOp::Div,
                Token::OpPercent => BinOp::Mod,
                _ => break,
            };
            let span = self.peek_span();
            self.bump();
            let rhs = self.parse_unary()?;
            lhs = self.alloc_expr(Expr::BinOp { op, lhs, rhs, span });
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let span = self.peek_span();
        let tok  = self.peek().clone();
        match tok {
            Token::KwNot => {
                self.bump();
                let operand = self.parse_unary()?;
                Ok(self.alloc_expr(Expr::UnOp { op: UnOp::Not, operand, span }))
            }
            Token::OpMinus => {
                self.bump();
                let operand = self.parse_unary()?;
                Ok(self.alloc_expr(Expr::UnOp { op: UnOp::Neg, operand, span }))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let span = self.peek_span();
        let tok  = self.peek().clone();
        match tok {
            Token::LitInt(n)   => { self.bump(); Ok(self.alloc_expr(Expr::LitInt(n, span))) }
            Token::LitFloat(v) => { self.bump(); Ok(self.alloc_expr(Expr::LitFloat(v, span))) }
            Token::LitStr(s)   => { self.bump(); Ok(self.alloc_expr(Expr::LitStr(s, span))) }
            Token::KwTrue      => { self.bump(); Ok(self.alloc_expr(Expr::LitBool(true,  span))) }
            Token::KwFalse     => { self.bump(); Ok(self.alloc_expr(Expr::LitBool(false, span))) }
            Token::KwNil       => { self.bump(); Ok(self.alloc_expr(Expr::Nil(span))) }
            Token::Ident(name) => {
                self.bump();
                if matches!(self.peek(), Token::LParen) {
                    let args = self.parse_arg_list()?;
                    Ok(self.alloc_expr(Expr::Call { name, args, span }))
                } else {
                    Ok(self.alloc_expr(Expr::Var(name, span)))
                }
            }
            Token::LParen => {
                self.bump();
                let inner = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            other => Err(ParseError {
                message: format!("expressão inválida: `{other}`"),
                span,
            }),
        }
    }
}

// ── ponto de entrada público ──────────────────────────────────────────────────

/// Analisa `src` e retorna a lista de instruções de nível superior.
/// Os nós da AST são alocados nas arenas fornecidas pelo chamador.
///
/// # Exemplo
/// ```
/// use typed_arena::Arena;
/// use tinylua_parser::{parse, Expr, Stmt};
///
/// let exprs = Arena::new();
/// let stmts = Arena::new();
/// let block = parse("local x = 1", &exprs, &stmts).unwrap();
/// assert_eq!(block.len(), 1);
/// ```
pub fn parse<'a>(
    src:   &str,
    exprs: &'a Arena<Expr<'a>>,
    stmts: &'a Arena<Stmt<'a>>,
) -> Result<Vec<&'a Stmt<'a>>, ParseError> {
    let mut p = Parser::new(src, exprs, stmts);
    let block = p.parse_block(&[])?;
    if !matches!(p.peek(), Token::Eof) {
        let found = p.peek().to_string();
        return Err(p.err(format!("token inesperado: `{found}`")));
    }
    Ok(block)
}

// ── testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn arenas<'a>() -> (Arena<Expr<'a>>, Arena<Stmt<'a>>) {
        (Arena::new(), Arena::new())
    }

    // ── programa vazio ────────────────────────────────────────────────────────

    #[test]
    fn empty_source_yields_empty_block() {
        let (e, s) = arenas();
        let block = parse("", &e, &s).unwrap();
        assert!(block.is_empty());
    }

    #[test]
    fn semicolons_only_yields_empty_block() {
        let (e, s) = arenas();
        let block = parse(";;;", &e, &s).unwrap();
        assert!(block.is_empty());
    }

    // ── local ─────────────────────────────────────────────────────────────────

    #[test]
    fn local_without_init() {
        let (e, s) = arenas();
        let block = parse("local x", &e, &s).unwrap();
        assert_eq!(block.len(), 1);
        assert!(matches!(block[0], Stmt::Local { init: None, .. }));
    }

    #[test]
    fn local_with_int_init() {
        let (e, s) = arenas();
        let block = parse("local x = 42", &e, &s).unwrap();
        assert!(matches!(
            block[0],
            Stmt::Local { init: Some(Expr::LitInt(42, _)), .. }
        ));
    }

    #[test]
    fn local_span_is_correct() {
        let (e, s) = arenas();
        let block = parse("local x = 1", &e, &s).unwrap();
        let span = block[0].span();
        assert_eq!(span.line, 1);
        assert_eq!(span.col,  1);
    }

    // ── assign ────────────────────────────────────────────────────────────────

    #[test]
    fn assignment() {
        let (e, s) = arenas();
        let block = parse("x = 10", &e, &s).unwrap();
        assert!(matches!(
            block[0],
            Stmt::Assign { value: Expr::LitInt(10, _), .. }
        ));
    }

    // ── call statement ────────────────────────────────────────────────────────

    #[test]
    fn call_statement_no_args() {
        let (e, s) = arenas();
        let block = parse("foo()", &e, &s).unwrap();
        assert!(matches!(block[0], Stmt::Call { ref name, ref args, .. }
            if name == "foo" && args.is_empty()));
    }

    #[test]
    fn call_statement_intrinsic() {
        let (e, s) = arenas();
        let block = parse("delay(500)", &e, &s).unwrap();
        assert!(matches!(block[0], Stmt::Call { ref name, ref args, .. }
            if name == "delay" && args.len() == 1));
        if let Stmt::Call { args, .. } = block[0] {
            assert!(matches!(args[0], Expr::LitInt(500, _)));
        }
    }

    // ── if ────────────────────────────────────────────────────────────────────

    #[test]
    fn if_then_end() {
        let (e, s) = arenas();
        let block = parse("if x then end", &e, &s).unwrap();
        assert!(matches!(
            block[0],
            Stmt::If { then_body: ref b, else_body: None, ref elseif_clauses, .. }
            if b.is_empty() && elseif_clauses.is_empty()
        ));
    }

    #[test]
    fn if_then_else_end() {
        let (e, s) = arenas();
        let block = parse("if x then y = 1 else y = 2 end", &e, &s).unwrap();
        if let Stmt::If { then_body, else_body, .. } = block[0] {
            assert_eq!(then_body.len(), 1);
            assert_eq!(else_body.as_ref().unwrap().len(), 1);
        } else {
            panic!("esperado Stmt::If");
        }
    }

    #[test]
    fn if_elseif_else_end() {
        let (e, s) = arenas();
        let src = "if a then x=1 elseif b then x=2 else x=3 end";
        let block = parse(src, &e, &s).unwrap();
        if let Stmt::If { then_body, elseif_clauses, else_body, .. } = block[0] {
            assert_eq!(then_body.len(), 1);
            assert_eq!(elseif_clauses.len(), 1);
            assert_eq!(else_body.as_ref().unwrap().len(), 1);
        } else {
            panic!("esperado Stmt::If");
        }
    }

    // ── while ─────────────────────────────────────────────────────────────────

    #[test]
    fn while_true_loop() {
        let (e, s) = arenas();
        let block = parse("while true do end", &e, &s).unwrap();
        assert!(matches!(
            block[0],
            Stmt::While { cond: Expr::LitBool(true, _), .. }
        ));
    }

    // ── for ───────────────────────────────────────────────────────────────────

    #[test]
    fn numeric_for_without_step() {
        let (e, s) = arenas();
        let block = parse("for i = 1, 10 do end", &e, &s).unwrap();
        assert!(matches!(
            block[0],
            Stmt::NumFor { ref var, step: None, .. } if var == "i"
        ));
    }

    #[test]
    fn numeric_for_with_step() {
        let (e, s) = arenas();
        let block = parse("for i = 0, 100, 2 do end", &e, &s).unwrap();
        assert!(matches!(
            block[0],
            Stmt::NumFor { step: Some(Expr::LitInt(2, _)), .. }
        ));
    }

    // ── function ──────────────────────────────────────────────────────────────

    #[test]
    fn function_no_params() {
        let (e, s) = arenas();
        let block = parse("function blink() end", &e, &s).unwrap();
        assert!(matches!(
            block[0],
            Stmt::Function { ref name, ref params, .. }
            if name == "blink" && params.is_empty()
        ));
    }

    #[test]
    fn function_with_params_and_return() {
        let (e, s) = arenas();
        let block = parse("function add(a, b) return a end", &e, &s).unwrap();
        if let Stmt::Function { params, body, .. } = block[0] {
            assert_eq!(params, &["a", "b"]);
            assert_eq!(body.len(), 1);
            assert!(matches!(body[0], Stmt::Return { value: Some(_), .. }));
        } else {
            panic!("esperado Stmt::Function");
        }
    }

    // ── return ────────────────────────────────────────────────────────────────

    #[test]
    fn return_with_value() {
        let (e, s) = arenas();
        let block = parse("return 99", &e, &s).unwrap();
        assert!(matches!(block[0], Stmt::Return { value: Some(Expr::LitInt(99, _)), .. }));
    }

    #[test]
    fn return_without_value() {
        let (e, s) = arenas();
        let block = parse("return", &e, &s).unwrap();
        assert!(matches!(block[0], Stmt::Return { value: None, .. }));
    }

    // ── expressões e precedência ──────────────────────────────────────────────

    #[test]
    fn precedence_mul_before_add() {
        // `1 + 2 * 3` deve ser `1 + (2 * 3)`, não `(1 + 2) * 3`
        let (e, s) = arenas();
        let block = parse("x = 1 + 2 * 3", &e, &s).unwrap();
        if let Stmt::Assign { value, .. } = block[0] {
            // raiz deve ser Add
            assert!(matches!(value, Expr::BinOp { op: BinOp::Add, .. }));
            if let Expr::BinOp { rhs, .. } = value {
                // rhs do Add deve ser Mul
                assert!(matches!(rhs, Expr::BinOp { op: BinOp::Mul, .. }));
            }
        } else {
            panic!("esperado Stmt::Assign");
        }
    }

    #[test]
    fn unary_neg() {
        let (e, s) = arenas();
        let block = parse("x = -5", &e, &s).unwrap();
        if let Stmt::Assign { value, .. } = block[0] {
            assert!(matches!(value, Expr::UnOp { op: UnOp::Neg, operand: Expr::LitInt(5, _), .. }));
        }
    }

    #[test]
    fn unary_not() {
        let (e, s) = arenas();
        let block = parse("x = not true", &e, &s).unwrap();
        if let Stmt::Assign { value, .. } = block[0] {
            assert!(matches!(
                value,
                Expr::UnOp { op: UnOp::Not, operand: Expr::LitBool(true, _), .. }
            ));
        }
    }

    #[test]
    fn expr_call_in_rhs() {
        let (e, s) = arenas();
        let block = parse("local v = analogRead(0)", &e, &s).unwrap();
        if let Stmt::Local { init: Some(expr), .. } = block[0] {
            assert!(matches!(expr, Expr::Call { ref name, .. } if name == "analogRead"));
        } else {
            panic!("esperado Stmt::Local com init");
        }
    }

    #[test]
    fn lit_float_is_parsed() {
        // O parser emite LitFloat; a sema rejeita em AVR.
        let (e, s) = arenas();
        let block = parse("x = 3.14", &e, &s).unwrap();
        if let Stmt::Assign { value, .. } = block[0] {
            assert!(matches!(value, Expr::LitFloat(_, _)));
        }
    }

    #[test]
    fn nil_and_bool_literals() {
        let (e, s) = arenas();
        let block = parse("x = nil", &e, &s).unwrap();
        assert!(matches!(
            block[0],
            Stmt::Assign { value: Expr::Nil(_), .. }
        ));
        let block = parse("x = false", &e, &s).unwrap();
        assert!(matches!(
            block[0],
            Stmt::Assign { value: Expr::LitBool(false, _), .. }
        ));
    }

    #[test]
    fn parenthesized_expr_overrides_precedence() {
        // `(1 + 2) * 3` — raiz deve ser Mul
        let (e, s) = arenas();
        let block = parse("x = (1 + 2) * 3", &e, &s).unwrap();
        if let Stmt::Assign { value, .. } = block[0] {
            assert!(matches!(value, Expr::BinOp { op: BinOp::Mul, .. }));
        }
    }

    // ── erros ─────────────────────────────────────────────────────────────────

    #[test]
    fn error_contains_line_col() {
        let (e, s) = arenas();
        let err = parse("@", &e, &s).unwrap_err();
        let msg = err.to_string();
        assert!(msg.starts_with("erro[1:1]:"), "mensagem: {msg}");
    }

    #[test]
    fn error_missing_then() {
        let (e, s) = arenas();
        let err = parse("if x end", &e, &s).unwrap_err();
        assert!(err.message.contains("then"), "mensagem: {}", err.message);
    }

    #[test]
    fn error_missing_end() {
        let (e, s) = arenas();
        let err = parse("while true do", &e, &s).unwrap_err();
        assert!(err.message.contains("end"), "mensagem: {}", err.message);
    }

    #[test]
    fn error_ident_without_eq_or_paren() {
        let (e, s) = arenas();
        let err = parse("x + 1", &e, &s).unwrap_err();
        assert!(err.message.contains("'='") || err.message.contains("'('"),
            "mensagem: {}", err.message);
    }

    // ── exemplo completo do CLAUDE.md (Blink com sensor) ─────────────────────

    #[test]
    fn blink_example_parses_correctly() {
        let src = r#"
local LED_PIN = 13
local SENSOR_PIN = 2
pinMode(LED_PIN, "OUTPUT")
pinMode(SENSOR_PIN, "INPUT")
while true do
  local state = digitalRead(SENSOR_PIN)
  if state == true then
    digitalWrite(LED_PIN, true)
    delay(500)
  else
    digitalWrite(LED_PIN, false)
  end
end
"#;
        let (e, s) = arenas();
        let block = parse(src, &e, &s).unwrap();
        assert_eq!(block.len(), 5);
        // dois locais
        assert!(matches!(block[0], Stmt::Local { ref name, .. } if name == "LED_PIN"));
        assert!(matches!(block[1], Stmt::Local { ref name, .. } if name == "SENSOR_PIN"));
        // dois calls de pinMode
        assert!(matches!(block[2], Stmt::Call { ref name, .. } if name == "pinMode"));
        assert!(matches!(block[3], Stmt::Call { ref name, .. } if name == "pinMode"));
        // while
        if let Stmt::While { cond, body, .. } = block[4] {
            assert!(matches!(cond, Expr::LitBool(true, _)));
            assert_eq!(body.len(), 2);
            // local state = digitalRead(...)
            assert!(matches!(body[0], Stmt::Local { ref name, init: Some(_), .. }
                if name == "state"));
            // if state == true then ... else ... end
            if let Stmt::If { cond, then_body, else_body, .. } = body[1] {
                assert!(matches!(cond, Expr::BinOp { op: BinOp::Eq, .. }));
                assert_eq!(then_body.len(), 2);
                assert_eq!(else_body.as_ref().unwrap().len(), 1);
            } else {
                panic!("esperado Stmt::If");
            }
        } else {
            panic!("esperado Stmt::While");
        }
    }
}
