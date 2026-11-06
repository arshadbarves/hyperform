//! Quotes and `$` expansions inside a word.

use super::super::syntax::Word;
use super::scanner::Lexer;

impl Lexer {
    pub(super) fn single_quoted(&mut self, word: &mut Word) {
        word.quoted = true;
        loop {
            match self.bump() {
                Some('\'') => return,
                Some(c) => word.text.push(c),
                None => {
                    self.ok = false;
                    return;
                }
            }
        }
    }

    pub(super) fn double_quoted(&mut self, word: &mut Word) {
        word.quoted = true;
        loop {
            match self.peek() {
                None => {
                    self.ok = false;
                    return;
                }
                Some('"') => {
                    self.pos += 1;
                    return;
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.bump() {
                        Some('\n') => {}
                        Some(c @ ('$' | '`' | '"' | '\\')) => word.text.push(c),
                        Some(c) => {
                            word.text.push('\\');
                            word.text.push(c);
                        }
                        None => {
                            self.ok = false;
                            return;
                        }
                    }
                }
                Some('$') => self.dollar(word, true),
                Some('`') => {
                    self.pos += 1;
                    self.backtick(word);
                }
                Some(c) => {
                    self.pos += 1;
                    word.text.push(c);
                }
            }
        }
    }

    /// A `$` construct; `in_double` disables `$'..'` and `$".."`.
    pub(super) fn dollar(&mut self, word: &mut Word, in_double: bool) {
        self.pos += 1;
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                self.substitution(word, '$');
            }
            Some('{') => {
                self.pos += 1;
                self.parameter(word);
            }
            Some('\'') if !in_double => {
                self.pos += 1;
                self.ansi_c(word);
            }
            Some('"') if !in_double => {
                self.pos += 1;
                self.double_quoted(word);
            }
            Some('[') => {
                // `$[..]` is deprecated arithmetic that bash still evaluates.
                word.param = true;
                self.dynamic = true;
                word.text.push('$');
            }
            Some(c) if c == '_' || c.is_ascii_alphabetic() => {
                word.param = true;
                word.text.push('$');
                while let Some(n) = self
                    .peek()
                    .filter(|n| *n == '_' || n.is_ascii_alphanumeric())
                {
                    word.text.push(n);
                    self.pos += 1;
                }
            }
            Some(c) if c.is_ascii_digit() || "@*#?$!-".contains(c) => {
                word.param = true;
                word.text.push('$');
                word.text.push(c);
                self.pos += 1;
            }
            _ => word.text.push('$'),
        }
    }

    /// `${..}`, entered after the opening brace.
    fn parameter(&mut self, word: &mut Word) {
        word.param = true;
        word.text.push_str("${");
        let mut depth = 1usize;
        while depth > 0 {
            match self.peek() {
                None => {
                    self.ok = false;
                    return;
                }
                Some('\'') => {
                    self.pos += 1;
                    self.single_quoted(word);
                }
                Some('"') => {
                    self.pos += 1;
                    self.double_quoted(word);
                }
                Some('$') => self.dollar(word, true),
                Some('`') => {
                    self.pos += 1;
                    self.backtick(word);
                }
                Some('\\') => {
                    self.pos += 1;
                    word.text.extend(self.bump());
                }
                Some(c) => {
                    self.pos += 1;
                    match c {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    word.text.push(c);
                }
            }
        }
    }

    /// `$'..'`, entered after the opening quote. Escapes are decoded the way
    /// bash does, so `$'\x72m'` is seen as `rm`.
    fn ansi_c(&mut self, word: &mut Word) {
        word.quoted = true;
        loop {
            match self.bump() {
                Some('\'') => return,
                Some('\\') => self.ansi_escape(word),
                Some(c) => word.text.push(c),
                None => {
                    self.ok = false;
                    return;
                }
            }
        }
    }

    fn ansi_escape(&mut self, word: &mut Word) {
        let Some(c) = self.bump() else {
            self.ok = false;
            return;
        };
        let decoded = match c {
            'n' => Some('\n'),
            't' => Some('\t'),
            'r' => Some('\r'),
            'a' => Some('\u{7}'),
            'b' => Some('\u{8}'),
            'e' | 'E' => Some('\u{1b}'),
            'f' => Some('\u{c}'),
            'v' => Some('\u{b}'),
            'x' => self.code_point(16, 2),
            'u' => self.code_point(16, 4),
            'U' => self.code_point(16, 8),
            '0'..='7' => {
                self.pos -= 1;
                // Bash keeps only the low byte of an octal escape.
                self.code_point(8, 3)
                    .map(|c| char::from((u32::from(c) & 0xff) as u8))
            }
            'c' => self.bump().map(|n| char::from((u32::from(n) & 0x1f) as u8)),
            '\\' | '\'' | '"' | '?' => Some(c),
            other => {
                word.text.push('\\');
                Some(other)
            }
        };
        word.text.extend(decoded);
    }

    /// Up to `max` digits in `radix`; `None` when there are none.
    fn code_point(&mut self, radix: u32, max: usize) -> Option<char> {
        let mut value = 0u32;
        let mut digits = 0;
        while digits < max {
            let Some(digit) = self.peek().and_then(|c| c.to_digit(radix)) else {
                break;
            };
            value = value * radix + digit;
            self.pos += 1;
            digits += 1;
        }
        if digits == 0 {
            return None;
        }
        char::from_u32(value)
    }
}
