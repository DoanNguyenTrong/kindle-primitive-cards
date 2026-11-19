# Primitive Cards

Create decks and text cards, review due cards with spaced repetition, and track recall by deck. Copy an Anki `.apkg`
to Documents to import the first two text fields of each card. The importer supports legacy SQLite collections and
compressed modern collections. Media, custom templates and upstream scheduling data are skipped. Export writes a
tab-separated UTF-8 text file to Documents.

Scheduling uses [SuperMemo SM-2](https://www.supermemo.com/en/supermemo-method) with four recall choices and one-day
rescheduling after a miss. The Kindle and desktop use the same card state.

Run `cargo test --offline --workspace` and `cargo run --offline -p cards-desktop` with the shared Tools and
Launcher repositories next to this one. The ARMHF package uses the launcher's fonts.

Source is PolyForm Noncommercial 1.0.0. Documentation is CC BY-NC-SA 4.0.
