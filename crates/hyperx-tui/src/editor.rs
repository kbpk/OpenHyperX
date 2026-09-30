use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditorRecovery {
    pub text: String,
    pub row: usize,
    pub column: usize,
    pub single_line: bool,
    pub selected: bool,
}

/// Small Unicode-safe text editor. Content is only a draft until Ctrl+S parses it.
pub struct Editor {
    pub lines: Vec<String>,
    pub row: usize,
    pub column: usize,
    pub single_line: bool,
    pub selected: bool,
}

impl Editor {
    pub(crate) fn recovery(&self) -> EditorRecovery {
        EditorRecovery {
            text: self.text(),
            row: self.row,
            column: self.column,
            single_line: self.single_line,
            selected: self.selected,
        }
    }

    pub(crate) fn from_recovery(saved: EditorRecovery) -> anyhow::Result<Self> {
        anyhow::ensure!(
            saved.text.len() <= hyperx_app::MAX_PROFILE_BYTES,
            "local editor snapshot exceeds profile text limit"
        );
        anyhow::ensure!(
            !saved.single_line || !saved.text.contains('\n'),
            "single-line editor snapshot contains a newline"
        );
        let mut editor = Self::new(&saved.text, saved.single_line);
        anyhow::ensure!(
            saved.row < editor.lines.len()
                && saved.column <= editor.lines[saved.row].chars().count(),
            "local editor snapshot has an invalid cursor"
        );
        editor.row = saved.row;
        editor.column = saved.column;
        editor.selected = saved.selected;
        Ok(editor)
    }

    pub fn new(text: &str, single_line: bool) -> Self {
        Self {
            lines: text.split('\n').map(str::to_owned).collect(),
            row: 0,
            column: 0,
            single_line,
            selected: single_line && !text.is_empty(),
        }
    }
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }
    fn byte_index(&self) -> usize {
        self.lines[self.row]
            .char_indices()
            .nth(self.column)
            .map_or(self.lines[self.row].len(), |(index, _)| index)
    }
    fn length(&self) -> usize {
        self.lines[self.row].chars().count()
    }
    pub fn paste(&mut self, text: &str) {
        // Never insert terminal control sequences. Newlines remain meaningful
        // in TOML, but path/source-ID prompts are single-line.
        if (if self.selected { 0 } else { self.text().len() }).saturating_add(text.len())
            > hyperx_app::MAX_PROFILE_BYTES
        {
            return;
        }
        let cleaned: String = text
            .chars()
            .filter(|value| !value.is_control() || matches!(value, '\n' | '\t'))
            .filter(|value| !self.single_line || *value != '\n')
            .collect();
        if cleaned.is_empty() {
            return;
        }
        if self.selected {
            self.lines = vec![String::new()];
            self.row = 0;
            self.column = 0;
            self.selected = false;
        }
        // Splice once, rather than scanning character positions per pasted
        // codepoint (quadratic for long single-line text).
        let index = self.byte_index();
        let tail = self.lines[self.row].split_off(index);
        let prefix = std::mem::take(&mut self.lines[self.row]);
        let mut inserted: Vec<String> = cleaned.split('\n').map(str::to_owned).collect();
        let added_rows = inserted.len() - 1;
        let final_column = inserted.last().unwrap().chars().count();
        inserted[0].insert_str(0, &prefix);
        inserted.last_mut().unwrap().push_str(&tail);
        self.lines.splice(self.row..=self.row, inserted);
        self.row += added_rows;
        self.column = if added_rows == 0 {
            self.column + final_column
        } else {
            final_column
        };
    }
    fn newline(&mut self) {
        let index = self.byte_index();
        let next = self.lines[self.row].split_off(index);
        self.row += 1;
        self.lines.insert(self.row, next);
        self.column = 0;
    }
    pub fn key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('a') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.selected = true;
            return;
        }
        if self.selected && matches!(key.code, KeyCode::Backspace | KeyCode::Delete) {
            self.lines = vec![String::new()];
            self.row = 0;
            self.column = 0;
            self.selected = false;
            return;
        }
        if matches!(
            key.code,
            KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::Enter
        ) {
            self.selected = false;
        }
        match key.code {
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.paste(&character.to_string())
            }
            KeyCode::Enter
                if !self.single_line && self.text().len() < hyperx_app::MAX_PROFILE_BYTES =>
            {
                self.newline()
            }
            KeyCode::Tab if !self.single_line => self.paste("    "),
            KeyCode::Left if self.column > 0 => self.column -= 1,
            KeyCode::Left if self.row > 0 => {
                self.row -= 1;
                self.column = self.length();
            }
            KeyCode::Right if self.column < self.length() => self.column += 1,
            KeyCode::Right if self.row + 1 < self.lines.len() => {
                self.row += 1;
                self.column = 0;
            }
            KeyCode::Up => {
                self.row = self.row.saturating_sub(1);
                self.column = self.column.min(self.length());
            }
            KeyCode::Down => {
                self.row = (self.row + 1).min(self.lines.len() - 1);
                self.column = self.column.min(self.length());
            }
            KeyCode::Home => self.column = 0,
            KeyCode::End => self.column = self.length(),
            KeyCode::Backspace if self.column > 0 => {
                self.column -= 1;
                let index = self.byte_index();
                self.lines[self.row].remove(index);
            }
            KeyCode::Backspace if self.row > 0 => {
                let next = self.lines.remove(self.row);
                self.row -= 1;
                self.column = self.length();
                self.lines[self.row].push_str(&next);
            }
            KeyCode::Delete if self.column < self.length() => {
                let index = self.byte_index();
                self.lines[self.row].remove(index);
            }
            KeyCode::Delete if self.row + 1 < self.lines.len() => {
                let next = self.lines.remove(self.row + 1);
                self.lines[self.row].push_str(&next);
            }
            _ => {}
        }
    }
}
