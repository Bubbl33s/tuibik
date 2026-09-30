//! Session menu state: an overlay for creating, renaming, switching, and
//! deleting sessions.

use store::Session;

/// What the session menu is currently doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionMode {
    /// Browsing the list of sessions.
    Browse,
    /// Typing a name for a new session.
    Creating { input: String },
    /// Typing a new name for the selected session.
    Renaming { input: String },
    /// Confirming deletion of the selected session.
    ConfirmDelete,
}

/// State of the session overlay.
#[derive(Debug, Clone)]
pub struct SessionMenu {
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub mode: SessionMode,
}

impl SessionMenu {
    pub fn new(sessions: Vec<Session>, active_id: i64) -> Self {
        let selected = sessions.iter().position(|s| s.id == active_id).unwrap_or(0);
        SessionMenu {
            sessions,
            selected,
            mode: SessionMode::Browse,
        }
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.sessions.get(self.selected)
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        let max = self.sessions.len().saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
        }
    }

    pub fn begin_create(&mut self) {
        self.mode = SessionMode::Creating {
            input: String::new(),
        };
    }

    pub fn begin_rename(&mut self) {
        if let Some(s) = self.selected_session() {
            self.mode = SessionMode::Renaming {
                input: s.name.clone(),
            };
        }
    }

    pub fn begin_confirm_delete(&mut self) {
        // Only allow if more than one session exists (never delete the last).
        if self.sessions.len() > 1 {
            self.mode = SessionMode::ConfirmDelete;
        }
    }

    pub fn cancel_edit(&mut self) {
        self.mode = SessionMode::Browse;
    }

    /// Push a char into the active text input, if editing.
    pub fn push_char(&mut self, c: char) {
        match &mut self.mode {
            SessionMode::Creating { input } | SessionMode::Renaming { input } => {
                input.push(c);
            }
            _ => {}
        }
    }

    /// Backspace on the active text input, if editing.
    pub fn backspace(&mut self) {
        match &mut self.mode {
            SessionMode::Creating { input } | SessionMode::Renaming { input } => {
                input.pop();
            }
            _ => {}
        }
    }

    /// The current text buffer, if editing.
    pub fn current_input(&self) -> Option<&str> {
        match &self.mode {
            SessionMode::Creating { input } | SessionMode::Renaming { input } => Some(input),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sess(id: i64, name: &str) -> Session {
        Session {
            id,
            name: name.to_string(),
            created_at: 0,
        }
    }

    #[test]
    fn selects_active_session() {
        let menu = SessionMenu::new(vec![sess(1, "A"), sess(2, "B"), sess(3, "C")], 2);
        assert_eq!(menu.selected, 1);
        assert_eq!(menu.selected_session().unwrap().name, "B");
    }

    #[test]
    fn navigation_clamps() {
        let mut menu = SessionMenu::new(vec![sess(1, "A"), sess(2, "B")], 1);
        assert_eq!(menu.selected, 0);
        menu.up();
        assert_eq!(menu.selected, 0);
        menu.down();
        assert_eq!(menu.selected, 1);
        menu.down();
        assert_eq!(menu.selected, 1);
    }

    #[test]
    fn text_editing() {
        let mut menu = SessionMenu::new(vec![sess(1, "A")], 1);
        menu.begin_create();
        menu.push_char('O');
        menu.push_char('H');
        assert_eq!(menu.current_input(), Some("OH"));
        menu.backspace();
        assert_eq!(menu.current_input(), Some("O"));
        menu.cancel_edit();
        assert_eq!(menu.mode, SessionMode::Browse);
    }

    #[test]
    fn rename_prefills_current_name() {
        let mut menu = SessionMenu::new(vec![sess(1, "Practice")], 1);
        menu.begin_rename();
        assert_eq!(menu.current_input(), Some("Practice"));
    }

    #[test]
    fn cannot_delete_last_session() {
        let mut menu = SessionMenu::new(vec![sess(1, "Only")], 1);
        menu.begin_confirm_delete();
        assert_eq!(
            menu.mode,
            SessionMode::Browse,
            "must not allow delete of last"
        );
    }

    #[test]
    fn can_delete_when_multiple() {
        let mut menu = SessionMenu::new(vec![sess(1, "A"), sess(2, "B")], 1);
        menu.begin_confirm_delete();
        assert_eq!(menu.mode, SessionMode::ConfirmDelete);
    }
}
