use std::{
    fs,
    path::{Path, PathBuf},
};

use cards_core::{Card, Deck, Library};
use cards_render::View;
use primitive_app::{font_paths, frame, write_png, App, Change, HEIGHT, WIDTH};
use primitive_ui::{Canvas, Input};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .unwrap_or_else(|| "build/previews".into()),
    );
    fs::create_dir_all(&output)?;
    let fonts =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../doannt-kindle-launcher/assets/fonts");
    let mut canvas = Canvas::new(WIDTH, HEIGHT, &font_paths(&fonts))?;
    let mut library = Library::default();
    library.decks.push(Deck {
        name: "Vietnamese greetings".into(),
        cards: vec![
            Card::new("Good morning".into(), "Chào buổi sáng".into()),
            Card::new("See you later".into(), "Hẹn gặp lại".into()),
        ],
    });
    let mut app = View::new(library, None, output.clone());
    write_png(
        &output.join("cards-decks.png"),
        WIDTH,
        HEIGHT,
        &frame(&app, &mut canvas, false),
    )?;
    assert_eq!(app.input(Input::Tap(140.0, 280.0)), Change::Full);
    assert_eq!(app.input(Input::Tap(400.0, 1000.0)), Change::Full);
    write_png(
        &output.join("cards-question.png"),
        WIDTH,
        HEIGHT,
        &frame(&app, &mut canvas, false),
    )?;
    assert_eq!(app.input(Input::Tap(400.0, 1080.0)), Change::Full);
    write_png(
        &output.join("cards-answer-dark.png"),
        WIDTH,
        HEIGHT,
        &frame(&app, &mut canvas, true),
    )?;

    let front = (1..=12)
        .map(|i| format!("Question line {i}."))
        .collect::<Vec<_>>()
        .join("\n");
    let back = (1..=12)
        .map(|i| format!("Dòng đáp án {i}."))
        .collect::<Vec<_>>()
        .join("\n");
    let back = format!("{back}\n{}", "W".repeat(220));
    let mut library = Library::default();
    library.decks.push(Deck {
        name: "Long card".into(),
        cards: vec![Card::new(front, back)],
    });
    let mut app = View::new(library, None, output.clone());
    app.input(Input::Tap(140.0, 280.0));
    app.input(Input::Tap(400.0, 1000.0));
    let first = frame(&app, &mut canvas, false);
    assert_eq!(app.input(Input::PageForward), Change::Partial);
    assert_ne!(first, frame(&app, &mut canvas, false));
    assert_eq!(app.input(Input::PageForward), Change::None);
    assert_eq!(app.input(Input::Tap(400.0, 1080.0)), Change::Full);
    let first_answer = frame(&app, &mut canvas, false);
    assert_eq!(app.input(Input::PageBack), Change::None);
    assert_eq!(app.input(Input::Tap(1000.0, 1395.0)), Change::Partial);
    let second_answer = frame(&app, &mut canvas, false);
    assert_ne!(first_answer, second_answer);
    assert_eq!(app.input(Input::PageForward), Change::Partial);
    write_png(
        &output.join("cards-answer-next-page.png"),
        WIDTH,
        HEIGHT,
        &second_answer,
    )?;
    let documents = output.join("test-documents");
    fs::create_dir_all(&documents)?;
    fs::write(
        documents.join("Weather words.tsv"),
        "Rain\tMưa\nCloud\tMây\nRain\tMưa\n",
    )?;
    let mut app = View::new(Library::default(), None, documents);
    assert_eq!(app.input(Input::Tap(800.0, 1510.0)), Change::Full);
    assert_eq!(app.input(Input::Tap(150.0, 280.0)), Change::Partial);
    let start = std::time::Instant::now();
    loop {
        if app.tick(std::time::Instant::now()) == Change::Full {
            break;
        }
        assert!(start.elapsed().as_secs() < 3);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(app.library.decks.is_empty());
    write_png(
        &output.join("cards-import-preview.png"),
        WIDTH,
        HEIGHT,
        &frame(&app, &mut canvas, false),
    )?;
    assert_eq!(app.input(Input::Tap(240.0, 1320.0)), Change::Full);
    assert_eq!(app.library.decks[0].cards.len(), 2);
    assert_eq!(app.input(Input::Tap(240.0, 1290.0)), Change::Full);
    assert_eq!(app.input(Input::Tap(400.0, 1080.0)), Change::Full);
    write_png(
        &output.join("cards-self-test.png"),
        WIDTH,
        HEIGHT,
        &frame(&app, &mut canvas, true),
    )?;
    assert_eq!(app.input(Input::Tap(300.0, 1100.0)), Change::Full);
    assert_eq!(app.input(Input::Tap(400.0, 1080.0)), Change::Full);
    assert_eq!(app.input(Input::Tap(850.0, 1100.0)), Change::Full);
    assert_eq!(app.library.self_test(0).unwrap().missed_count(), 1);
    assert_eq!(app.library.decks[0].statistics(), (0, 0));
    assert_eq!(app.input(Input::Tap(400.0, 1080.0)), Change::Full);
    assert_eq!(app.library.self_test(0).unwrap().current(), Some(0));
    Ok(())
}
