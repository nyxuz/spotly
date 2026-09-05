use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode},
    execute,
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};

const GLYPH_HEIGHT: usize = 7;

// 1.5x scaling = 3 / 2
const SCALE_NUMERATOR: usize = 3;
const SCALE_DENOMINATOR: usize = 2;

const LETTER_SPACING: usize = 2;

pub fn init() -> io::Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();

    execute!(
        stdout,
        EnterAlternateScreen,
        Hide,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )?;

    stdout.flush()?;

    Ok(())
}

pub fn cleanup() {
    let mut stdout = io::stdout();

    let _ = execute!(
        stdout,
        Show,
        Clear(ClearType::All),
        MoveTo(0, 0),
        LeaveAlternateScreen
    );

    let _ = disable_raw_mode();

    let _ = stdout.flush();
}

pub fn should_quit() -> bool {
    match event::poll(Duration::from_millis(0)) {
        Ok(true) => match event::read() {
            Ok(Event::Key(key)) => {
                matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            }
            _ => false,
        },

        _ => false,
    }
}

pub fn render(word: Option<&str>) {
    let mut stdout = io::stdout();

    let word = match word {
        Some(word) if !word.trim().is_empty() => word.trim(),
        _ => "♪",
    };

    let lines = render_word(word);

    let (terminal_width, terminal_height) = terminal_size();

    let content_width = lines
        .iter()
        .map(|line| display_width(line))
        .max()
        .unwrap_or(0);

    let content_height = lines.len();

    let left_padding = if content_width < terminal_width {
        (terminal_width - content_width) / 2
    } else {
        0
    };

    let top_padding = if content_height < terminal_height {
        (terminal_height - content_height) / 2
    } else {
        0
    };

    let _ = execute!(
        stdout,
        MoveTo(0, 0),
        Clear(ClearType::All),
        MoveTo(left_padding as u16, top_padding as u16)
    );

    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            let _ = write!(stdout, "\r\n{}", " ".repeat(left_padding));
        }

        let _ = write!(stdout, "{line}");
    }

    let _ = stdout.flush();
}

fn render_word(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();

    let scaled_rows = scale_rows(GLYPH_HEIGHT);

    let mut lines = Vec::with_capacity(scaled_rows.len());

    for &source_row in &scaled_rows {
        let mut line = String::new();

        for (char_index, character) in chars.iter().enumerate() {
            let glyph = glyph(*character);
            let row = glyph[source_row];

            let scaled = scale_row(row);

            line.push_str(&scaled);

            if char_index + 1 < chars.len() {
                line.push_str(&" ".repeat(LETTER_SPACING));
            }
        }

        lines.push(line);
    }

    lines
}

/// Scale the 5x7 glyph vertically by exactly 1.5x.
///
/// 7 source rows become 10 output rows.
///
/// Mapping:
///
/// 0
/// 0
/// 1
/// 2
/// 2
/// 3
/// 4
/// 4
/// 5
/// 6
fn scale_rows(height: usize) -> Vec<usize> {
    let output_height = height * SCALE_NUMERATOR / SCALE_DENOMINATOR;

    (0..output_height)
        .map(|output_row| {
            let source_row = output_row * SCALE_DENOMINATOR / SCALE_NUMERATOR;

            source_row.min(height - 1)
        })
        .collect()
}

/// Scale a single 5-column row horizontally by 1.5x.
///
/// 5 source columns become 7 output columns.
fn scale_row(row: &str) -> String {
    let chars: Vec<char> = row.chars().collect();

    let output_width = chars.len() * SCALE_NUMERATOR / SCALE_DENOMINATOR;

    let mut result = String::with_capacity(output_width);

    for output_column in 0..output_width {
        let source_column = output_column * SCALE_DENOMINATOR / SCALE_NUMERATOR;

        let character = chars.get(source_column).copied().unwrap_or(' ');

        result.push(character);
    }

    result
}

