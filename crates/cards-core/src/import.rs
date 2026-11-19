use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use rusqlite::{Connection, OpenFlags};
use zip::ZipArchive;

use super::{Card, Deck};

const MAX_DATABASE: u64 = 64 * 1024 * 1024;
const MAX_NOTES: usize = 10_000;
const FIELD_LIMIT: usize = 8_192;
const MAX_TEXT: usize = 8 * 1024 * 1024;
const MIN_DB_SIZE: u64 = 512;
static TEMP_ID: AtomicUsize = AtomicUsize::new(0);

pub fn import_apkg(path: &Path, scratch: &Path) -> Result<Vec<Deck>, String> {
    let scratch = scratch.canonicalize().map_err(|error| error.to_string())?;
    let directory = scratch.join(format!(
        "primitive-cards-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).map_err(|error| error.to_string())?;
    let file = directory.join("collection.sqlite");
    let result = (|| {
        unpack(path, &file)?;
        open_decks(
            &file,
            path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Imported"),
        )
    })();
    let _ = fs::remove_file(&file);
    let _ = fs::remove_dir(&directory);
    result
}

fn unpack(path: &Path, file: &Path) -> Result<(), String> {
    let input = File::open(path).map_err(|error| error.to_string())?;
    let mut zip = ZipArchive::new(input).map_err(|_| "The Anki package is unreadable.")?;
    let name = [
        "collection.anki21b",
        "collection.anki21",
        "collection.anki2",
    ]
    .into_iter()
    .find(|name| zip.index_for_name(name).is_some())
    .ok_or("The package has no Anki collection.")?;
    let entry = zip.by_name(name).map_err(|error| error.to_string())?;
    if entry.size() > MAX_DATABASE {
        return Err("The Anki collection exceeds 64 MB.".into());
    }
    let mut input: Box<dyn Read> = if name.ends_with("21b") {
        let mut decoder = zstd::stream::read::Decoder::new(entry)
            .map_err(|_| "The compressed Anki collection is unreadable.")?;
        decoder
            .window_log_max(23)
            .map_err(|_| "The Anki compression window could not be limited.")?;
        Box::new(decoder)
    } else {
        Box::new(entry)
    };
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)
        .map_err(|error| error.to_string())?;
    let mut prefix = [0; 16];
    input
        .read_exact(&mut prefix)
        .map_err(|_| "The Anki collection is incomplete.")?;
    if &prefix != b"SQLite format 3\0" {
        return Err("The Anki collection is not a SQLite file.".into());
    }
    output
        .write_all(&prefix)
        .map_err(|error| error.to_string())?;
    let bytes = io::copy(
        &mut input.take(MAX_DATABASE + 1 - prefix.len() as u64),
        &mut output,
    )
    .map_err(|error| error.to_string())?
        + prefix.len() as u64;
    if !(MIN_DB_SIZE..=MAX_DATABASE).contains(&bytes) {
        return Err("The collection is incomplete or exceeds 64 MB.".into());
    }
    Ok(())
}

fn deck_names(connection: &Connection) -> BTreeMap<i64, String> {
    if let Ok(mut names) = connection.prepare("SELECT id, name FROM decks LIMIT 10000") {
        if let Ok(rows) = names.query_map([], |row| {
            Ok((row.get(0)?, row.get::<_, String>(1)?.replace('\x1f', " › ")))
        }) {
            return rows.flatten().collect();
        }
    }
    let names = connection
        .query_row("SELECT decks FROM col LIMIT 1", [], |row| {
            row.get::<_, String>(0)
        })
        .ok();
    let names = names
        .and_then(|raw| serde_json::from_str::<BTreeMap<String, serde_json::Value>>(&raw).ok())
        .unwrap_or_default();
    names
        .into_iter()
        .filter_map(|(key, value)| {
            Some((
                key.parse().ok()?,
                value.get("name")?.as_str()?.replace("::", " › "),
            ))
        })
        .collect()
}

fn open_decks(database: &Path, fallback: &str) -> Result<Vec<Deck>, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    let connection =
        Connection::open_with_flags(database, flags).map_err(|error| error.to_string())?;
    connection
        .execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")
        .map_err(|error| error.to_string())?;
    let names = deck_names(&connection);
    let mut statement = connection.prepare(
        "SELECT DISTINCT cards.did, notes.id, substr(notes.flds, 1, 32768) FROM notes JOIN cards ON cards.nid = notes.id LIMIT 10001"
    ).map_err(|_| "The Anki package has no readable notes.")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(2)?))
        })
        .map_err(|error| error.to_string())?;
    let mut decks = BTreeMap::<i64, Deck>::new();
    let mut text_size = 0;
    for (index, row) in rows.enumerate() {
        if index == MAX_NOTES {
            return Err("Choose an Anki package with at most 10,000 notes.".into());
        }
        let (id, fields) = row.map_err(|error| error.to_string())?;
        let mut fields = fields.split('\x1f');
        let front = plain(fields.next().unwrap_or(""));
        let back = plain(fields.next().unwrap_or(""));
        text_size += front.len() + back.len();
        if text_size > MAX_TEXT {
            return Err("The Anki package contains too much text for this Kindle.".into());
        }
        if front.is_empty() || back.is_empty() {
            continue;
        }
        let deck = decks.entry(id).or_insert_with(|| Deck {
            name: names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| fallback.to_owned()),
            cards: Vec::new(),
        });
        deck.cards.push(Card::new(front, back));
    }
    if decks.is_empty() {
        return Err("The Anki package has no cards with two text fields.".into());
    }
    Ok(decks.into_values().collect())
}

