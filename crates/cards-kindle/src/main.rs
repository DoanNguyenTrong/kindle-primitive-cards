use cards_core::Library;
use cards_render::View;
use std::{fs, path::PathBuf};

fn main() -> Result<(), String> {
    let dir = PathBuf::from("/mnt/us/cards");
    let data = dir.join("decks.json");
    let library = Library::restore(&fs::read(&data).unwrap_or_default());
    primitive_app::run(
        View::new(library, Some(data), PathBuf::from("/mnt/us/documents")),
        &dir,
    )
}
