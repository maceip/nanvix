// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//! Vanilla Wasmtime 49 on the current Nanvix guest ABI.
//!
//! The host `build.rs` precompiles the rustls server to Pulley32. This binary
//! interprets that module and supplies `wasi_snapshot_preview1` socket calls
//! on top of Nanvix TCP. System calls go through `nvx-crt0` / `sys`.

#![no_std]
#![no_main]

extern crate alloc;
extern crate nvx;
extern crate nvx_crt0;

use alloc::format;
use core::cell::UnsafeCell;
use sys::{
    error::Error,
    kcall::debug,
};
use wasmtime::{
    Config,
    Engine,
    Linker,
    Module,
    Store,
};

mod sockets;

const HTTP_CWASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/workload.cwasm"));

struct Tls {
    slots: UnsafeCell<[*mut u8; 2]>,
}

// The first Wasmtime guest is single-threaded. `wasmtime_tls_*` are called
// from the same thread that runs `main`.
unsafe impl Sync for Tls {}

static TLS: Tls = Tls {
    slots: UnsafeCell::new([core::ptr::null_mut(); 2]),
};

#[unsafe(no_mangle)]
pub extern "C" fn wasmtime_tls_get(slot: usize) -> *mut u8 {
    unsafe { (*TLS.slots.get())[slot] }
}

#[unsafe(no_mangle)]
pub extern "C" fn wasmtime_tls_set(slot: usize, ptr: *mut u8) {
    unsafe {
        (*TLS.slots.get())[slot] = ptr;
    }
}

fn debug_print(msg: &str) {
    for chunk in msg.as_bytes().chunks(80) {
        let _ = debug::__kcall_debug(chunk.as_ptr(), chunk.len());
    }
}

fn engine() -> Result<Engine, wasmtime::Error> {
    let mut config = Config::new();
    config.target("pulley32")?;
    config.memory_init_cow(false);
    config.signals_based_traps(false);
    config.concurrency_support(false);
    config.collector(wasmtime::Collector::Null);
    config.memory_reservation(0);
    config.memory_guard_size(0);
    config.memory_reservation_for_growth(32 << 20);
    config.guard_before_linear_memory(false);
    Engine::new(&config)
}

fn run_server() -> Result<alloc::string::String, wasmtime::Error> {
    let engine = engine()?;
    let module = unsafe { Module::deserialize(&engine, HTTP_CWASM)? };
    let mut linker = Linker::new(&engine);
    sockets::define(&mut linker)?;
    let mut store = Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &module)?;
    let serve = instance.get_typed_func::<(), i32>(&mut store, "serve")?;
    let report_ptr = instance.get_typed_func::<(), i32>(&mut store, "report_ptr")?;
    let report_len = instance.get_typed_func::<(), i32>(&mut store, "report_len")?;
    let n = serve.call(&mut store, ())?;
    if n < 0 {
        return Ok(format!("rustls-error {n}\n"));
    }
    let ptr = report_ptr.call(&mut store, ())? as usize;
    let len = report_len.call(&mut store, ())? as usize;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| wasmtime::Error::msg("wasm module has no memory"))?;
    let bytes = memory
        .data(&store)
        .get(ptr..ptr + len)
        .ok_or_else(|| wasmtime::Error::msg("report is outside wasm memory"))?;
    let text = core::str::from_utf8(bytes).unwrap_or("report is not utf-8");
    Ok(format!("rustls-bytes={n} {text}"))
}

#[unsafe(no_mangle)]
pub fn main() -> Result<(), Error> {
    let wasm_bytes = env!("WASM_BYTES");
    let cwasm_bytes = env!("CWASM_BYTES");
    match run_server() {
        Ok(report) => {
            debug_print(&format!("wasm-rustls wasm={wasm_bytes} cwasm={cwasm_bytes} {report}"))
        },
        Err(error) => debug_print(&format!("wasm-rustls error {error:?}\n")),
    }
    Ok(())
}
