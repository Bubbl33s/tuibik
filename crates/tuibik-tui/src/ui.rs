//! Layout computation and rendering.
//!
//! [`compute_layout`] is a pure function of the screen area and a little view
//! state, so the responsive breakpoints are unit-testable; the `render_*`
//! functions then draw into the rectangles it returns.

use cube::render::{NET_COLS, NET_ROWS};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Block, BorderType, Borders, Chart, Clear, Dataset, GraphType, List, ListItem, Paragraph,
    Wrap,
};
use ratatui::Frame;
use stats::{StatValue, Summary};
use store::{Penalty, Solve};

use crate::app::{App, Overlay, Screen, ToastKind};
use crate::bigtext::render_big;
use crate::sessions::SessionMode;
use crate::settings::{Row, ROWS};
use crate::theme::Theme;
use crate::timer::{format_ms, Phase};

/// Minimum terminal size for the compact layout.
pub const MIN_W: u16 = 40;
pub const MIN_H: u16 = 12;
/// Width of the sidebar (the stats table needs 28 inner columns).
pub const SIDEBAR_W: u16 = 30;
/// Height of the stats panel: table header + 5 rows + mean line + borders.
const STATS_H: u16 = 9;
/// Rows the timer must keep when the tools row is squeezed.
const MIN_TIMER_H: u16 = 5;

/// The screen regions for the current terminal size and state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutPlan {
    /// Below the compact minimum: only a message is shown.
    TooSmall,
    /// Only the big timer, on the whole screen.
    Focus { timer: Rect },
    /// Small terminals: no sidebar or tools row.
    Compact {
        header: Rect,
        scramble: Rect,
        timer: Rect,
        summary: Rect,
        footer: Rect,
    },
    /// Medium and wide terminals.
    Dashboard {
        header: Rect,
        sidebar_stats: Rect,
        sidebar_history: Rect,
        scramble: Rect,
        timer: Rect,
        tools_cube: Rect,
        /// Only on wide terminals.
        tools_chart: Option<Rect>,
        footer: Rect,
    },
}

/// The view state the layout depends on.
#[derive(Debug, Clone, Copy)]
pub struct LayoutCtx<'a> {
    pub scramble: &'a str,
    /// The timer is engaged (arming, ready, inspecting or running).
    pub focus: bool,
}

/// Greedy word-wrap of whitespace-separated `tokens` to `width` columns,
/// returning the token indices on each line. A token wider than the width sits
/// alone on its line. Always returns at least one (possibly empty) line.
pub fn wrap_indices(tokens: &[&str], width: usize) -> Vec<Vec<usize>> {
    let width = width.max(1);
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut used = 0usize;
    for (i, tok) in tokens.iter().enumerate() {
        let w = tok.chars().count();
        let cur = lines.last_mut().expect("at least one line");
        if cur.is_empty() {
            cur.push(i);
            used = w;
        } else if used + 1 + w <= width {
            cur.push(i);
            used += 1 + w;
        } else {
            lines.push(vec![i]);
            used = w;
        }
    }
    lines
}

/// Word-wrap `text` to `width` columns.
pub fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    wrap_indices(&tokens, width)
        .into_iter()
        .map(|idxs| {
            idxs.iter()
                .map(|&i| tokens[i])
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// Compute the screen regions for `area`.
pub fn compute_layout(area: Rect, ctx: &LayoutCtx) -> LayoutPlan {
    if area.width == 0 || area.height == 0 {
        return LayoutPlan::TooSmall;
    }
    // While solving, the timer must stay visible whatever the size.
    if ctx.focus {
        return LayoutPlan::Focus { timer: area };
    }
    if area.width < MIN_W || area.height < MIN_H {
        return LayoutPlan::TooSmall;
    }
    let wide = area.width >= 110 && area.height >= 32;
    let medium = area.width >= 80 && area.height >= 24;
    if wide || medium {
        dashboard_layout(area, ctx, wide)
    } else {
        compact_layout(area, ctx)
    }
}

fn compact_layout(area: Rect, ctx: &LayoutCtx) -> LayoutPlan {
    let lines = wrap_words(ctx.scramble, area.width as usize).len() as u16;
    let max_scramble = area.height.saturating_sub(3 + 4).max(1);
    let scramble_h = lines.clamp(1, max_scramble);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(scramble_h),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    LayoutPlan::Compact {
        header: rows[0],
        scramble: rows[1],
        timer: rows[2],
        summary: rows[3],
        footer: rows[4],
    }
}

fn dashboard_layout(area: Rect, ctx: &LayoutCtx, wide: bool) -> LayoutPlan {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    let (header, body, footer) = (rows[0], rows[1], rows[2]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(SIDEBAR_W), Constraint::Min(1)])
        .split(body);
    let (sidebar, main) = (cols[0], cols[1]);

    let side = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(STATS_H), Constraint::Min(0)])
        .split(sidebar);

    // Scramble: wrapped line count at the inner width, plus the border.
    let inner_w = main.width.saturating_sub(2) as usize;
    let scramble_lines = wrap_words(ctx.scramble, inner_w).len().max(1) as u16;
    let scramble_h = (scramble_lines + 2).min(main.height / 2).max(3);

    // Tools row: the net at scale 2 (20 rows) when it fits within 40% of the
    // column, otherwise at scale 1 (11 rows); never squeeze the timer.
    let net_w_needed = if wide { 50 + 24 } else { 50 };
    let big = main.height * 4 / 10 >= (NET_ROWS as u16) * 2 + 2 && main.width >= net_w_needed;
    let desired = if big {
        NET_ROWS as u16 * 2 + 2
    } else {
        NET_ROWS as u16 + 2
    };
    let tools_h = desired.min(main.height.saturating_sub(scramble_h + MIN_TIMER_H));

    let mr = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(scramble_h),
            Constraint::Min(1),
            Constraint::Length(tools_h),
        ])
        .split(main);

    let (tools_cube, tools_chart) = if wide {
        let cube_w = if big { 50 } else { NET_COLS as u16 * 2 + 2 };
        let t = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(cube_w), Constraint::Min(1)])
            .split(mr[2]);
        (t[0], Some(t[1]))
    } else {
        (mr[2], None)
    };

    LayoutPlan::Dashboard {
        header,
        sidebar_stats: side[0],
        sidebar_history: side[1],
        scramble: mr[0],
        timer: mr[1],
        tools_cube,
        tools_chart,
        footer,
    }
}

// ===== Formatting helpers =====

fn fmt_stat(v: StatValue) -> String {
    match v {
        StatValue::None => "—".to_string(),
        StatValue::Dnf => "DNF".to_string(),
        StatValue::Time(ms) => format_ms(ms.round().max(0.0) as u64),
    }
}

/// Signed difference, e.g. `+0.52` / `-1.03`.
fn fmt_delta(ms: i64) -> String {
    let sign = if ms < 0 { "-" } else { "+" };
    format!("{sign}{}", format_ms(ms.unsigned_abs()))
}

/// A solve's time as shown in lists: `14.34+` for +2, `DNF` for DNF.
fn solve_time_text(s: &Solve) -> String {
    match stats::effective_ms(s) {
        None => "DNF".to_string(),
        Some(ms) => {
            let t = format_ms(ms.max(0) as u64);
            if s.penalty == Penalty::PlusTwo {
                format!("{t}+")
            } else {
                t
            }
        }
    }
}

/// `YYYY-MM-DD HH:MM UTC` for a unix timestamp (no time-zone database needed).
pub fn format_epoch_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m) = (rem / 3600, rem % 3600 / 60);
    // Days since 1970-01-01 -> civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mth <= 2);
    format!("{y:04}-{mth:02}-{d:02} {h:02}:{m:02} UTC")
}

fn panel<'a>(title: &str, theme: &Theme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(theme.border))
}

fn popup_block<'a>(title: &str, theme: &Theme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(theme.accent))
}

fn centered_fixed(w: u16, h: u16, area: Rect) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

// ===== Top level =====

/// Render the whole UI.
pub fn render(frame: &mut Frame, app: &App) {
    let theme = app.theme();
    let area = frame.area();
    let ctx = LayoutCtx {
        scramble: &app.scramble.text,
        focus: app.screen() == Screen::Focus,
    };
    let plan = compute_layout(area, &ctx);
    let too_small = plan == LayoutPlan::TooSmall;
    match plan {
        LayoutPlan::TooSmall => render_too_small(frame, theme, area),
        LayoutPlan::Focus { timer } => render_timer(frame, app, theme, timer, true),
        LayoutPlan::Compact {
            header,
            scramble,
            timer,
            summary,
            footer,
        } => {
            render_header(frame, app, theme, header);
            render_scramble(frame, app, theme, scramble, false);
            render_timer(frame, app, theme, timer, false);
            render_summary_line(frame, app, theme, summary);
            render_footer(frame, app, theme, footer);
        }
        LayoutPlan::Dashboard {
            header,
            sidebar_stats,
            sidebar_history,
            scramble,
            timer,
            tools_cube,
            tools_chart,
            footer,
        } => {
            render_header(frame, app, theme, header);
            render_stats(frame, app, theme, sidebar_stats);
            render_history(frame, app, theme, sidebar_history);
            render_scramble(frame, app, theme, scramble, true);
            render_timer(frame, app, theme, timer, false);
            render_cube(frame, app, theme, tools_cube);
            if let Some(chart) = tools_chart {
                render_chart(frame, app, theme, chart);
            }
            render_footer(frame, app, theme, footer);
        }
    }
    if !too_small && app.screen() == Screen::Dashboard {
        render_overlay(frame, app, theme);
    }
}

