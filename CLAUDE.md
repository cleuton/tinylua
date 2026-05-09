# TinyLua — Contexto do Projeto

Compilador estático minimalista de Lua para microcontroladores, escrito em Rust.
Versão atual: PoC 3.0 — transpiler Lua → Rust no_std.

## Decisão arquitetural central

O backend desta PoC é um **transpiler estático**: o compilador gera código Rust
`no_std` a partir da AST Lua, e delega a emissão de binário ao `rustc` (target
`avr-atmel-none`) ou `avr-gcc`. Essa decisão é revisável — o frontend
(Lexer → Parser → AST) é independente do backend.

## Target prioritário

**Arduino AVR (ATmega328P)** — `avr-atmel-none`.
ESP32 é fase posterior. Toda decisão de tipo deve considerar AVR primeiro.

## Regras absolutas da PoC

- **Sem heap**: nenhuma alocação dinâmica. Sem `Box`, `Vec`, `malloc`.
- **Sem GC**: ciclo de vida de todos os valores é determinístico.
- **Sem closures**: funções são estáticas, sem captura de variáveis externas.
- **Sem runtime Lua**: nenhuma biblioteca Lua padrão é carregada.
- **Stack estática**: todas as variáveis locais têm tamanho previsível em
  tempo de compilação.
- **Float proibido em AVR**: `LitFloat` é emitido pelo lexer, mas a análise
  semântica deve rejeitar com erro claro referenciando linha/coluna do código
  Lua original — nunca como erro do `rustc`.

## Estrutura de crates (workspace)

```
tinylua/
├── CLAUDE.md
├── Cargo.toml                  # workspace
├── crates/
│   ├── tinylua-lexer/          # Fase 1: tokens + lexer
│   ├── tinylua-parser/         # Fase 2: AST + parser recursivo descendente
│   ├── tinylua-sema/           # Fase 3: análise semântica + verificação de tipos
│   ├── tinylua-codegen/        # Fase 4: gerador de código Rust no_std
│   └── tinylua-hal/            # Fase 3a: HAL para AVR (ATmega328P)
└── tests/                      # testes de integração end-to-end
```

Cada crate é independente. O frontend (lexer, parser, sema) não importa nada
do backend (codegen, hal).

## Subconjunto Lua suportado na PoC

### Tipos
| Tipo Lua     | Rust (AVR)         | Rust (ESP32) |
|--------------|--------------------|--------------|
| number (int) | `i16` / `i32`      | `i32`        |
| number (float)| **ERRO em AVR**   | `f32`        |
| boolean      | `bool`             | `bool`       |
| nil          | sentinela compilação| sentinela   |
| function     | fn estática        | fn estática  |

### Estruturas de controle suportadas
- `if / then / elseif / else / end`
- `while / do / end`
- `for i = start, stop, step do / end` (apenas numérico)
- Funções com parâmetros e retorno simples

### Fora do escopo da PoC
Tabelas dinâmicas, metatabelas, coroutines, strings mutáveis, `pcall/xpcall`,
`require`, múltiplos retornos, closures capturando variáveis externas,
`break`, `repeat/until`, `goto`.

## Tokens definidos (tinylua-lexer)

```rust
// Palavras-chave
KwIf, KwThen, KwElse, KwElseif, KwEnd, KwWhile, KwDo, KwFor,
KwLocal, KwFunction, KwReturn, KwTrue, KwFalse, KwNil, KwAnd, KwOr, KwNot

// Operadores
OpPlus(+), OpMinus(-), OpStar(*), OpSlash(/), OpPercent(%)
OpEqEq(==), OpTildeEq(~=), OpLt(<), OpLtEq(<=), OpGt(>), OpGtEq(>=), OpEq(=)

// Literais
LitInt(i32)     // inteiro
LitFloat(f32)   // float — lexer emite, semântica rejeita em AVR
LitStr(String)  // apenas para args de intrínsecas, ex: pinMode(p, "OUTPUT")

// Identificadores
Ident(String)   // variáveis e funções; intrínsecas chegam como Ident aqui

// Delimitadores
LParen, RParen, Comma, Semicolon

// Controle
Eof, Unknown(char)
```

Intrínsecas chegam como `Ident` no lexer. A distinção ocorre na análise
semântica via tabela de intrínsecas conhecidas.

## Intrínsecas de hardware (reconhecidas pela análise semântica)

| Função Lua              | HAL Rust gerado                        |
|-------------------------|----------------------------------------|
| `digitalRead(pin)`      | `hal::digital_read(pin: u8) -> bool`  |
| `digitalWrite(pin, val)`| `hal::digital_write(pin: u8, val: bool)` |
| `analogRead(pin)`       | `hal::analog_read(pin: u8) -> u16`    |
| `analogWrite(pin, val)` | `hal::analog_write(pin: u8, val: u8)` |
| `delay(ms)`             | `hal::delay_ms(ms: u32)`              |
| `pinMode(pin, mode)`    | `hal::pin_mode(pin: u8, mode: PinMode)` |

Erros de tipo nos argumentos das intrínsecas devem ser detectados na análise
semântica com mensagem clara referenciando a linha do código Lua original.

## Mensagens de erro — regra geral

Todo erro deve referenciar **linha e coluna no código Lua original**.
Erros do `rustc` jamais devem vazar para o usuário.
Formato sugerido: `erro[linha:col]: <mensagem clara em português ou inglês>`

## Exemplos de código Lua válido na PoC

### Exemplo 1 — Blink com sensor digital
```lua
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
```

### Exemplo 2 — Sensor analógico e PWM
```lua
local LDR_PIN = 0
local PWM_LED = 9
while true do
  local luz = analogRead(LDR_PIN)
  local brilho = luz / 4
  analogWrite(PWM_LED, brilho)
  delay(100)
end
```

## Roadmap das fases

| Fase | Nome                  | Status     |
|------|-----------------------|------------|
| 1    | Lexer + Tokens        | ✅ Concluído |
| 2    | Parser + AST          | 🔄 Próximo  |
| 3    | Análise Semântica     | ⏳ Pendente |
| 4    | Gerador Rust no_std   | ⏳ Pendente |
| 3a   | Integração HAL AVR    | ⏳ Pendente |
| 3b   | Integração HAL ESP32  | ⏳ Futuro   |

## Convenções de código

- Tudo em inglês no código (nomes de tipos, funções, variáveis).
- Comentários podem ser em português.
- Mensagens de erro para o usuário final em português.
- `cargo test` deve passar sempre antes de avançar de fase.
- Nenhum `unwrap()` em código de produção — use `Result` com erros tipados.