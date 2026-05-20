![](logo.png)

# TinyLua

A compiler for a subset of Lua targeting microcontrollers, with an initial focus on AVR (Arduino) and planned support for ESP32.

> 🇧🇷 [Versão em português](README.md)

---

## Prerequisites

### macOS

```bash
brew tap osx-cross/avr
brew install avr-gcc
brew install avrdude
```

Verify the installation:

```bash
avr-gcc --version
avrdude -v 2>&1 | head -1
```

### Linux (Ubuntu/Debian)

```bash
sudo apt update
sudo apt install gcc-avr avrdude
```

Verify the installation:

```bash
avr-gcc --version
avrdude -v 2>&1 | head -1
```

### Rust toolchain configuration

Create `rust-toolchain.toml` at the project root:

```toml
[toolchain]
channel = "nightly"
components = ["rust-src"]
```

`.cargo/config.toml` (workspace root — target flags only, no global build target):

```toml
[target.avr-none]
linker = "avr-gcc"
rustflags = [
    "-C", "target-cpu=atmega328p",
    "-C", "link-arg=-mmcu=atmega328p",
]
```

`crates/tinylua-hal/.cargo/config.toml` (read only when building the HAL):

```toml
[build]
target = "avr-none"

[target.avr-none]
linker = "avr-gcc"
rustflags = [
    "-C", "target-cpu=atmega328p",
    "-C", "link-arg=-mmcu=atmega328p",
]

[unstable]
build-std = ["core"]
```

This separation ensures that host crates (`tinylua-cli`, `tinylua-lexer`, etc.) compile normally for the host without missing `String`/`Vec` errors.

To compile the HAL for AVR (validates the toolchain):

```bash
cd crates/tinylua-hal
cargo +nightly build --release
```

> **Note:** `avr-unknown-gnu-atmega328` is not a built-in Rust target. The correct target is `avr-none` with `-C target-cpu=atmega328p`. The `build-std = ["core"]` flag compiles `core` from source, since no precompiled `rust-std` exists for AVR.

---

## Status

**PoC 3.0 — Lua → Rust `no_std` transpiler**

| Phase | Name | Status |
|-------|------|--------|
| 1 | Lexer + Tokens | ✅ Done (53 tests) |
| 2 | Parser + AST | ✅ Done (86 tests) |
| 3 | Semantic Analysis | ✅ Done (9 tests) |
| 4 | Rust `no_std` Generator | ✅ Done (8 tests) |
| 3a | AVR HAL (ATmega328P) | ✅ Done — tested on a real Arduino UNO |
| CLI | tinylua-cli | ✅ Done — full end-to-end pipeline |
| 3b | ESP32 HAL | ⏳ Future |

---

## Full Pipeline

The `Lua → Rust no_std` pipeline is functional end to end:

```
file.lua → Lexer → Parser → Sema → Codegen → file.rs → avr-gcc → .hex
```

To compile a Lua program for AVR:

```bash
# 1. Generate .rs from .lua
cargo run -p tinylua-cli -- blink.lua
# → writes blink.rs next to blink.lua

# 2. Copy the generated .rs to the HAL binary
cp blink.rs crates/tinylua-hal/src/main.rs

# 3. Compile for AVR (from the HAL directory)
cd crates/tinylua-hal
cargo +nightly build --release
cd ../..

# 4. Generate the .hex file
avr-objcopy -O ihex -R .eeprom \
    target/avr-none/release/tinylua-hal.elf blink.hex

# 5. Flash to Arduino UNO (adjust the serial port)
avrdude -c arduino -p atmega328p -P /dev/cu.usbmodem1201 -b 115200 \
    -U flash:w:blink.hex
```

---

## Workspace

```
crates/
├── tinylua-lexer    ← implemented (Phase 1)
├── tinylua-parser   ← implemented (Phase 2)
├── tinylua-sema     ← implemented (Phase 3)
├── tinylua-codegen  ← implemented (Phase 4)
├── tinylua-hal      ← implemented (Phase 3a)
└── tinylua-cli      ← compiler CLI
```

Pipeline: `Lexer → Parser → Sema → Codegen → HAL`

---

## tinylua-cli