fn render_too_small(frame: &mut Frame, theme: &Theme, area: Rect) {
    let msg = format!(
        "Terminal too small: {}×{} (need {}×{})",
        area.width, area.height, MIN_W, MIN_H
    );
    let lines = (msg.chars().count() as u16)
        .div_ceil(area.width.max(1))
        .max(1);
    let r = centered_fixed(area.width, lines.min(area.height), area);
    frame.render_widget(
        Paragraph::new(msg)
            .style(Style::default().fg(theme.text))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        r,
    );
}

// ===== Header / footer / summary =====

fn render_header(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let n = app.solves.len();
    let count = if n == 1 {
        "1 solve".to_string()
    } else {
        format!("{n} solves")
    };
    let sep = Span::styled("  ·  ", Style::default().fg(theme.muted));
    let line = Line::from(vec![
        Span::styled(
            " tuibik",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        sep.clone(),
        Span::styled(
            app.session_name.clone(),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        sep.clone(),
        Span::styled(count, Style::default().fg(theme.text)),
        sep,
        Span::styled(theme.name, Style::default().fg(theme.muted)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_summary_line(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let s = app.summary();
    let text = format!(
        "{} · ao5 {} · ao12 {}",
        if s.count == 1 {
            "1 solve".to_string()
        } else {
            format!("{} solves", s.count)
        },
        fmt_stat(s.ao5),
        fmt_stat(s.ao12)
    );
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(theme.muted))
            .alignment(Alignment::Center),
        area,
    );
}

/// The key hints valid in the current context, most important first.
pub fn footer_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    match app.overlay {
        Overlay::None => vec![
            ("Space", "start"),
            ("?", "help"),
            ("q", "quit"),
            ("n", "scramble"),
            ("↑↓", "history"),
            ("Enter", "detail"),
            ("1/2/3", "penalty"),
            ("d", "delete"),
            ("p", "preview"),
            ("s", "sessions"),
            ("o", "settings"),
            ("t", "theme"),
        ],
        Overlay::Preview => vec![("←/→", "step"), ("Esc", "close")],
        Overlay::Sessions => match app.session_menu.as_ref().map(|m| &m.mode) {
            Some(SessionMode::Creating { .. }) | Some(SessionMode::Renaming { .. }) => {
                vec![("Enter", "confirm"), ("Esc", "cancel")]
            }
            Some(SessionMode::ConfirmDelete) => vec![("d", "confirm"), ("Esc", "cancel")],
            _ => vec![
                ("↑↓", "select"),
                ("Enter", "switch"),
                ("n", "new"),
                ("r", "rename"),
                ("d", "delete"),
                ("Esc", "close"),
            ],
        },
        Overlay::Settings => vec![("↑↓", "select"), ("←/→/Enter", "change"), ("Esc", "close")],
        Overlay::Help => vec![("Esc", "close")],
        Overlay::Detail => vec![
            ("1/2/3", "penalty"),
            ("d", "delete"),
            ("↑↓", "prev/next"),
            ("Esc", "close"),
        ],
        Overlay::ConfirmDelete => vec![("y", "confirm"), ("n", "cancel")],
    }
}

fn render_footer(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    if let Some(toast) = &app.toast {
        let color = match toast.kind {
            ToastKind::Info => theme.text,
            ToastKind::Success => theme.timer_ready,
            ToastKind::Warn => theme.timer_arming,
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" {}", toast.text),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            area,
        );
        return;
    }
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    let mut used = 1usize;
    for (i, (key, desc)) in footer_hints(app).into_iter().enumerate() {
        let sep = if i == 0 { 0 } else { 2 };
        let w = sep + key.chars().count() + 1 + desc.chars().count();
        // Truncate from the least important end rather than wrapping.
        if used + w > area.width as usize {
            break;
        }
        if sep > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(key, Style::default().fg(theme.accent)));
        spans.push(Span::styled(
            format!(" {desc}"),
            Style::default().fg(theme.muted),
        ));
        used += w;
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ===== Scramble =====

fn render_scramble(frame: &mut Frame, app: &App, theme: &Theme, area: Rect, bordered: bool) {
    let inner_w = if bordered {
        area.width.saturating_sub(2)
    } else {
        area.width
    } as usize;
    let tokens: Vec<&str> = app.scramble.text.split_whitespace().collect();
    let previewing = app.overlay == Overlay::Preview;
    let lines: Vec<Line> = wrap_indices(&tokens, inner_w)
        .into_iter()
        .map(|idxs| {
            let mut spans: Vec<Span> = Vec::new();
            for (n, i) in idxs.into_iter().enumerate() {
                if n > 0 {
                    spans.push(Span::raw(" "));
                }
                let style = if previewing && i + 1 == app.preview_step {
                    Style::default()
                        .fg(theme.select_fg)
                        .bg(theme.select_bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                };
                spans.push(Span::styled(tokens[i].to_string(), style));
            }
            Line::from(spans)
        })
        .collect();
    let mut p = Paragraph::new(lines).alignment(Alignment::Center);
    if bordered {
        p = p.block(panel("Scramble", theme));
    }
    frame.render_widget(p, area);
}

// ===== Timer =====

struct TimerView {
    text: String,
    label: String,
    color: Color,
}

fn inspection_text(app: &App) -> String {
    match app.timer.inspection_seconds_left() {
        Some(n) => n.to_string(),
        None => "+2".to_string(),
    }
}

fn timer_view(app: &App, theme: &Theme) -> TimerView {
    let t = &app.timer;
    let nested = t.in_inspection();
    match t.phase {
        Phase::Idle | Phase::Stopped => {
            let (text, color) = match &app.last_result {
                Some(r) => {
                    let text = match r.ms {
                        None => "DNF".to_string(),
                        Some(ms) if r.penalty == Penalty::PlusTwo => {
                            format!("{}+", format_ms(ms.max(0) as u64))
                        }
                        Some(ms) => format_ms(ms.max(0) as u64),
                    };
                    (text, theme.timer_stopped)
                }
                None => (format_ms(0), theme.timer_idle),
            };
            let hint = if app.enhanced {
                "hold Space to start"
            } else {
                "press Space to start"
            };
            TimerView {
                text,
                label: hint.to_string(),
                color,
            }
        }
        Phase::Arming => TimerView {
            text: if nested {
                inspection_text(app)
            } else {
                format_ms(0)
            },
            label: "HOLD".to_string(),
            color: theme.timer_arming,
        },
        Phase::Ready => TimerView {
            text: if nested {
                inspection_text(app)
            } else {
                format_ms(0)
            },
            label: "READY".to_string(),
            color: theme.timer_ready,
        },
        Phase::Inspecting => {
            let mut label = "INSPECT".to_string();
            let color = if t.inspection_overtime() {
                label.push_str("  +2");
                theme.timer_arming
            } else if t.cue_12s() {
                label.push_str("  12s!");
                theme.timer_arming
            } else if t.cue_8s() {
                label.push_str("  8s!");
                theme.timer_running
            } else {
                theme.timer_idle
            };
            TimerView {
                text: inspection_text(app),
                label,
                color,
            }
        }
        Phase::Running => TimerView {
            text: if app.settings.show_running_time {
                format_ms(t.display_ms())
            } else {
                "SOLVING".to_string()
            },
            label: "SOLVING".to_string(),
            color: theme.timer_running,
        },
    }
}

/// Post-solve feedback lines shown under the timer on the dashboard.
fn feedback_lines(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    let Some(r) = &app.last_result else {
        return Vec::new();
    };
    let s = app.summary();
    let mut parts: Vec<Span> = Vec::new();
    let dot = || Span::styled("  ·  ", Style::default().fg(theme.muted));
    if let Some(d) = r.delta_prev {
        let color = if d < 0 {
            theme.timer_ready
        } else {
            theme.timer_arming
        };
        parts.push(Span::styled(
            fmt_delta(d),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }
    for (name, v) in [("ao5", s.ao5), ("ao12", s.ao12)] {
        if v == StatValue::None {
            continue;
        }
        if !parts.is_empty() {
            parts.push(dot());
        }
        parts.push(Span::styled(
            format!("{name} {}", fmt_stat(v)),
            Style::default().fg(theme.text),
        ));
    }
    let mut lines = Vec::new();
    if !parts.is_empty() {
        lines.push(Line::from(parts));
    }
    if !r.pbs.is_empty() {
        let text = r
            .pbs
            .iter()
            .map(|p| format!("★ New PB {}!", p.label()))
            .collect::<Vec<_>>()
            .join("  ");
        lines.push(Line::from(Span::styled(
            text,
            Style::default()
                .fg(theme.timer_ready)
                .add_modifier(Modifier::BOLD),
        )));
    }
    lines
}

fn render_timer(frame: &mut Frame, app: &App, theme: &Theme, area: Rect, focus: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let view = timer_view(app, theme);
    let feedback = if focus {
        Vec::new()
    } else {
        feedback_lines(app, theme)
    };
    // Reserve a gap + the state label + the feedback rows below the digits.
    let reserved = 2 + feedback.len() as u16;
    let big_area = Rect {
        height: area.height.saturating_sub(reserved).max(1),
        ..area
    };
    let big = render_big(&view.text, big_area, Style::default().fg(view.color));
    let total = big.height + reserved;
    let mut y = area.y + area.height.saturating_sub(total) / 2;
    let bottom = area.bottom();

    let draw = |frame: &mut Frame, line: Line<'static>, y: u16| {
        if y < bottom {
            frame.render_widget(
                Paragraph::new(line).alignment(Alignment::Center),
                Rect::new(area.x, y, area.width, 1),
            );
        }
    };
    for line in big.lines {
        draw(frame, line, y);
        y += 1;
    }
    y += 1; // gap
            // State labels take the phase color; the idle hint is muted.
    let label_color = match view.label.as_str() {
        "HOLD" | "READY" | "SOLVING" => view.color,
        l if l.starts_with("INSPECT") => view.color,
        _ => theme.muted,
    };
    draw(
        frame,
        Line::from(Span::styled(
            view.label,
            Style::default()
                .fg(label_color)
                .add_modifier(Modifier::BOLD),
        )),
        y,
    );
    y += 1;
    for line in feedback {
        draw(frame, line, y);
        y += 1;
    }
}

// ===== Stats =====

/// The rows of the current/best table plus the mean line.
pub fn stats_lines(s: &Summary, theme: &Theme) -> Vec<Line<'static>> {
    let muted = Style::default().fg(theme.muted);
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{:<7}", ""), muted),
        Span::styled(format!("{:>10}", "current"), muted),
        Span::styled(format!("{:>10}", "best"), muted),
    ])];
    let best_single = s
        .best
        .map_or(StatValue::None, |b| StatValue::Time(b as f64));
    let rows: [(&str, StatValue, StatValue); 5] = [
        ("single", s.current_single, best_single),
        ("mo3", s.mo3, s.best_mo3),
        ("ao5", s.ao5, s.best_ao5),
        ("ao12", s.ao12, s.best_ao12),
        ("ao100", s.ao100, s.best_ao100),
    ];
    for (label, current, best) in rows {
        // A current value equal to the best is a record: highlight it.
        let is_record =
            matches!((current, best), (StatValue::Time(c), StatValue::Time(b)) if c == b);
        let cur_style = if is_record {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{label:<7}"), muted),
            Span::styled(format!("{:>10}", fmt_stat(current)), cur_style),
            Span::styled(
                format!("{:>10}", fmt_stat(best)),
                Style::default().fg(theme.text),
            ),
        ]));
    }
    let mut mean = format!(
        "mean {} ({}/{})",
        s.mean
            .map_or_else(|| "—".to_string(), |m| fmt_stat(StatValue::Time(m))),
        s.mean_counted,
        s.count
    );
    if let Some(sd) = s.stddev {
        mean.push_str(&format!(" σ {}", fmt_stat(StatValue::Time(sd))));
    }
    lines.push(Line::from(Span::styled(mean, muted)));
    lines
}

