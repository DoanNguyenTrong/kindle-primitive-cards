use std::{
    cell::RefCell,
    fs, io,
    ops::Range,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use cards_core::{import_apkg, Card, Deck, Library};
use primitive_app::{App, Change};
use primitive_ui::{
    back_button, header, keyboard_key, Canvas, Editor, Input, Layout, Press, Rect, TextStyle, BODY,
    CAPTION,
};

const PAGE_SIZE: usize = 7;
const NEW: Rect = Rect::new(40, 1460, 450, 100);
const IMPORT: Rect = Rect::new(513, 1460, 710, 100);
const MAIN_TEXT: Rect = Rect::new(40, 220, 1184, 650);
const OPEN_CARD: Rect = Rect::new(40, 1040, 1184, 100);
const PREVIOUS: Rect = Rect::new(40, 1350, 280, 100);
const NEXT: Rect = Rect::new(944, 1350, 280, 100);
const FACE_STYLE: TextStyle = TextStyle::new(57)
    .bold()
    .center()
    .lines(8)
    .break_long_words();
type PendingImport = mpsc::Receiver<Result<Vec<Deck>, String>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Library,
    Deck(usize),
    NewDeck,
    Front(usize),
    Back(usize),
    Import,
    Review(usize, usize, bool),
}

pub struct View {
    pub library: Library,
    pub data: Option<PathBuf>,
    pub documents: PathBuf,
    mode: Mode,
    editor: Editor,
    front: String,
    files: Vec<PathBuf>,
    page: usize,
    review_pages: RefCell<Vec<Range<usize>>>,
    notice: String,
    importing: Option<PendingImport>,
    import_check: Option<Instant>,
}

pub fn day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400
}

impl View {
    pub fn new(library: Library, data: Option<PathBuf>, documents: PathBuf) -> Self {
        Self {
            library,
            data,
            documents,
            mode: Mode::Library,
            editor: Editor::new(512),
            front: String::new(),
            files: Vec::new(),
            page: 0,
            review_pages: RefCell::new(Vec::new()),
            notice: String::new(),
            importing: None,
            import_check: None,
        }
    }

    fn go(&mut self, mode: Mode) -> Change {
        self.mode = mode;
        self.page = 0;
        self.review_pages.get_mut().clear();
        self.notice.clear();
        Change::Full
    }

    fn remember(&mut self, message: &str) {
        self.notice = match self.save() {
            Ok(()) => message.into(),
            Err(error) => format!("Could not save. {error}"),
        };
    }

    fn row(index: usize) -> Rect {
        Rect::new(40, 240 + index as i32 * 160, 1184, 136)
    }

    fn review(&mut self, index: usize) -> Change {
        let next = self.library.decks[index].due(day()).next().map(|(i, _)| i);
        if let Some(card) = next {
            self.go(Mode::Review(index, card, false))
        } else {
            self.go(Mode::Deck(index));
            self.notice = "All caught up for today.".into();
            Change::Full
        }
    }

    fn page(&mut self, delta: isize) -> Change {
        let pages = match self.mode {
            Mode::Import => self.files.len().div_ceil(PAGE_SIZE),
            Mode::Library => self.library.decks.len().div_ceil(PAGE_SIZE),
            Mode::Review(..) => self.review_pages.get_mut().len(),
            _ => return Change::None,
        };
        let next = self
            .page
            .saturating_add_signed(delta)
            .min(pages.saturating_sub(1));
        if self.page == next {
            return Change::None;
        }
        self.page = next;
        Change::Partial
    }

