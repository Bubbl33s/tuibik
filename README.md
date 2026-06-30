# tuibik

A terminal TUI for 3×3 Rubik's cube speedsolving practice.

## Features

- 20-move practice scrambles with no same-face or opposite-face consecutive moves
- Centisecond-precision timer
- Kitty keyboard protocol support (hold-release Space) with 300ms arm indicator
- Fallback mode (press-press Space) for terminals without Kitty support
- Session statistics: ao5, ao12, ao100, best single, session mean
- Fits in 80 columns

## Install

```sh
go install github.com/user/tuibik@latest
```

Or build from source:

```sh
make build
```

## Usage

```sh
./tuibik
```

Controls:

- **Kitty mode** (Kitty-capable terminal): hold Space for 300ms until the indicator turns green, then release to start the timer. Press Space to stop.
- **Fallback mode** (standard terminal): press Space to start the timer. Press Space again to stop.
- **q** or **Ctrl+C**: quit (only when the timer is idle or stopped).

> **Note on input mode:** Kitty keyboard protocol is detected automatically. If your terminal does not support it, tuibik falls back to press-press mode automatically.

> **Note on practice scrambles:** Scrambles are generated for practice purposes only. They follow the WCA consecutive-face constraint but are not WCA-competition scrambles.

## Terminal Compatibility

| Terminal | Kitty mode | Fallback mode |
|----------|-----------|---------------|
| kitty | Yes | — |
| WezTerm | Yes | — |
| Ghostty | Yes | — |
| iTerm2 | No | Yes |
| Terminal.app | No | Yes |
| GNOME Terminal | No | Yes |

## Development

Requirements: Go 1.24+

```sh
make test    # run all tests
make vet     # run go vet
make build   # build local binary
make clean   # remove build artifacts
```
