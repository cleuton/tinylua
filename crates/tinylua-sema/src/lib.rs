use std::collections::HashMap;
use std::fmt;

use tinylua_parser::ast::{BinOp, Expr, Span, Stmt, UnOp};

// ── Tipos públicos ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target { Avr, Esp32 }

pub struct SemaConfig {
    pub target: Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TinyType { Int, Float, Bool, Str, Nil }

impl fmt::Display for TinyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TinyType::Int   => write!(f, "Int"),
            TinyType::Float => write!(f, "Float"),
            TinyType::Bool  => write!(f, "Bool"),
            TinyType::Str   => write!(f, "Str"),
            TinyType::Nil   => write!(f, "Nil"),
        }
    }
}

pub struct SemaError {
    pub message: String,
    pub span:    Span,
}

impl fmt::Display for SemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro[{}:{}]: {}", self.span.line, self.span.col, self.message)
    }
}

pub struct SemaOutput {
    /// Mapa de endereço do nó `Expr` (como `usize`) para seu tipo inferido.
    /// O endereço é único enquanto a arena viver — nunca é desreferenciado.
    pub expr_types: HashMap<usize, TinyType>,
    pub errors:     Vec<SemaError>,
}

// ── Tabela de símbolos ────────────────────────────────────────────────────────

#[derive(Clone)]
struct VarInfo {
    ty:   TinyType,
    #[allow(dead_code)]
    span: Span,
}

struct Scope {
    frames: Vec<HashMap<String, VarInfo>>,
}

impl Scope {
    fn new() -> Self {
        Self { frames: vec![HashMap::new()] }
    }

    fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.frames.pop();
    }

    fn define(&mut self, name: &str, info: VarInfo) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name.to_string(), info);
        }
    }

    fn lookup(&self, name: &str) -> Option<&VarInfo> {
        for frame in self.frames.iter().rev() {
            if let Some(info) = frame.get(name) {
                return Some(info);
            }
        }
        None
    }
}

// ── Assinaturas de intrínsecas ────────────────────────────────────────────────

fn intrinsic_sig(name: &str) -> Option<(&'static [TinyType], Option<TinyType>)> {
    match name {
        "digitalRead"  => Some((&[TinyType::Int],                         Some(TinyType::Bool))),
        "digitalWrite" => Some((&[TinyType::Int, TinyType::Bool],          None)),
        "analogRead"   => Some((&[TinyType::Int],                         Some(TinyType::Int))),
        "analogWrite"  => Some((&[TinyType::Int, TinyType::Int],           None)),
        "delay"        => Some((&[TinyType::Int],                         None)),
        "pinMode"      => Some((&[TinyType::Int, TinyType::Str],           None)),
        _ => None,
    }
}

// ── Analisador ────────────────────────────────────────────────────────────────

struct Analyser<'a> {
    config:     &'a SemaConfig,
    scope:      Scope,
    errors:     Vec<SemaError>,
    expr_types: HashMap<usize, TinyType>,
    /// Nome da função sendo analisada agora (para detectar recursão).
    current_fn: Option<String>,
}