fn render_stats(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    frame.render_widget(
        Paragraph::new(stats_lines(app.summary(), theme)).block(panel("Stats", theme)),
        area,
    );
}

// ===== History =====

fn render_history(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let block = panel(&format!("History ({})", app.solves.len()), theme);
    if app.solves.is_empty() {
        let hint = if app.enhanced {
            "No solves yet — hold Space to start"
        } else {
            "No solves yet — press Space to start"
        };
        frame.render_widget(
            Paragraph::new(hint)
                .style(Style::default().fg(theme.muted))
                .wrap(Wrap { trim: true })
                .block(block),
            area,
        );
        return;
    }
    let best = app.summary().best;
    let total = app.solves.len();
    let items: Vec<ListItem> = app
        .solves
        .iter()
        .rev()
        .enumerate()
        .map(|(i, solve)| {
            let number = total - i;
            let dnf = solve.penalty == Penalty::Dnf;
            let is_best = !dnf && best.is_some() && stats::effective_ms(solve) == best;
            let time_style = if dnf {
                Style::default().fg(theme.muted)
            } else {
                Style::default().fg(theme.text)
            };
            let marker = if is_best {
                Span::styled(" ★", Style::default().fg(theme.accent))
            } else {
                Span::raw("")
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{number:>4}  "), Style::default().fg(theme.muted)),
                Span::styled(format!("{:>9}", solve_time_text(solve)), time_style),
                marker,
            ]))
        })
        .collect();
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .fg(theme.select_fg)
            .bg(theme.select_bg)
            .add_modifier(Modifier::BOLD),
    );
    let mut state = app.history_state.borrow_mut();
    state.select(Some(app.history_selected.min(total - 1)));
    frame.render_stateful_widget(list, area, &mut *state);
}

// ===== Cube =====

fn render_cube(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let (cube, highlight, title) = if app.overlay == Overlay::Preview {
        (
            app.preview_cube(),
            app.preview_highlight(),
            format!("Cube — step {}/{}", app.preview_step, app.scramble_len()),
        )
    } else {
        (app.cube.clone(), Vec::new(), "Cube".to_string())
    };
    let block = panel(&title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let scale = crate::cube_widget::net_scale(area);
    let lines = crate::cube_widget::net_lines_scaled(&cube, &theme.stickers, &highlight, scale);
    let h = (lines.len() as u16).min(inner.height);
    let target = Rect::new(inner.x, inner.y + (inner.height - h) / 2, inner.width, h);
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), target);
}

// ===== Chart =====

/// Y-axis bounds (seconds) for the plotted values: min/max with 5% padding
/// (at least 0.5 s) rather than starting at zero.
pub fn chart_bounds(values: &[f64]) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for &v in values {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if lo > hi {
        return (0.0, 1.0);
    }
    let pad = ((hi - lo) * 0.05).max(0.5);
    ((lo - pad).max(0.0), hi + pad)
}

struct ChartData {
    times: Vec<(f64, f64)>,
    ao5: Vec<(f64, f64)>,
    ao12: Vec<(f64, f64)>,
    first_no: usize,
    last_no: usize,
}

/// The most recent solves that fit (2 points per cell with Braille), with the
/// effective times and per-solve rolling ao5/ao12; DNFs are omitted.
fn chart_data(solves: &[Solve], inner_w: u16) -> ChartData {
    let cap = (inner_w as usize * 2).max(2);
    let total = solves.len();
    let start = total.saturating_sub(cap);
    // Rolling averages need up to 11 solves before the window's first point.
    let lead = start.min(11);
    let slice = &solves[start - lead..];
    let r5 = stats::rolling_aon(slice, 5);
    let r12 = stats::rolling_aon(slice, 12);
    let (mut times, mut ao5, mut ao12) = (Vec::new(), Vec::new(), Vec::new());
    for (i, solve) in solves[start..].iter().enumerate() {
        let x = i as f64;
        if let Some(ms) = stats::effective_ms(solve) {
            times.push((x, ms as f64 / 1000.0));
        }
        if let StatValue::Time(v) = r5[lead + i] {
            ao5.push((x, v / 1000.0));
        }
        if let StatValue::Time(v) = r12[lead + i] {
            ao12.push((x, v / 1000.0));
        }
    }
    ChartData {
        times,
        ao5,
        ao12,
        first_no: start + 1,
        last_no: total,
    }
}

