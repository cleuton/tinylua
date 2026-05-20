use std::{env, fs, process};

use tinylua_parser::{parse, Arena, Expr, Stmt};
use tinylua_sema::{analyse, SemaConfig, Target};
use tinylua_codegen::{generate, CodegenConfig};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: tinylua-cli <arquivo.lua> [--target avr|esp32]");
        process::exit(1);
    }

    let lua_path = &args[1];

    // Target opcional (padrão: avr)
    let target = args.iter()
        .position(|a| a == "--target")
        .and_then(|i| args.get(i + 1))
        .map(|t| match t.as_str() {
            "esp32" => Target::Esp32,
            _       => Target::Avr,
        })
        .unwrap_or(Target::Avr);

    // Lê o fonte Lua
    let src = fs::read_to_string(lua_path).unwrap_or_else(|e| {
        eprintln!("Erro ao ler {lua_path}: {e}");
        process::exit(1);
    });

    // Pipeline completo
    let arena_expr  = Arena::<Expr>::new();
    let arena_stmt  = Arena::<Stmt>::new();

    let stmts = parse(&src, &arena_expr, &arena_stmt).unwrap_or_else(|e| {
        eprintln!("Erro de sintaxe: {e}");
        process::exit(1);
    });

    let sema_out = analyse(&stmts, &SemaConfig { target });
    if !sema_out.errors.is_empty() {
        for e in &sema_out.errors {
            eprintln!("Erro semântico: {e}");
        }
        process::exit(1);
    }

    let rust_src = generate(&stmts, &sema_out, &CodegenConfig { target });

    // Grava o .rs ao lado do .lua
    let out_path = lua_path.replace(".lua", ".rs");
    fs::write(&out_path, &rust_src).unwrap_or_else(|e| {
        eprintln!("Erro ao gravar {out_path}: {e}");
        process::exit(1);
    });

    println!("Gerado: {out_path}");
}