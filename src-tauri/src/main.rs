// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
// Evita una consola extra en Windows; no tiene efecto en Linux.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Si `ssh` ha lanzado la aplicación como SSH_ASKPASS (0.10.0, D-077), solo
    // responde con el secreto que le da la aplicación por su socket privado y
    // termina: no abre ninguna ventana.
    if let Some(code) = cmc_engine::ssh::askpass::client_main() {
        std::process::exit(code);
    }
    // Paso de los datos del nombre anterior, sin ventana (install.sh, 1.8.0).
    if let Some(code) = cmc_app::migrate_main() {
        std::process::exit(code);
    }
    // Copia programada (temporizador de systemd, 1.4.0): sin ventana.
    if let Some(code) = cmc_app::headless_main() {
        std::process::exit(code);
    }
    cmc_app::run();
}
