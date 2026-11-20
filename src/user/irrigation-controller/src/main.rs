// CONDUIT irrigation controller, Nanvix guest front end.
// Reads command lines from stdin and writes JSON state lines to stdout using
// Nanvix syscalls. The decision logic is the verified `ctl.rs`, unchanged.

#![no_std]
#![no_main]

extern crate alloc;
extern crate libc_string;
extern crate nvx;
extern crate nvx_crt0;

mod ctl;
mod protocol;

use alloc::{string::String, vec::Vec};
use ::sys::error::Error;
use ::sysapi::unistd::{STDIN_FILENO, STDOUT_FILENO};
use ::syscall::unistd::{read, write};

const RUNTIME: &str = "nanvix";

fn write_all(bytes: &[u8]) {
    let mut off = 0usize;
    while off < bytes.len() {
        match write(STDOUT_FILENO, &bytes[off..]) {
            Ok(n) if n > 0 => off += usize::try_from(n).unwrap_or(bytes.len()),
            _ => return,
        }
    }
}

fn emit(line: String) {
    write_all(line.as_bytes());
    write_all(b"\n");
}

#[unsafe(no_mangle)]
pub fn main() -> Result<(), Error> {
    let mut c = ctl::Ctl::new(ctl::TANK_CAP);
    emit(protocol::state_json(&c, true, "ready", RUNTIME));
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let n = match read(STDIN_FILENO, &mut buf) {
            Ok(n) if n > 0 => usize::try_from(n).unwrap_or(0),
            _ => return Ok(()), // EOF or error: stop cleanly
        };
        pending.extend_from_slice(&buf[..n]);
        while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = pending.drain(..=pos).collect();
            let text = core::str::from_utf8(&line).unwrap_or("").trim();
            if text.is_empty() {
                continue;
            }
            match protocol::handle(&mut c, text, RUNTIME) {
                Some(reply) => emit(reply),
                None => return Ok(()),
            }
        }
    }
}
