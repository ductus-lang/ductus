//! Byte offsets ↔ LSP positions.
//!
//! Everything inside the server works in UTF-8 byte offsets, as tree-sitter
//! does. LSP positions count a line's characters in the encoding agreed at
//! initialization: UTF-8 bytes when the client offers it, otherwise UTF-16
//! code units, the protocol's default.

use lsp_types::{Position, PositionEncodingKind, Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16,
}

impl Encoding {
    /// UTF-8 when the client lists it, else the mandatory UTF-16.
    pub fn negotiate(offered: Option<&[PositionEncodingKind]>) -> Self {
        let utf8 = offered.is_some_and(|kinds| kinds.contains(&PositionEncodingKind::UTF8));
        if utf8 { Self::Utf8 } else { Self::Utf16 }
    }

    pub fn kind(self) -> PositionEncodingKind {
        match self {
            Self::Utf8 => PositionEncodingKind::UTF8,
            Self::Utf16 => PositionEncodingKind::UTF16,
        }
    }

    fn width(self, character: char) -> usize {
        match self {
            Self::Utf8 => character.len_utf8(),
            Self::Utf16 => character.len_utf16(),
        }
    }
}

/// The position of byte `offset`, which must lie on a character boundary.
pub fn position(text: &str, offset: usize, encoding: Encoding) -> Position {
    let before = &text[..offset];
    let line = before.matches('\n').count();
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let character: usize = before[line_start..]
        .chars()
        .map(|character| encoding.width(character))
        .sum();
    Position::new(line as u32, character as u32)
}

pub fn range(text: &str, start: usize, end: usize, encoding: Encoding) -> Range {
    Range::new(
        position(text, start, encoding),
        position(text, end, encoding),
    )
}

/// The byte offset of `position`. Positions past the end of a line clamp to
/// the line's end, and lines past the end of the text clamp to its end, as
/// the protocol asks.
pub fn offset(text: &str, position: Position, encoding: Encoding) -> usize {
    let mut line_start = 0;
    for _ in 0..position.line {
        match text[line_start..].find('\n') {
            Some(index) => line_start += index + 1,
            None => return text.len(),
        }
    }
    let line = &text[line_start..];
    let line = &line[..line.find('\n').unwrap_or(line.len())];
    let mut units = 0;
    for (index, character) in line.char_indices() {
        if units >= position.character as usize {
            return line_start + index;
        }
        units += encoding.width(character);
    }
    line_start + line.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(text: &str, encoding: Encoding) {
        for (offset, _) in text.char_indices().chain([(text.len(), ' ')]) {
            let position = position(text, offset, encoding);
            assert_eq!(
                super::offset(text, position, encoding),
                offset,
                "{position:?}"
            );
        }
    }

    #[test]
    fn ascii() {
        let text = "u = 100;\nv = 2;\n";
        assert_eq!(position(text, 13, Encoding::Utf16), Position::new(1, 4));
        round_trip(text, Encoding::Utf8);
        round_trip(text, Encoding::Utf16);
    }

    #[test]
    fn malayalam_counts_one_utf16_unit_and_three_bytes() {
        // "ത" is U+0D24: three UTF-8 bytes, one UTF-16 unit.
        let text = "# ത\nx = 1;\n";
        let after = text.find('\n').unwrap();
        assert_eq!(position(text, after, Encoding::Utf16), Position::new(0, 3));
        assert_eq!(position(text, after, Encoding::Utf8), Position::new(0, 5));
        round_trip(text, Encoding::Utf8);
        round_trip(text, Encoding::Utf16);
    }

    #[test]
    fn outside_the_bmp_takes_two_utf16_units() {
        let text = "# 𝒪 x\n";
        let x = text.find('x').unwrap();
        assert_eq!(position(text, x, Encoding::Utf16), Position::new(0, 5));
        round_trip(text, Encoding::Utf16);
    }

    #[test]
    fn clamps_past_line_and_text_end() {
        let text = "ab\ncd";
        assert_eq!(offset(text, Position::new(0, 10), Encoding::Utf16), 2);
        assert_eq!(offset(text, Position::new(5, 0), Encoding::Utf16), 5);
    }

    #[test]
    fn prefers_utf8_when_offered() {
        let offered = [PositionEncodingKind::UTF16, PositionEncodingKind::UTF8];
        assert_eq!(Encoding::negotiate(Some(&offered)), Encoding::Utf8);
        assert_eq!(Encoding::negotiate(None), Encoding::Utf16);
    }
}
