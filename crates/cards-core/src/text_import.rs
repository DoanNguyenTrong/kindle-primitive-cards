use std::{collections::HashSet, fs::File, io::Read, path::Path};

use crate::{Card, Deck, Library};

const MAX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ROWS: usize = 10_000;
const MAX_FIELD: usize = 8_192;

#[derive(Debug)]
pub struct TextImport {
    pub deck: Deck,
    pub skipped: usize,
}

impl TextImport {
    /// Compare both faces, keeping identically named or overlapping decks usable.
    pub fn skip_existing(&mut self, library: &Library) {
        let existing = library
            .decks
            .iter()
            .flat_map(|deck| deck.cards.iter())
            .map(|card| (card.front.as_str(), card.back.as_str()))
            .collect::<HashSet<_>>();
        let before = self.deck.cards.len();
        self.deck
            .cards
            .retain(|card| !existing.contains(&(card.front.as_str(), card.back.as_str())));
        self.skipped += before - self.deck.cards.len();
    }
}

pub fn import_tsv(path: &Path) -> Result<TextImport, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(MAX_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|error| format!("Could not read this file. {error}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Text deck files must be 8 MB or smaller.".into());
    }
    let text = String::from_utf8(bytes).map_err(|_| "Save the deck as UTF-8 text first.")?;
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Text deck");
    parse(
        &text,
        &name
            .chars()
            .filter(|c| !c.is_control())
            .take(120)
            .collect::<String>(),
    )
}

fn parse(text: &str, name: &str) -> Result<TextImport, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut seen = HashSet::new();
    let mut cards = Vec::new();
    let mut skipped = 0;
    for (line_index, row) in text.lines().enumerate() {
        if line_index >= MAX_ROWS {
            return Err("Text decks can contain up to 10,000 lines.".into());
        }
        if row.trim().is_empty() {
            continue;
        }
        let (front, back) = row
            .split_once('\t')
            .filter(|(_, back)| !back.contains('\t'))
            .ok_or_else(|| format!("Line {} needs two tab-separated fields.", line_index + 1))?;
        if front.len() > MAX_FIELD || back.len() > MAX_FIELD {
            return Err(format!(
                "Line {} contains a field that is too long.",
                line_index + 1
            ));
        }
        let front = unescape(front);
        let back = unescape(back);
        if front.trim().is_empty() || back.trim().is_empty() {
            return Err(format!("Line {} needs text on both sides.", line_index + 1));
        }
        if seen.insert((front.clone(), back.clone())) {
            cards.push(Card::new(front, back));
        } else {
            skipped += 1;
        }
    }
    if cards.is_empty() {
        return Err("This file contains no cards.".into());
    }
    let name = if name.trim().is_empty() {
        "Text deck"
    } else {
        name
    };
    Ok(TextImport {
        deck: Deck {
            name: name.into(),
            cards,
        },
        skipped,
    })
}

/// Reverse the escaping used in Cards exports. Preserve unknown escapes.
fn unescape(field: &str) -> String {
    let mut decoded = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            decoded.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => decoded.push('\n'),
            Some('t') => decoded.push('\t'),
            Some('\\') => decoded.push('\\'),
            other => {
                decoded.push('\\');
                decoded.extend(other);
            }
        }
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_utf8_bom_crlf_and_round_trips_escaped_exports() {
        let mut original = Deck::default();
        original.cards.push(Card::new(
            "hẹn gặp\tlater".into(),
            "new\nline\\value".into(),
        ));
        let text = format!("\u{feff}{}", original.export().replace('\n', "\r\n"));
        let parsed = parse(&text, "Imported practice").unwrap();
        assert_eq!(parsed.deck.export(), original.export());
    }

    #[test]
    fn rejects_malformed_rows_and_skips_duplicate_pairs() {
        let mut parsed = parse("A\tB\nA\tB\nA\tC\n", "Example").unwrap();
        assert_eq!(parsed.deck.cards.len(), 2);
        assert_eq!(parsed.skipped, 1);
        let mut library = Library::default();
        library.decks.push(Deck {
            name: "Existing".into(),
            cards: vec![Card::new("A".into(), "C".into())],
        });
        parsed.skip_existing(&library);
        assert_eq!(parsed.deck.cards.len(), 1);
        assert_eq!(parsed.skipped, 2);
        assert!(parse("A\tB\tC", "Example").is_err());
        assert!(parse("missing", "Example").is_err());
        assert!(parse("A\t\\n", "Example").is_err());
    }
}
