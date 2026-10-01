// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

use std::{
    env,
    fs,
    path::PathBuf,
    process,
};

fn guest_config() -> wasmtime::Config {
    let mut config = wasmtime::Config::new();
    config
        .target("pulley32")
        .expect("pulley32 is a vanilla Wasmtime target");
    // Match the no_std guest runtime: no virtual memory, no signal traps,
    // no component model, null collector.
    config.memory_init_cow(false);
    config.signals_based_traps(false);
    config.concurrency_support(false);
    config.wasm_threads(false);
    config.wasm_component_model(false);
    config.collector(wasmtime::Collector::Null);
    // No virtual memory in the guest: linear memory is a Vec, which rejects
    // a static reservation and guard pages.
    config.memory_reservation(0);
    config.memory_guard_size(0);
    config.memory_reservation_for_growth(32 << 20);
    config.guard_before_linear_memory(false);
    config
}

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "nanvix" {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let nanvix_root = manifest_dir
        .join("../../..")
        .canonicalize()
        .expect("nanvix repository root");
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86".to_string());
    let linker_arch = if arch == "x86_64" { "x86_64" } else { "x86" };
    let linker_script = nanvix_root
        .join("build/user/linker")
        .join(linker_arch)
        .join("user.ld");
    println!("cargo:rustc-link-arg=-T{}", linker_script.display());
    println!("cargo:rerun-if-changed={}", linker_script.display());

    let wasm_path = nanvix_root.join(
        "src/user/rustls-simpleserver/target/wasm32-unknown-unknown/release/rustls_simpleserver.wasm",
    );
    let wasm_path = wasm_path.canonicalize().unwrap_or_else(|_| {
        eprintln!(
            "missing {}; build src/user/rustls-simpleserver for wasm32-unknown-unknown first",
            wasm_path.display()
        );
        process::exit(1);
    });
    println!("cargo:rerun-if-changed={}", wasm_path.display());
    let wasm = fs::read(&wasm_path).expect("read wasm");

    let engine = wasmtime::Engine::new(&guest_config()).expect("host engine");
    let compiled = engine
        .precompile_module(&wasm)
        .expect("precompile pulley32");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("workload.cwasm"), &compiled).expect("write cwasm");
    println!("cargo:rustc-env=WASM_BYTES={}", wasm.len());
    println!("cargo:rustc-env=CWASM_BYTES={}", compiled.len());
    println!("cargo:rerun-if-changed=build.rs");
}
