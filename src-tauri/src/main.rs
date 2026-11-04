// Evita una consola extra en Windows; no tiene efecto en Linux.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dcc_app::run();
}
