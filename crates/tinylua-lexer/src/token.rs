/// Tokens do subconjunto Lua suportado pelo TinyLua (PoC AVR-first).
///
/// Decisões de design:
/// - `LitFloat` é emitido pelo lexer, mas **rejeitado pela análise semântica**
///   quando o target é AVR. O lexer não sabe nada sobre targets.
/// - `LitStr` existe apenas para argumentos de intrínsecas (ex: pinMode(p, "OUTPUT")).
///   Não é um valor de primeira classe.
/// - Identificadores de intrínsecas (digitalRead, etc.) chegam como `Ident` aqui.
///   A separação ocorre na análise semântica, mantendo o lexer desacoplado do HAL.
/// - `Unknown(char)` preserva o caractere inválido para erros com linha/coluna.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── Palavras-chave ────────────────────────────────────────────────────────
    KwIf,
    KwThen,
    KwElse,
    KwElseif,
    KwEnd,
    KwWhile,
    KwDo,
    KwFor,
    KwLocal,
    KwFunction,
    KwReturn,
    KwTrue,
    KwFalse,
    KwNil,
    KwAnd,
    KwOr,
    KwNot,

    // ── Operadores aritméticos ────────────────────────────────────────────────
    OpPlus,     // +
    OpMinus,    // -
    OpStar,     // *
    OpSlash,    // /
    OpPercent,  // %

    // ── Operadores de comparação e atribuição ─────────────────────────────────
    OpEqEq,     // ==
    OpTildeEq,  // ~=   (not-equal em Lua)
    OpLt,       // <
    OpLtEq,     // <=
    OpGt,       // >
    OpGtEq,     // >=
    OpEq,       // =    (atribuição)

    // ── Literais ──────────────────────────────────────────────────────────────
    /// Inteiro. Em AVR representado como i16/i32; no ESP32 como i32.
    LitInt(i32),

    /// Float. Permitido apenas no target ESP32 (f32). A análise semântica
    /// emite erro claro se encontrado em target AVR, antes de gerar qualquer código.
    LitFloat(f32),

    /// String literal. Restrita a argumentos de intrínsecas — não é um valor
    /// de primeira classe na PoC. Ex: pinMode(LED_PIN, "OUTPUT").
    LitStr(String),

    // ── Identificadores ───────────────────────────────────────────────────────
    /// Nome de variável ou função. Intrínsecas de hardware chegam como Ident
    /// aqui; a análise semântica os distingue via tabela de intrínsecas conhecidas.
    Ident(String),

    // ── Delimitadores e pontuação ─────────────────────────────────────────────
    LParen,     // (
    RParen,     // )
    Comma,      // ,
    Semicolon,  // ;

    // ── Controle de fluxo do lexer ────────────────────────────────────────────
    /// Fim do arquivo. O parser para ao encontrar este token.
    Eof,

    /// Caractere não reconhecido. O parser usa o span associado (veja `Spanned`)
    /// para emitir a mensagem de erro referenciando linha e coluna no código Lua.
    Unknown(char),
}

/// Um token com sua posição no código-fonte original.
/// O span é usado exclusivamente para mensagens de erro — nunca em runtime.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub line: u32,
    pub col: u32,
}

pub type SpannedToken = Spanned<Token>;