fn glyph(character: char) -> [&'static str; GLYPH_HEIGHT] {
    match character.to_ascii_uppercase() {
        'A' => [
            "  █  ",
            " █ █ ",
            "█   █",
            "█████",
            "█   █",
            "█   █",
            "█   █",
        ],

        'B' => [
            "████ ",
            "█   █",
            "█   █",
            "████ ",
            "█   █",
            "█   █",
            "████ ",
        ],

        'C' => [
            " ████",
            "█    ",
            "█    ",
            "█    ",
            "█    ",
            "█    ",
            " ████",
        ],

        'D' => [
            "████ ",
            "█   █",
            "█   █",
            "█   █",
            "█   █",
            "█   █",
            "████ ",
        ],

        'E' => [
            "█████",
            "█    ",
            "█    ",
            "████ ",
            "█    ",
            "█    ",
            "█████",
        ],

        'F' => [
            "█████",
            "█    ",
            "█    ",
            "████ ",
            "█    ",
            "█    ",
            "█    ",
        ],

        'G' => [
            " ████",
            "█    ",
            "█    ",
            "█ ███",
            "█   █",
            "█   █",
            " ████",
        ],

        'H' => [
            "█   █",
            "█   █",
            "█   █",
            "█████",
            "█   █",
            "█   █",
            "█   █",
        ],

        'I' => [
            "█████",
            "  █  ",
            "  █  ",
            "  █  ",
            "  █  ",
            "  █  ",
            "█████",
        ],

        'J' => [
            "█████",
            "    █",
            "    █",
            "    █",
            "    █",
            "█   █",
            " ███ ",
        ],

        'K' => [
            "█   █",
            "█  █ ",
            "█ █  ",
            "███  ",
            "█ █  ",
            "█  █ ",
            "█   █",
        ],

        'L' => [
            "█    ",
            "█    ",
            "█    ",
            "█    ",
            "█    ",
            "█    ",
            "█████",
        ],

        'M' => [
            "█   █",
            "██ ██",
            "█ █ █",
            "█ █ █",
            "█   █",
            "█   █",
            "█   █",
        ],

        'N' => [
            "█   █",
            "██  █",
            "██  █",
            "█ █ █",
            "█  ██",
            "█  ██",
            "█   █",
        ],

        'O' => [
            " ███ ",
            "█   █",
            "█   █",
            "█   █",
            "█   █",
            "█   █",
            " ███ ",
        ],

        'P' => [
            "████ ",
            "█   █",
            "█   █",
            "████ ",
            "█    ",
            "█    ",
            "█    ",
        ],

        'Q' => [
            " ███ ",
            "█   █",
            "█   █",
            "█   █",
            "█ █ █",
            "█  ██",
            " ████",
        ],

        'R' => [
            "████ ",
            "█   █",
            "█   █",
            "████ ",
            "█ █  ",
            "█  █ ",
            "█   █",
        ],

        'S' => [
            " ████",
            "█    ",
            "█    ",
            " ███ ",
            "    █",
            "    █",
            "████ ",
        ],

        'T' => [
            "█████",
            "  █  ",
            "  █  ",
            "  █  ",
            "  █  ",
            "  █  ",
            "  █  ",
        ],

        'U' => [
            "█   █",
            "█   █",
            "█   █",
            "█   █",
            "█   █",
            "█   █",
            " ███ ",
        ],

        'V' => [
            "█   █",
            "█   █",
            "█   █",
            "█   █",
            "█   █",
            " █ █ ",
            "  █  ",
        ],

        'W' => [
            "█   █",
            "█   █",
            "█   █",
            "█ █ █",
            "█ █ █",
            "██ ██",
            "█   █",
        ],

        'X' => [
            "█   █",
            "█   █",
            " █ █ ",
            "  █  ",
            " █ █ ",
            "█   █",
            "█   █",
        ],

        'Y' => [
            "█   █",
            "█   █",
            " █ █ ",
            "  █  ",
            "  █  ",
            "  █  ",
            "  █  ",
        ],

        'Z' => [
            "█████",
            "    █",
            "   █ ",
            "  █  ",
            " █   ",
            "█    ",
            "█████",
        ],

        '0' => [
            " ███ ",
            "█   █",
            "█  ██",
            "█ █ █",
            "██  █",
            "█   █",
            " ███ ",
        ],

        '1' => [
            "  █  ",
            " ██  ",
            "█ █  ",
            "  █  ",
            "  █  ",
            "  █  ",
            "█████",
        ],

        '2' => [
            " ███ ",
            "█   █",
            "    █",
            "   █ ",
            "  █  ",
            " █   ",
            "█████",
        ],

        '3' => [
            "████ ",
            "    █",
            "    █",
            " ███ ",
            "    █",
            "    █",
            "████ ",
        ],

        '4' => [
            "   █ ",
            "  ██ ",
            " █ █ ",
            "█  █ ",
            "█████",
            "   █ ",
            "   █ ",
        ],

        '5' => [
            "█████",
            "█    ",
            "█    ",
            "████ ",
            "    █",
            "    █",
            "████ ",
        ],

        '6' => [
            " ███ ",
            "█    ",
            "█    ",
            "████ ",
            "█   █",
            "█   █",
            " ███ ",
        ],

        '7' => [
            "█████",
            "    █",
            "   █ ",
            "  █  ",
            " █   ",
            " █   ",
            " █   ",
        ],

        '8' => [
            " ███ ",
            "█   █",
            "█   █",
            " ███ ",
            "█   █",
            "█   █",
            " ███ ",
        ],

        '9' => [
            " ███ ",
            "█   █",
            "█   █",
            " ████",
            "    █",
            "    █",
            " ███ ",
        ],

        '!' => [
            "  █  ", "  █  ", "  █  ", "  █  ", "  █  ", "     ", "  █  ",
        ],

        '?' => [
            " ███ ",
            "█   █",
            "    █",
            "   █ ",
            "  █  ",
            "     ",
            "  █  ",
        ],

        '.' => [
            "     ", "     ", "     ", "     ", "     ", "  █  ", "  █  ",
        ],

        ',' => [
            "     ", "     ", "     ", "     ", "     ", "  █  ", " █   ",
        ],

        '-' => [
            "     ",
            "     ",
            "     ",
            "█████",
            "     ",
            "     ",
            "     ",
        ],

        '\'' => [
            "  █  ", "  █  ", "     ", "     ", "     ", "     ", "     ",
        ],

        '"' => [
            " █ █ ",
            " █ █ ",
            "     ",
            "     ",
            "     ",
            "     ",
            "     ",
        ],

        '♪' => [
            "    █",
            "    █",
            "    █",
            "    █",
            "  ███",
            " █ █ ",
            "█  █ ",
        ],

        ' ' => [
            "     ", "     ", "     ", "     ", "     ", "     ", "     ",
        ],

        _ => [
            "     ",
            "     ",
            "     ",
            " ███ ",
            "     ",
            "     ",
            "     ",
        ],
    }
}

fn terminal_size() -> (usize, usize) {
    match terminal::size() {
        Ok((width, height)) if width > 0 && height > 0 => (width as usize, height as usize),

        _ => (80, 24),
    }
}

fn display_width(text: &str) -> usize {
    text.chars().count()
}
