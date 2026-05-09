![](logo.png)

# TinyLua

Compilador de um subconjunto de Lua para microcontroladores, com foco inicial em AVR (Arduino) e suporte planejado para ESP32.

## Status

Fase PoC — lexer implementado e testado.

## Workspace

```
crates/
├── tinylua-lexer    ← implementado
├── tinylua-parser   ← placeholder
├── tinylua-sema     ← placeholder
├── tinylua-codegen  ← placeholder
└── tinylua-hal      ← placeholder
```

Pipeline: `lexer → parser → sema → codegen → hal`

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

### Testes

53 unit tests cobrindo:

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
