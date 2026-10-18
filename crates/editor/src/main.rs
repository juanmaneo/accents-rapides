//! Editor binary entry point for accents-rapides

fn main() {
    tracing_subscriber::fmt::init();

    if let Err(e) = accents_rapides_editor::run_editor() {
        eprintln!("Failed to run editor: {e}");
        std::process::exit(1);
    }
}
