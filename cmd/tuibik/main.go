package main

import (
	"flag"
	"fmt"
	"os"

	tea "charm.land/bubbletea/v2"
	"github.com/user/tuibik/internal/ui"
)

// version is set at build time via -ldflags "-X main.version=<tag>".
var version = "dev"

func main() {
	forceKitty := flag.Bool("kitty", false, "force Kitty keyboard protocol (hold-release Space)")
	forceStd := flag.Bool("no-kitty", false, "force standard (press-press) input mode")
	showVersion := flag.Bool("version", false, "print version and exit")
	flag.Parse()

	if *showVersion {
		fmt.Println("tuibik", version)
		os.Exit(0)
	}

	if *forceKitty && *forceStd {
		fmt.Fprintln(os.Stderr, "error: --kitty and --no-kitty are mutually exclusive")
		os.Exit(1)
	}

	// kitty=true  → use kittyStrategy from the start; View() will request key
	//               releases so they fire correctly.
	// kitty=false → start with fallbackStrategy; if the terminal supports key
	//               releases, Update() auto-upgrades via KeyboardEnhancementsMsg.
	//               (--no-kitty also maps to kitty=false; the auto-upgrade is
	//               acceptable since View() sets ReportEventTypes every frame.)
	kitty := *forceKitty

	m := ui.New(kitty)
	p := tea.NewProgram(m)
	if _, err := p.Run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