fn render_chart(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let s = app.summary();
    let title = match (s.best, s.mean) {
        (Some(b), Some(m)) => format!(
            "Progress  best {}  mean {}",
            format_ms(b.max(0) as u64),
            format_ms(m.round().max(0.0) as u64)
        ),
        _ => "Progress".to_string(),
    };
    let block = panel(&title, theme);
    let inner = block.inner(area);
    if app.solves.is_empty() || app.solves.iter().all(|x| x.penalty == Penalty::Dnf) {
        let hint = if app.solves.is_empty() {
            "No solves yet"
        } else {
            "No finished solves yet"
        };
        frame.render_widget(
            Paragraph::new(hint)
                .style(Style::default().fg(theme.muted))
                .block(block),
            area,
        );
        return;
    }
    let data = chart_data(&app.solves, inner.width);
    let all: Vec<f64> = data
        .times
        .iter()
        .chain(&data.ao5)
        .chain(&data.ao12)
        .map(|p| p.1)
        .collect();
    let (lo, hi) = chart_bounds(&all);
    let n = (data.last_no - data.first_no + 1).max(2);
    let x_max = (n - 1) as f64;
    let label = |secs: f64| {
        Span::styled(
            format_ms((secs * 1000.0).round().max(0.0) as u64),
            Style::default().fg(theme.muted),
        )
    };
    let datasets = vec![
        Dataset::default()
            .name("time")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.graph))
            .data(&data.times),
        Dataset::default()
            .name("ao5")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.accent))
            .data(&data.ao5),
        Dataset::default()
            .name("ao12")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.title))
            .data(&data.ao12),
    ];
    let mut chart = Chart::new(datasets)
        .block(block)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(theme.border))
                .bounds([0.0, x_max])
                .labels(vec![
                    Span::styled(
                        format!("#{}", data.first_no),
                        Style::default().fg(theme.muted),
                    ),
                    Span::styled(
                        format!("#{}", data.last_no),
                        Style::default().fg(theme.muted),
                    ),
                ]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(theme.border))
                .bounds([lo, hi])
                .labels(vec![label(lo), label((lo + hi) / 2.0), label(hi)]),
        );
    if area.height < 8 {
        // Too short for a legend.
        chart = chart.hidden_legend_constraints((Constraint::Length(0), Constraint::Length(0)));
    }
    frame.render_widget(chart, area);
}

// ===== Overlays =====

fn render_overlay(frame: &mut Frame, app: &App, theme: &Theme) {
    let area = frame.area();
    match app.overlay {
        Overlay::None | Overlay::Preview => {}
        Overlay::Sessions => render_session_overlay(frame, app, theme, area),
        Overlay::Help => render_help(frame, app, theme, area),
        Overlay::Settings => render_settings(frame, app, theme, area),
        Overlay::Detail => render_detail(frame, app, theme, area),
        Overlay::ConfirmDelete => {
            if app.confirm_return() == Overlay::Detail {
                render_detail(frame, app, theme, area);
            }
            render_confirm_delete(frame, app, theme, area);
        }
    }
}

fn kv_line(key: &str, value: Vec<Span<'static>>, theme: &Theme) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{key:<10}"),
        Style::default().fg(theme.muted),
    )];
    spans.extend(value);
    Line::from(spans)
}

fn render_detail(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let Some(solve) = app.selected_solve() else {
        return;
    };
    let number = app.selected_number();
    let idx = number - 1;
    let text = Style::default().fg(theme.text);
    let raw = format_ms(solve.time_ms.max(0) as u64);
    let (time, penalty) = match solve.penalty {
        Penalty::Ok => (raw, "OK".to_string()),
        Penalty::PlusTwo => (solve_time_text(solve), format!("+2  (raw {raw})")),
        Penalty::Dnf => ("DNF".to_string(), format!("DNF  (raw {raw})")),
    };
    let upto = &app.solves[..=idx];
    let lines = vec![
        kv_line(
            "Time",
            vec![Span::styled(
                time,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )],
            theme,
        ),
        kv_line("Penalty", vec![Span::styled(penalty, text)], theme),
        kv_line(
            "Date",
            vec![Span::styled(format_epoch_utc(solve.created_at), text)],
            theme,
        ),
        kv_line(
            "ao5",
            vec![Span::styled(fmt_stat(stats::current_aon(upto, 5)), text)],
            theme,
        ),
        kv_line(
            "ao12",
            vec![Span::styled(fmt_stat(stats::current_aon(upto, 12)), text)],
            theme,
        ),
        Line::from(""),
        Line::from(Span::styled("Scramble", Style::default().fg(theme.muted))),
        Line::from(Span::styled(
            solve.scramble.clone(),
            Style::default().fg(theme.accent),
        )),
    ];
    let r = centered_fixed(60, 15, area);
    frame.render_widget(Clear, r);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(popup_block(&format!("Solve #{number}"), theme)),
        r,
    );
}

