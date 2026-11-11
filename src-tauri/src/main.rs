// Evita una consola extra en Windows; no tiene efecto en Linux.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Si `ssh` ha lanzado la aplicación como SSH_ASKPASS (0.10.0, D-077), solo
    // responde con el secreto que le da la aplicación por su socket privado y
    // termina: no abre ninguna ventana.
    if let Some(code) = dcc_engine::ssh::askpass::client_main() {
        std::process::exit(code);
    }
    dcc_app::run();
}
