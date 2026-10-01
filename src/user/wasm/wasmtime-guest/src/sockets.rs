// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//! `wasi_snapshot_preview1` socket calls, forwarded to Nanvix `socket` /
//! `connect` / `send` / `recv`. Names are the preview1 socket entry points.
//! The argument layout is the small integer form this guest implements:
//! IPv4 address as a host-order `u32`, port as a host-order `u32`.

use alloc::vec;
use wasmtime::{
    Caller,
    Extern,
    Linker,
    Memory,
    Result,
};

const AF_INET: i32 = 2;

#[repr(C)]
struct SockaddrIn {
    sin_len: u8,
    sin_family: u8,
    sin_port: u16,
    sin_addr: u32,
    sin_zero: [u8; 8],
}

unsafe extern "C" {
    fn socket(domain: i32, typ: i32, protocol: i32) -> i32;
    fn bind(sockfd: i32, addr: *const SockaddrIn, len: u32) -> i32;
    fn listen(sockfd: i32, backlog: i32) -> i32;
    fn accept(sockfd: i32, addr: *mut SockaddrIn, len: *mut u32) -> i32;
    fn connect(sockfd: i32, addr: *const SockaddrIn, len: u32) -> i32;
    fn send(sockfd: i32, buf: *const u8, len: usize, flags: i32) -> isize;
    fn recv(sockfd: i32, buf: *mut u8, len: usize, flags: i32) -> isize;
    fn close(fd: i32) -> i32;
}

fn trace(msg: &str) {
    let _ = sys::kcall::debug::__kcall_debug(msg.as_ptr(), msg.len());
}

fn memory(caller: &mut Caller<'_, ()>) -> Option<Memory> {
    match caller.get_export("memory")? {
        Extern::Memory(memory) => Some(memory),
        _ => None,
    }
}

pub fn define(linker: &mut Linker<()>) -> Result<()> {
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "sock_open",
        |family: i32, socktype: i32| -> i32 {
            let fd = unsafe { socket(family, socktype, 0) };
            trace(&alloc::format!("sock_open fd={fd}\n"));
            fd
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "sock_connect",
        |fd: i32, ip: u32, port: u32| -> i32 {
            let addr = SockaddrIn {
                sin_len: core::mem::size_of::<SockaddrIn>() as u8,
                sin_family: AF_INET as u8,
                sin_port: (port as u16).to_be(),
                sin_addr: ip.to_be(),
                sin_zero: [0; 8],
            };
            let rc = unsafe { connect(fd, &addr, core::mem::size_of::<SockaddrIn>() as u32) };
            trace(&alloc::format!("sock_connect rc={rc}\n"));
            rc
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "sock_bind",
        |fd: i32, ip: u32, port: u32| -> i32 {
            let addr = SockaddrIn {
                sin_len: core::mem::size_of::<SockaddrIn>() as u8,
                sin_family: AF_INET as u8,
                sin_port: (port as u16).to_be(),
                sin_addr: ip.to_be(),
                sin_zero: [0; 8],
            };
            let rc = unsafe { bind(fd, &addr, core::mem::size_of::<SockaddrIn>() as u32) };
            trace(&alloc::format!("sock_bind rc={rc} port={port}\n"));
            rc
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "sock_listen",
        |fd: i32, backlog: i32| -> i32 {
            let rc = unsafe { listen(fd, backlog) };
            trace(&alloc::format!("sock_listen rc={rc}\n"));
            rc
        },
    )?;
    linker.func_wrap("wasi_snapshot_preview1", "sock_accept", |fd: i32| -> i32 {
        let rc = unsafe { accept(fd, core::ptr::null_mut(), core::ptr::null_mut()) };
        trace(&alloc::format!("sock_accept fd={rc}\n"));
        rc
    })?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "sock_send",
        |mut caller: Caller<'_, ()>, fd: i32, ptr: i32, len: i32| -> i32 {
            if len < 0 {
                return -1;
            }
            let Some(memory) = memory(&mut caller) else {
                return -1;
            };
            let mut buf = vec![0u8; len as usize];
            if memory.read(&caller, ptr as usize, &mut buf).is_err() {
                return -1;
            }
            unsafe { send(fd, buf.as_ptr(), buf.len(), 0) as i32 }
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "sock_recv",
        |mut caller: Caller<'_, ()>, fd: i32, ptr: i32, len: i32| -> i32 {
            if len <= 0 {
                return -1;
            }
            let Some(memory) = memory(&mut caller) else {
                return -1;
            };
            let mut buf = vec![0u8; len as usize];
            let n = unsafe { recv(fd, buf.as_mut_ptr(), buf.len(), 0) };
            if n < 0 {
                return n as i32;
            }
            if memory
                .write(&mut caller, ptr as usize, &buf[..n as usize])
                .is_err()
            {
                return -1;
            }
            n as i32
        },
    )?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_close", |fd: i32| -> i32 {
        unsafe { close(fd) }
    })?;
    Ok(())
}
