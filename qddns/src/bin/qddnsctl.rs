fn main() {
    let cli = qddns::cli::parse_from_env();
    match cli.and_then(qddns::cli::run) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
