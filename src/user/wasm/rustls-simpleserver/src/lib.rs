//! The rustls `simpleserver` example, retargeted to `wasm32-unknown-unknown`.
//!
//! The upstream example binds a `std::net::TcpListener` and reads the
//! certificate from the filesystem. This target has neither, so the
//! certificate is embedded and the socket calls are the same
//! `wasi_snapshot_preview1` imports the Nanvix guest provides, plus bind,
//! listen, and accept.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

getrandom::register_custom_getrandom!(guest_entropy);

fn guest_entropy(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    static STATE: AtomicU64 = AtomicU64::new(0xA5A5_1234_89AB_CDEF);
    let mut state = STATE.fetch_add(0x9E37_79B9, Ordering::Relaxed);
    for chunk in buf.chunks_mut(8) {
        state = state
            .wrapping_mul(0x6C07_8966_F4A7_C15)
            .wrapping_add(1);
        let bytes = state.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    Ok(())
}

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection};

const AF_INET: i32 = 2;
const SOCK_STREAM: i32 = 1;

#[link(wasm_import_module = "wasi_snapshot_preview1")]
unsafe extern "C" {
    fn sock_open(family: i32, socktype: i32) -> i32;
    fn sock_bind(fd: i32, ip: u32, port: u32) -> i32;
    fn sock_listen(fd: i32, backlog: i32) -> i32;
    fn sock_accept(fd: i32) -> i32;
    fn sock_send(fd: i32, ptr: i32, len: i32) -> i32;
    fn sock_recv(fd: i32, ptr: i32, len: i32) -> i32;
    fn fd_close(fd: i32) -> i32;
}

struct GuestSocket {
    fd: i32,
}

impl Read for GuestSocket {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = unsafe { sock_recv(self.fd, buf.as_mut_ptr() as i32, buf.len() as i32) };
        if n < 0 {
            Err(std::io::Error::other("sock_recv"))
        } else {
            Ok(n as usize)
        }
    }
}

impl Write for GuestSocket {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = unsafe { sock_send(self.fd, buf.as_ptr() as i32, buf.len() as i32) };
        if n < 0 {
            Err(std::io::Error::other("sock_send"))
        } else {
            Ok(n as usize)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

static mut REPORT: [u8; 128] = [0; 128];
static mut REPORT_LEN: usize = 0;

#[no_mangle]
pub extern "C" fn report_ptr() -> i32 {
    core::ptr::addr_of!(REPORT) as i32
}

#[no_mangle]
pub extern "C" fn report_len() -> i32 {
    unsafe { REPORT_LEN as i32 }
}

fn set_report(text: &str) {
    let bytes = text.as_bytes();
    let n = bytes.len().min(128);
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), REPORT.as_mut_ptr(), n);
        REPORT_LEN = n;
    }
}

/// Accept one client on port 4443 and complete the rustls server handshake.
#[no_mangle]
pub extern "C" fn serve() -> i32 {
    match serve_result() {
        Ok(n) => n,
        Err(code) => code,
    }
}

fn serve_result() -> Result<i32, i32> {
    let _ = rustls_rustcrypto::provider().install_default();

    let certs = CertificateDer::pem_slice_iter(include_bytes!("cert.pem"))
        .map(|item| item.map_err(|_| -10))
        .collect::<Result<Vec<_>, i32>>()?;
    let private_key = PrivateKeyDer::from_pem_slice(include_bytes!("key.pem")).map_err(|_| -11)?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, private_key)
        .map_err(|_| -12)?;

    let listener = unsafe { sock_open(AF_INET, SOCK_STREAM) };
    if listener < 0 {
        return Err(listener);
    }
    if unsafe { sock_bind(listener, 0, 4443) } < 0 {
        return Err(-2);
    }
    if unsafe { sock_listen(listener, 1) } < 0 {
        return Err(-3);
    }
    let connfd = unsafe { sock_accept(listener) };
    if connfd < 0 {
        return Err(connfd);
    }

    let mut tcp = GuestSocket { fd: connfd };
    let mut conn = ServerConnection::new(Arc::new(config)).map_err(|_| -13)?;
    let mut tls = rustls::Stream::new(&mut conn, &mut tcp);
    tls.write_all(b"Hello from the server").map_err(|_| -14)?;
    tls.flush().map_err(|_| -15)?;
    let mut buf = [0u8; 64];
    let len = tls.read(&mut buf).unwrap_or(0);
    drop(tls);
    let version = conn.protocol_version();
    let suite = conn.negotiated_cipher_suite().map(|suite| suite.suite());
    set_report(&format!("rustls {version:?} {suite:?} read {len}\n"));
    unsafe { fd_close(connfd) };
    unsafe { fd_close(listener) };
    Ok(len as i32)
}