Command-line interface that runs the full pipeline (Lexer → Parser → Sema → Codegen) and writes the generated `.rs` to disk.

### Usage

```
tinylua-cli <file.lua> [--target avr|esp32]
```

The default target is `avr`. The `.rs` file is written next to the `.lua` input, with the same base name.

### Example — blink.lua

```lua
-- Turns the LED on for 1 second, off for 1 second, repeats forever.
-- Pin 13 = LED_BUILTIN on the Arduino UNO

local LED_PIN = 13

pinMode(LED_PIN, "OUTPUT")

while true do
    digitalWrite(LED_PIN, true)   -- HIGH
    delay(1000)
    digitalWrite(LED_PIN, false)  -- LOW
    delay(1000)
end
```

Compile and flash to an Arduino UNO:

```bash
# Generate blink.rs at the project root
cargo run -p tinylua-cli -- blink.lua

# Copy to the HAL binary and compile for AVR
cp blink.rs crates/tinylua-hal/src/main.rs
cd crates/tinylua-hal && cargo +nightly build --release && cd ../..

# Generate the .hex file
avr-objcopy -O ihex -R .eeprom \
    target/avr-none/release/tinylua-hal.elf blink.hex

# Flash to Arduino UNO (Linux: use /dev/ttyUSB0)
avrdude -c arduino -p atmega328p -P /dev/cu.usbmodem1201 -b 115200 \
    -U flash:w:blink.hex
```

> Physically tested on an Arduino UNO (ATmega328P) — LED blinked as expected.

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

## tinylua-sema

Semantic analysis phase that consumes the parser's AST and validates the Lua program before code generation. It accumulates **all** errors without early-return — the user sees the complete list of issues at once.

### API

```rust
use tinylua_sema::{analyse, SemaConfig, Target};

let config = SemaConfig { target: Target::Avr };
let out = analyse(&stmts, &config);

for err in &out.errors {
    eprintln!("{err}"); // "erro[line:col]: message"
}
// out.expr_types: HashMap<usize, TinyType> — inferred type of each Expr node
```

### Types

```rust
pub enum TinyType { Int, Float, Bool, Str, Nil }
```

### Implemented rules

| # | Rule | Error message |
|---|------|---------------|
| 1 | `LitFloat` with `target == Avr` | `"ponto flutuante não é suportado no target AVR (line:col)"` |
| 2 | `local x` without init → type `Nil`; used in expression | `"variável 'x' usada antes de ser inicializada"` |
| 3 | Variable not present in any scope frame | `"variável 'x' não declarada"` |
| 4 | Function parameters | registered with type `Int` by default |
| 5 | Direct recursion inside a function body | `"recursão não é suportada na PoC (função 'f')"` |
| 6 | `return` at the top level | accepted without error |
| 7 | Intrinsic arity mismatch | `"'delay' espera 1 argumento, recebeu 2"` |
| 8 | Intrinsic argument type mismatch | `"argumento 1 de 'digitalWrite' deve ser Bool, recebeu Int"` |

### Recognized intrinsics

| Lua | Parameters | Return |
|-----|-----------|--------|
| `digitalRead(pin)` | `Int` | `Bool` |
| `digitalWrite(pin, val)` | `Int, Bool` | — |
| `analogRead(pin)` | `Int` | `Int` |
| `analogWrite(pin, val)` | `Int, Int` | — |
| `delay(ms)` | `Int` | — |
| `pinMode(pin, mode)` | `Int, Str` | — |

### Scopes

`push/pop` per block: `while`, `if/elseif/else`, numeric `for` (loop variable typed `Int`), `function` (parameters typed `Int`). Assigning to a `Nil`-typed variable inside an inner scope promotes its type in the outer frame where it was declared.

### Tests — 10 cases

- Float on AVR → error with correct span at line 1
- Float on ESP32 → no error
- Undeclared variable
- Variable used before initialization (`local x` without a value)
- Direct recursion is rejected
- `delay("text")` → argument 1 type error
- `delay(1, 2)` → arity error
- Full digital-sensor blink example (AVR) → 0 errors
- Analog sensor with PWM example (ESP32) → 0 errors
- `local x` initialized inside an `if` block and used afterwards → 0 errors

