use gtk::{gdk, prelude::*, SearchEntry};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    BeginningOfLine,
    EndOfLine,
    BackwardChar,
    ForwardChar,
    BackwardWord,
    ForwardWord,
    BackwardDeleteChar,
    DeleteChar,
    UnixWordRubout,
    BackwardKillWord,
    KillWord,
    UnixLineDiscard,
    KillLine,
    Yank,
    TransposeChars,
    PreviousHistory,
    NextHistory,
}

#[derive(Default)]
pub(crate) struct ReadlineEditor {
    kill_buffer: String,
    history_index: Option<usize>,
    history_draft: String,
}

impl ReadlineEditor {
    pub(crate) fn handle_key(
        &mut self,
        entry: &SearchEntry,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
        history: &[String],
    ) -> bool {
        let Some(command) = command_for_key(key, modifiers) else {
            self.reset_history_navigation();
            return false;
        };

        match command {
            Command::PreviousHistory => self.previous_history(entry, history),
            Command::NextHistory => self.next_history(entry, history),
            command => {
                self.reset_history_navigation();
                self.apply_edit(entry, command);
            }
        }
        true
    }

    fn apply_edit(&mut self, entry: &SearchEntry, command: Command) {
        let text = entry.text().to_string();
        let cursor = char_position(entry.position(), &text);
        let selection = entry
            .selection_bounds()
            .map(|(start, end)| (char_position(start, &text), char_position(end, &text)));
        let edited = edit(&text, cursor, selection, command, &mut self.kill_buffer);

        if edited.text != text {
            entry.set_text(&edited.text);
        }
        entry.set_position(edited.cursor as i32);
    }

    fn previous_history(&mut self, entry: &SearchEntry, history: &[String]) {
        if history.is_empty() {
            return;
        }

        let index = match self.history_index {
            Some(index) => (index + 1).min(history.len() - 1),
            None => {
                self.history_draft = entry.text().to_string();
                0
            }
        };
        self.history_index = Some(index);
        replace_text(entry, &history[index]);
    }

    fn next_history(&mut self, entry: &SearchEntry, history: &[String]) {
        let Some(index) = self.history_index else {
            return;
        };

        if index == 0 {
            self.history_index = None;
            replace_text(entry, &self.history_draft);
        } else {
            let next = index - 1;
            self.history_index = Some(next);
            if let Some(query) = history.get(next) {
                replace_text(entry, query);
            } else {
                self.reset_history_navigation();
            }
        }
    }

    fn reset_history_navigation(&mut self) {
        self.history_index = None;
        self.history_draft.clear();
    }
}

#[derive(Debug, Eq, PartialEq)]
struct EditedText {
    text: String,
    cursor: usize,
}

fn command_for_key(key: gdk::Key, modifiers: gdk::ModifierType) -> Option<Command> {
    let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
    let alt = modifiers.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::META_MASK);
    let key = key.to_lower();

    match (control, alt, key) {
        (true, false, gdk::Key::a) => Some(Command::BeginningOfLine),
        (true, false, gdk::Key::e) => Some(Command::EndOfLine),
        (true, false, gdk::Key::b) => Some(Command::BackwardChar),
        (true, false, gdk::Key::f) => Some(Command::ForwardChar),
        (true, false, gdk::Key::h) => Some(Command::BackwardDeleteChar),
        (true, false, gdk::Key::d) => Some(Command::DeleteChar),
        (true, false, gdk::Key::w) => Some(Command::UnixWordRubout),
        (true, false, gdk::Key::u) => Some(Command::UnixLineDiscard),
        (true, false, gdk::Key::k) => Some(Command::KillLine),
        (true, false, gdk::Key::y) => Some(Command::Yank),
        (true, false, gdk::Key::t) => Some(Command::TransposeChars),
        (true, false, gdk::Key::p) => Some(Command::PreviousHistory),
        (true, false, gdk::Key::n) => Some(Command::NextHistory),
        (false, true, gdk::Key::b) => Some(Command::BackwardWord),
        (false, true, gdk::Key::f) => Some(Command::ForwardWord),
        (false, true, gdk::Key::d) => Some(Command::KillWord),
        (false, true, gdk::Key::BackSpace) => Some(Command::BackwardKillWord),
        _ => None,
    }
}

