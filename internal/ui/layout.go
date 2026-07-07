package ui

import (
	"fmt"
	"strings"

	"charm.land/lipgloss/v2"
)

// render dispatches to the appropriate width-band layout.
// Width bands:
//
//	>=100  full layout: one-row stats + history list
//	80–99  wide layout: two-row stats + history list
//	60–79  medium layout: two-row stats, no history list
//	<60    minimal layout: badge + scramble (truncated) + timer only
func (m Model) render() string {
	w := m.width
	if w <= 0 {
		w = 80 // safe default before the first WindowSizeMsg arrives
	}
	switch {
	case w >= 100:
		return m.renderFull(w)
	case w >= 80:
		return m.renderWide(w)
	case w >= 60:
		return m.renderMedium(w)
	default:
		return m.renderMinimal(w)
	}
}

// center returns str centered inside a block of width w using Lipgloss.
func center(str string, w int) string {
	return lipgloss.NewStyle().Width(w).AlignHorizontal(lipgloss.Center).Render(str)
}

// rightAlign returns str right-aligned inside a block of width w.
func rightAlign(str string, w int) string {
	return lipgloss.NewStyle().Width(w).AlignHorizontal(lipgloss.Right).Render(str)
}

// truncate truncates str to at most max visible runes (adds "…" if cut).
func truncate(str string, max int) string {
	runes := []rune(str)
	if len(runes) <= max {
		return str
	}
	if max <= 1 {
		return "…"
	}
	return string(runes[:max-1]) + "…"
}

// statRow returns a formatted "label: value" pair.
func (m Model) statPair(label, value string) string {
	return m.styles.statLabel.Render(label+":") + " " + m.styles.statValue.Render(value)
}

// buildStatsLine1 returns the first stats row (best + mean).
func (m Model) buildStatsLine1() string {
	best := formatStat(m.session.Best())
	mean := formatStat(m.session.Mean())
	return m.statPair("best", best) + "   " + m.statPair("mean", mean)
}

// buildStatsLine2 returns the second stats row (ao5, ao12, ao100).
func (m Model) buildStatsLine2() string {
	a5, ok5 := m.session.Average(5)
	a12, ok12 := m.session.Average(12)
	a100, ok100 := m.session.Average(100)
	return m.statPair("ao5", formatAoStat(a5, ok5)) + "   " +
		m.statPair("ao12", formatAoStat(a12, ok12)) + "   " +
		m.statPair("ao100", formatAoStat(a100, ok100))
}

// buildHistoryLines returns at most 10 solve rows, newest first.
func (m Model) buildHistoryLines(w int) []string {
	solves := m.session.Solves()
	if len(solves) > 10 {
		solves = solves[:10]
	}
	lines := make([]string, 0, len(solves))
	for _, sv := range solves {
		dur := "-"
		if sv.DNF {
			dur = "DNF"
		} else {
			dur = formatDuration(sv.Duration)
		}
		scr := truncate(sv.Scramble, w-20)
		line := fmt.Sprintf("  #%-3d %-8s %s", sv.Index, dur, scr)
		lines = append(lines, m.styles.historyItem.Render(line))
	}
	return lines
}

// timerLine returns the styled timer string.
func (m Model) timerLine(w int) string {
	text, armed := m.timerDisplay()
	var styled string
	if armed {
		styled = m.styles.timerArmed.Render(text)
	} else {
		styled = m.styles.timerNormal.Render(text)
	}
	return center(styled, w)
}

// scrambleLine returns the styled scramble string centered.
func (m Model) scrambleLine(w int) string {
	return center(m.styles.scramble.Render(m.scramble), w)
}

// badgeLine returns the mode badge right-aligned.
func (m Model) badgeLine(w int) string {
	return rightAlign(m.styles.badge.Render(m.strategy.Badge()), w)
}

// renderFull is the ≥100 column layout.
func (m Model) renderFull(w int) string {
	var b strings.Builder

	b.WriteString(m.badgeLine(w))
	b.WriteRune('\n')
	b.WriteString(m.scrambleLine(w))
	b.WriteRune('\n')
	b.WriteRune('\n')
	b.WriteString(m.timerLine(w))
	b.WriteRune('\n')
	b.WriteRune('\n')

	// Single stats row: best, mean, ao5, ao12, ao100.
	b.WriteString(m.styles.statLabel.Render("best:"))
	b.WriteString(" ")
	b.WriteString(m.styles.statValue.Render(formatStat(m.session.Best())))
	b.WriteString("   ")
	b.WriteString(m.styles.statLabel.Render("mean:"))
	b.WriteString(" ")
	b.WriteString(m.styles.statValue.Render(formatStat(m.session.Mean())))
	b.WriteString("   ")
	a5, ok5 := m.session.Average(5)
	b.WriteString(m.styles.statLabel.Render("ao5:"))
	b.WriteString(" ")
	b.WriteString(m.styles.statValue.Render(formatAoStat(a5, ok5)))
	b.WriteString("   ")
	a12, ok12 := m.session.Average(12)
	b.WriteString(m.styles.statLabel.Render("ao12:"))
	b.WriteString(" ")
	b.WriteString(m.styles.statValue.Render(formatAoStat(a12, ok12)))
	b.WriteString("   ")
	a100, ok100 := m.session.Average(100)
	b.WriteString(m.styles.statLabel.Render("ao100:"))
	b.WriteString(" ")
	b.WriteString(m.styles.statValue.Render(formatAoStat(a100, ok100)))
	b.WriteRune('\n')
	b.WriteRune('\n')

	for _, line := range m.buildHistoryLines(w) {
		b.WriteString(line)
		b.WriteRune('\n')
	}

	return b.String()
}

// renderWide is the 80–99 column layout (two stats rows, history shown).
func (m Model) renderWide(w int) string {
	var b strings.Builder

	b.WriteString(m.badgeLine(w))
	b.WriteRune('\n')
	b.WriteString(m.scrambleLine(w))
	b.WriteRune('\n')
	b.WriteRune('\n')
	b.WriteString(m.timerLine(w))
	b.WriteRune('\n')
	b.WriteRune('\n')
	b.WriteString(m.buildStatsLine1())
	b.WriteRune('\n')
	b.WriteString(m.buildStatsLine2())
	b.WriteRune('\n')
	b.WriteRune('\n')

	for _, line := range m.buildHistoryLines(w) {
		b.WriteString(line)
		b.WriteRune('\n')
	}

	return b.String()
}

// renderMedium is the 60–79 column layout (two stats rows, no history).
func (m Model) renderMedium(w int) string {
	var b strings.Builder

	b.WriteString(m.badgeLine(w))
	b.WriteRune('\n')
	b.WriteString(m.scrambleLine(w))
	b.WriteRune('\n')
	b.WriteRune('\n')
	b.WriteString(m.timerLine(w))
	b.WriteRune('\n')
	b.WriteRune('\n')
	b.WriteString(m.buildStatsLine1())
	b.WriteRune('\n')
	b.WriteString(m.buildStatsLine2())
	b.WriteRune('\n')

	return b.String()
}

// renderMinimal is the <60 column minimal layout.
func (m Model) renderMinimal(w int) string {
	var b strings.Builder

	b.WriteString(m.styles.badge.Render(m.strategy.Badge()))
	b.WriteRune('\n')
	b.WriteString(m.styles.scramble.Render(truncate(m.scramble, w-2)))
	b.WriteRune('\n')
	b.WriteRune('\n')
	b.WriteString(m.timerLine(w))
	b.WriteRune('\n')

	return b.String()
}
