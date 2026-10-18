//! Main application entry point for accents-rapides

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "accents-rapides")]
#[command(version)]
#[command(about = "Rust port of schnelle-umlaute - quick access to special characters", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the editor UI
    Editor,

    /// Launch the overlay UI
    Overlay,

    /// Print configuration information
    Config,
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Editor => {
            // Launch the editor
            accents_rapides_editor::run_editor()
        }
        Commands::Overlay => {
            // Launch the overlay
            accents_rapides_overlay::run_overlay()
        }
        Commands::Config => print_config_info(),
    }
}

/// Print configuration information
fn print_config_info() -> Result<()> {
    use accents_rapides_core::{mappings_path, usage_path, Config};

    let config_path = Config::default_config_path().expect("Could not determine config path");

    println!("Configuration path: {}", config_path.display());
    println!("Mappings path: {}", mappings_path(&config_path).display());
    println!("Usage path: {}", usage_path(&config_path).display());

    if config_path.exists() {
        let config = Config::load_from_file(&config_path)?;
        println!("\nCurrent configuration:");
        println!("Leader key: {:?}", config.leader_key);
        println!("Time window: {} ms", config.time_window_ms);
        println!("Show overlay: {}", config.show_overlay);
        println!("App filter mode: {:?}", config.app_filter_mode);
        println!("Filtered apps: {:?}", config.filtered_apps);
    } else {
        println!("\nNo configuration file found. Default configuration will be used.");
    }

    Ok(())
}