fn edit(
    text: &str,
    cursor: usize,
    selection: Option<(usize, usize)>,
    command: Command,
    kill_buffer: &mut String,
) -> EditedText {
    let mut chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let selection = normalized_selection(selection, chars.len());
    let mut next_cursor = cursor;

    match command {
        Command::BeginningOfLine => next_cursor = 0,
        Command::EndOfLine => next_cursor = chars.len(),
        Command::BackwardChar => next_cursor = cursor.saturating_sub(1),
        Command::ForwardChar => next_cursor = (cursor + 1).min(chars.len()),
        Command::BackwardWord => next_cursor = backward_word_start(&chars, cursor),
        Command::ForwardWord => next_cursor = forward_word_end(&chars, cursor),
        Command::BackwardDeleteChar => {
            let range = selection.or_else(|| (cursor > 0).then_some((cursor - 1, cursor)));
            if let Some((start, end)) = range {
                chars.drain(start..end);
                next_cursor = start;
            }
        }
        Command::DeleteChar => {
            let range = selection
                .or_else(|| (cursor < chars.len()).then_some((cursor, cursor.saturating_add(1))));
            if let Some((start, end)) = range {
                chars.drain(start..end);
                next_cursor = start;
            }
        }
        Command::UnixWordRubout => {
            let start = backward_whitespace_word_start(&chars, cursor);
            kill(
                &mut chars,
                selection.unwrap_or((start, cursor)),
                kill_buffer,
            );
            next_cursor = selection.map_or(start, |(start, _)| start);
        }
        Command::BackwardKillWord => {
            let start = backward_word_start(&chars, cursor);
            kill(
                &mut chars,
                selection.unwrap_or((start, cursor)),
                kill_buffer,
            );
            next_cursor = selection.map_or(start, |(start, _)| start);
        }
        Command::KillWord => {
            let end = forward_word_end(&chars, cursor);
            let range = selection.unwrap_or((cursor, end));
            next_cursor = range.0;
            kill(&mut chars, range, kill_buffer);
        }
        Command::UnixLineDiscard => {
            let range = selection.unwrap_or((0, cursor));
            next_cursor = range.0;
            kill(&mut chars, range, kill_buffer);
        }
        Command::KillLine => {
            let range = selection.unwrap_or((cursor, chars.len()));
            next_cursor = range.0;
            kill(&mut chars, range, kill_buffer);
        }
        Command::Yank => {
            if !kill_buffer.is_empty() {
                let (start, end) = selection.unwrap_or((cursor, cursor));
                let inserted: Vec<char> = kill_buffer.chars().collect();
                let inserted_len = inserted.len();
                chars.splice(start..end, inserted);
                next_cursor = start + inserted_len;
            }
        }
        Command::TransposeChars => {
            if cursor > 0 && chars.len() > 1 {
                let right = if cursor == chars.len() {
                    cursor - 1
                } else {
                    cursor
                };
                chars.swap(right - 1, right);
                next_cursor = (right + 1).min(chars.len());
            }
        }
        Command::PreviousHistory | Command::NextHistory => unreachable!(),
    }

    EditedText {
        text: chars.into_iter().collect(),
        cursor: next_cursor,
    }
}

fn kill(chars: &mut Vec<char>, (start, end): (usize, usize), kill_buffer: &mut String) {
    if start >= end {
        return;
    }

    let killed: String = chars.drain(start..end).collect();
    *kill_buffer = killed;
}

fn normalized_selection(selection: Option<(usize, usize)>, len: usize) -> Option<(usize, usize)> {
    selection.and_then(|(start, end)| {
        let start = start.min(len);
        let end = end.min(len);
        let range = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        (range.0 != range.1).then_some(range)
    })
}

fn backward_word_start(chars: &[char], cursor: usize) -> usize {
    let mut index = cursor.min(chars.len());
    while index > 0 && !is_word_char(chars[index - 1]) {
        index -= 1;
    }
    while index > 0 && is_word_char(chars[index - 1]) {
        index -= 1;
    }
    index
}

fn backward_whitespace_word_start(chars: &[char], cursor: usize) -> usize {
    let mut index = cursor.min(chars.len());
    while index > 0 && chars[index - 1].is_whitespace() {
        index -= 1;
    }
    while index > 0 && !chars[index - 1].is_whitespace() {
        index -= 1;
    }
    index
}