fn plain(html: &str) -> String {
    let mut output = String::new();
    let mut tag = None::<String>;
    for ch in html.chars().take(FIELD_LIMIT) {
        match (&mut tag, ch) {
            (None, '<') => tag = Some(String::new()),
            (Some(name), '>') => {
                let name = name.trim().to_ascii_lowercase();
                if ["br", "br/", "br /", "/div", "/p"].contains(&name.as_str()) {
                    output.push('\n');
                }
                tag = None;
            }
            (Some(name), ch) => name.push(ch),
            (None, ch) => output.push(ch),
        }
    }
    output
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    #[test]
    fn imports_basic_and_modern_text_packages_and_preserves_deck_names() {
        let scratch = std::env::temp_dir().join(format!(
            "primitive-cards-import-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&scratch).unwrap();
        let database = scratch.join("source.sqlite");
        let db = Connection::open(&database).unwrap();
        db.execute_batch("CREATE TABLE notes(id INTEGER, flds TEXT); CREATE TABLE cards(did INTEGER, nid INTEGER);
            CREATE TABLE decks(id INTEGER, name TEXT); INSERT INTO cards VALUES (4, 1); INSERT INTO decks VALUES (4, 'Local deck');").unwrap();
        db.execute(
            "INSERT INTO notes VALUES (1, ?)",
            ["<b>Hello</b>\x1fXin chào<br>Good morning"],
        )
        .unwrap();
        drop(db);
        let collection = fs::read(database).unwrap();
        for (name, data) in [
            ("collection.anki2", collection.clone()),
            (
                "collection.anki21b",
                zstd::stream::encode_all(&collection[..], 1).unwrap(),
            ),
        ] {
            let bundle = scratch.join("cards.apkg");
            let mut zip = zip::ZipWriter::new(File::create(&bundle).unwrap());
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(&data).unwrap();
            zip.finish().unwrap();
            let decks = import_apkg(&bundle, &scratch).unwrap();
            assert_eq!(decks[0].name, "Local deck");
            assert_eq!(decks[0].cards[0].front, "Hello");
            assert_eq!(decks[0].cards[0].back, "Xin chào\nGood morning");
        }
        fs::remove_dir_all(scratch).unwrap();
    }
}
