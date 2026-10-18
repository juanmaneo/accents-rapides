//! Overlay binary entry point for accents-rapides

fn main() {
    tracing_subscriber::fmt::init();

    if let Err(e) = accents_rapides_overlay::run_overlay() {
        eprintln!("Failed to run overlay: {e}");
        std::process::exit(1);
    }
}