impl<'a> Analyser<'a> {
    fn new(config: &'a SemaConfig) -> Self {
        Self {
            config,
            scope:      Scope::new(),
            errors:     Vec::new(),
            expr_types: HashMap::new(),
            current_fn: None,
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(SemaError { message: message.into(), span });
    }

    // ── Expressões ────────────────────────────────────────────────────────────

    fn check_expr(&mut self, expr: &'a Expr<'a>) -> TinyType {
        let ty = match expr {
            Expr::LitInt(_, _) => TinyType::Int,

            Expr::LitFloat(_, span) => {
                if self.config.target == Target::Avr {
                    self.error(
                        format!("ponto flutuante não é suportado no target AVR ({}:{})",
                                span.line, span.col),
                        *span,
                    );
                }
                TinyType::Float
            }

            Expr::LitStr(_, _)  => TinyType::Str,
            Expr::LitBool(_, _) => TinyType::Bool,
            Expr::Nil(_)        => TinyType::Nil,

            Expr::Var(name, span) => {
                match self.scope.lookup(name) {
                    None => {
                        self.error(format!("variável '{name}' não declarada"), *span);
                        TinyType::Nil
                    }
                    Some(info) => {
                        let ty = info.ty;
                        if ty == TinyType::Nil {
                            self.error(
                                format!("variável '{name}' usada antes de ser inicializada"),
                                *span,
                            );
                        }
                        ty
                    }
                }
            }

            Expr::BinOp { op, lhs, rhs, .. } => {
                let lt = self.check_expr(lhs);
                let rt = self.check_expr(rhs);
                self.binop_type(*op, lt, rt)
            }

            Expr::UnOp { op, operand, .. } => {
                let ot = self.check_expr(operand);
                match op {
                    UnOp::Neg => ot,
                    UnOp::Not => TinyType::Bool,
                }
            }

            Expr::Call { name, args, span } => {
                // Detecta recursão
                if let Some(ref fname) = self.current_fn.clone() {
                    if name == fname {
                        self.error(
                            format!("recursão não é suportada na PoC (função '{name}')"),
                            *span,
                        );
                    }
                }

                let arg_types: Vec<TinyType> = args.iter()
                    .map(|a| self.check_expr(a))
                    .collect();

                if let Some((param_tys, ret_ty)) = intrinsic_sig(name) {
                    self.check_call_args(name, param_tys, &arg_types, args, *span);
                    ret_ty.unwrap_or(TinyType::Nil)
                } else {
                    // Função do usuário — retorno Int por padrão
                    TinyType::Int
                }
            }
        };

        self.expr_types.insert(expr as *const Expr<'a> as usize, ty);
        ty
    }

    fn binop_type(&self, op: BinOp, _lt: TinyType, _rt: TinyType) -> TinyType {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => TinyType::Int,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => TinyType::Bool,
            BinOp::And | BinOp::Or => TinyType::Bool,
        }
    }

    fn check_call_args(
        &mut self,
        name:      &str,
        expected:  &[TinyType],
        got_types: &[TinyType],
        args:      &[&'a Expr<'a>],
        span:      Span,
    ) {
        if got_types.len() != expected.len() {
            self.error(
                format!("'{}' espera {} argumento{}, recebeu {}",
                        name,
                        expected.len(),
                        if expected.len() == 1 { "" } else { "s" },
                        got_types.len()),
                span,
            );
            return;
        }
        for (i, (exp, got)) in expected.iter().zip(got_types.iter()).enumerate() {
            if exp != got {
                let arg_span = args[i].span();
                self.error(
                    format!("argumento {} de '{}' deve ser {}, recebeu {}",
                            i + 1, name, exp, got),
                    arg_span,
                );
            }
        }
    }

    // ── Instruções ────────────────────────────────────────────────────────────

    fn check_block(&mut self, stmts: &[&'a Stmt<'a>]) {
        for s in stmts {
            self.check_stmt(s);
        }
    }

    fn check_stmt(&mut self, stmt: &'a Stmt<'a>) {
        match stmt {
            Stmt::Local { name, init, span } => {
                let ty = match init {
                    Some(expr) => self.check_expr(expr),
                    None       => TinyType::Nil,
                };
                self.scope.define(name, VarInfo { ty, span: *span });
            }

            Stmt::Assign { name, value, span } => {
                let vty = self.check_expr(value);
                match self.scope.lookup(name) {
                    None => {
                        self.error(format!("variável '{name}' não declarada"), *span);
                    }
                    Some(info) => {
                        let prev_ty = info.ty;
                        // Atualiza tipo no frame mais interno que contém a variável
                        if prev_ty == TinyType::Nil {
                            // Atualiza para o tipo do valor atribuído
                            for frame in self.scope.frames.iter_mut().rev() {
                                if let Some(entry) = frame.get_mut(name) {
                                    entry.ty = vty;
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            Stmt::Call { name, args, span } => {
                // Detecta recursão
                if let Some(ref fname) = self.current_fn.clone() {
                    if name == fname {
                        self.error(
                            format!("recursão não é suportada na PoC (função '{name}')"),
                            *span,
                        );
                    }
                }

                let arg_types: Vec<TinyType> = args.iter()
                    .map(|a| self.check_expr(a))
                    .collect();

                if let Some((param_tys, _)) = intrinsic_sig(name) {
                    self.check_call_args(name, param_tys, &arg_types, args, *span);
                }
                // chamadas a funções do usuário como stmt não exigem validação adicional
            }

            Stmt::While { cond, body, .. } => {
                self.check_expr(cond);
                self.scope.push();
                self.check_block(body);
                self.scope.pop();
            }

            Stmt::If { cond, then_body, elseif_clauses, else_body, .. } => {
                self.check_expr(cond);

                self.scope.push();
                self.check_block(then_body);
                self.scope.pop();

                for clause in elseif_clauses {
                    self.check_expr(clause.cond);
                    self.scope.push();
                    self.check_block(&clause.body);
                    self.scope.pop();
                }

                if let Some(body) = else_body {
                    self.scope.push();
                    self.check_block(body);
                    self.scope.pop();
                }
            }

            Stmt::NumFor { var, start, limit, step, body, span } => {
                self.check_expr(start);
                self.check_expr(limit);
                if let Some(s) = step { self.check_expr(s); }

                self.scope.push();
                self.scope.define(var, VarInfo { ty: TinyType::Int, span: *span });
                self.check_block(body);
                self.scope.pop();
            }

            Stmt::Function { name, params, body, span } => {
                // Registra a função no escopo externo antes de analisar o corpo,
                // mas a recursão será detectada dentro do corpo.
                self.scope.define(name, VarInfo { ty: TinyType::Int, span: *span });

                let prev_fn = self.current_fn.replace(name.clone());

                self.scope.push();
                for p in params {
                    self.scope.define(p, VarInfo { ty: TinyType::Int, span: *span });
                }
                self.check_block(body);
                self.scope.pop();

                self.current_fn = prev_fn;
            }

            Stmt::Return { value, .. } => {
                if let Some(expr) = value {
                    self.check_expr(expr);
                }
            }
        }
    }
}

// ── Ponto de entrada público ──────────────────────────────────────────────────

pub fn analyse<'a>(block: &[&'a Stmt<'a>], config: &SemaConfig) -> SemaOutput {
    let mut analyser = Analyser::new(config);
    analyser.check_block(block);

    SemaOutput {
        expr_types: analyser.expr_types,
        errors:     analyser.errors,
    }
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tinylua_parser::{parse, Arena};

    fn avr() -> SemaConfig  { SemaConfig { target: Target::Avr } }
    fn esp() -> SemaConfig  { SemaConfig { target: Target::Esp32 } }

    fn run<'a>(src: &'a str, cfg: &SemaConfig, arena_e: &'a Arena<Expr<'a>>, arena_s: &'a Arena<Stmt<'a>>) -> SemaOutput {
        let stmts = parse(src, arena_e, arena_s).expect("parse failed");
        let refs: Vec<&Stmt<'a>> = stmts.iter().map(|s| *s).collect();
        analyse(&refs, cfg)
    }

    #[test]
    fn float_avr_gera_erro() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("local x = 3.14", &avr(), &ae, &as_);
        assert!(!out.errors.is_empty(), "deveria ter erro de float em AVR");
        assert!(out.errors[0].message.contains("ponto flutuante"));
        // span deve apontar para linha 1
        assert_eq!(out.errors[0].span.line, 1);
    }

    #[test]
    fn float_esp32_sem_erro() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("local x = 3.14", &esp(), &ae, &as_);
        assert!(out.errors.is_empty(), "ESP32 aceita float");
    }

    #[test]
    fn variavel_nao_declarada() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("x = 1", &avr(), &ae, &as_);
        assert!(out.errors.iter().any(|e| e.message.contains("não declarada")));
    }

    #[test]
    fn variavel_usada_antes_de_init() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("local x\nlocal y = x + 1", &avr(), &ae, &as_);
        assert!(out.errors.iter().any(|e| e.message.contains("antes de ser inicializada")));
    }

    #[test]
    fn recursao_direta_proibida() {
        let ae = Arena::new(); let as_ = Arena::new();
        let src = "function f()\n  f()\nend";
        let out = run(src, &avr(), &ae, &as_);
        assert!(out.errors.iter().any(|e| e.message.contains("recursão")));
    }

    #[test]
    fn delay_tipo_errado() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("delay(\"texto\")", &avr(), &ae, &as_);
        assert!(out.errors.iter().any(|e| e.message.contains("argumento 1 de 'delay'")));
    }

    #[test]
    fn delay_aridade_errada() {
        let ae = Arena::new(); let as_ = Arena::new();
        let out = run("delay(1, 2)", &avr(), &ae, &as_);
        assert!(out.errors.iter().any(|e| e.message.contains("espera 1 argumento")));
    }

    #[test]
    fn blink_avr_sem_erros() {
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
        let out = run(src, &avr(), &ae, &as_);
        for e in &out.errors { eprintln!("{e}"); }
        assert!(out.errors.is_empty(), "blink AVR não deve ter erros semânticos");
    }

    #[test]
    fn sensor_analogico_esp32_sem_erros() {
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
        let out = run(src, &esp(), &ae, &as_);
        for e in &out.errors { eprintln!("{e}"); }
        assert!(out.errors.is_empty(), "sensor analógico ESP32 não deve ter erros semânticos");
    }

    #[test]
    fn variavel_inicializada_dentro_de_if() {
        let ae = Arena::new(); let as_ = Arena::new();
        let src = "local x\nif true then\n  x = 42\nend\nlocal y = x";
        let out = run(src, &avr(), &ae, &as_);
        for e in &out.errors { eprintln!("{e}"); }
        assert!(out.errors.is_empty(), "x foi inicializado antes do uso");
    }
}
