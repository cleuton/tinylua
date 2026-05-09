use crate::token::{Spanned, SpannedToken, Token};

pub struct Lexer<'src> {
    src: &'src str,
    pos: usize,
    line: u32,
    col: u32,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self { src, pos: 0, line: 1, col: 1 }
    }

    /// Tokenize `src` retornando todos os tokens inclusive o `Eof` final.
    pub fn tokenize(src: &'src str) -> Vec<SpannedToken> {
        let mut lex = Self::new(src);
        let mut out = Vec::new();
        loop {
            let t = lex.next_token();
            let done = t.node == Token::Eof;
            out.push(t);
            if done { break; }
        }
        out
    }

    /// Retorna o próximo token; sempre retorna `Eof` após o fim da fonte.
    pub fn next_token(&mut self) -> SpannedToken {
        self.skip_whitespace_and_comments();
        let line = self.line;
        let col  = self.col;
        let tok = match self.advance() {
            None        => Token::Eof,
            Some('+')   => Token::OpPlus,
            Some('-')   => Token::OpMinus,
            Some('*')   => Token::OpStar,
            Some('/')   => Token::OpSlash,
            Some('%')   => Token::OpPercent,
            Some('(')   => Token::LParen,
            Some(')')   => Token::RParen,
            Some(',')   => Token::Comma,
            Some(';')   => Token::Semicolon,
            Some('=')   => {
                if self.peek() == Some('=') { self.advance(); Token::OpEqEq }
                else { Token::OpEq }
            }
            Some('<')   => {
                if self.peek() == Some('=') { self.advance(); Token::OpLtEq }
                else { Token::OpLt }
            }
            Some('>')   => {
                if self.peek() == Some('=') { self.advance(); Token::OpGtEq }
                else { Token::OpGt }
            }
            // `~=` é o not-equal de Lua. `~` sozinho não existe no subconjunto.
            Some('~')   => {
                if self.peek() == Some('=') { self.advance(); Token::OpTildeEq }
                else { Token::Unknown('~') }
            }
            Some('"')                              => self.lex_string(),
            Some(c @ '0'..='9')                   => self.lex_number(c),
            Some(c) if c.is_alphabetic() || c == '_' => self.lex_ident(c),
            Some(c) => Token::Unknown(c),
        };
        Spanned { node: tok, line, col }
    }

    // ── primitivas ────────────────────────────────────────────────────────────

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        let mut it = self.src[self.pos..].chars();
        it.next();
        it.next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        if c == '\n' { self.line += 1; self.col = 1; } else { self.col += 1; }
        Some(c)
    }

    // ── skip ──────────────────────────────────────────────────────────────────

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => { self.advance(); }
                // Comentário de linha Lua: `--` até fim da linha.
                Some('-') if self.peek2() == Some('-') => {
                    while self.peek().map_or(false, |c| c != '\n') {
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    // ── sub-lexers ────────────────────────────────────────────────────────────

    fn lex_ident(&mut self, first: char) -> Token {
        let mut s = String::from(first);
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' { s.push(c); self.advance(); } else { break; }
        }
        Token::from_keyword(&s).unwrap_or(Token::Ident(s))
    }

    fn lex_number(&mut self, first: char) -> Token {
        // Literal hexadecimal: 0x / 0X
        if first == '0' && matches!(self.peek(), Some('x') | Some('X')) {
            self.advance(); // consome 'x'|'X'
            let mut hex = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() { hex.push(c); self.advance(); } else { break; }
            }
            if hex.is_empty() {
                // `0x` sem dígitos é inválido; Unknown preserva o char problemático.
                return Token::Unknown('x');
            }
            let n = i64::from_str_radix(&hex, 16).unwrap_or(i64::MAX);
            return Token::LitInt(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        }

        let mut s = String::from(first);
        let mut has_dot = false;
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_digit() => { s.push(c); self.advance(); }
                // Ponto decimal: aceita apenas um. `3.14` → LitFloat; `3.` → LitFloat(3.0).
                Some('.') if !has_dot => { has_dot = true; s.push('.'); self.advance(); }
                _ => break,
            }
        }

        if has_dot {
            Token::LitFloat(s.parse().unwrap_or(0.0))
        } else {
            // Overflow → i32::MAX/MIN. A sema pode emitir diagnóstico mais fino se necessário.
            let n: i64 = s.parse().unwrap_or(i64::MAX);
            Token::LitInt(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
        }
    }

    fn lex_string(&mut self) -> Token {
        let mut s = String::new();
        loop {
            match self.advance() {
                // Fim de arquivo ou newline dentro da string → string não terminada.
                None | Some('\n') => return Token::Unknown('"'),
                Some('"')  => return Token::LitStr(s),
                Some('\\') => match self.advance() {
                    Some('n')  => s.push('\n'),
                    Some('t')  => s.push('\t'),
                    Some('r')  => s.push('\r'),
                    Some('"')  => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some(c)    => { s.push('\\'); s.push(c); }
                    None       => return Token::Unknown('"'),
                },
                Some(c) => s.push(c),
            }
        }
    }
}

