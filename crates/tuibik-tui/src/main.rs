//! `tuibik` — csTimer-style TUI Rubik's cube timer.

mod app;
mod bigtext;
mod cube3d;
mod cube_widget;
mod event;
mod sessions;
mod settings;
mod theme;
mod timer;
mod ui;

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::event as cevent;
use crossterm::event::{
    Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::App;
use event::Input;

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Tick interval for live timer updates / redraws while something animates.
const TICK: Duration = Duration::from_millis(30);
/// Tick interval while a 3D axis turn animates (~60 fps).
const ANIM_TICK: Duration = Duration::from_millis(16);
/// Tick interval otherwise (toast expiry); input still wakes the loop at once.
const IDLE_TICK: Duration = Duration::from_millis(250);

const USAGE: &str = "Usage: tuibik [OPTIONS]\n\nA csTimer-style terminal Rubik's Cube timer.\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandLineAction {
    Run,
    Help,
    Version,
}

fn main() -> Result<()> {
    match parse_command_line(std::env::args_os().skip(1)) {
        Ok(CommandLineAction::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Ok(CommandLineAction::Version) => {
            println!("tuibik {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Ok(CommandLineAction::Run) => {}
        Err(argument) => {
            anyhow::bail!(
                "unsupported argument: {}\n\nTry 'tuibik --help' for more information.",
                argument.to_string_lossy()
            );
        }
    }

    let mut terminal = init_terminal()?;
    // Whether the terminal supports the Kitty keyboard protocol (key release
    // events), needed for the hold-to-arm timer.
    let enhanced = try_enable_keyboard_enhancement();

    let mut app = App::new(enhanced)?;
    let res = run(&mut terminal, &mut app);

    restore_terminal(enhanced);
    res
}

/// Recognize the small non-interactive interface before the TUI changes the
/// terminal state. Any unsupported or additional argument is rejected.
fn parse_command_line(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<CommandLineAction, std::ffi::OsString> {
    let mut arguments = arguments.into_iter();
    let Some(argument) = arguments.next() else {
        return Ok(CommandLineAction::Run);
    };

    if arguments.next().is_some() {
        return Err(argument);
    }

    match argument.to_str() {
        Some("-h" | "--help") => Ok(CommandLineAction::Help),
        Some("-V" | "--version") => Ok(CommandLineAction::Version),
        _ => Err(argument),
    }
}

/// Set up raw mode + alternate screen and a panic hook that restores the
/// terminal before the panic message is printed.
fn init_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Panic hook: restore terminal so a panic doesn't leave it garbled.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Attempt to push Kitty keyboard-enhancement flags so we receive key-release
/// events. Returns whether it was enabled.
fn try_enable_keyboard_enhancement() -> bool {
    match crossterm::terminal::supports_keyboard_enhancement() {
        Ok(true) => {
            let mut stdout = io::stdout();
            // Only REPORT_EVENT_TYPES is needed to receive key press/release
            // events for the hold-to-arm timer. REPORT_ALL_KEYS_AS_ESCAPE_CODES
            // is unnecessary here and can interfere with plain key handling.
            execute!(
                stdout,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
            )
            .is_ok()
        }
        _ => false,
    }
}

/// Undo terminal setup.
fn restore_terminal(enhanced: bool) {
    let mut stdout = io::stdout();
    if enhanced {
        let _ = execute!(stdout, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(stdout, LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

/// The main event loop.
fn run(terminal: &mut Tui, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::render(frame, app))?;

        // Poll for input with a timeout so we still tick for the live timer
        // and the moving 3D view; poll slowly when nothing moves.
        let timeout = if app.is_animating() {
            ANIM_TICK
        } else if app.needs_fast_ticks() {
            TICK
        } else {
            IDLE_TICK
        };
        if cevent::poll(timeout)? {
            // Apply everything already queued before redrawing once, so held
            // keys (auto-repeat) never build a backlog behind slow frames.
            loop {
                match cevent::read()? {
                    Event::Key(key) => {
                        let input = event::classify_key(key, app.key_context());
                        if input != Input::None {
                            app.handle_input(input);
                        }
                    }
                    // The next loop iteration redraws at the new size.
                    Event::Resize(_, _) => {}
                    _ => {}
                }
                if app.should_quit || !cevent::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        // Always advance time-based transitions.
        app.tick();
    }
    Ok(())
}
