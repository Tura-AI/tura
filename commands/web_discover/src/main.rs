fn main() {
    if let Err(error) = tura_llm_rust::install_rustls_crypto_provider() {
        eprintln!("web_discover failed to install the rustls crypto provider: {error}");
        std::process::exit(1);
    }
    tura_command_web_discover::main();
}
