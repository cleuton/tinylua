![](logo.png)

# TinyLua

Compilador de um subconjunto de Lua para microcontroladores, com foco inicial em AVR (Arduino) e suporte planejado para ESP32.

> 🇺🇸 [English version](ENGLISH.md)

---

## Status

**PoC 3.0 — transpiler Lua → Rust `no_std`**

| Fase | Nome | Status |
|------|------|--------|
| 1 | Lexer + Tokens | ✅ Concluído |
| 2 | Parser + AST | ✅ Concluído |
| 3 | Análise Semântica | ⏳ Pendente |
| 4 | Gerador Rust `no_std` | ⏳ Pendente |
| 3a | Integração HAL AVR | ⏳ Pendente |
| 3b | Integração HAL ESP32 | ⏳ Futuro |

---

## Workspace

```
crates/
├── tinylua-lexer    ← implementado (Fase 1)
├── tinylua-parser   ← implementado (Fase 2)
├── tinylua-sema     ← placeholder  (Fase 3)
├── tinylua-codegen  ← placeholder  (Fase 4)
└── tinylua-hal      ← placeholder  (Fase 3a)
```

Pipeline: `Lexer → Parser → Sema → Codegen → HAL`

---

## tinylua-lexer

Tokeniza o subconjunto Lua suportado. Decisões de design relevantes:

- `LitFloat` é emitido normalmente; a **sema** rejeita se o target for AVR.
- `LitStr` existe apenas para argumentos de intrínsecas (`pinMode(p, "OUTPUT")`), não é valor de primeira classe.
- Intrínsecas de hardware (`digitalRead`, `pinMode`, …) chegam como `Ident`; a sema as distingue via tabela de intrínsecas, mantendo o lexer desacoplado do HAL.
- `Unknown(char)` preserva o caractere inválido com `line`/`col` para mensagens de erro precisas.
- Literais negativos não existem: `-3` é `OpMinus + LitInt(3)`.

### Tokens suportados

| Categoria | Exemplos |
|---|---|
| Palavras-chave | `if then else elseif end while do for local function return true false nil and or not` |
| Operadores aritméticos | `+ - * / %` |
| Comparação / atribuição | `== ~= < <= > >= =` |
| Literais | `LitInt(i32)`, `LitFloat(f32)`, `LitStr(String)` |
| Identificadores | `Ident(String)` |
| Delimitadores | `( ) , ;` |
| Controle | `Eof`, `Unknown(char)` |

`~=` é o not-equal de Lua (não `!=`). `~` sozinho → `Unknown('~')`.

### API

```rust
// Tokenizar tudo de uma vez (inclui Eof final)
let tokens: Vec<SpannedToken> = Lexer::tokenize(src);

// Streaming token a token
let mut lex = Lexer::new(src);
let tok = lex.next_token(); // sempre retorna Eof após fim

// Iterator (para antes do Eof)
for spanned in Lexer::new(src) {
    println!("{:?} @ {}:{}", spanned.node, spanned.line, spanned.col);
}
```

### Testes — 53 casos

- Vazio / só espaço → `Eof`
- Comentários `--` (inclusive sem `\n` final)
- `~=` vs `~` solitário, `=` vs `==`, `<`/`<=`, `>`/`>=`
- `elseif` como token único (não `else` + `if`)
- Prefixo de keyword em identificador (`iffy` → `Ident`)
- Identificadores com `_`
- Inteiros, overflow de `i32` → clamp para `i32::MAX`
- Hex `0xFF` / `0XFF`; `0x` sem dígitos → `Unknown('x')`
- Float `3.14`, `3.` (dot trailing); rejeitado pela sema em AVR
- Strings com escapes `\n \t \r \" \\`; não terminada → `Unknown('"')`
- String encerrada por newline → `Unknown('"')`
- Rastreamento de linha e coluna
- Snippets completos (`local`, `if/then/end`, `while`, chamada de intrínseca)

```
cargo test -p tinylua-lexer
```

---

## tinylua-parser

Parser recursivo descendente que consome os `SpannedToken` do lexer e produz uma AST alocada em `typed-arena`.

### AST

Dois enums principais, ambos com `Span { line, col }` em cada nó:

```
Expr<'a>
├── LitInt(i32)
├── LitFloat(f32)     ← parser emite; sema rejeita em AVR
├── LitStr(String)    ← apenas para args de intrínsecas
├── LitBool(bool)
├── Nil
├── Var(String)
├── BinOp { op: BinOp, lhs, rhs }
├── UnOp  { op: UnOp,  operand }
└── Call  { name, args: Vec<&Expr> }

Stmt<'a>
├── Local    { name, init: Option<&Expr> }
├── Assign   { name, value: &Expr }
├── While    { cond, body: Vec<&Stmt> }
├── If       { cond, then_body, elseif_clauses, else_body }
├── NumFor   { var, start, limit, step: Option, body }
├── Function { name, params: Vec<String>, body }
├── Return   { value: Option<&Expr> }
└── Call     { name, args: Vec<&Expr> }
```

#### Operadores

| Grupo | Operadores | Precedência |
|-------|-----------|-------------|
| `BinOp` lógicos | `or`, `and` | mais fraca |
| `BinOp` comparação | `== ~= < <= > >=` | — |
| `BinOp` aditivos | `+ -` | — |
| `BinOp` multiplicativos | `* / %` | mais forte |
| `UnOp` | `- not` | direita para esquerda |

### Arenas

Os nós são alocados pelo chamador — o crate re-exporta `typed_arena::Arena`:

```rust
use tinylua_parser::{parse, Arena, Expr, Stmt};

let exprs: Arena<Expr<'_>> = Arena::new();
let stmts: Arena<Stmt<'_>> = Arena::new();

let block = parse(src, &exprs, &stmts)?;
```

### Erros

```
erro[linha:col]: <mensagem em português>
```

Todos os erros referenciam a posição exata no código Lua original. Erros do `rustc` nunca chegam ao usuário.

### Testes — 30 casos + 1 doctest

- Programa vazio / só ponto-e-vírgula
- `local` sem e com inicializador
- Atribuição simples
- Chamada de função/intrínseca como instrução e como expressão
- `if / then / end`, `if / else`, `if / elseif / else`
- `while true do end`
- `for` numérico sem e com passo
- `function` sem e com parâmetros + `return`
- Precedência: `1 + 2 * 3` → raiz `Add`, filho direito `Mul`
- Parênteses sobrepõem precedência
- Unário `-` e `not`
- `LitFloat` parseado (sema rejeitará em AVR)
- `nil`, `false`
- Erros com `linha:col` corretos
- Exemplo completo do blink com sensor digital do CLAUDE.md

```
cargo test -p tinylua-parser
```

---

## Rodando todos os testes

```
cargo test --workspace
```

Resultado atual: **86 testes, 0 falhas.**
