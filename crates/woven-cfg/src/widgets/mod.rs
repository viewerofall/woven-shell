//! Shared widget state types used across tabs.

/// Single-line text input state.
#[derive(Debug, Clone, Default)]
pub struct TextInput {
    pub value:  String,
    pub cursor: usize,
    pub focused: bool,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        let v = value.into();
        let c = v.len();
        Self { value: v, cursor: c, focused: false }
    }

    pub fn push(&mut self, ch: char) {
        let byte = self.cursor;
        self.value.insert(byte, ch);
        self.cursor += ch.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 { return; }
        let mut c = self.cursor;
        loop {
            c -= 1;
            if self.value.is_char_boundary(c) { break; }
        }
        self.value.remove(c);
        self.cursor = c;
    }

    pub fn delete_word(&mut self) {
        while self.cursor > 0 {
            let prev = self.value[..self.cursor].chars().next_back();
            if prev.map_or(false, |c| c == ' ') && self.cursor != self.value.len() { break; }
            self.backspace();
            if self.value[..self.cursor].ends_with(' ') { break; }
        }
    }
}

/// State for a keybind capture widget (one key combo slot).
#[derive(Debug, Clone, Default)]
pub struct KeyCapture {
    pub value:    String,
    pub capturing: bool,
}

impl KeyCapture {
    pub fn new(value: impl Into<String>) -> Self {
        Self { value: value.into(), capturing: false }
    }
}

/// Boolean toggle.
#[derive(Debug, Clone)]
pub struct Toggle {
    pub value: bool,
}

impl Toggle {
    pub fn new(v: bool) -> Self { Self { value: v } }
    pub fn flip(&mut self) { self.value = !self.value; }
}
