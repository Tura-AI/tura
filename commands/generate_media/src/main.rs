fn main() {
    if let Err(error) = tura_llm_rust::install_rustls_crypto_provider() {
        eprintln!("generate_media failed to install the rustls crypto provider: {error}");
        std::process::exit(1);
    }
    tura_command_generate_media::main();
}