/// Acesso streaming: `Eof` termina o iterador (não é emitido como `Item`).
/// Use `Lexer::tokenize` quando precisar do `Eof` explícito.
impl<'src> Iterator for Lexer<'src> {
    type Item = SpannedToken;
    fn next(&mut self) -> Option<Self::Item> {
        let t = self.next_token();
        if t.node == Token::Eof { None } else { Some(t) }
    }
}

// ── testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::Token;

    fn toks(src: &str) -> Vec<Token> {
        Lexer::tokenize(src).into_iter().map(|s| s.node).collect()
    }

    fn lex(src: &str) -> Vec<SpannedToken> {
        Lexer::tokenize(src)
    }

    fn first(src: &str) -> Token {
        Lexer::tokenize(src).remove(0).node
    }

    // ── vazio / espaço ────────────────────────────────────────────────────────

    #[test]
    fn empty_source_yields_only_eof() {
        assert_eq!(toks(""), vec![Token::Eof]);
    }

    #[test]
    fn whitespace_only_yields_only_eof() {
        assert_eq!(toks("   \t\n  "), vec![Token::Eof]);
    }

    // ── comentários ───────────────────────────────────────────────────────────

    #[test]
    fn line_comment_is_skipped() {
        assert_eq!(
            toks("+ -- comentário de linha\n-"),
            vec![Token::OpPlus, Token::OpMinus, Token::Eof]
        );
    }

    #[test]
    fn comment_at_eof_without_newline() {
        // `--` sem `\n` final não deve travar o lexer
        assert_eq!(toks("-- sem newline no fim"), vec![Token::Eof]);
    }

    #[test]
    fn token_after_comment_on_next_line() {
        assert_eq!(
            toks("x -- ignorar\ny"),
            vec![Token::Ident("x".into()), Token::Ident("y".into()), Token::Eof]
        );
    }

    // ── operadores ────────────────────────────────────────────────────────────

    #[test]
    fn single_char_arithmetic_ops() {
        assert_eq!(
            toks("+ - * / %"),
            vec![Token::OpPlus, Token::OpMinus, Token::OpStar, Token::OpSlash, Token::OpPercent, Token::Eof]
        );
    }

    #[test]
    fn assignment_vs_equality() {
        assert_eq!(toks("= =="), vec![Token::OpEq, Token::OpEqEq, Token::Eof]);
    }

    #[test]
    fn tilde_eq_is_lua_not_equal() {
        // Decisão de design: `~=` é o operador not-equal do Lua (não `!=`).
        assert_eq!(first("~="), Token::OpTildeEq);
    }

    #[test]
    fn bare_tilde_is_unknown() {
        // `~` sozinho não é um operador válido no subconjunto TinyLua.
        assert_eq!(first("~"), Token::Unknown('~'));
    }

    #[test]
    fn tilde_followed_by_non_eq_is_unknown() {
        let tokens = toks("~x");
        assert_eq!(tokens[0], Token::Unknown('~'));
        assert_eq!(tokens[1], Token::Ident("x".into()));
    }

    #[test]
    fn lt_and_lteq() {
        assert_eq!(toks("< <="), vec![Token::OpLt, Token::OpLtEq, Token::Eof]);
    }

    #[test]
    fn gt_and_gteq() {
        assert_eq!(toks("> >="), vec![Token::OpGt, Token::OpGtEq, Token::Eof]);
    }

    #[test]
    fn delimiters() {
        assert_eq!(
            toks("( ) , ;"),
            vec![Token::LParen, Token::RParen, Token::Comma, Token::Semicolon, Token::Eof]
        );
    }

    // ── palavras-chave ────────────────────────────────────────────────────────

    #[test]
    fn all_keywords_recognized() {
        let src = "if then else elseif end while do for local function return true false nil and or not";
        assert_eq!(
            toks(src),
            vec![
                Token::KwIf, Token::KwThen, Token::KwElse, Token::KwElseif, Token::KwEnd,
                Token::KwWhile, Token::KwDo, Token::KwFor, Token::KwLocal, Token::KwFunction,
                Token::KwReturn, Token::KwTrue, Token::KwFalse, Token::KwNil,
                Token::KwAnd, Token::KwOr, Token::KwNot,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn elseif_is_one_token_not_two() {
        // `elseif` deve ser KwElseif, não KwElse + KwIf.
        assert_eq!(toks("elseif"), vec![Token::KwElseif, Token::Eof]);
    }

    // ── identificadores ───────────────────────────────────────────────────────

    #[test]
    fn keyword_prefix_yields_ident() {
        // `iffy` começa com `if` mas é um identificador.
        assert_eq!(first("iffy"), Token::Ident("iffy".into()));
    }

    #[test]
    fn underscore_prefix_ident() {
        // `_count` é um identificador Lua válido.
        assert_eq!(first("_count"), Token::Ident("_count".into()));
    }

    #[test]
    fn solo_underscore_is_ident() {
        assert_eq!(first("_"), Token::Ident("_".into()));
    }

    #[test]
    fn intrinsic_names_arrive_as_ident() {
        // digitalRead, pinMode etc. chegam como Ident; a sema os distingue.
        assert_eq!(first("digitalRead"), Token::Ident("digitalRead".into()));
        assert_eq!(first("pinMode"),     Token::Ident("pinMode".into()));
    }

    // ── literais inteiros ─────────────────────────────────────────────────────

    #[test]
    fn lit_int_zero() {
        assert_eq!(first("0"), Token::LitInt(0));
    }

    #[test]
    fn lit_int_positive() {
        assert_eq!(first("42"), Token::LitInt(42));
    }

    #[test]
    fn lit_int_i32_max() {
        assert_eq!(first("2147483647"), Token::LitInt(i32::MAX));
    }

    #[test]
    fn lit_int_overflow_clamps_to_i32_max() {
        // 9 999 999 999 > i32::MAX → saturado. Diagnóstico fino fica na sema.
        assert_eq!(first("9999999999"), Token::LitInt(i32::MAX));
    }

    #[test]
    fn negative_number_is_minus_plus_litint() {
        // Em Lua `-3` é OpMinus + LitInt(3), não um literal negativo.
        assert_eq!(toks("-3"), vec![Token::OpMinus, Token::LitInt(3), Token::Eof]);
    }

    // ── hex ───────────────────────────────────────────────────────────────────

    #[test]
    fn hex_lowercase_x() {
        assert_eq!(first("0xff"), Token::LitInt(255));
    }

    #[test]
    fn hex_uppercase_x_and_digits() {
        assert_eq!(first("0XFF"), Token::LitInt(255));
    }

    #[test]
    fn hex_avr_pin_constant() {
        // Constantes de registrador AVR como 0x3F são comuns.
        assert_eq!(first("0x3F"), Token::LitInt(0x3F));
    }

    #[test]
    fn hex_overflow_clamps() {
        // 0xFFFFFFFF = 4 294 967 295 > i32::MAX
        assert_eq!(first("0xFFFFFFFF"), Token::LitInt(i32::MAX));
    }

    #[test]
    fn hex_no_digits_is_unknown() {
        // `0x` sem dígitos hexadecimais é inválido.
        let tokens = toks("0x");
        assert_eq!(tokens[0], Token::Unknown('x'));
    }

    // ── literais float ────────────────────────────────────────────────────────

    #[test]
    fn lit_float_basic() {
        // O lexer EMITE LitFloat. A sema rejeita se target == AVR.
        match first("3.14") {
            Token::LitFloat(v) => assert!((v - 3.14_f32).abs() < 1e-4),
            other => panic!("esperava LitFloat, obteve {other:?}"),
        }
    }

    #[test]
    fn lit_float_trailing_dot_is_valid() {
        // Em Lua `3.` == 3.0; o lexer deve emitir LitFloat.
        match first("3.") {
            Token::LitFloat(v) => assert!((v - 3.0_f32).abs() < 1e-6),
            other => panic!("esperava LitFloat, obteve {other:?}"),
        }
    }

    #[test]
    fn lit_float_zero_dot_zero() {
        match first("0.0") {
            Token::LitFloat(v) => assert!(v == 0.0_f32),
            other => panic!("{other:?}"),
        }
    }

    // ── strings ───────────────────────────────────────────────────────────────

    #[test]
    fn lit_str_simple() {
        assert_eq!(first(r#""hello""#), Token::LitStr("hello".into()));
    }

    #[test]
    fn lit_str_intrinsic_arg() {
        // Caso de uso central: pinMode(LED_PIN, "OUTPUT")
        assert_eq!(first(r#""OUTPUT""#), Token::LitStr("OUTPUT".into()));
    }

    #[test]
    fn lit_str_escape_newline() {
        assert_eq!(first(r#""\n""#), Token::LitStr("\n".into()));
    }

    #[test]
    fn lit_str_escape_tab() {
        assert_eq!(first(r#""\t""#), Token::LitStr("\t".into()));
    }

    #[test]
    fn lit_str_escape_quote() {
        assert_eq!(first(r#""say \"hi\"""#), Token::LitStr(r#"say "hi""#.into()));
    }

    #[test]
    fn lit_str_escape_backslash() {
        assert_eq!(first(r#""a\\b""#), Token::LitStr("a\\b".into()));
    }

    #[test]
    fn unterminated_string_yields_unknown() {
        // Sem `"` de fechamento → Unknown('"'). Linha/col preservados para diagnóstico.
        assert_eq!(first(r#""sem fechar"#), Token::Unknown('"'));
    }

    #[test]
    fn newline_inside_string_yields_unknown() {
        // Newline implícito encerra a string como inválida (sem multiline no subconjunto).
        assert_eq!(first("\"linha1\nlinha2\""), Token::Unknown('"'));
    }

    // ── rastreamento de posição ───────────────────────────────────────────────

    #[test]
    fn line_tracking_across_newlines() {
        let tokens = lex("x\ny\nz");
        assert_eq!(tokens[0].line, 1, "x deve estar na linha 1");
        assert_eq!(tokens[1].line, 2, "y deve estar na linha 2");
        assert_eq!(tokens[2].line, 3, "z deve estar na linha 3");
    }

    #[test]
    fn col_tracking_on_same_line() {
        let tokens = lex("x y");
        assert_eq!(tokens[0].col, 1, "x na coluna 1");
        assert_eq!(tokens[1].col, 3, "y na coluna 3");
    }

    #[test]
    fn col_resets_after_newline() {
        let tokens = lex("ab\ncd");
        assert_eq!(tokens[0].col, 1);
        assert_eq!(tokens[1].col, 1, "primeiro token da linha 2 deve ser col 1");
    }

    #[test]
    fn multichar_token_position_is_start() {
        // `==` começa na coluna 1; o token deve apontar para lá.
        let tokens = lex("==");
        assert_eq!(tokens[0].col, 1);
    }

    // ── caracteres desconhecidos ──────────────────────────────────────────────

    #[test]
    fn unknown_chars() {
        assert_eq!(first("@"), Token::Unknown('@'));
        assert_eq!(first("#"), Token::Unknown('#'));
        assert_eq!(first("$"), Token::Unknown('$'));
    }

    // ── snippets completos ────────────────────────────────────────────────────

    #[test]
    fn complete_lua_snippet_with_comment() {
        let src = "local x = 42 -- inicializa\nif x > 0 then\n    return x\nend";
        assert_eq!(
            toks(src),
            vec![
                Token::KwLocal, Token::Ident("x".into()), Token::OpEq, Token::LitInt(42),
                Token::KwIf, Token::Ident("x".into()), Token::OpGt, Token::LitInt(0), Token::KwThen,
                Token::KwReturn, Token::Ident("x".into()),
                Token::KwEnd,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn intrinsic_call_snippet() {
        // pinMode chega como Ident; "OUTPUT" como LitStr. A sema faz a ligação com o HAL.
        let src = r#"pinMode(13, "OUTPUT")"#;
        assert_eq!(
            toks(src),
            vec![
                Token::Ident("pinMode".into()),
                Token::LParen,
                Token::LitInt(13),
                Token::Comma,
                Token::LitStr("OUTPUT".into()),
                Token::RParen,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn while_loop_snippet() {
        let src = "while x ~= 0 do x = x - 1 end";
        assert_eq!(
            toks(src),
            vec![
                Token::KwWhile,
                Token::Ident("x".into()),
                Token::OpTildeEq,
                Token::LitInt(0),
                Token::KwDo,
                Token::Ident("x".into()), Token::OpEq,
                Token::Ident("x".into()), Token::OpMinus, Token::LitInt(1),
                Token::KwEnd,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn iterator_impl_stops_before_eof() {
        // Iterator não emite Eof; tokenize() sim.
        let tokens: Vec<_> = Lexer::new("a b").collect();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].node, Token::Ident("a".into()));
        assert_eq!(tokens[1].node, Token::Ident("b".into()));
    }
}
