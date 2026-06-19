//! Gestion des touches vim dans le composer.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::cursor::TextCursor;

const MAX_UNDO: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    Insert,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    Delete,
    Change,
    Yank,
}

#[derive(Debug, Clone)]
enum NormalPending {
  /// Premier `d` d'une séquence `dd` / `dw`…
    OperatorLead(Operator),
    Operator(Operator),
}

/// État vim du composer (persisté via `tui-preferences.json`).
#[derive(Debug, Clone)]
pub struct VimComposer {
    pub enabled: bool,
    pub mode: VimMode,
    pub cursor: TextCursor,
    pending: Option<NormalPending>,
    register: String,
    undo_stack: Vec<String>,
}

impl Default for VimComposer {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: VimMode::Insert,
            cursor: TextCursor { index: 0 },
            pending: None,
            register: String::new(),
            undo_stack: Vec::new(),
        }
    }
}

impl VimComposer {
    pub fn set_enabled(&mut self, enabled: bool, buffer: &str) {
        self.enabled = enabled;
        self.mode = VimMode::Insert;
        self.pending = None;
        self.cursor = TextCursor::at_end(buffer);
    }

    pub fn sync_cursor_end(&mut self, buffer: &str) {
        self.cursor = TextCursor::at_end(buffer);
        self.cursor.clamp(buffer);
    }

    pub fn insert_text(&mut self, buffer: &mut String, text: &str) {
        if !self.enabled {
            buffer.push_str(text);
            return;
        }
        self.snapshot(buffer);
        let byte = self.cursor.byte_index(buffer);
        buffer.insert_str(byte, text);
        self.cursor.index += text.chars().count();
        self.mode = VimMode::Insert;
    }

    pub fn handle_key(&mut self, key: &KeyEvent, buffer: &mut String) -> VimKeyResult {
        if !self.enabled {
            return VimKeyResult::NotHandled;
        }
        self.cursor.clamp(buffer);

        match self.mode {
            VimMode::Insert => self.handle_insert(key, buffer),
            VimMode::Normal => self.handle_normal(key, buffer),
        }
    }

    fn handle_insert(&mut self, key: &KeyEvent, buffer: &mut String) -> VimKeyResult {
        match key.code {
            KeyCode::Esc => {
                self.enter_normal(buffer);
                VimKeyResult::Handled {
                    refresh_suggestions: false,
                    status: Some("-- NORMAL --".into()),
                }
            }
            KeyCode::Backspace => {
                if self.cursor.index == 0 {
                    return VimKeyResult::Handled {
                        refresh_suggestions: false,
                        status: None,
                    };
                }
                self.snapshot(buffer);
                let start = self.cursor.index - 1;
                let (s, e) = TextCursor::byte_range(buffer, start, self.cursor.index);
                buffer.replace_range(s..e, "");
                self.cursor.index = start;
                VimKeyResult::Handled {
                    refresh_suggestions: true,
                    status: None,
                }
            }
            KeyCode::Left => {
                self.cursor.move_left();
                VimKeyResult::Handled {
                    refresh_suggestions: false,
                    status: None,
                }
            }
            KeyCode::Right => {
                self.cursor.move_right(buffer);
                VimKeyResult::Handled {
                    refresh_suggestions: false,
                    status: None,
                }
            }
            KeyCode::Up => {
                self.cursor.move_up_line(buffer);
                VimKeyResult::Handled {
                    refresh_suggestions: false,
                    status: None,
                }
            }
            KeyCode::Down => {
                self.cursor.move_down_line(buffer);
                VimKeyResult::Handled {
                    refresh_suggestions: false,
                    status: None,
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.snapshot(buffer);
                let byte = self.cursor.byte_index(buffer);
                buffer.insert(byte, c);
                self.cursor.index += 1;
                VimKeyResult::Handled {
                    refresh_suggestions: true,
                    status: None,
                }
            }
            _ => VimKeyResult::NotHandled,
        }
    }

    fn handle_normal(&mut self, key: &KeyEvent, buffer: &mut String) -> VimKeyResult {
        let input = match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => c,
            KeyCode::Left => 'h',
            KeyCode::Right => 'l',
            KeyCode::Up => 'k',
            KeyCode::Down => 'j',
            _ => {
                return VimKeyResult::Handled {
                    refresh_suggestions: false,
                    status: None,
                };
            }
        };

        if let Some(pending) = self.pending.take() {
            return self.dispatch_pending(pending, input, buffer);
        }

        match input {
            'h' => {
                self.cursor.move_left();
                handled_none()
            }
            'l' => {
                self.cursor.move_right(buffer);
                handled_none()
            }
            'j' => {
                self.cursor.move_down_line(buffer);
                handled_none()
            }
            'k' => {
                self.cursor.move_up_line(buffer);
                handled_none()
            }
            '0' => {
                self.cursor.move_line_start(buffer);
                handled_none()
            }
            '$' => {
                self.cursor.move_line_end(buffer);
                handled_none()
            }
            'w' => {
                self.cursor.move_word_forward(buffer);
                handled_none()
            }
            'b' => {
                self.cursor.move_word_back(buffer);
                handled_none()
            }
            'e' => {
                self.cursor.move_word_end(buffer);
                handled_none()
            }
            'x' => {
                if self.cursor.index < TextCursor::len_chars(buffer) {
                    self.snapshot(buffer);
                    self.delete_range(buffer, self.cursor.index, self.cursor.index + 1);
                }
                handled_refresh()
            }
            'i' => self.enter_insert_at(self.cursor.index),
            'I' => {
                let ls = line_start(buffer, self.cursor.index);
                self.enter_insert_at(ls)
            }
            'a' => {
                let at = if self.cursor.index < TextCursor::len_chars(buffer) {
                    self.cursor.index + 1
                } else {
                    self.cursor.index
                };
                self.enter_insert_at(at)
            }
            'A' => {
                let le = line_end_exclusive(buffer, self.cursor.index);
                self.enter_insert_at(le)
            }
            'o' => self.open_line(buffer, true),
            'O' => self.open_line(buffer, false),
            'p' => self.paste_after(buffer),
            'P' => self.paste_before(buffer),
            'u' => self.undo(buffer),
            'd' => {
                self.pending = Some(NormalPending::OperatorLead(Operator::Delete));
                handled_none()
            }
            'c' => {
                self.pending = Some(NormalPending::OperatorLead(Operator::Change));
                handled_none()
            }
            'y' => {
                self.pending = Some(NormalPending::OperatorLead(Operator::Yank));
                handled_none()
            }
            _ => handled_none(),
        }
    }