    fn editor_tap(&mut self, x: f32, y: f32) -> Change {
        let layout = Layout::new(1264, 1680, 4, 5);
        let Some(key) = keyboard_key(&layout, &self.editor, "Save", x, y) else {
            return Change::None;
        };
        match self.editor.press(key) {
            Press::Nothing => Change::None,
            Press::Go => {
                let value = self.editor.string();
                let value = value.trim();
                if value.is_empty() {
                    self.notice = "Type something first.".into();
                    return Change::Partial;
                }
                match self.mode {
                    Mode::NewDeck => {
                        self.library.decks.push(Deck {
                            name: value.into(),
                            cards: Vec::new(),
                        });
                        self.go(Mode::Deck(self.library.decks.len() - 1));
                        self.remember("Deck created.");
                        Change::Full
                    }
                    Mode::Front(index) => {
                        self.front = value.into();
                        self.editor = Editor::new(512);
                        self.go(Mode::Back(index))
                    }
                    Mode::Back(index) => {
                        self.library.decks[index]
                            .cards
                            .push(Card::new(std::mem::take(&mut self.front), value.into()));
                        self.go(Mode::Deck(index));
                        self.remember("Card added.");
                        Change::Full
                    }
                    _ => Change::None,
                }
            }
            _ => Change::Partial,
        }
    }

    fn list_files(&mut self) -> io::Result<()> {
        self.files = primitive_app::with_storage(|| {
            Ok(fs::read_dir(&self.documents)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("apkg"))
                })
                .collect())
        })?;
        self.files.sort_unstable();
        Ok(())
    }

    fn draw_pager(&self, canvas: &mut Canvas, pages: usize) {
        if pages > 1 {
            canvas.button(PREVIOUS, "Previous", false, false);
            canvas.button(NEXT, "Next", false, false);
            canvas.text(
                Rect::new(339, 1377, 584, 60),
                &format!("Page {} of {pages}", self.page + 1),
                CAPTION.center(),
            );
        }
    }
}

