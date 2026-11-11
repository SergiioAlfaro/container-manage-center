//! `dcc-diagnose`: diagnóstico de la conexión con Docker desde la terminal.
//!
//! Usa exactamente el mismo código de detección y conexión que la aplicación.
//! No modifica nada del sistema.
//!
//! Uso:
//!   dcc-diagnose                 detección automática
//!   dcc-diagnose --socket RUTA   un socket concreto
//!   dcc-diagnose --json          salida en JSON
//!   dcc-diagnose --compose       además, detecta Docker Compose
//!
//! Código de salida: 0 conectado, 2 sin conexión, 64 uso incorrecto.

use dcc_activity::ActivityLog;
use dcc_engine::{ConnectionMode, ConnectionState, EngineManager};

fn usage() -> ! {
    eprintln!("Uso: dcc-diagnose [--socket RUTA] [--json] [--compose]");
    std::process::exit(64);
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut mode = ConnectionMode::Auto;
    let mut json = false;
    let mut compose = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--socket" => {
                mode = ConnectionMode::Custom {
                    socket_path: args.next().unwrap_or_else(|| usage()),
                }
            }
            "--json" => json = true,
            "--compose" => compose = true,
            "-h" | "--help" => usage(),
            _ => usage(),
        }
    }

    let log = ActivityLog::new(50);
    let manager = EngineManager::new(log.clone(), mode);
    let state = manager.connect().await;
    let compose_status = if compose {
        Some(manager.compose_status().await)
    } else {
        None
    };

    if json {
        let out = serde_json::json!({ "state": state, "compose": compose_status, "operations": log.snapshot() });
        println!(
            "{}",
            serde_json::to_string_pretty(&out).expect("serializable")
        );
    } else {
        match &state {
            ConnectionState::Idle => println!("Sin conectar"),
            ConnectionState::Connected {
                endpoint, engine, ..
            } => {
                println!(
                    "Conectado a Docker Engine {} (API {})",
                    engine.version, engine.api_version
                );
                println!("  Socket:   {}", endpoint.socket_path);
                println!("  Origen:   {:?}", endpoint.source);
                println!("  Rootless: {}", if engine.rootless { "sí" } else { "no" });
                println!("  Sistema:  {} ({})", engine.os, engine.arch);
            }
            ConnectionState::Disconnected { diagnosis, .. } => {
                println!("Sin conexión con Docker.");
                println!("  Causa: {:?}", diagnosis.problem);
                println!("  Sockets comprobados:");
                for c in &diagnosis.candidates {
                    println!(
                        "    {:<40} {:?} ({:?})",
                        c.endpoint.socket_path, c.result, c.endpoint.source
                    );
                }
                println!(
                    "  CLI: {}",
                    diagnosis
                        .installation
                        .cli_path
                        .as_deref()
                        .unwrap_or("no encontrado")
                );
                println!(
                    "  Daemon: {}",
                    diagnosis
                        .installation
                        .daemon_path
                        .as_deref()
                        .unwrap_or("no encontrado")
                );
                println!(
                    "  Usuario: {} ({})",
                    diagnosis.user.name, diagnosis.user.uid
                );
            }
            ConnectionState::Connecting => println!("Conectando…"),
        }
        if let Some(c) = &compose_status {
            use dcc_engine::ComposeStatus as C;
            let line = match c {
                C::Available {
                    version,
                    kind,
                    program,
                } => format!("disponible: versión {version} ({kind:?}, {program})"),
                C::NotInstalled { .. } => "no instalado".to_owned(),
                C::PluginUnusable {
                    plugin_path,
                    message,
                } => format!("plugin presente en {plugin_path} pero el CLI no lo usa: {message}"),
                C::Legacy { version, program } => {
                    format!("solo Compose v1 {version} ({program}), no admitido")
                }
                C::Unrecognized { program, output } => {
                    format!("salida no reconocida de {program}: {output}")
                }
                C::Error { message } => format!("error: {message}"),
                C::RemotePending => "servidor remoto: se ejecuta en el servidor".to_owned(),
            };
            println!("Docker Compose: {line}");
        }
        println!("Operaciones realizadas:");
        for r in log.snapshot().into_iter().rev() {
            let t = match &r.transport {
                dcc_activity::Transport::Api { method, path, .. } => format!("{method} {path}"),
                dcc_activity::Transport::Cli { program, args } => {
                    dcc_activity::quote::render_argv(program, args)
                }
                dcc_activity::Transport::App { action } => action.clone(),
            };
            println!(
                "  {:?}  {t}{}",
                r.status,
                r.error.map(|e| format!("  ({e})")).unwrap_or_default()
            );
        }
    }
    std::process::exit(if matches!(state, ConnectionState::Connected { .. }) {
        0
    } else {
        2
    });
}