    fn dispatch_pending(
        &mut self,
        pending: NormalPending,
        input: char,
        buffer: &mut String,
    ) -> VimKeyResult {
        match pending {
            NormalPending::OperatorLead(op) if input == op.lead_key() => {
                // `dd`, `cc`, `yy`
                match op {
                    Operator::Delete => self.delete_line(buffer),
                    Operator::Change => {
                        self.yank_line(buffer);
                        self.delete_line(buffer);
                        self.enter_insert_at(self.cursor.index)
                    }
                    Operator::Yank => {
                        self.yank_line(buffer);
                        handled_none()
                    }
                }
            }
            NormalPending::OperatorLead(op) => {
                self.pending = Some(NormalPending::Operator(op));
                self.dispatch_pending(NormalPending::Operator(op), input, buffer)
            }
            NormalPending::Operator(op) => match input {
                'w' => self.operator_to_word(buffer, op),
                '$' => self.operator_to_line_end(buffer, op),
                'h' => self.operator_char(buffer, op, false),
                'l' => self.operator_char(buffer, op, true),
                _ => {
                    self.pending = None;
                    handled_none()
                }
            },
        }
    }

    fn operator_to_word(&mut self, buffer: &mut String, op: Operator) -> VimKeyResult {
        let from = self.cursor.index;
        let to = next_word_char(buffer, from);
        self.apply_operator(buffer, op, from, to)
    }

    fn operator_to_line_end(&mut self, buffer: &mut String, op: Operator) -> VimKeyResult {
        let from = self.cursor.index;
        let to = line_end_exclusive(buffer, from);
        self.apply_operator(buffer, op, from, to)
    }

    fn operator_char(&mut self, buffer: &mut String, op: Operator, forward: bool) -> VimKeyResult {
        let (from, to) = if forward {
            (self.cursor.index, self.cursor.index + 1)
        } else if self.cursor.index > 0 {
            (self.cursor.index - 1, self.cursor.index)
        } else {
            return handled_none();
        };
        self.apply_operator(buffer, op, from, to)
    }

    fn apply_operator(
        &mut self,
        buffer: &mut String,
        op: Operator,
        from: usize,
        to: usize,
    ) -> VimKeyResult {
        if from >= to && op != Operator::Yank {
            return handled_none();
        }
        let slice = slice_chars(buffer, from, to);
        match op {
            Operator::Yank => {
                self.register = slice;
                handled_none()
            }
            Operator::Delete => {
                self.snapshot(buffer);
                self.delete_range(buffer, from, to);
                self.cursor.index = from;
                handled_refresh()
            }
            Operator::Change => {
                self.register = slice;
                self.snapshot(buffer);
                self.delete_range(buffer, from, to);
                self.cursor.index = from;
                self.enter_insert_at(from)
            }
        }
    }

    fn delete_line(&mut self, buffer: &mut String) -> VimKeyResult {
        let (from, to) = line_range(buffer, self.cursor.index);
        self.snapshot(buffer);
        self.delete_range(buffer, from, to);
        self.cursor.index = from.min(TextCursor::len_chars(buffer));
        handled_refresh()
    }

    fn yank_line(&mut self, buffer: &mut String) {
        let (from, to) = line_range(buffer, self.cursor.index);
        self.register = slice_chars(buffer, from, to);
    }

