//! Curseur texte et motions vim simplifiées (leak : `Cursor.ts`, `motions.ts`).

/// Position en indices **caractères** (pas octets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCursor {
    pub index: usize,
}

impl TextCursor {
    #[must_use]
    pub fn at_end(text: &str) -> Self {
        Self {
            index: text.chars().count(),
        }
    }

    #[must_use]
    pub fn len_chars(text: &str) -> usize {
        text.chars().count()
    }

    pub fn clamp(&mut self, text: &str) {
        self.index = self.index.min(Self::len_chars(text));
    }

    pub fn move_left(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    pub fn move_right(&mut self, text: &str) {
        if self.index < Self::len_chars(text) {
            self.index += 1;
        }
    }

    pub fn move_line_start(&mut self, text: &str) {
        self.index = line_start_char(text, self.index);
    }

    pub fn move_line_end(&mut self, text: &str) {
        self.index = line_end_char(text, self.index);
    }

    pub fn move_up_line(&mut self, text: &str) {
        self.index = prev_line_char(text, self.index);
    }

    pub fn move_down_line(&mut self, text: &str) {
        self.index = next_line_char(text, self.index);
    }

    pub fn move_word_forward(&mut self, text: &str) {
        self.index = next_word_char(text, self.index);
    }

    pub fn move_word_back(&mut self, text: &str) {
        self.index = prev_word_char(text, self.index);
    }

    pub fn move_word_end(&mut self, text: &str) {
        self.index = word_end_char(text, self.index);
    }

    /// Octet de début du caractère sous le curseur.
    #[must_use]
    pub fn byte_index(&self, text: &str) -> usize {
        text.char_indices()
            .nth(self.index)
            .map(|(i, _)| i)
            .unwrap_or_else(|| text.len())
    }

    /// Plage octets [start, end) pour une plage de caractères [a, b).
    #[must_use]
    pub fn byte_range(text: &str, start_char: usize, end_char: usize) -> (usize, usize) {
        let start = text
            .char_indices()
            .nth(start_char)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        let end = text
            .char_indices()
            .nth(end_char)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        (start, end)
    }
}

#[must_use]
fn line_start_char(text: &str, char_idx: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = char_idx.min(chars.len());
    while i > 0 && chars[i - 1] != '\n' {
        i -= 1;
    }
    i
}

#[must_use]
fn line_end_char(text: &str, char_idx: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = char_idx.min(len);
    while i < len && chars[i] != '\n' {
        i += 1;
    }
    if i > 0 && (i == len || chars[i] == '\n') {
        i -= 1;
    }
    i
}

#[must_use]
fn prev_line_char(text: &str, char_idx: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    if char_idx == 0 {
        return 0;
    }
    let col = char_idx - line_start_char(text, char_idx);
    let mut line_start = char_idx;
    while line_start > 0 && chars[line_start - 1] != '\n' {
        line_start -= 1;
    }
    if line_start == 0 {
        return 0;
    }
    let prev_line_start = line_start - 1;
    let mut ps = prev_line_start;
    while ps > 0 && chars[ps - 1] != '\n' {
        ps -= 1;
    }
    let prev_line_len = prev_line_start - ps;
    ps + col.min(prev_line_len.saturating_sub(1))
}

#[must_use]
fn next_line_char(text: &str, char_idx: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if char_idx >= len {
        return len;
    }
    let col = char_idx - line_start_char(text, char_idx);
    let mut i = char_idx;
    while i < len && chars[i] != '\n' {
        i += 1;
    }
    if i >= len {
        return len;
    }
  // début ligne suivante
    let next_start = i + 1;
    let mut next_end = next_start;
    while next_end < len && chars[next_end] != '\n' {
        next_end += 1;
    }
    let next_len = next_end - next_start;
    if next_len == 0 {
        return next_start;
    }
    next_start + col.min(next_len - 1)
}

#[must_use]
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[must_use]
fn next_word_char(text: &str, from: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = from.min(len);
    if i < len && is_word_char(chars[i]) {
        while i < len && is_word_char(chars[i]) {
            i += 1;
        }
    }
    while i < len && !is_word_char(chars[i]) {
        i += 1;
    }
    i
}

#[must_use]
fn prev_word_char(text: &str, from: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    if from == 0 {
        return 0;
    }
    let mut i = from.min(chars.len()).saturating_sub(1);
    while i > 0 && !is_word_char(chars[i]) {
        i -= 1;
    }
    while i > 0 && is_word_char(chars[i - 1]) {
        i -= 1;
    }
    i
}

#[must_use]
fn word_end_char(text: &str, from: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = from.min(len.saturating_sub(1));
    if i < len && !is_word_char(chars[i]) {
        i = next_word_char(text, i).min(len.saturating_sub(1));
    }
    while i + 1 < len && is_word_char(chars[i + 1]) {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_motions() {
        let text = "ab\ncd";
        let mut c = TextCursor { index: 4 }; // 'd'
        c.move_line_start(text);
        assert_eq!(c.index, 3); // 'c' — début de la 2e ligne
        c.move_line_end(text);
        assert_eq!(c.index, 4); // 'd'
    }

    #[test]
    fn word_forward() {
        let text = "foo bar";
        let mut c = TextCursor { index: 0 };
        c.move_word_forward(text);
        assert_eq!(c.index, 4); // début de « bar »
    }
}
