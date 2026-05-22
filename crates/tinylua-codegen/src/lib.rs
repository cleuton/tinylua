use tinylua_parser::ast::{BinOp, Expr, Stmt, UnOp};
use tinylua_sema::{SemaOutput, Target};

// ── Tipos públicos ────────────────────────────────────────────────────────────

pub struct CodegenConfig {
    pub target: Target,
}

// ── Ponto de entrada ──────────────────────────────────────────────────────────

pub fn generate(block: &[&Stmt<'_>], _sema: &SemaOutput, _config: &CodegenConfig) -> String {
    let mut gen = Codegen { indent: 0 };

    let mut fns_buf  = String::new();
    let mut main_buf = String::new();

    for stmt in block {
        match stmt {
            Stmt::Function { name, params, body, .. } => {
                gen.indent = 0;
                gen.emit_function(name, params, body, &mut fns_buf);
                fns_buf.push('\n');
            }
            _ => {
                gen.indent = 1;
                gen.emit_stmt(stmt, &mut main_buf);
            }
        }
    }

    let mut out = String::new();
    out.push_str("#![no_std]\n");
    out.push_str("#![no_main]\n");
    out.push_str("use tinylua_hal as hal;\n");
    out.push('\n');
    out.push_str("#[panic_handler]\n");                                              // ← adicionar
    out.push_str("fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }\n");        // ← adicionar
    out.push('\n'); 
    if !fns_buf.is_empty() {
        out.push_str(&fns_buf);
    }
    out.push_str("#[no_mangle]\n");
    out.push_str("pub extern \"C\" fn main() -> ! {\n");
    out.push_str(&main_buf);
    out.push_str("}\n");
    out
}

// ── Gerador ───────────────────────────────────────────────────────────────────

struct Codegen {
    indent: usize,
}

impl Codegen {
    fn pad(&self) -> String {
        "    ".repeat(self.indent)
    }

    // ── Expressões (não mudam indent) ─────────────────────────────────────────

    fn emit_expr(&self, expr: &Expr<'_>) -> String {
        match expr {
            Expr::LitInt(n, _)   => n.to_string(),
            Expr::LitFloat(f, _) => format!("{f}_f32"),
            Expr::LitStr(s, _)   => format!("\"{s}\""),
            Expr::LitBool(b, _)  => b.to_string(),
            Expr::Nil(_)         => "0".to_string(),
            Expr::Var(name, _)   => name.clone(),

            Expr::BinOp { op, lhs, rhs, .. } => format!(
                "({} {} {})",
                self.emit_expr(lhs),
                binop_str(*op),
                self.emit_expr(rhs),
            ),

            Expr::UnOp { op, operand, .. } => match op {
                UnOp::Neg => format!("(-{})", self.emit_expr(operand)),
                UnOp::Not => format!("(!{})", self.emit_expr(operand)),
            },

            Expr::Call { name, args, .. } => self.emit_call_expr(name, args),
        }
    }

    fn emit_call_expr(&self, name: &str, args: &[&Expr<'_>]) -> String {
        match name {
            "digitalRead" => format!(
                "hal::digital_read({}  as u8)",
                self.emit_expr(args[0])
            ),
            "digitalWrite" => format!(
                "hal::digital_write({}  as u8, {})",
                self.emit_expr(args[0]),
                self.emit_expr(args[1])
            ),
            "analogRead" => format!(
                "hal::analog_read({})",
                self.emit_expr(args[0])
            ),
            "analogWrite" => format!(
                "hal::analog_write({}, {})",
                self.emit_expr(args[0]),
                self.emit_expr(args[1])
            ),
            "delay" => format!(
                "hal::delay_ms({} as u32)",
                self.emit_expr(args[0])
            ),
            "pinMode" => format!(
                "hal::pin_mode({}  as u8, {})",
                self.emit_expr(args[0]),
                pinmode_str(args[1])
            ),
            _ => {
                let arg_strs: Vec<String> = args.iter().map(|a| self.emit_expr(a)).collect();
                format!("{}({})", name, arg_strs.join(", "))
            }
        }
    }

    // ── Instruções ────────────────────────────────────────────────────────────

    fn emit_stmt(&mut self, stmt: &Stmt<'_>, out: &mut String) {
        // Captura o pad do nível atual — válido para abertura e fechamento deste nó.
        let pad = self.pad();

        match stmt {
            Stmt::Local { name, init, .. } => {
                let val = init.map_or_else(|| "0".to_string(), |e| self.emit_expr(e));
                out.push_str(&format!("{pad}let mut {name}: i32 = {val};\n"));
            }

            Stmt::Assign { name, value, .. } => {
                let val = self.emit_expr(value);
                out.push_str(&format!("{pad}{name} = {val};\n"));
            }

            Stmt::Call { name, args, .. } => {
                let call = self.emit_call_expr(name, args);
                out.push_str(&format!("{pad}{call};\n"));
            }

            Stmt::Return { value, .. } => match value {
                Some(expr) => {
                    let val = self.emit_expr(expr);
                    out.push_str(&format!("{pad}return {val};\n"));
                }
                None => out.push_str(&format!("{pad}return;\n")),
            },

            Stmt::While { cond, body, .. } => {
                // `while true` → `loop` para satisfazer `-> !` no main.
                match cond {
                    Expr::LitBool(true, _) => out.push_str(&format!("{pad}loop {{\n")),
                    _ => {
                        let c = self.emit_expr(cond);
                        out.push_str(&format!("{pad}while {c} {{\n"));
                    }
                }
                self.indent += 1;
                self.emit_block(body, out);
                self.indent -= 1;
                out.push_str(&format!("{pad}}}\n"));
            }

            Stmt::If { cond, then_body, elseif_clauses, else_body, .. } => {
                let c = self.emit_expr(cond);
                out.push_str(&format!("{pad}if {c} {{\n"));
                self.indent += 1;
                self.emit_block(then_body, out);
                self.indent -= 1;

                for clause in elseif_clauses {
                    let cc = self.emit_expr(clause.cond);
                    out.push_str(&format!("{pad}}} else if {cc} {{\n"));
                    self.indent += 1;
                    self.emit_block(&clause.body, out);
                    self.indent -= 1;
                }

                if let Some(body) = else_body {
                    out.push_str(&format!("{pad}}} else {{\n"));
                    self.indent += 1;
                    self.emit_block(body, out);
                    self.indent -= 1;
                }

                out.push_str(&format!("{pad}}}\n"));
            }

            // `for i = start, limit do` → for range Rust
            Stmt::NumFor { var, start, limit, step: None, body, .. } => {
                let s = self.emit_expr(start);
                let l = self.emit_expr(limit);
                out.push_str(&format!("{pad}for {var} in {s}..={l} {{\n"));
                self.indent += 1;
                self.emit_block(body, out);
                self.indent -= 1;
                out.push_str(&format!("{pad}}}\n"));
            }

            // `for i = start, limit, step do` → while com incremento manual
            Stmt::NumFor { var, start, limit, step: Some(step_expr), body, .. } => {
                let s  = self.emit_expr(start);
                let l  = self.emit_expr(limit);
                let st = self.emit_expr(step_expr);
                // Pré-computa pads para todos os níveis antes de alterar self.indent.
                let inner_pad = "    ".repeat(self.indent + 1);
                let body_pad  = "    ".repeat(self.indent + 2);
                out.push_str(&format!("{pad}{{\n"));
                out.push_str(&format!("{inner_pad}let mut {var}: i32 = {s};\n"));
                out.push_str(&format!("{inner_pad}while {var} <= {l} {{\n"));
                self.indent += 2;
                self.emit_block(body, out);
                self.indent -= 2;
                // Incremento fica dentro do while, depois do corpo.
                out.push_str(&format!("{body_pad}{var} += {st};\n"));
                out.push_str(&format!("{inner_pad}}}\n"));
                out.push_str(&format!("{pad}}}\n"));
            }

            Stmt::Function { name, params, body, .. } => {
                // Função aninhada (não prevista na PoC) — emite inline.
                self.emit_function(name, params, body, out);
            }
        }
    }

    fn emit_block(&mut self, stmts: &[&Stmt<'_>], out: &mut String) {
        for s in stmts {
            self.emit_stmt(s, out);
        }
    }

    fn emit_function(&mut self, name: &str, params: &[String], body: &[&Stmt<'_>], out: &mut String) {
        let params_str = params.iter()
            .map(|p| format!("{p}: i32"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("fn {name}({params_str}) -> i32 {{\n"));
        self.indent = 1;
        self.emit_block(body, out);
        self.indent = 0;
        out.push_str("}\n");
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn binop_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",  BinOp::Sub => "-",
        BinOp::Mul => "*",  BinOp::Div => "/",  BinOp::Mod => "%",
        BinOp::Eq  => "==", BinOp::Ne  => "!=",
        BinOp::Lt  => "<",  BinOp::Le  => "<=",
        BinOp::Gt  => ">",  BinOp::Ge  => ">=",
        BinOp::And => "&&", BinOp::Or  => "||",
    }
}

fn pinmode_str(expr: &Expr<'_>) -> &'static str {
    match expr {
        Expr::LitStr(s, _) => match s.as_str() {
            "OUTPUT" => "hal::PinMode::Output",
            "INPUT"  => "hal::PinMode::Input",
            _        => "hal::PinMode::Input",
        },
        _ => "hal::PinMode::Input",
    }
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tinylua_parser::{parse, Arena, Expr, Stmt};
    use tinylua_sema::{analyse, SemaConfig};

    fn avr() -> CodegenConfig { CodegenConfig { target: Target::Avr } }
    fn esp() -> CodegenConfig { CodegenConfig { target: Target::Esp32 } }

    fn run<'a>(
        src: &'a str,
        cfg: CodegenConfig,
        ae:  &'a Arena<Expr<'a>>,
        as_: &'a Arena<Stmt<'a>>,
    ) -> String {
        let stmts = parse(src, ae, as_).expect("parse failed");
        let sema_out = analyse(&stmts, &SemaConfig { target: cfg.target });
        for e in &sema_out.errors { eprintln!("sema: {e}"); }
        generate(&stmts, &sema_out, &cfg)
    }

    #[test]
    fn local_com_init() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("local x = 42", avr(), &ae, &as_);
        assert!(out.contains("let mut x: i32 = 42;"), "output:\n{out}");
    }

    #[test]
    fn local_sem_init() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("local x", avr(), &ae, &as_);
        assert!(out.contains("let mut x: i32 = 0;"), "output:\n{out}");
    }

    #[test]
    fn for_sem_step() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("for i = 1, 10 do end", avr(), &ae, &as_);
        assert!(out.contains("for i in 1..=10 {"), "output:\n{out}");
    }

    #[test]
    fn for_com_step() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("for i = 0, 10, 2 do end", avr(), &ae, &as_);
        assert!(out.contains("let mut i: i32 = 0;"), "output:\n{out}");
        assert!(out.contains("while i <= 10 {"), "output:\n{out}");
        assert!(out.contains("i += 2;"), "output:\n{out}");
    }

    #[test]
    fn funcao_emitida_antes_do_main() {
        let ae = Arena::new(); let as_ = Arena::new();
        let src = "function add(a, b)\n  return a\nend";
        let out = run(src, avr(), &ae, &as_);
        assert!(out.contains("fn add(a: i32, b: i32) -> i32 {"), "output:\n{out}");
        let fn_pos   = out.find("fn add").unwrap();
        let main_pos = out.find("fn main").unwrap();
        assert!(fn_pos < main_pos, "função deve aparecer antes do main\n{out}");
    }

    #[test]
    fn if_elseif_else() {
        let ae = Arena::new(); let as_ = Arena::new();
        let src = "local x = 0\nlocal y = 0\nlocal z = 0\n\
                   if x == 1 then\n  y = 1\n\
                   elseif z == 1 then\n  y = 2\n\
                   else\n  y = 3\nend";
        let out = run(src, avr(), &ae, &as_);
        assert!(out.contains("if (x == 1) {"),       "output:\n{out}");
        assert!(out.contains("} else if (z == 1) {"), "output:\n{out}");
        assert!(out.contains("} else {"),             "output:\n{out}");
    }

    #[test]
    fn blink_hal_calls() {
        let ae = Arena::new(); let as_ = Arena::new();
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
        let out = run(src, avr(), &ae, &as_);
        assert!(out.contains("hal::pin_mode("),      "output:\n{out}");
        assert!(out.contains("hal::digital_read("),  "output:\n{out}");
        assert!(out.contains("hal::digital_write("), "output:\n{out}");
        assert!(out.contains("hal::delay_ms("),      "output:\n{out}");
        assert!(out.contains("loop {"),              "output:\n{out}");
    }

    #[test]
    fn sensor_analogico_casts() {
        let ae = Arena::new(); let as_ = Arena::new();
        let src = r#"
local LDR_PIN = 0
local PWM_LED = 9
while true do
  local luz = analogRead(LDR_PIN)
  local brilho = luz / 4
  analogWrite(PWM_LED, brilho)
  delay(100)
end
"#;
        let out = run(src, esp(), &ae, &as_);
        assert!(out.contains("hal::analog_read("),  "output:\n{out}");
        assert!(out.contains("hal::analog_write("), "output:\n{out}");
        assert!(out.contains("as u8"),              "output:\n{out}");
        assert!(out.contains("as u32"),             "output:\n{out}");
    }

    #[test]
    fn blink_estrutura_completa() {
        let ae = Arena::new(); let as_ = Arena::new();
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
        let out = run(src, avr(), &ae, &as_);
        assert!(out.contains("#![no_std]"),                       "output:\n{out}");
        assert!(out.contains("#![no_main]"),                      "output:\n{out}");
        assert!(out.contains("use tinylua_hal as hal;"),          "output:\n{out}");
        assert!(out.contains("pub extern \"C\" fn main() -> !"), "output:\n{out}");
        // Imprime o output para inspeção visual nos logs do test.
        println!("{out}");
    }
}
