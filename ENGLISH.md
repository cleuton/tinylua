![](logo.png)

# TinyLua

A compiler for a subset of Lua targeting microcontrollers, with an initial focus on AVR (Arduino) and planned support for ESP32.

> 🇧🇷 [Versão em português](README.md)

---

## Status

**PoC 3.0 — Lua → Rust `no_std` transpiler**

| Phase | Name | Status |
|-------|------|--------|
| 1 | Lexer + Tokens | ✅ Done |
| 2 | Parser + AST | ✅ Done |
| 3 | Semantic Analysis | ⏳ Pending |
| 4 | Rust `no_std` Code Generator | ⏳ Pending |
| 3a | AVR HAL Integration | ⏳ Pending |
| 3b | ESP32 HAL Integration | ⏳ Future |

---

## Workspace

```
crates/
├── tinylua-lexer    ← implemented (Phase 1)
├── tinylua-parser   ← implemented (Phase 2)
├── tinylua-sema     ← placeholder (Phase 3)
├── tinylua-codegen  ← placeholder (Phase 4)
└── tinylua-hal      ← placeholder (Phase 3a)
```

Pipeline: `Lexer → Parser → Sema → Codegen → HAL`

---

## tinylua-lexer

Tokenizes the supported Lua subset. Key design decisions:

- `LitFloat` is emitted normally; the **sema** phase rejects it when the target is AVR.
- `LitStr` exists only as arguments to hardware intrinsics (`pinMode(p, "OUTPUT")`); it is not a first-class value.
- Hardware intrinsics (`digitalRead`, `pinMode`, …) arrive as `Ident` tokens; the sema phase distinguishes them via an intrinsics table, keeping the lexer decoupled from the HAL.
- `Unknown(char)` preserves the invalid character along with `line`/`col` for precise error messages.
- Negative literals do not exist: `-3` is tokenized as `OpMinus + LitInt(3)`.

### Supported tokens

| Category | Examples |
|---|---|
| Keywords | `if then else elseif end while do for local function return true false nil and or not` |
| Arithmetic operators | `+ - * / %` |
| Comparison / assignment | `== ~= < <= > >= =` |
| Literals | `LitInt(i32)`, `LitFloat(f32)`, `LitStr(String)` |
| Identifiers | `Ident(String)` |
| Delimiters | `( ) , ;` |
| Control | `Eof`, `Unknown(char)` |

`~=` is Lua's not-equal operator (not `!=`). A bare `~` produces `Unknown('~')`.

### API

```rust
// Tokenize everything at once (includes the final Eof)
let tokens: Vec<SpannedToken> = Lexer::tokenize(src);

// Stream token by token
let mut lex = Lexer::new(src);
let tok = lex.next_token(); // always returns Eof after end of input

// Iterator (stops before Eof)
for spanned in Lexer::new(src) {
    println!("{:?} @ {}:{}", spanned.node, spanned.line, spanned.col);
}
```

### Tests — 53 cases

- Empty / whitespace-only input → `Eof`
- `--` line comments (including without a trailing `\n`)
- `~=` vs bare `~`, `=` vs `==`, `<`/`<=`, `>`/`>=`
- `elseif` as a single token (not `else` + `if`)
- Keyword prefix inside identifier (`iffy` → `Ident`)
- Identifiers starting with `_`
- Integer literals; `i32` overflow → clamped to `i32::MAX`
- Hex `0xFF` / `0XFF`; `0x` with no digits → `Unknown('x')`
- Float `3.14`, `3.` (trailing dot); rejected by sema on AVR targets
- Strings with escape sequences `\n \t \r \" \\`; unterminated → `Unknown('"')`
- String terminated by newline → `Unknown('"')`
- Line and column tracking
- Full snippets (`local`, `if/then/end`, `while`, intrinsic call)

```
cargo test -p tinylua-lexer
```

---

## tinylua-parser

A recursive-descent parser that consumes `SpannedToken`s from the lexer and produces an AST allocated in a `typed-arena`.

### AST

Two main enums, both carrying `Span { line, col }` on every node:

```
Expr<'a>
├── LitInt(i32)
├── LitFloat(f32)     ← parser emits; sema rejects on AVR
├── LitStr(String)    ← only for intrinsic arguments
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

#### Operators

| Group | Operators | Precedence |
|-------|-----------|------------|
| `BinOp` logical | `or`, `and` | lowest |
| `BinOp` comparison | `== ~= < <= > >=` | — |
| `BinOp` additive | `+ -` | — |
| `BinOp` multiplicative | `* / %` | highest |
| `UnOp` | `- not` | right-to-left |

### Arenas

Nodes are allocated by the caller — the crate re-exports `typed_arena::Arena`:

```rust
use tinylua_parser::{parse, Arena, Expr, Stmt};

let exprs: Arena<Expr<'_>> = Arena::new();
let stmts: Arena<Stmt<'_>> = Arena::new();

let block = parse(src, &exprs, &stmts)?;
```

### Errors

```
erro[line:col]: <message>
```

Every error references the exact position in the original Lua source. Errors from `rustc` never surface to the user.

### Tests — 30 cases + 1 doctest

- Empty program / semicolons only
- `local` without and with an initializer
- Simple assignment
- Function/intrinsic call as a statement and as an expression
- `if / then / end`, `if / else`, `if / elseif / else`
- `while true do end`
- Numeric `for` without and with a step
- `function` without and with parameters + `return`
- Precedence: `1 + 2 * 3` → root is `Add`, right child is `Mul`
- Parentheses override precedence
- Unary `-` and `not`
- `LitFloat` is parsed (sema will reject on AVR)
- `nil`, `false`
- Errors carry correct `line:col`
- Full digital-sensor blink example from CLAUDE.md

```
cargo test -p tinylua-parser
```

---

## Running all tests

```
cargo test --workspace
```

Current result: **86 tests, 0 failures.**
