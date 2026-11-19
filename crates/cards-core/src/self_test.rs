use std::collections::HashSet;

use serde::{Deserialize, Serialize};

const SIZE: usize = 10;

/// A practice session is independent of each card's due date and review statistics.
#[derive(Debug, Serialize, Deserialize)]
pub struct SelfTest {
    cards: Vec<usize>,
    position: usize,
    revealed: bool,
    missed: Vec<usize>,
    next_start: usize,
}

impl SelfTest {
    pub(super) fn start(length: usize, previous: Option<&Self>) -> Option<Self> {
        if length == 0 {
            return None;
        }
        let start = previous.map_or(0, |test| test.next_start % length);
        let count = length.min(SIZE);
        let cards = (0..count).map(|offset| (start + offset) % length).collect();
        Some(Self::from_cards(cards, (start + count) % length))
    }

    fn from_cards(cards: Vec<usize>, next_start: usize) -> Self {
        Self {
            cards,
            position: 0,
            revealed: false,
            missed: Vec::new(),
            next_start,
        }
    }

    pub fn progress(&self) -> (usize, usize) {
        (self.position, self.cards.len())
    }

    pub fn current(&self) -> Option<usize> {
        self.cards.get(self.position).copied()
    }

    pub fn is_revealed(&self) -> bool {
        self.revealed
    }

    pub fn missed_count(&self) -> usize {
        self.missed.len()
    }

    pub fn reveal(&mut self) {
        if self.current().is_some() {
            self.revealed = true;
        }
    }

    pub fn grade(&mut self, recalled: bool) -> bool {
        let Some(card) = self.current().filter(|_| self.revealed) else {
            return false;
        };
        if !recalled {
            self.missed.push(card);
        }
        self.position += 1;
        self.revealed = false;
        true
    }

    pub fn retry(&mut self) -> bool {
        if self.current().is_some() || self.missed.is_empty() {
            return false;
        }
        *self = Self::from_cards(std::mem::take(&mut self.missed), self.next_start);
        true
    }

    pub(super) fn valid(&self, deck_size: usize) -> bool {
        (1..=SIZE).contains(&self.cards.len())
            && self.position <= self.cards.len()
            && self.cards.iter().all(|&index| index < deck_size)
            && self.cards.iter().collect::<HashSet<_>>().len() == self.cards.len()
            && self.missed.iter().collect::<HashSet<_>>().len() == self.missed.len()
            && self
                .missed
                .iter()
                .all(|entry| self.cards[..self.position].contains(entry))
            && (!self.revealed || self.current().is_some())
    }
}