impl Token {
    /// Retorna `true` se este token é uma palavra-chave.
    /// Útil para erros do tipo "esperava identificador, encontrou palavra reservada".
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            Token::KwIf
                | Token::KwThen
                | Token::KwElse
                | Token::KwElseif
                | Token::KwEnd
                | Token::KwWhile
                | Token::KwDo
                | Token::KwFor
                | Token::KwLocal
                | Token::KwFunction
                | Token::KwReturn
                | Token::KwTrue
                | Token::KwFalse
                | Token::KwNil
                | Token::KwAnd
                | Token::KwOr
                | Token::KwNot
        )
    }

    /// Tenta mapear uma string para uma palavra-chave.
    /// Usado pelo lexer ao encontrar um identificador.
    pub fn from_keyword(s: &str) -> Option<Token> {
        match s {
            "if"       => Some(Token::KwIf),
            "then"     => Some(Token::KwThen),
            "else"     => Some(Token::KwElse),
            "elseif"   => Some(Token::KwElseif),
            "end"      => Some(Token::KwEnd),
            "while"    => Some(Token::KwWhile),
            "do"       => Some(Token::KwDo),
            "for"      => Some(Token::KwFor),
            "local"    => Some(Token::KwLocal),
            "function" => Some(Token::KwFunction),
            "return"   => Some(Token::KwReturn),
            "true"     => Some(Token::KwTrue),
            "false"    => Some(Token::KwFalse),
            "nil"      => Some(Token::KwNil),
            "and"      => Some(Token::KwAnd),
            "or"       => Some(Token::KwOr),
            "not"      => Some(Token::KwNot),
            _          => None,
        }
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::KwIf       => write!(f, "if"),
            Token::KwThen     => write!(f, "then"),
            Token::KwElse     => write!(f, "else"),
            Token::KwElseif   => write!(f, "elseif"),
            Token::KwEnd      => write!(f, "end"),
            Token::KwWhile    => write!(f, "while"),
            Token::KwDo       => write!(f, "do"),
            Token::KwFor      => write!(f, "for"),
            Token::KwLocal    => write!(f, "local"),
            Token::KwFunction => write!(f, "function"),
            Token::KwReturn   => write!(f, "return"),
            Token::KwTrue     => write!(f, "true"),
            Token::KwFalse    => write!(f, "false"),
            Token::KwNil      => write!(f, "nil"),
            Token::KwAnd      => write!(f, "and"),
            Token::KwOr       => write!(f, "or"),
            Token::KwNot      => write!(f, "not"),
            Token::OpPlus     => write!(f, "+"),
            Token::OpMinus    => write!(f, "-"),
            Token::OpStar     => write!(f, "*"),
            Token::OpSlash    => write!(f, "/"),
            Token::OpPercent  => write!(f, "%"),
            Token::OpEqEq     => write!(f, "=="),
            Token::OpTildeEq  => write!(f, "~="),
            Token::OpLt       => write!(f, "<"),
            Token::OpLtEq     => write!(f, "<="),
            Token::OpGt       => write!(f, ">"),
            Token::OpGtEq     => write!(f, ">="),
            Token::OpEq       => write!(f, "="),
            Token::LitInt(n)  => write!(f, "{n}"),
            Token::LitFloat(v)=> write!(f, "{v}"),
            Token::LitStr(s)  => write!(f, "\"{s}\""),
            Token::Ident(s)   => write!(f, "{s}"),
            Token::LParen     => write!(f, "("),
            Token::RParen     => write!(f, ")"),
            Token::Comma      => write!(f, ","),
            Token::Semicolon  => write!(f, ";"),
            Token::Eof        => write!(f, "<EOF>"),
            Token::Unknown(c) => write!(f, "<unknown '{c}'>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_are_recognized() {
        assert_eq!(Token::from_keyword("if"),    Some(Token::KwIf));
        assert_eq!(Token::from_keyword("while"), Some(Token::KwWhile));
        assert_eq!(Token::from_keyword("true"),  Some(Token::KwTrue));
        assert_eq!(Token::from_keyword("foo"),   None);
    }

    #[test]
    fn is_keyword_covers_all_kw_variants() {
        let keywords = [
            Token::KwIf, Token::KwThen, Token::KwElse, Token::KwElseif,
            Token::KwEnd, Token::KwWhile, Token::KwDo, Token::KwFor,
            Token::KwLocal, Token::KwFunction, Token::KwReturn,
            Token::KwTrue, Token::KwFalse, Token::KwNil,
            Token::KwAnd, Token::KwOr, Token::KwNot,
        ];
        for kw in &keywords {
            assert!(kw.is_keyword(), "{kw:?} deveria ser keyword");
        }
        assert!(!Token::Ident("x".into()).is_keyword());
        assert!(!Token::LitInt(42).is_keyword());
    }

    #[test]
    fn display_roundtrips_operators() {
        assert_eq!(Token::OpEqEq.to_string(),    "==");
        assert_eq!(Token::OpTildeEq.to_string(), "~=");
        assert_eq!(Token::OpLtEq.to_string(),    "<=");
    }

    #[test]
    fn litfloat_exists_but_is_avr_invalid() {
        // O lexer PODE emitir LitFloat. É responsabilidade da análise semântica
        // rejeitar com erro claro quando target == AVR.
        let tok = Token::LitFloat(3.14);
        assert_eq!(format!("{tok}"), "3.14");
    }
}
