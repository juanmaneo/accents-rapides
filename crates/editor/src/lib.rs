//! Configuration editor for accents-rapides, built with iced.

use accents_rapides_core::Config;
use anyhow::Result;

pub mod app;

use app::Editor;

/// Load (or create) the configuration and mappings into a new editor.
pub fn load_editor() -> Result<Editor> {
    let (config, mappings) = Config::load_or_create()?;
    Ok(Editor::new(config, mappings))
}

/// Run the editor UI
pub fn run_editor() -> Result<()> {
    launch_editor(load_editor()?)
}

/// Open the editor window.
///
/// Not measured by coverage: opening a desktop window cannot run headless
/// (cargo-llvm-cov sets `cfg(coverage)`, so coverage builds skip the launch
/// and only the setup logic above is measured).
#[cfg(not(coverage))]
fn launch_editor(editor: Editor) -> Result<()> {
    iced::application(move || editor.clone(), Editor::update, Editor::view)
        .title("Accents Rapides Editor")
        .window_size((800.0, 600.0))
        .run()?;
    Ok(())
}

/// Coverage builds cannot open a window; `run_editor` still exercises the
/// setup logic and returns normally.
#[cfg(coverage)]
fn launch_editor(_editor: Editor) -> Result<()> {
    Ok(())
}
