# tuibik

A csTimer-style **Rubik's Cube 3×3 timer for your terminal**, written in Rust
with [ratatui](https://ratatui.rs) + [crossterm](https://github.com/crossterm-rs/crossterm).

- **WCA random-state scrambles** via the Kociemba two-phase solver
  ([`m2p-core`](https://github.com/RuiminYan/min2phase-rust)); the scramble
  always wraps so every move is readable.
- **Big, centered timer** with a text state label (`HOLD` / `READY` /
  `INSPECT` / `SOLVING`). While you arm, inspect or solve, every panel gets out
  of the way (**focus mode**); the dashboard returns when the solve stops.
- **Post-solve feedback**: the delta against your previous solve, the updated
  ao5 / ao12, and a notice when you set a new personal best (single, ao5, ao12).
- **Penalties**: `+2` and `DNF` per solve, honoured by every statistic using the
  WCA rules (a DNF is the slowest result; too many DNFs make an average DNF).
- **Optional WCA inspection**: 15 s countdown with 8 s / 12 s cues, an
  automatic `+2` when you start between 15 s and 17 s, and a DNF at 17 s.
- **Responsive dashboard**: header (session, solve count, theme), a
  current/best statistics table and a scrolling history on the left, and the
  cube net and a progress chart (times + ao5 + ao12) below the timer. It adapts
  to wide, medium and compact terminals, with a "terminal too small" screen
  below 40×12.
- **Solve details**: date, scramble, raw and effective time, and the ao5 / ao12
  at that solve; delete asks for confirmation.
- **Persistent history & statistics** in SQLite: mean (with counted/total),
  best, worst, standard deviation, mo3, Ao5 / Ao12 / Ao100 (WCA trimming) and
  the best historical values. Databases from older versions upgrade
  automatically.
- **Multiple sessions**: create, rename, switch, and delete; the active session
  is remembered between runs.
- **Step-by-step scramble preview**: walk the scramble move by move.
- **Settings overlay**: inspection, show/hide the running time, and theme; all
  persisted.
- **Color themes**: several modern palettes, changed live and persisted.

## Build & run

Requires a Rust toolchain (stable). The first build downloads `m2p-core` from
git, so `.cargo/config.toml` enables `git-fetch-with-cli`.

```bash
cargo run --release
```

## Controls

Press `?` in the app for the same list.

**While timing**

| Key         | Action                                                    |
|-------------|-----------------------------------------------------------|
| `Space`     | Hold to arm, release to start (tap to start in tap mode)  |
| any key     | Stop the solve                                            |
| `Esc`       | Cancel arming / inspection (records nothing)              |

**Dashboard**

| Key                 | Action                                              |
|---------------------|-----------------------------------------------------|
| `n`                 | New scramble                                        |
| `↑` / `↓` (`k`/`j`) | Select a solve in the history                       |
| `Home` / `End` (`g`/`G`) | Jump to the newest / oldest solve              |
| `Enter`             | Solve details                                       |
| `1` / `2` / `3`     | Set the selected solve to OK / +2 / DNF             |
| `d`                 | Delete the selected solve (asks first: `y` / `n`)   |
| `p`                 | Scramble preview (`←`/`→` or `h`/`l` to step)       |
| `s`                 | Sessions (`n` new, `r` rename, `d` delete, `Enter` switch) |
| `o`                 | Settings                                            |
| `t`                 | Next theme                                          |
| `?`                 | Help                                                |
| `q`                 | Quit                                                |

**Everywhere**

| Key      | Action                                                      |
|----------|-------------------------------------------------------------|
| `Esc`    | Close the open overlay. It never quits the app.             |
| `Ctrl+C` | Quit from any state                                         |

> **Changed:** `Esc` no longer quits (it cancels/closes) and `Enter` no longer
> starts the timer (it opens the selected solve); `q` quits only from the idle
> dashboard, and `Space` is the only start key.

### Timing modes

The hold-to-start behavior uses the Kitty keyboard protocol to receive
key-release events. When the terminal doesn't support it, tuibik switches to
**tap-to-start**: press `Space` to start, press any key to stop, and a
notification says so at startup. (Without release events a held key can't be
told apart from repeated presses, so hold-to-start is impossible; tap mode can
never get stuck.) The help overlay shows which mode is active.

### Inspection

With WCA inspection on (`o` → *WCA inspection*), the first `Space` starts a
15-second countdown. Start the solve the normal way (hold and release, or a tap
in tap mode). Starting after 15 s gives a `+2`; if 17 s pass, the attempt is
recorded as a DNF. `Esc` cancels inspection without recording anything.

## Workspace layout

- `cube` — cube model (cubie-level), WCA move notation, and net rendering data.
- `scramble` — WCA random-state scramble generation (wraps `m2p-core`).
- `stats` — speedcubing statistics (means, deviations, WCA averages).
- `store` — SQLite persistence for sessions, solves, and config.
- `tuibik-tui` — the terminal UI (`tuibik` binary).

## License

GPL-3.0-or-later. This project links `m2p-core`, which is GPL-3.0-or-later; see
[`LICENSE`](LICENSE).
