// Same link setup as Nanvix's own Rust guests (see src/user/hello-rust-nostd/build.rs).
fn main() {
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86".to_string());
    println!("cargo::rustc-link-arg=-Tbuild/user/linker/{target_arch}/user.ld");
}
