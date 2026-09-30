//! Diagnostic: prints raw key events (kind + code) to a log file so we can see
//! exactly what the terminal reports for Space press/release.
//! Run: cargo run -p tuibik-tui --bin keydiag  (press Space a few times, then q)

use std::io::{self, Write};
use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

fn main() -> io::Result<()> {
    let supports = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);
    enable_raw_mode()?;
    let mut out = io::stdout();
    let mut enhanced = false;
    if supports {
        enhanced = execute!(
            out,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )
        .is_ok();
    }

    let mut log = std::fs::File::create("/tmp/tuibik-keydiag.log")?;
    writeln!(log, "supports_keyboard_enhancement = {supports}")?;
    writeln!(log, "enhanced pushed = {enhanced}")?;

    let mut last_evt: Option<std::time::Instant> = None;
    loop {
        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(k) = event::read()? {
                let now = std::time::Instant::now();
                let delta = last_evt
                    .map(|p| now.duration_since(p).as_millis())
                    .unwrap_or(0);
                last_evt = Some(now);
                writeln!(log, "+{:>4}ms  code={:?} kind={:?}", delta, k.code, k.kind)?;
                log.flush()?;
                if k.code == KeyCode::Char('q') && k.kind == KeyEventKind::Press {
                    break;
                }
            }
        }
    }

    if enhanced {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    disable_raw_mode()?;
    println!("wrote /tmp/tuibik-keydiag.log");
    Ok(())
}