fn render_confirm_delete(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let Some(solve) = app.selected_solve() else {
        return;
    };
    let number = app.selected_number();
    let r = centered_fixed(44, 6, area);
    frame.render_widget(Clear, r);
    let lines = vec![
        Line::from(Span::styled(
            format!("Delete solve #{number} ({})?", solve_time_text(solve)),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", Style::default().fg(theme.accent)),
            Span::styled(" delete   ", Style::default().fg(theme.muted)),
            Span::styled("[n]", Style::default().fg(theme.accent)),
            Span::styled(" cancel", Style::default().fg(theme.muted)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(popup_block("Delete solve", theme)),
        r,
    );
}

fn render_help(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let key = |k: &str| Span::styled(format!("  {k:<12}"), Style::default().fg(theme.accent));
    let desc = |d: &str| Span::styled(d.to_string(), Style::default().fg(theme.text));
    let head = |h: &str| {
        Line::from(Span::styled(
            h.to_string(),
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ))
    };
    let mode = if app.enhanced {
        "Timing: hold-to-start (hold Space until READY, release to start)"
    } else {
        "Timing: tap-to-start (this terminal has no key-release events)"
    };
    let row = |k: &str, d: &str| Line::from(vec![key(k), desc(d)]);
    let lines = vec![
        Line::from(Span::styled(
            mode,
            Style::default()
                .fg(theme.timer_ready)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        head("While timing"),
        row(
            "Space",
            if app.enhanced {
                "hold to arm, release to start"
            } else {
                "press to start"
            },
        ),
        row("any key", "stop the solve"),
        row("Esc", "cancel arming / inspection"),
        Line::from(""),
        head("Dashboard"),
        row("n", "new scramble"),
        row("↑↓  k j", "select solve"),
        row("Home End", "newest / oldest  (g G)"),
        row("Enter", "solve details"),
        row("1  2  3", "OK / +2 / DNF for selected solve"),
        row("d", "delete selected solve (asks first)"),
        row("p", "scramble preview (←/→ step)"),
        row("s", "sessions"),
        row("o", "settings"),
        row("t", "next theme"),
        row("?", "this help"),
        row("q", "quit"),
        Line::from(""),
        head("Overlays"),
        row("Esc", "close  (never quits)"),
        row("y / n", "confirm / cancel a delete"),
        row("Ctrl+C", "quit from anywhere"),
    ];
    let h = lines.len() as u16 + 2;
    let r = centered_fixed(72, h, area);
    frame.render_widget(Clear, r);
    frame.render_widget(Paragraph::new(lines).block(popup_block("Help", theme)), r);
}

fn render_settings(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let on_off = |b: bool| if b { "on" } else { "off" };
    let lines: Vec<Line> = ROWS
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let selected = i == app.settings_selected;
            let (name, value) = match row {
                Row::Inspection => (
                    "WCA inspection",
                    on_off(app.settings.inspection).to_string(),
                ),
                Row::ShowRunningTime => (
                    "Show running time",
                    on_off(app.settings.show_running_time).to_string(),
                ),
                Row::Theme => ("Theme", format!("◂ {} ▸", app.theme().name)),
            };
            let style = if selected {
                Style::default()
                    .fg(theme.select_fg)
                    .bg(theme.select_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            Line::from(Span::styled(format!(" {name:<20}{value:<14}"), style))
        })
        .collect();
    let r = centered_fixed(44, ROWS.len() as u16 + 2, area);
    frame.render_widget(Clear, r);
    frame.render_widget(
        Paragraph::new(lines).block(popup_block("Settings", theme)),
        r,
    );
}

fn render_session_overlay(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let Some(menu) = app.session_menu.as_ref() else {
        return;
    };
    let hint = |pairs: &[(&str, &str)]| {
        let mut spans = Vec::new();
        for (i, (k, d)) in pairs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(
                k.to_string(),
                Style::default().fg(theme.accent),
            ));
            spans.push(Span::styled(
                format!(" {d}"),
                Style::default().fg(theme.muted),
            ));
        }
        Line::from(spans)
    };
    let rows = (menu.sessions.len() as u16)
        .min(area.height.saturating_sub(8))
        .max(1);
    let r = centered_fixed(56, rows + 5, area);
    frame.render_widget(Clear, r);

    match &menu.mode {
        SessionMode::Creating { input } | SessionMode::Renaming { input } => {
            let title = if matches!(menu.mode, SessionMode::Creating { .. }) {
                "New session"
            } else {
                "Rename session"
            };
            let lines = vec![
                Line::from(Span::styled(
                    format!("> {input}_"),
                    Style::default().fg(theme.text),
                )),
                Line::from(""),
                hint(&[("Enter", "confirm"), ("Esc", "cancel")]),
            ];
            let r = centered_fixed(56, 5, area);
            frame.render_widget(Clear, r);
            frame.render_widget(Paragraph::new(lines).block(popup_block(title, theme)), r);
        }
        SessionMode::ConfirmDelete => {
            let name = menu
                .selected_session()
                .map(|s| s.name.as_str())
                .unwrap_or("");
            let lines = vec![
                Line::from(Span::styled(
                    format!("Delete '{name}' and all its solves?"),
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                hint(&[("d", "confirm"), ("Esc", "cancel")]),
            ];
            let r = centered_fixed(56, 5, area);
            frame.render_widget(Clear, r);
            frame.render_widget(
                Paragraph::new(lines).block(popup_block("Delete session", theme)),
                r,
            );
        }
        SessionMode::Browse => {
            let mut lines: Vec<Line> = menu
                .sessions
                .iter()
                .enumerate()
                .take(rows as usize)
                .map(|(i, s)| {
                    let selected = i == menu.selected;
                    let active = s.id == app.session_id;
                    let marker = if active { "● " } else { "  " };
                    let style = if selected {
                        Style::default()
                            .fg(theme.select_fg)
                            .bg(theme.select_bg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    };
                    Line::from(vec![
                        Span::styled(marker, Style::default().fg(theme.timer_ready)),
                        Span::styled(s.name.clone(), style),
                    ])
                })
                .collect();
            while lines.len() < rows as usize {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(""));
            lines.push(hint(&[
                ("↑↓", "select"),
                ("Enter", "switch"),
                ("n", "new"),
                ("r", "rename"),
                ("d", "delete"),
                ("Esc", "close"),
            ]));
            frame.render_widget(
                Paragraph::new(lines).block(popup_block("Sessions", theme)),
                r,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Overlay;
    use crate::event::Input;
    use crate::timer::{ARM_THRESHOLD_MS, RELEASE_DEBOUNCE_MS};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn ctx(scramble: &str, focus: bool) -> LayoutCtx<'_> {
        LayoutCtx { scramble, focus }
    }

    const SCRAMBLE: &str = "R U R' U' F2 D B' L2 U2 R2 F D' L B2 U R' F' D2 L' B";

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    }

    fn rects(plan: &LayoutPlan) -> Vec<Rect> {
        match plan {
            LayoutPlan::TooSmall => vec![],
            LayoutPlan::Focus { timer } => vec![*timer],
            LayoutPlan::Compact {
                header,
                scramble,
                timer,
                summary,
                footer,
            } => vec![*header, *scramble, *timer, *summary, *footer],
            LayoutPlan::Dashboard {
                header,
                sidebar_stats,
                sidebar_history,
                scramble,
                timer,
                tools_cube,
                tools_chart,
                footer,
            } => {
                let mut v = vec![
                    *header,
                    *sidebar_stats,
                    *sidebar_history,
                    *scramble,
                    *timer,
                    *tools_cube,
                    *footer,
                ];
                v.extend(*tools_chart);
                v
            }
        }
    }

    fn assert_sane(plan: &LayoutPlan, area: Rect) {
        let rs = rects(plan);
        for r in &rs {
            assert!(r.x >= area.x && r.y >= area.y, "{r:?}");
            assert!(
                r.right() <= area.right() && r.bottom() <= area.bottom(),
                "{r:?}"
            );
            assert!(r.width > 0 && r.height > 0, "empty rect {r:?} in {plan:?}");
        }
        for (i, a) in rs.iter().enumerate() {
            for b in &rs[i + 1..] {
                assert!(!overlaps(*a, *b), "{a:?} overlaps {b:?}");
            }
        }
    }

    fn draw(app: &App, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| render(f, app)).unwrap();
        terminal
    }

    fn screen_text(terminal: &Terminal<TestBackend>) -> String {
        let buf = terminal.backend().buffer();
        let w = buf.area.width as usize;
        buf.content()
            .chunks(w)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_text(app: &App, w: u16, h: u16) -> String {
        screen_text(&draw(app, w, h))
    }

    fn app_with(times: &[u64]) -> App {
        let mut app = App::test_app(7, true);
        for &t in times {
            app.test_record_solve(t);
        }
        app
    }

    // ----- layout -----

    #[test]
    fn wide_layout_has_every_region_without_overlap() {
        for (w, h) in [(110, 32), (120, 40), (160, 60), (200, 50)] {
            let area = Rect::new(0, 0, w, h);
            let plan = compute_layout(area, &ctx(SCRAMBLE, false));
            assert!(
                matches!(
                    plan,
                    LayoutPlan::Dashboard {
                        tools_chart: Some(_),
                        ..
                    }
                ),
                "{w}x{h}: {plan:?}"
            );
            assert_sane(&plan, area);
        }
    }

    #[test]
    fn medium_layout_has_no_chart() {
        for (w, h) in [(90, 26), (80, 24), (109, 40), (140, 31)] {
            let area = Rect::new(0, 0, w, h);
            let plan = compute_layout(area, &ctx(SCRAMBLE, false));
            match &plan {
                LayoutPlan::Dashboard {
                    tools_chart,
                    tools_cube,
                    ..
                } => {
                    assert!(tools_chart.is_none(), "{w}x{h}");
                    assert!(tools_cube.height > 0);
                }
                other => panic!("{w}x{h}: expected dashboard, got {other:?}"),
            }
            assert_sane(&plan, area);
        }
    }

    #[test]
    fn compact_layout_at_60x16_and_boundaries() {
        for (w, h) in [(60, 16), (40, 12), (79, 40), (100, 23)] {
            let area = Rect::new(0, 0, w, h);
            let plan = compute_layout(area, &ctx(SCRAMBLE, false));
            assert!(
                matches!(plan, LayoutPlan::Compact { .. }),
                "{w}x{h}: {plan:?}"
            );
            assert_sane(&plan, area);
        }
    }

    #[test]
    fn too_small_below_compact() {
        for (w, h) in [(30, 10), (39, 30), (100, 11), (1, 1)] {
            let area = Rect::new(0, 0, w, h);
            assert_eq!(
                compute_layout(area, &ctx(SCRAMBLE, false)),
                LayoutPlan::TooSmall,
                "{w}x{h}"
            );
        }
    }

    #[test]
    fn focus_layout_is_the_whole_screen() {
        let area = Rect::new(0, 0, 100, 30);
        assert_eq!(
            compute_layout(area, &ctx(SCRAMBLE, true)),
            LayoutPlan::Focus { timer: area }
        );
        // Even on a tiny terminal the solve stays visible.
        let tiny = Rect::new(0, 0, 20, 5);
        assert_eq!(
            compute_layout(tiny, &ctx(SCRAMBLE, true)),
            LayoutPlan::Focus { timer: tiny }
        );
    }

    #[test]
    fn layout_is_sane_for_every_size() {
        for w in (1..=200).step_by(7) {
            for h in (1..=60).step_by(5) {
                let area = Rect::new(0, 0, w, h);
                for focus in [false, true] {
                    assert_sane(&compute_layout(area, &ctx(SCRAMBLE, focus)), area);
                }
            }
        }
    }

    #[test]
    fn layout_offsets_are_respected() {
        let area = Rect::new(3, 2, 120, 40);
        assert_sane(&compute_layout(area, &ctx(SCRAMBLE, false)), area);
    }

    #[test]
    fn scramble_height_grows_with_wrapping() {
        let long = SCRAMBLE.repeat(3);
        let area = Rect::new(0, 0, 90, 30);
        let h = |s: &str| match compute_layout(area, &ctx(s, false)) {
            LayoutPlan::Dashboard { scramble, .. } => scramble.height,
            other => panic!("{other:?}"),
        };
        assert_eq!(h("R U"), 3, "one line + border");
        let lines = wrap_words(&long, 58).len() as u16;
        assert!(lines > 1);
        assert_eq!(h(&long), lines + 2);
    }

    #[test]
    fn tall_wide_terminal_uses_scale_two_cube() {
        let area = Rect::new(0, 0, 200, 60);
        match compute_layout(area, &ctx(SCRAMBLE, false)) {
            LayoutPlan::Dashboard {
                tools_cube,
                tools_chart,
                ..
            } => {
                assert_eq!(crate::cube_widget::net_scale(tools_cube), 2);
                assert!(tools_chart.is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn wrap_words_never_exceeds_width_and_keeps_all_tokens() {
        for width in [1usize, 5, 10, 23, 40, 80] {
            let lines = wrap_words(SCRAMBLE, width);
            let joined = lines.join(" ");
            assert_eq!(joined, SCRAMBLE, "width {width}");
            for l in &lines {
                let longest = SCRAMBLE.split(' ').map(|t| t.len()).max().unwrap();
                assert!(l.chars().count() <= width.max(longest), "{l:?} @ {width}");
            }
        }
        assert_eq!(wrap_words("", 10), vec![String::new()]);
    }

    // ----- rendering -----

    #[test]
    fn wide_dashboard_renders_every_region() {
        let app = app_with(&[12_000, 13_000, 11_500]);
        let text = render_text(&app, 110, 32);
        for needle in [
            "tuibik",
            "Session 1",
            "3 solves",
            "Stats",
            "History",
            "Scramble",
            "Cube",
            "Progress",
            "single",
            "Space",
        ] {
            assert!(text.contains(needle), "missing {needle:?}\n{text}");
        }
        assert!(text.contains('█'), "cube net / big digits");
    }

    #[test]
    fn medium_dashboard_hides_chart_but_shows_cube() {
        let app = app_with(&[12_000]);
        let text = render_text(&app, 90, 26);
        assert!(text.contains("Stats") && text.contains("History") && text.contains("Cube"));
        assert!(!text.contains("Progress"));
    }

    #[test]
    fn compact_shows_only_header_scramble_timer_summary_footer() {
        let app = app_with(&[12_000; 6]);
        let text = render_text(&app, 60, 16);
        assert!(text.contains("tuibik"));
        assert!(text.contains("6 solves · ao5 12.00 · ao12 —"), "{text}");
        assert!(text.contains("Space"), "footer");
        for hidden in ["Stats", "History", "Cube", "Progress"] {
            assert!(!text.contains(hidden), "{hidden} should be hidden\n{text}");
        }
    }

    #[test]
    fn too_small_message_and_app_keeps_running() {
        let app = app_with(&[]);
        let text = render_text(&app, 30, 10);
        // The message wraps at 30 columns; compare with whitespace collapsed.
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains("Terminal too small: 30×10 (need 40×12)"),
            "{text}"
        );
        assert!(!text.contains("Stats"));
        // Tiny sizes must not panic either.
        for (w, h) in [(1, 1), (5, 2), (10, 1)] {
            let _ = render_text(&app, w, h);
        }
    }

    #[test]
    fn live_resize_switches_layout() {
        let app = app_with(&[12_000]);
        let big = render_text(&app, 120, 40);
        assert!(big.contains("History"));
        let small = render_text(&app, 70, 20);
        assert!(!small.contains("History") && small.contains("ao5"));
    }

    fn assert_scramble_visible(text: &str, scramble: &str) {
        let collapsed = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(collapsed.contains(scramble), "scramble truncated:\n{text}");
    }

    #[test]
    fn narrow_terminals_wrap_the_whole_scramble() {
        let app = app_with(&[]);
        for (w, h) in [(60, 16), (45, 16), (40, 14)] {
            let text = render_text(&app, w, h);
            // Compare per-line tokens so wrapping cannot hide a truncated move.
            let shown: Vec<&str> = text.lines().flat_map(|l| l.split_whitespace()).collect();
            let mut it = shown.iter();
            for mv in app.scramble.text.split_whitespace() {
                assert!(
                    it.any(|t| *t == mv),
                    "move {mv} missing at {w}x{h}:\n{text}"
                );
            }
        }
        // And on the dashboard's bordered panel at 60 columns of main width.
        let text = render_text(&app, 92, 26);
        for mv in app.scramble.text.split_whitespace() {
            assert!(text.contains(mv));
        }
        assert_scramble_visible(&render_text(&app, 200, 60), &app.scramble.text);
    }

    #[test]
    fn header_shows_session_and_count_after_switch() {
        let mut app = app_with(&[]);
        app.handle_input(Input::OpenSessions);
        app.handle_input(Input::NewScramble);
        for c in "OH".chars() {
            app.handle_input(Input::Char(c));
        }
        app.handle_input(Input::Confirm);
        app.test_record_solve(9_000);
        let text = render_text(&app, 110, 32);
        let header = text.lines().next().unwrap();
        assert!(
            header.contains("OH") && header.contains("1 solve"),
            "{header}"
        );
        assert!(header.contains(app.theme().name));
    }

    #[test]
    fn timer_labels_per_phase() {
        let mut app = App::test_app(7, true);
        // Idle
        assert!(render_text(&app, 100, 30).contains("hold Space to start"));
        // Arming
        app.timer.press(0);
        assert!(render_text(&app, 100, 30).contains("HOLD"));
        // Ready
        app.timer.tick(ARM_THRESHOLD_MS + 1);
        let text = render_text(&app, 100, 30);
        assert!(text.contains("READY"), "{text}");
        // Running
        app.timer.release(ARM_THRESHOLD_MS + 1);
        app.timer
            .tick(ARM_THRESHOLD_MS + 1 + RELEASE_DEBOUNCE_MS + 1);
        app.timer.tick(ARM_THRESHOLD_MS + 5_000);
        let text = render_text(&app, 100, 30);
        assert!(text.contains("SOLVING"), "{text}");
        assert!(text.contains('█'), "running digits are big");
    }

    #[test]
    fn idle_hint_differs_in_fallback_mode() {
        let app = App::test_app(7, false);
        assert!(render_text(&app, 100, 30).contains("press Space to start"));
    }

    #[test]
    fn inspection_countdown_cues_and_overtime_are_visible() {
        let mut app = App::test_app(7, true);
        app.handle_input(Input::Settings);
        app.handle_input(Input::Confirm); // inspection on
        app.handle_input(Input::Cancel);
        assert!(app.timer.config().inspection);
        app.handle_input_at(Input::HoldPress, 0);
        assert_eq!(app.timer.phase, Phase::Inspecting);
        let text = render_text(&app, 100, 30);
        assert!(text.contains("INSPECT") && !text.contains("8s!"), "{text}");
        app.tick_at(8_500);
        assert!(render_text(&app, 100, 30).contains("8s!"));
        app.tick_at(12_500);
        assert!(render_text(&app, 100, 30).contains("12s!"));
        app.tick_at(15_500);
        assert!(render_text(&app, 100, 30).contains("+2"));
    }

    #[test]
    fn hidden_running_time_shows_only_solving() {
        let mut app = App::test_app(7, false);
        app.settings.show_running_time = false;
        app.handle_input_at(Input::HoldPress, 0);
        app.tick_at(4_000);
        assert_eq!(app.timer.phase, Phase::Running);
        let text = render_text(&app, 100, 30);
        assert!(text.contains("SOLVING"));
        assert!(!text.contains('█'), "no digits while hidden:\n{text}");
        // Once stopped, the final time is shown.
        app.handle_input_at(Input::HoldPress, 4_000);
        assert!(render_text(&app, 100, 30).contains('█'));
    }

    #[test]
    fn focus_mode_hides_panels_while_ready_and_restores_after_stop() {
        let mut app = App::test_app(7, true);
        app.test_record_solve(9_000);
        let idle = render_text(&app, 120, 40);
        assert!(idle.contains("Scramble") && idle.contains("History"));

        app.timer.press(0);
        app.timer.tick(ARM_THRESHOLD_MS + 1);
        assert_eq!(app.timer.phase, Phase::Ready);
        let focus = render_text(&app, 120, 40);
        for hidden in [
            "Scramble", "History", "Stats", "Cube", "Progress", "tuibik", "Space",
        ] {
            assert!(!focus.contains(hidden), "{hidden} visible in focus mode");
        }
        assert!(focus.contains("READY"));

        app.timer.cancel();
        assert!(render_text(&app, 120, 40).contains("Scramble"));
    }

    #[test]
    fn focus_timer_is_centred() {
        let mut app = App::test_app(7, true);
        app.timer.press(0);
        let term = draw(&app, 100, 30);
        let text = screen_text(&term);
        let rows: Vec<(usize, &str)> = text
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains('█'))
            .collect();
        assert!(rows.len() >= 3);
        let mid = (rows.first().unwrap().0 + rows.last().unwrap().0) as f64 / 2.0;
        assert!(
            (mid - 14.5).abs() < 4.0,
            "digits vertically centred, got row {mid}"
        );
        let l = rows[0].1;
        let left = l.chars().take_while(|c| *c == ' ').count();
        let right = l.chars().rev().take_while(|c| *c == ' ').count();
        assert!((left as i64 - right as i64).abs() <= 2, "{left} vs {right}");
    }

    #[test]
    fn post_solve_feedback_shows_delta_averages_and_pb() {
        let mut app = app_with(&[13_000, 13_500, 14_000, 13_200]);
        app.test_record_solve(12_480); // 5th: first ao5, single PB, -0.72
        let text = render_text(&app, 120, 40);
        assert!(text.contains("-0.72"), "{text}");
        assert!(text.contains("ao5 13.23"), "{text}");
        assert!(text.contains("New PB single!"), "{text}");
        // The final time is shown big and stays until the next solve starts.
        assert!(text.contains('█'));
        // First ao5 is not a PB.
        assert!(!text.contains("New PB ao5"));
    }

    #[test]
    fn first_solve_has_no_delta_line_and_does_not_error() {
        let app = app_with(&[13_000]);
        let text = render_text(&app, 120, 40);
        assert!(!text.contains("vs"));
        assert!(!text.contains("PB"));
    }

    #[test]
    fn penalised_last_solve_shows_plus_and_dnf_in_timer_area() {
        let mut app = app_with(&[12_340]);
        app.handle_input(Input::Penalty(Penalty::PlusTwo));
        let text = render_text(&app, 120, 40);
        assert!(
            text.contains("14.34+"),
            "history shows effective time:\n{text}"
        );
        app.handle_input(Input::Penalty(Penalty::Dnf));
        assert!(render_text(&app, 120, 40).contains("DNF"));
    }

    #[test]
    fn stats_table_after_six_solves() {
        let app = app_with(&[12_000, 12_500, 13_000, 11_000, 12_200, 12_100]);
        let text = render_text(&app, 110, 32);
        assert!(text.contains("current") && text.contains("best"));
        let line = |label: &str| {
            text.lines()
                .find(|l| l.contains(label))
                .unwrap_or_else(|| panic!("no {label} row\n{text}"))
                .to_string()
        };
        for label in ["single", "mo3", "ao5"] {
            assert!(!line(label).contains('—'), "{label}: {}", line(label));
        }
        for label in ["ao12", "ao100"] {
            assert_eq!(line(label).matches('—').count(), 2, "{label}");
        }
        assert!(
            text.contains("mean 12.13 (6/6)") && text.contains('σ'),
            "{text}"
        );
    }

    #[test]
    fn stats_table_shows_dnf_and_counted_solves() {
        let mut app = app_with(&[12_000, 13_000, 14_000]);
        app.handle_input(Input::Penalty(Penalty::Dnf));
        let text = render_text(&app, 110, 32);
        assert!(text.contains("(2/3)"), "{text}");
        let single = text.lines().find(|l| l.contains("single")).unwrap();
        assert!(single.contains("DNF"), "{single}");
    }

    #[test]
    fn current_equal_to_best_is_accented() {
        let theme = crate::theme::all()[0].clone();
        // Improving solves: current ao5 is the best ao5.
        let mut app = app_with(&[10_000; 5]);
        let lines = stats_lines(app.summary(), &theme);
        let ao5 = lines
            .iter()
            .find(|l| l.to_string().starts_with("ao5"))
            .unwrap();
        assert_eq!(ao5.spans[1].style.fg, Some(theme.accent));
        // Slower solves: current no longer equals best.
        for _ in 0..3 {
            app.test_record_solve(12_000);
        }
        let lines = stats_lines(app.summary(), &theme);
        let ao5 = lines
            .iter()
            .find(|l| l.to_string().starts_with("ao5"))
            .unwrap();
        assert_eq!(ao5.spans[1].style.fg, Some(theme.text));
    }

    #[test]
    fn history_scrolls_to_follow_selection() {
        let mut app = App::test_app(7, true);
        for i in 0..25 {
            app.test_record_solve(10_000 + i * 10);
        }
        app.history_selected = 5; // solve #20
                                  // 90x26: history panel is short (~13 rows).
        let text = render_text(&app, 90, 26);
        let row = text
            .lines()
            .find(|l| l.contains("  20  "))
            .unwrap_or_else(|| panic!("solve #20 not visible\n{text}"));
        assert!(row.contains("10.19"));
        // Move to the very oldest, then back to the newest.
        app.handle_input(Input::JumpBottom);
        assert!(render_text(&app, 90, 26).contains("   1  "));
        app.handle_input(Input::JumpTop);
        assert!(render_text(&app, 90, 26).contains("  25  "));
    }

    #[test]
    fn history_area_of_eight_rows_shows_selected_solve() {
        let mut app = App::test_app(7, true);
        for i in 0..30 {
            app.test_record_solve(9_000 + i * 100);
        }
        let mut terminal = Terminal::new(TestBackend::new(30, 10)).unwrap();
        app.history_selected = 19; // solve #11
        terminal
            .draw(|f| render_history(f, &app, app.theme(), f.area()))
            .unwrap();
        let text = screen_text(&terminal);
        assert!(
            text.contains("  11  "),
            "selected row scrolled into view:\n{text}"
        );
    }

    #[test]
    fn history_marks_best_and_styles_dnf() {
        let mut app = app_with(&[12_000, 9_000, 15_000]);
        app.history_selected = 2;
        app.handle_input(Input::Penalty(Penalty::Dnf));
        let text = render_text(&app, 110, 32);
        assert!(
            text.lines().any(|l| l.contains("9.00") && l.contains('★')),
            "best single carries a marker:\n{text}"
        );
        assert!(text.lines().any(|l| l.contains("DNF")));
    }

    #[test]
    fn empty_history_and_chart_show_hints() {
        let app = app_with(&[]);
        let text = render_text(&app, 110, 32);
        assert!(text.contains("No solves yet — hold Space"), "{text}");
        assert!(text.contains("No solves yet"));
    }

    #[test]
    fn chart_y_bounds_exclude_zero_for_14_to_18_seconds() {
        let (lo, hi) = chart_bounds(&[14.0, 15.5, 18.0, 16.0]);
        assert!(lo > 10.0 && lo <= 14.0, "lo={lo}");
        assert!((18.0..20.0).contains(&hi), "hi={hi}");
        // Flat data still gets a visible range.
        let (lo, hi) = chart_bounds(&[12.0, 12.0]);
        assert!(hi - lo >= 1.0);
        assert_eq!(chart_bounds(&[]), (0.0, 1.0));
    }

    #[test]
    fn chart_data_omits_dnf_and_windows_to_recent_solves() {
        let mut app = App::test_app(7, true);
        for i in 0..40 {
            app.test_record_solve(14_000 + i * 100);
        }
        app.history_selected = 0;
        app.handle_input(Input::Penalty(Penalty::Dnf));
        let d = chart_data(&app.solves, 10); // capacity 20 points
        assert_eq!(d.last_no, 40);
        assert_eq!(d.first_no, 21);
        assert_eq!(d.times.len(), 19, "DNF omitted, 20 solves in window");
        assert!(d.ao5.len() >= 15 && d.ao12.len() >= 8);
        // Averages exist right from the first window point thanks to the lead-in.
        assert_eq!(d.ao12.first().unwrap().0, 0.0);
    }

    #[test]
    fn chart_renders_with_data_and_dnf_only() {
        let mut app = app_with(&[14_000, 15_000, 16_000, 17_000, 18_000, 15_500]);
        let text = render_text(&app, 130, 40);
        assert!(
            text.contains("Progress") && text.contains("best 14.00"),
            "{text}"
        );
        assert!(text.contains("#1") && text.contains("#6"));
        // An all-DNF session must not crash and shows a hint.
        for i in 0..app.solves.len() {
            app.history_selected = i;
            app.handle_input(Input::Penalty(Penalty::Dnf));
        }
        assert!(render_text(&app, 130, 40).contains("No finished solves yet"));
    }

    #[test]
    fn cube_net_uses_scale_two_in_a_big_region() {
        let app = app_with(&[]);
        let term = draw(&app, 200, 60);
        let text = screen_text(&term);
        // Scale 2 stickers are 4 columns wide: 4 consecutive blocks appear.
        assert!(text.contains("████████"), "scale-2 net");
        let small = render_text(&app, 110, 32);
        assert!(small.contains("██"));
    }

    #[test]
    fn preview_footer_differs_from_dashboard_footer() {
        let mut app = app_with(&[]);
        let dash = footer_hints(&app);
        app.handle_input(Input::TogglePreview);
        let prev = footer_hints(&app);
        assert_ne!(dash, prev);
        assert!(prev.iter().any(|(_, d)| *d == "step"));
        assert!(!prev.iter().any(|(k, _)| *k == "Space"));
        let text = render_text(&app, 110, 32);
        let footer = text.lines().last().unwrap();
        assert!(
            footer.contains("step") && footer.contains("close"),
            "{footer}"
        );
        assert!(!footer.contains("delete"));
        assert!(text.contains("step "), "cube title shows the step");
    }

    #[test]
    fn footer_is_context_aware_for_every_overlay() {
        let mut seen: Vec<Vec<(&str, &str)>> = Vec::new();
        for which in [
            Overlay::None,
            Overlay::Sessions,
            Overlay::Preview,
            Overlay::Help,
            Overlay::Settings,
            Overlay::Detail,
            Overlay::ConfirmDelete,
        ] {
            let mut app = app_with(&[10_000]);
            match which {
                Overlay::Sessions => app.handle_input(Input::OpenSessions),
                Overlay::Preview => app.handle_input(Input::TogglePreview),
                Overlay::Help => app.handle_input(Input::Help),
                Overlay::Settings => app.handle_input(Input::Settings),
                Overlay::Detail => app.handle_input(Input::Confirm),
                Overlay::ConfirmDelete => app.handle_input(Input::Delete),
                Overlay::None => {}
            }
            assert_eq!(app.overlay, which);
            let hints = footer_hints(&app);
            assert!(
                !seen.contains(&hints),
                "{which:?} duplicates another footer"
            );
            seen.push(hints);
        }
    }

    #[test]
    fn dashboard_footer_keeps_help_and_truncates_from_the_end() {
        let app = app_with(&[]);
        let narrow = render_text(&app, 60, 16);
        let footer = narrow.lines().last().unwrap();
        assert!(footer.contains("? help"), "{footer}");
        assert!(
            !footer.contains("theme"),
            "least important hint dropped: {footer}"
        );
        assert!(footer.trim_end().chars().count() <= 60);
        let wide = render_text(&app, 140, 40);
        assert!(wide.lines().last().unwrap().contains("theme"));
    }

    #[test]
    fn toast_replaces_footer_hints() {
        let mut app = app_with(&[10_000]);
        app.handle_input(Input::Penalty(Penalty::PlusTwo));
        let text = render_text(&app, 110, 32);
        let footer = text.lines().last().unwrap();
        assert!(footer.contains("Solve #1: +2"), "{footer}");
        assert!(!footer.contains("history"));
    }

    #[test]
    fn detail_overlay_shows_solve_information() {
        let mut app = App::test_app(7, true);
        for i in 1..=12 {
            app.test_record_solve(10_000 + i * 100);
        }
        app.history_selected = 0; // solve #12
        app.handle_input(Input::Confirm);
        assert_eq!(app.overlay, Overlay::Detail);
        let text = render_text(&app, 110, 32);
        assert!(text.contains("Solve #12"), "{text}");
        assert!(text.contains("11.20"), "time");
        assert!(text.contains(" UTC"), "date");
        assert!(text.contains("Penalty") && text.contains("OK"));
        // ao5 and ao12 ending at this solve.
        let ao5_row = text
            .lines()
            .find(|l| l.contains("ao5") && l.contains("11."))
            .unwrap();
        assert!(!ao5_row.contains('—'));
        assert!(text
            .lines()
            .any(|l| l.contains("ao12") && !l.contains('—') && l.contains("11.")));
        // The scramble is shown in full (it wraps inside the popup).
        for mv in app.solves.last().unwrap().scramble.split_whitespace() {
            assert!(text.contains(mv));
        }
    }

    #[test]
    fn detail_of_penalised_solve_shows_raw_time() {
        let mut app = app_with(&[12_340]);
        app.handle_input(Input::Confirm);
        app.handle_input(Input::Penalty(Penalty::PlusTwo));
        let text = render_text(&app, 110, 32);
        assert!(
            text.contains("14.34+") && text.contains("raw 12.34"),
            "{text}"
        );
        app.handle_input(Input::Penalty(Penalty::Dnf));
        let text = render_text(&app, 110, 32);
        assert!(text.contains("DNF  (raw 12.34)"), "{text}");
    }

    #[test]
    fn detail_of_early_solve_has_no_averages() {
        let mut app = app_with(&[12_000, 13_000]);
        app.handle_input(Input::Confirm);
        let text = render_text(&app, 110, 32);
        assert!(text.contains("Solve #2"));
        for label in ["ao5", "ao12"] {
            // The detail popup rows read `ao5      —`; the stats table rows differ.
            assert!(
                text.lines().any(|l| l.contains(&format!("{label:<10}—"))),
                "{label} should be unavailable:\n{text}"
            );
        }
    }

    #[test]
    fn delete_dialog_names_solve_and_time() {
        let mut app = App::test_app(7, true);
        for i in 1..=10 {
            app.test_record_solve(i * 1_000);
        }
        app.history_selected = 5; // solve #5 = 5.00
        app.handle_input(Input::Delete);
        let text = render_text(&app, 110, 32);
        assert!(text.contains("Delete solve #5 (5.00)?"), "{text}");
        assert!(text.contains("[y]") && text.contains("[n]"));
        // Cancel path renders the dashboard again.
        app.handle_input(Input::Cancel);
        assert!(!render_text(&app, 110, 32).contains("Delete solve #5"));
    }

    #[test]
    fn help_overlay_lists_bindings_and_timing_mode() {
        let mut app = App::test_app(7, true);
        app.handle_input(Input::Help);
        let text = render_text(&app, 110, 40);
        assert!(text.contains("hold-to-start"), "{text}");
        assert!(!text.contains("tap-to-start"));
        for needle in [
            "While timing",
            "Dashboard",
            "Overlays",
            "new scramble",
            "OK / +2 / DNF",
            "sessions",
            "settings",
            "next theme",
            "quit",
            "Ctrl+C",
            "never quits",
            "cancel arming",
        ] {
            assert!(text.contains(needle), "missing {needle:?}");
        }
        let mut app = App::test_app(7, false);
        app.handle_input(Input::Help);
        let text = render_text(&app, 110, 40);
        assert!(
            text.contains("tap-to-start") && !text.contains("hold-to-start"),
            "{text}"
        );
    }

    #[test]
    fn settings_overlay_shows_values_and_theme_name() {
        let mut app = App::test_app(7, true);
        app.handle_input(Input::Settings);
        let text = render_text(&app, 110, 32);
        assert!(text.contains("WCA inspection") && text.contains("off"));
        assert!(text.contains("Show running time") && text.contains("on"));
        assert!(text.contains(&format!("◂ {} ▸", app.theme().name)));
        app.handle_input(Input::Confirm);
        assert!(render_text(&app, 110, 32).contains("WCA inspection      on"));
    }

    #[test]
    fn sessions_overlay_renders_list_marker_and_hints() {
        let mut app = App::test_app(7, true);
        app.handle_input(Input::OpenSessions);
        let text = render_text(&app, 110, 32);
        assert!(
            text.contains("Sessions") && text.contains("● Session 1"),
            "{text}"
        );
        assert!(
            text.contains("Enter switch") && text.contains("╭"),
            "rounded border"
        );
        app.handle_input(Input::NewScramble);
        app.handle_input(Input::Char('x'));
        let text = render_text(&app, 110, 32);
        assert!(
            text.contains("New session") && text.contains("> x_"),
            "{text}"
        );
    }

    #[test]
    fn overlays_render_on_every_supported_size_without_panicking() {
        for which in ALL {
            for (w, h) in [(40, 12), (60, 16), (90, 26), (110, 32), (200, 60)] {
                let mut app = app_with(&[10_000, 11_000]);
                match which {
                    Overlay::Sessions => app.handle_input(Input::OpenSessions),
                    Overlay::Preview => app.handle_input(Input::TogglePreview),
                    Overlay::Help => app.handle_input(Input::Help),
                    Overlay::Settings => app.handle_input(Input::Settings),
                    Overlay::Detail => app.handle_input(Input::Confirm),
                    Overlay::ConfirmDelete => app.handle_input(Input::Delete),
                    Overlay::None => {}
                }
                let _ = render_text(&app, w, h);
            }
        }
    }

    const ALL: [Overlay; 7] = [
        Overlay::None,
        Overlay::Sessions,
        Overlay::Preview,
        Overlay::Help,
        Overlay::Settings,
        Overlay::Detail,
        Overlay::ConfirmDelete,
    ];

    #[test]
    fn preview_highlights_current_move_in_scramble() {
        let mut app = app_with(&[]);
        app.handle_input(Input::TogglePreview);
        app.handle_input(Input::Left);
        let term = draw(&app, 110, 32);
        let buf = term.backend().buffer();
        let theme = app.theme();
        let highlighted = buf
            .content()
            .iter()
            .filter(|c| c.style().bg == Some(theme.select_bg))
            .count();
        assert!(highlighted > 0, "current move is highlighted");
    }

    #[test]
    fn render_with_each_theme_does_not_panic() {
        let mut app = App::test_app(7, true);
        for _ in 0..crate::theme::all().len() + 1 {
            let _ = render_text(&app, 100, 40);
            app.cycle_theme();
        }
    }

    #[test]
    fn epoch_formatting() {
        assert_eq!(format_epoch_utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(format_epoch_utc(1_000_000_000), "2001-09-09 01:46 UTC");
        assert_eq!(format_epoch_utc(1_709_164_800), "2024-02-29 00:00 UTC");
        assert_eq!(format_epoch_utc(-1), "1969-12-31 23:59 UTC");
    }

    #[test]
    fn delta_formatting() {
        assert_eq!(fmt_delta(-520), "-0.52");
        assert_eq!(fmt_delta(1030), "+1.03");
        assert_eq!(fmt_delta(0), "+0.00");
        assert_eq!(fmt_delta(-61_000), "-1:01.00");
    }
}
