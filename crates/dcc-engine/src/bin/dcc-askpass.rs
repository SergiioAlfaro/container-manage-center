//! Programa `SSH_ASKPASS` para las pruebas del motor. En la aplicación
//! instalada, el propio ejecutable de Docker Control Center hace este papel
//! (ver `dcc_engine::ssh::askpass::client_main`).

fn main() {
    let code = dcc_engine::ssh::askpass::client_main().unwrap_or_else(|| {
        eprintln!(
            "dcc-askpass: solo lo usa ssh (falta {})",
            dcc_engine::ssh::askpass::SOCKET_ENV
        );
        2
    });
    std::process::exit(code);
}
