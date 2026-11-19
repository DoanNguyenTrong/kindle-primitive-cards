mod import;
mod self_test;
mod text_import;

pub use import::import_apkg;
pub use self_test::SelfTest;
pub use text_import::{import_tsv, TextImport};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub front: String,
    pub back: String,
    pub due: u64,
    ease: i32,
    interval: u32,
    consecutive: u32,
    pub reviews: u32,
    pub remembered: u32,
}

impl Card {
    pub fn new(front: String, back: String) -> Self {
        Self {
            front,
            back,
            due: 0,
            ease: 2500,
            interval: 0,
            consecutive: 0,
            reviews: 0,
            remembered: 0,
        }
    }

    pub fn review(&mut self, day: u64, quality: u8) {
        let quality = i32::from(quality.min(5));
        if quality < 3 {
            self.consecutive = 0;
            self.interval = 1;
        } else {
            self.interval = match self.consecutive {
                0 => 1,
                1 => 6,
                _ => ((u64::from(self.interval) * self.ease.clamp(1300, 100_000) as u64)
                    .div_ceil(1000))
                .min(36_500) as u32,
            };
            self.consecutive = self.consecutive.saturating_add(1);
            self.remembered = self.remembered.saturating_add(1);
        }
        let gap = 5 - quality;
        self.ease = self
            .ease
            .clamp(1300, 100_000)
            .saturating_add(100 - gap * (80 + gap * 20))
            .max(1300);
        self.reviews = self.reviews.saturating_add(1);
        self.due = day.saturating_add(self.interval.into());
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Deck {
    pub name: String,
    pub cards: Vec<Card>,
}

impl Deck {
    pub fn due(&self, day: u64) -> impl Iterator<Item = (usize, &Card)> {
        self.cards
            .iter()
            .enumerate()
            .filter(move |(_, card)| card.due <= day)
    }

    pub fn statistics(&self) -> (u64, u64) {
        self.cards.iter().fold((0, 0), |(total, recalled), card| {
            (
                total + u64::from(card.reviews),
                recalled + u64::from(card.remembered),
            )
        })
    }

    pub fn export(&self) -> String {
        fn escape(value: &str) -> String {
            value
                .replace('\\', "\\\\")
                .replace('\t', "\\t")
                .replace('\n', "\\n")
                .replace('\r', "")
        }
        self.cards
            .iter()
            .map(|card| format!("{}\t{}\n", escape(&card.front), escape(&card.back)))
            .collect()
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Library {
    pub decks: Vec<Deck>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    self_tests: BTreeMap<usize, SelfTest>,
}

impl Library {
    pub fn restore(bytes: &[u8]) -> Self {
        let mut library = serde_json::from_slice::<Self>(bytes).unwrap_or_default();
        library.self_tests.retain(|index, test| {
            library
                .decks
                .get(*index)
                .is_some_and(|deck| test.valid(deck.cards.len()))
        });
        library
    }

    pub fn self_test(&self, deck: usize) -> Option<&SelfTest> {
        self.self_tests.get(&deck)
    }

    pub fn self_test_mut(&mut self, deck: usize) -> Option<&mut SelfTest> {
        self.self_tests.get_mut(&deck)
    }

    pub fn start_test(&mut self, deck: usize) -> bool {
        let Some(test) = self
            .decks
            .get(deck)
            .and_then(|deck_data| SelfTest::start(deck_data.cards.len(), self.self_test(deck)))
        else {
            return false;
        };
        self.self_tests.insert(deck, test);
        true
    }

    pub fn encoded(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("cards contain serializable text and numbers")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_test_reopens_retries_and_never_changes_scheduled_reviews() {
        let mut library = Library::default();
        let mut first = Card::new("Prompt".into(), "Answer".into());
        first.review(100, 5);
        let before = serde_json::to_value(&first).unwrap();
        let mut cards = vec![first];
        cards.extend((1..12).map(|index| Card::new(format!("Prompt {index}"), "Answer".into())));
        library.decks.push(Deck {
            name: "Deck".into(),
            cards,
        });
        assert!(library.start_test(0));
        let test = library.self_test_mut(0).unwrap();
        assert!(!test.grade(true));
        test.reveal();
        assert!(test.grade(false));
        test.reveal();
        let mut library = Library::restore(&library.encoded());
        let test = library.self_test_mut(0).unwrap();
        assert_eq!(test.progress(), (1, 10));
        assert_eq!(test.current(), Some(1));
        assert!(test.is_revealed());
        for _ in 1..10 {
            test.reveal();
            assert!(test.grade(true));
        }
        assert_eq!(test.missed_count(), 1);
        assert!(test.retry());
        assert_eq!(test.current(), Some(0));
        assert!(library.start_test(0));
        assert_eq!(library.self_test(0).unwrap().current(), Some(10));
        assert_eq!(
            before,
            serde_json::to_value(&library.decks[0].cards[0]).unwrap()
        );
    }

    #[test]
    fn successful_repetitions_space_out_and_a_miss_resets_the_interval() {
        let mut card = Card::new("Front".into(), "Back".into());
        card.review(100, 4);
        assert_eq!(card.due, 101);
        card.review(101, 4);
        assert_eq!(card.due, 107);
        card.review(107, 4);
        assert_eq!(card.due, 122);
        card.review(122, 1);
        assert_eq!(card.due, 123);
        assert_eq!(card.reviews, 4);
        assert_eq!(card.remembered, 3);
    }
}
