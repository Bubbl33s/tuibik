package ui

import "charm.land/lipgloss/v2"

// styleSet holds all Lipgloss styles used by the UI.
type styleSet struct {
	timerNormal lipgloss.Style
	timerArmed  lipgloss.Style
	scramble    lipgloss.Style
	badge       lipgloss.Style
	statValue   lipgloss.Style
	statLabel   lipgloss.Style
	historyItem lipgloss.Style
}

// newStyles returns a styleSet with the default visual configuration.
func newStyles() styleSet {
	amber := lipgloss.Color("#FFB000") // warm amber for the Armed indicator
	muted := lipgloss.Color("#666666") // muted gray for secondary text

	return styleSet{
		timerNormal: lipgloss.NewStyle().Bold(true),
		timerArmed:  lipgloss.NewStyle().Bold(true).Foreground(amber),
		scramble:    lipgloss.NewStyle().Foreground(muted),
		badge:       lipgloss.NewStyle().Faint(true),
		statValue:   lipgloss.NewStyle().Bold(true),
		statLabel:   lipgloss.NewStyle().Faint(true),
		historyItem: lipgloss.NewStyle().Faint(true),
	}
}