impl App for View {
    fn draw(&self, canvas: &mut Canvas) {
        match self.mode {
            Mode::Library => {
                header(canvas, "Cards", "Done");
                for (i, deck) in self
                    .library
                    .decks
                    .iter()
                    .skip(self.page * PAGE_SIZE)
                    .take(PAGE_SIZE)
                    .enumerate()
                {
                    let row = Self::row(i);
                    let pressed = canvas.control_pressed(row, false);
                    canvas.panel(row, 22, 2, if pressed { 220 } else { 255 });
                    canvas.text(
                        Rect::new(row.x + 24, row.y + 15, row.w - 48, 65),
                        &deck.name,
                        TextStyle::new(45).bold(),
                    );
                    canvas.text(
                        Rect::new(row.x + 24, row.y + 75, row.w - 48, 55),
                        &format!(
                            "{} cards · {} due",
                            deck.cards.len(),
                            deck.due(day()).count()
                        ),
                        CAPTION,
                    );
                }
                if self.library.decks.is_empty() {
                    canvas.text(
                        MAIN_TEXT,
                        "Create a deck or import one from Documents.",
                        BODY,
                    );
                }
                self.draw_pager(canvas, self.library.decks.len().div_ceil(PAGE_SIZE));
                canvas.button(NEW, "New deck", true, false);
                canvas.button(IMPORT, "Import Anki deck", false, false);
            }
            Mode::Import => {
                if self.importing.is_some() {
                    header(canvas, "Import Anki deck", "Waiting");
                    canvas.text(
                        MAIN_TEXT,
                        "Reading the Anki package. This can take a moment.",
                        BODY.lines(2),
                    );
                    return;
                }
                header(canvas, "Import Anki deck", "Back");
                for (i, file) in self
                    .files
                    .iter()
                    .skip(self.page * PAGE_SIZE)
                    .take(PAGE_SIZE)
                    .enumerate()
                {
                    let row = Self::row(i);
                    canvas.button(
                        row,
                        file.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("Anki package"),
                        false,
                        false,
                    );
                }
                if self.files.is_empty() {
                    canvas.text(
                        MAIN_TEXT,
                        "Copy an Anki .apkg file to Documents, then reopen this page.",
                        BODY.lines(3),
                    );
                }
                self.draw_pager(canvas, self.files.len().div_ceil(PAGE_SIZE));
            }
            Mode::Deck(index) => {
                header(canvas, "Deck", "Back");
                let deck = &self.library.decks[index];
                canvas.text(
                    Rect::new(40, 220, 1184, 160),
                    &deck.name,
                    TextStyle::new(68).bold().lines(2),
                );
                let (reviews, remembered) = deck.statistics();
                let facts = format!(
                    "{} cards\n{} due today\n{reviews} reviews · {remembered} remembered",
                    deck.cards.len(),
                    deck.due(day()).count()
                );
                canvas.text(
                    Rect::new(40, 465, 1184, 400),
                    &facts,
                    TextStyle::new(43).lines(5),
                );
                canvas.button(
                    Rect::new(40, 975, 1184, 110),
                    "Review due cards",
                    true,
                    false,
                );
                canvas.button(Rect::new(40, 1115, 570, 110), "New card", false, false);
                canvas.button(Rect::new(645, 1115, 579, 110), "Export text", false, false);
            }
            Mode::NewDeck | Mode::Front(_) | Mode::Back(_) => {
                let title = match self.mode {
                    Mode::NewDeck => "Name the deck",
                    Mode::Front(_) => "Card front",
                    _ => "Card back",
                };
                header(canvas, title, "Cancel");
                canvas.card(MAIN_TEXT);
                canvas.text(
                    MAIN_TEXT.inset(34),
                    &self.editor.string(),
                    TextStyle::new(48).lines(9),
                );
                canvas.text(Rect::new(40, 883, 1184, 62), &self.notice, CAPTION);
                canvas.keyboard(self.editor.layer, "Save", None);
            }
            Mode::Review(deck, index, flipped) => {
                header(canvas, "Review", "Back");
                let deck = &self.library.decks[deck];
                let card = &deck.cards[index];
                canvas.text(
                    Rect::new(40, 172, 1184, 57),
                    &format!("{} · {} due", deck.name, deck.due(day()).count()),
                    CAPTION,
                );
                let pressed = !flipped && canvas.control_pressed(MAIN_TEXT, false);
                canvas.panel(MAIN_TEXT, 22, 2, if pressed { 220 } else { 255 });
                let rect = MAIN_TEXT.inset(42);
                let face = if flipped { &card.back } else { &card.front };
                let mut pages = self.review_pages.borrow_mut();
                if pages.is_empty() {
                    *pages = canvas.text_pages(rect, face, FACE_STYLE);
                }
                if let Some(range) = pages.get(self.page) {
                    canvas.text(rect, face[range.clone()].trim_end(), FACE_STYLE);
                }
                self.draw_pager(canvas, pages.len());
                if flipped {
                    for (i, label) in ["Again", "Hard", "Good", "Easy"].iter().enumerate() {
                        canvas.button(
                            Rect::new(40 + 300 * i as i32, 1050, 284, 112),
                            label,
                            i == 2,
                            false,
                        );
                    }
                } else {
                    canvas.button(OPEN_CARD, "Show answer", true, false);
                }
            }
        }
        canvas.text(
            Rect::new(40, 1586, 1184, 66),
            &self.notice,
            CAPTION.lines(2),
        );
    }

