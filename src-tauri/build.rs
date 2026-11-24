// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//
// 1.11.1: con MSVC, el manifiesto de Windows (dependencia de Common Controls
// v6) se enlaza en **todos** los ejecutables, también en los de pruebas. Con
// el manifiesto por defecto de tauri-build solo lo lleva el programa, y los
// ejecutables de `cargo test` no arrancan en Windows (0xc0000139,
// STATUS_ENTRYPOINT_NOT_FOUND: falta TaskDialogIndirect de comctl32 v6). El
// contenido es el mismo que el de tauri-build (windows-app-manifest.xml). Con
// MinGW y en Linux no cambia nada.
fn main() {
    let msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !msvc {
        tauri_build::build();
        return;
    }
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("tauri-build con el manifiesto de Windows");
}