    fn open_line(&mut self, buffer: &mut String, below: bool) -> VimKeyResult {
        self.snapshot(buffer);
        let insert_at = if below {
            let end = line_end_exclusive(buffer, self.cursor.index);
            let byte = TextCursor::byte_range(buffer, end, end).0;
            buffer.insert(byte, '\n');
            end + 1
        } else {
            let start = line_start(buffer, self.cursor.index);
            let byte = TextCursor::byte_range(buffer, start, start).0;
            buffer.insert(byte, '\n');
            start
        };
        self.cursor.index = insert_at;
        self.mode = VimMode::Insert;
        VimKeyResult::Handled {
            refresh_suggestions: true,
            status: Some("-- INSERT --".into()),
        }
    }

    fn paste_after(&mut self, buffer: &mut String) -> VimKeyResult {
        if self.register.is_empty() {
            return handled_none();
        }
        self.snapshot(buffer);
        let byte = self.cursor.byte_index(buffer);
        buffer.insert_str(byte, &self.register);
        self.cursor.index += self.register.chars().count();
        handled_refresh()
    }

    fn paste_before(&mut self, buffer: &mut String) -> VimKeyResult {
        if self.register.is_empty() {
            return handled_none();
        }
        if self.cursor.index > 0 {
            self.cursor.index -= 1;
        }
        self.paste_after(buffer)
    }

    fn undo(&mut self, buffer: &mut String) -> VimKeyResult {
        if let Some(prev) = self.undo_stack.pop() {
            *buffer = prev;
            self.cursor.clamp(buffer);
            VimKeyResult::Handled {
                refresh_suggestions: true,
                status: Some("Annulé".into()),
            }
        } else {
            handled_none()
        }
    }

    fn enter_normal(&mut self, buffer: &str) {
        if self.cursor.index > 0 {
            let chars: Vec<char> = buffer.chars().collect();
            if self.cursor.index < chars.len() && chars[self.cursor.index] == '\n' {
                self.cursor.move_left();
            } else if self.cursor.index > 0 {
                let prev = chars[self.cursor.index - 1];
                if prev != '\n' {
                    self.cursor.move_left();
                }
            }
        }
        self.mode = VimMode::Normal;
        self.pending = None;
    }

    fn enter_insert_at(&mut self, index: usize) -> VimKeyResult {
        self.cursor.index = index;
        self.mode = VimMode::Insert;
        self.pending = None;
        VimKeyResult::Handled {
            refresh_suggestions: false,
            status: Some("-- INSERT --".into()),
        }
    }

    fn delete_range(&mut self, buffer: &mut String, from: usize, to: usize) {
        let (s, e) = TextCursor::byte_range(buffer, from, to);
        buffer.replace_range(s..e, "");
    }

    fn snapshot(&mut self, buffer: &str) {
        if self.undo_stack.len() >= MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(buffer.to_string());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimKeyResult {
    NotHandled,
    Handled {
        refresh_suggestions: bool,
        status: Option<String>,
    },
}

impl Operator {
    fn lead_key(self) -> char {
        match self {
            Self::Delete => 'd',
            Self::Change => 'c',
            Self::Yank => 'y',
        }
    }
}

fn handled_none() -> VimKeyResult {
    VimKeyResult::Handled {
        refresh_suggestions: false,
        status: None,
    }
}

fn handled_refresh() -> VimKeyResult {
    VimKeyResult::Handled {
        refresh_suggestions: true,
        status: None,
    }
}

fn line_start(text: &str, char_idx: usize) -> usize {
    let mut c = TextCursor { index: char_idx };
    c.move_line_start(text);
    c.index
}

fn line_end_exclusive(text: &str, char_idx: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = char_idx.min(len);
    while i < len && chars[i] != '\n' {
        i += 1;
    }
    i
}

fn line_range(text: &str, char_idx: usize) -> (usize, usize) {
    let start = line_start(text, char_idx);
    let mut end = line_end_exclusive(text, char_idx);
    if end < text.chars().count() && text.chars().nth(end) == Some('\n') {
        end += 1;
    }
    (start, end)
}

fn slice_chars(text: &str, from: usize, to: usize) -> String {
    text.chars().skip(from).take(to.saturating_sub(from)).collect()
}

fn next_word_char(text: &str, from: usize) -> usize {
    let mut c = TextCursor { index: from };
    c.move_word_forward(text);
    c.index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key_char(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty())
    }

    fn key_esc() -> KeyEvent {
        KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())
    }

    #[test]
    fn insert_and_normal_round_trip() {
        let mut vim = VimComposer::default();
        vim.set_enabled(true, "");
        let mut buf = String::new();
        assert!(matches!(
            vim.handle_key(&key_char('h'), &mut buf),
            VimKeyResult::Handled { .. }
        ));
        assert_eq!(buf, "h");
        assert!(matches!(vim.handle_key(&key_esc(), &mut buf), VimKeyResult::Handled { .. }));
        assert_eq!(vim.mode, VimMode::Normal);
    }

    #[test]
    fn dd_deletes_line() {
        let mut vim = VimComposer::default();
        let mut buf = "a\nb".to_string();
        vim.set_enabled(true, &buf);
        vim.mode = VimMode::Normal;
        vim.cursor.index = 2;
        let _ = vim.handle_key(&key_char('d'), &mut buf);
        let _ = vim.handle_key(&key_char('d'), &mut buf);
        assert_eq!(buf, "a\n");
    }
}