    fn input(&mut self, input: Input) -> Change {
        if self.importing.is_some() {
            return Change::None;
        }
        if matches!(input, Input::PageBack | Input::PageForward) {
            return self.page(if input == Input::PageBack { -1 } else { 1 });
        }
        let Input::Tap(x, y) = input else {
            return Change::None;
        };
        if back_button(1264).contains(x, y) {
            return match self.mode {
                Mode::Library => Change::Exit,
                Mode::Front(deck) | Mode::Back(deck) | Mode::Review(deck, _, _) => {
                    self.go(Mode::Deck(deck))
                }
                _ => self.go(Mode::Library),
            };
        }
        if matches!(self.mode, Mode::Library | Mode::Import | Mode::Review(..)) {
            if PREVIOUS.contains(x, y) {
                return self.page(-1);
            }
            if NEXT.contains(x, y) {
                return self.page(1);
            }
        }
        match self.mode {
            Mode::Library | Mode::Import => {
                if self.mode == Mode::Library && NEW.contains(x, y) {
                    self.editor = Editor::new(120);
                    return self.go(Mode::NewDeck);
                }
                if self.mode == Mode::Library && IMPORT.contains(x, y) {
                    if let Err(error) = self.list_files() {
                        self.notice = format!("Could not open Documents. {error}");
                        return Change::Partial;
                    }
                    return self.go(Mode::Import);
                }
                let Some(row) = (0..PAGE_SIZE).find(|&i| Self::row(i).contains(x, y)) else {
                    return Change::None;
                };
                let index = row + self.page * PAGE_SIZE;
                if self.mode == Mode::Library {
                    return if index < self.library.decks.len() {
                        self.go(Mode::Deck(index))
                    } else {
                        Change::None
                    };
                }
                if let Some(file) = self.files.get(index) {
                    let scratch = self
                        .data
                        .as_deref()
                        .and_then(std::path::Path::parent)
                        .map_or_else(std::env::temp_dir, std::path::Path::to_path_buf);
                    let file = file.clone();
                    let (tx, rx) = mpsc::channel();
                    let worker =
                        thread::Builder::new()
                            .name("import-cards".into())
                            .spawn(move || {
                                let decks = primitive_app::with_storage(|| {
                                    import_apkg(&file, &scratch).map_err(io::Error::other)
                                });
                                let _ = tx.send(decks.map_err(|error| error.to_string()));
                            });
                    if let Err(error) = worker {
                        self.notice = format!("Could not start the import. {error}");
                        return Change::Partial;
                    }
                    self.importing = Some(rx);
                    self.import_check = Some(Instant::now() + Duration::from_millis(150));
                    return Change::Partial;
                }
                Change::None
            }
            Mode::Deck(index) => {
                if Rect::new(40, 975, 1184, 110).contains(x, y) {
                    return self.review(index);
                }
                if Rect::new(40, 1115, 570, 110).contains(x, y) {
                    self.editor = Editor::new(512);
                    return self.go(Mode::Front(index));
                }
                if Rect::new(645, 1115, 579, 110).contains(x, y) {
                    let filename = format!("Primitive Cards deck {}.txt", index + 1);
                    self.notice = match primitive_app::save(
                        &self.documents.join(&filename),
                        self.library.decks[index].export().as_bytes(),
                    ) {
                        Ok(()) => format!("Exported {filename} to Documents."),
                        Err(error) => format!("Could not export. {error}"),
                    };
                    return Change::Partial;
                }
                Change::None
            }
            Mode::Review(deck, index, false)
                if OPEN_CARD.contains(x, y) || MAIN_TEXT.contains(x, y) =>
            {
                self.go(Mode::Review(deck, index, true))
            }
            Mode::Review(deck, index, true) => {
                for (i, quality) in [1, 3, 4, 5].into_iter().enumerate() {
                    if Rect::new(40 + 300 * i as i32, 1050, 284, 112).contains(x, y) {
                        self.library.decks[deck].cards[index].review(day(), quality);
                        let result = self.review(deck);
                        if let Err(error) = self.save() {
                            self.notice = format!("Could not save. {error}");
                        }
                        return result;
                    }
                }
                Change::None
            }
            Mode::Review(..) => Change::None,
            _ => self.editor_tap(x, y),
        }
    }

    fn deadline(&self) -> Option<Instant> {
        self.import_check
    }

    fn tick(&mut self, now: Instant) -> Change {
        let Some(job) = &self.importing else {
            return Change::None;
        };
        let result = match job.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                self.import_check = Some(now + Duration::from_millis(150));
                return Change::None;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("The import could not finish.".into()),
        };
        self.import_check = None;
        self.importing = None;
        match result {
            Ok(decks) => {
                let count = decks.len();
                self.library.decks.extend(decks);
                self.go(Mode::Library);
                self.remember(&format!(
                    "Imported {count} {}",
                    if count == 1 { "deck." } else { "decks." }
                ));
                Change::Full
            }
            Err(error) => {
                self.notice = error;
                Change::Partial
            }
        }
    }

    fn save(&mut self) -> io::Result<()> {
        if let Some(path) = &self.data {
            primitive_app::save(path, &self.library.encoded())?;
        }
        Ok(())
    }
}
