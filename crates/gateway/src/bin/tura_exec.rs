#[path = "../tura_exec/mod.rs"]
mod tura_exec;

fn main() {
    if let Err(error) = tura_llm_rust::install_rustls_crypto_provider() {
        eprintln!("tura_exec failed to install the rustls crypto provider: {error}");
        std::process::exit(1);
    }
    tura_exec::main();
}