fn forward_word_end(chars: &[char], cursor: usize) -> usize {
    let mut index = cursor.min(chars.len());
    while index < chars.len() && !is_word_char(chars[index]) {
        index += 1;
    }
    while index < chars.len() && is_word_char(chars[index]) {
        index += 1;
    }
    index
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn char_position(position: i32, text: &str) -> usize {
    usize::try_from(position)
        .unwrap_or_default()
        .min(text.chars().count())
}

fn replace_text(entry: &SearchEntry, text: &str) {
    entry.set_text(text);
    entry.set_position(text.chars().count() as i32);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str, cursor: usize, command: Command, kill_buffer: &mut String) -> EditedText {
        edit(text, cursor, None, command, kill_buffer)
    }

    #[test]
    fn maps_control_and_meta_bindings() {
        assert_eq!(
            command_for_key(gdk::Key::a, gdk::ModifierType::CONTROL_MASK),
            Some(Command::BeginningOfLine)
        );
        assert_eq!(
            command_for_key(gdk::Key::f, gdk::ModifierType::ALT_MASK),
            Some(Command::ForwardWord)
        );
        assert_eq!(
            command_for_key(gdk::Key::BackSpace, gdk::ModifierType::ALT_MASK),
            Some(Command::BackwardKillWord)
        );
        assert_eq!(
            command_for_key(gdk::Key::c, gdk::ModifierType::CONTROL_MASK),
            None
        );
    }

    #[test]
    fn moves_by_characters_and_words() {
        let mut kill_buffer = String::new();
        assert_eq!(
            apply(
                "hello, brave world",
                12,
                Command::BackwardWord,
                &mut kill_buffer
            )
            .cursor,
            7
        );
        assert_eq!(
            apply(
                "hello, brave world",
                5,
                Command::ForwardWord,
                &mut kill_buffer
            )
            .cursor,
            12
        );
        assert_eq!(
            apply("héllo", 2, Command::BackwardChar, &mut kill_buffer).cursor,
            1
        );
    }

    #[test]
    fn kills_and_yanks_text() {
        let mut kill_buffer = String::new();
        let killed = apply(
            "search some files",
            12,
            Command::UnixWordRubout,
            &mut kill_buffer,
        );
        assert_eq!(
            killed,
            EditedText {
                text: "search files".to_string(),
                cursor: 7
            }
        );
        assert_eq!(kill_buffer, "some ");

        let yanked = apply(&killed.text, killed.cursor, Command::Yank, &mut kill_buffer);
        assert_eq!(yanked.text, "search some files");
        assert_eq!(yanked.cursor, 12);
    }

    #[test]
    fn line_kills_respect_the_cursor() {
        let mut kill_buffer = String::new();
        let left = apply("alpha beta", 6, Command::UnixLineDiscard, &mut kill_buffer);
        assert_eq!(
            left,
            EditedText {
                text: "beta".to_string(),
                cursor: 0
            }
        );
        assert_eq!(kill_buffer, "alpha ");

        let right = apply("alpha beta", 5, Command::KillLine, &mut kill_buffer);
        assert_eq!(
            right,
            EditedText {
                text: "alpha".to_string(),
                cursor: 5
            }
        );
        assert_eq!(kill_buffer, " beta");
    }

    #[test]
    fn transpose_matches_readline_cursor_behavior() {
        let mut kill_buffer = String::new();
        assert_eq!(
            apply("abcd", 2, Command::TransposeChars, &mut kill_buffer),
            EditedText {
                text: "acbd".to_string(),
                cursor: 3
            }
        );
        assert_eq!(
            apply("abcd", 4, Command::TransposeChars, &mut kill_buffer),
            EditedText {
                text: "abdc".to_string(),
                cursor: 4
            }
        );
    }

    #[test]
    fn deletion_replaces_an_active_selection() {
        let mut kill_buffer = String::new();
        let edited = edit(
            "one two three",
            7,
            Some((4, 7)),
            Command::BackwardDeleteChar,
            &mut kill_buffer,
        );
        assert_eq!(
            edited,
            EditedText {
                text: "one  three".to_string(),
                cursor: 4
            }
        );
    }
}