```
cargo test -p tinylua-sema
```

---

## tinylua-codegen

The transpiler that walks the sema-validated AST and emits `no_std` Rust source ready to compile with `rustc` targeting AVR or ESP32.

### API

```rust
use tinylua_codegen::{generate, CodegenConfig};
use tinylua_sema::Target;

let config = CodegenConfig { target: Target::Avr };
let rust_src: String = generate(&stmts, &sema_out, &config);
```

### Generated file structure

```rust
#![no_std]
#![no_main]
use tinylua_hal as hal;

// user functions (Stmt::Function) — before main

#[no_mangle]
pub extern "C" fn main() -> ! {
    // program body (remaining statements)
}
```

### Lua → Rust mapping

| Lua AST | Generated Rust |
|---------|---------------|
| `local x = expr` | `let mut x: i32 = expr;` |
| `local x` | `let mut x: i32 = 0;` |
| `x = expr` | `x = expr;` |
| `while cond do` | `while cond {` |
| `while true do` | `loop {` (satisfies `-> !`) |
| `for i = s, l do` | `for i in s..=l {` |
| `for i = s, l, step do` | `while` block with manual increment |
| `if / elseif / else` | `if / } else if / } else {` |
| `function f(a, b)` | `fn f(a: i32, b: i32) -> i32 {` |
| `return expr` | `return expr;` |

### Intrinsics — HAL mapping

| Lua | Rust |
|-----|------|
| `digitalRead(pin)` | `hal::digital_read(pin)` |
| `digitalWrite(pin, val)` | `hal::digital_write(pin, val)` |
| `analogRead(pin)` | `hal::analog_read(pin)` |
| `analogWrite(pin, val)` | `hal::analog_write(pin, val as u8)` |
| `delay(ms)` | `hal::delay_ms(ms as u32)` |
| `pinMode(pin, "OUTPUT")` | `hal::pin_mode(pin, hal::PinMode::Output)` |
| `pinMode(pin, "INPUT")` | `hal::pin_mode(pin, hal::PinMode::Input)` |

Casts (`as u8`, `as u32`) are emitted only at the intrinsic call site — all variables are always declared as `i32`.

### Example output — digital-sensor blink

```lua
-- Lua input
local LED_PIN = 13
local SENSOR_PIN = 2
pinMode(LED_PIN, "OUTPUT")
while true do
  local state = digitalRead(SENSOR_PIN)
  if state == true then
    digitalWrite(LED_PIN, true)
    delay(500)
  else
    digitalWrite(LED_PIN, false)
  end
end
```

```rust
// Generated Rust output
#![no_std]
#![no_main]
use tinylua_hal as hal;

#[no_mangle]
pub extern "C" fn main() -> ! {
    let mut LED_PIN: i32 = 13;
    let mut SENSOR_PIN: i32 = 2;
    hal::pin_mode(LED_PIN, hal::PinMode::Output);
    loop {
        let mut state: i32 = hal::digital_read(SENSOR_PIN);
        if (state == true) {
            hal::digital_write(LED_PIN, true);
            hal::delay_ms(500 as u32);
        } else {
            hal::digital_write(LED_PIN, false);
        }
    }
}
```

### Tests — 9 cases

- `local x = 42` → `let mut x: i32 = 42;`
- `local x` → `let mut x: i32 = 0;`
- `for i = 1, 10 do end` → `for i in 1..=10 {`
- `for i = 0, 10, 2 do end` → while with `let mut i`, `while i <= 10`, `i += 2;`
- `function add(a, b) return a end` → `fn add` emitted before `fn main`
- `if / elseif / else` → `if / } else if / } else {`
- AVR blink → contains `hal::pin_mode`, `hal::digital_read`, `hal::digital_write`, `hal::delay_ms`, `loop {`
- Analog sensor → contains `hal::analog_read`, `hal::analog_write`, `as u8`, `as u32`
- Full blink → contains `#![no_std]`, `#![no_main]`, `use tinylua_hal as hal;`, `pub extern "C" fn main() -> !`

```
cargo test -p tinylua-codegen
```

---

## Running all tests

```
cargo test --workspace
```

Current result: **104 tests, 0 failures.**
