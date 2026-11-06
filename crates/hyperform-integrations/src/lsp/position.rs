//! Positions as the model sees them (1-based line and column, columns
//! counted in characters) and as language servers see them (0-based line,
//! column in UTF-16 code units, the only encoding this client offers).
//! Lines end at `\n`; a trailing `\r` belongs to the terminator.

use serde::{Deserialize, Serialize};

/// A 0-based LSP position in UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

/// Text of 0-based line `index`, without its terminator.
pub fn line_text(text: &str, index: usize) -> Option<&str> {
    text.split('\n')
        .nth(index)
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

/// UTF-16 offset of the first `chars` characters of `line` (clamped to the
/// line's end).
pub fn utf16_offset(line: &str, chars: usize) -> u32 {
    let units: usize = line.chars().take(chars).map(char::len_utf16).sum();
    u32::try_from(units).unwrap_or(u32::MAX)
}

/// 0-based character index of UTF-16 offset `units` in `line`. An offset
/// inside a surrogate pair maps to that character; offsets past the end
/// clamp to the line's length.
pub fn char_index(line: &str, units: u32) -> usize {
    let units = units as usize;
    let mut consumed = 0;
    for (index, c) in line.chars().enumerate() {
        if consumed >= units {
            return index;
        }
        consumed += c.len_utf16();
        if consumed > units {
            return index;
        }
    }
    line.chars().count()
}

/// Model position (1-based line and character column) to LSP. `None` when
/// the line is 0 or past the end of `text`; columns past the line's end
/// clamp to it.
pub fn to_lsp(text: &str, line: u32, column: u32) -> Option<LspPosition> {
    let index = line.checked_sub(1)?;
    let line_str = line_text(text, index as usize)?;
    Some(LspPosition {
        line: index,
        character: utf16_offset(line_str, column.saturating_sub(1) as usize),
    })
}

/// LSP position to model `(line, column)`, both 1-based, columns in
/// characters. A line past the end of `text` keeps its raw offset.
pub fn from_lsp(text: &str, position: LspPosition) -> (u32, u32) {
    let column = match line_text(text, position.line as usize) {
        Some(line) => u32::try_from(char_index(line, position.character)).unwrap_or(u32::MAX),
        None => position.character,
    };
    (position.line.saturating_add(1), column.saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "fn main() {\r\n    let s = \"héllo 🦀 wörld\"; greet(s);\n}\n";

    fn lsp(line: u32, character: u32) -> LspPosition {
        LspPosition { line, character }
    }

    #[test]
    fn ascii_positions_shift_by_one() {
        assert_eq!(to_lsp(TEXT, 1, 4), Some(lsp(0, 3)));
        assert_eq!(from_lsp(TEXT, lsp(0, 3)), (1, 4));
        assert_eq!(line_text(TEXT, 0), Some("fn main() {"));
    }

    #[test]
    fn multibyte_text_converts_both_ways() {
        let line = line_text(TEXT, 1).unwrap();
        let greet_chars = line.chars().position(|c| c == 'g').unwrap();
        // 'é' and 'ö' are one UTF-16 unit each, the crab is two.
        let expected_units = u32::try_from(greet_chars).unwrap() + 1;
        let column = u32::try_from(greet_chars).unwrap() + 1;
        assert_eq!(to_lsp(TEXT, 2, column), Some(lsp(1, expected_units)));
        assert_eq!(from_lsp(TEXT, lsp(1, expected_units)), (2, column));
        let crab = line.chars().position(|c| c == '🦀').unwrap();
        let crab_units = utf16_offset(line, crab);
        assert_eq!(char_index(line, crab_units + 1), crab, "inside the pair");
        assert_eq!(char_index(line, crab_units + 2), crab + 1);
    }

    #[test]
    fn every_character_round_trips() {
        let line = line_text(TEXT, 1).unwrap();
        for chars in 0..=line.chars().count() {
            let units = utf16_offset(line, chars);
            assert_eq!(char_index(line, units), chars);
            let column = u32::try_from(chars).unwrap() + 1;
            let position = to_lsp(TEXT, 2, column).unwrap();
            assert_eq!(from_lsp(TEXT, position), (2, column));
        }
    }

    #[test]
    fn out_of_range_inputs_clamp_or_fail() {
        assert_eq!(to_lsp(TEXT, 0, 1), None);
        assert_eq!(to_lsp(TEXT, 9, 1), None);
        assert_eq!(to_lsp(TEXT, 1, 500), Some(lsp(0, 11)));
        assert_eq!(to_lsp(TEXT, 1, 0), Some(lsp(0, 0)));
        assert_eq!(from_lsp(TEXT, lsp(0, 500)), (1, 12));
        assert_eq!(from_lsp(TEXT, lsp(40, 7)), (41, 8));
        assert_eq!(
            to_lsp(TEXT, 4, 1),
            Some(lsp(3, 0)),
            "the empty last line exists"
        );
    }
}
