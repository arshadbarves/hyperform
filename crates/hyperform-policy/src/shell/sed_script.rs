//! The `sed` script grammar, read the GNU way (which sees at least as many
//! commands as BSD sed). Only commands that print, delete, substitute, or
//! transliterate are accepted; `w`, `W`, `r`, `R`, `e`, the `s///w` and
//! `s///e` flags, and constructs GNU and BSD read differently are rejected.

pub(super) fn script_is_safe(script: &str) -> bool {
    let chars: Vec<char> = script.chars().collect();
    Script {
        chars: &chars,
        at: 0,
    }
    .commands()
}

struct Script<'a> {
    chars: &'a [char],
    at: usize,
}

impl Script<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        Some(c)
    }

    fn skip(&mut self, keep: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&keep) {
            self.at += 1;
        }
    }

    fn skip_blanks(&mut self) {
        self.skip(|c| c == ' ' || c == '\t');
    }

    fn commands(&mut self) -> bool {
        loop {
            self.skip(|c| c.is_whitespace() || c == ';');
            let Some(first) = self.peek() else {
                return true;
            };
            if first == '#' {
                self.skip(|c| c != '\n');
                continue;
            }
            if !self.address() {
                return false;
            }
            self.skip_blanks();
            if self.peek() == Some(',') {
                self.at += 1;
                self.skip_blanks();
                if !self.address() {
                    return false;
                }
            }
            self.skip_blanks();
            while self.peek() == Some('!') {
                self.at += 1;
                self.skip_blanks();
            }
            let ok = match self.next() {
                Some('{') => continue,
                Some(
                    'p' | 'P' | 'd' | 'D' | 'n' | 'N' | '=' | 'g' | 'G' | 'h' | 'H' | 'x' | 'z'
                    | 'F' | '}',
                ) => true,
                Some('l' | 'q' | 'Q') => {
                    self.skip_blanks();
                    self.skip(|c| c.is_ascii_digit());
                    true
                }
                Some('b' | 't' | 'T' | ':') => {
                    self.skip(|c| c != ';' && c != '\n');
                    true
                }
                Some('a' | 'i' | 'c') => {
                    self.skip(|c| c != '\n');
                    true
                }
                Some('s') => self.substitution(),
                Some('y') => self.transliteration(),
                _ => false,
            };
            self.skip_blanks();
            if !ok || !matches!(self.peek(), None | Some(';' | '\n' | '}' | '#')) {
                return false;
            }
        }
    }

    fn address(&mut self) -> bool {
        match self.peek() {
            Some(c) if c.is_ascii_digit() => {
                self.skip(|c| c.is_ascii_digit());
                if self.peek() == Some('~') {
                    self.at += 1;
                    self.skip(|c| c.is_ascii_digit());
                }
                true
            }
            Some('+' | '~') => {
                self.at += 1;
                self.skip(|c| c.is_ascii_digit());
                true
            }
            Some('$') => {
                self.at += 1;
                true
            }
            Some('/') => {
                self.at += 1;
                self.regex('/') && self.address_flags()
            }
            Some('\\') => {
                self.at += 1;
                match self.next() {
                    Some(delimiter) if usable_delimiter(delimiter) => {
                        self.regex(delimiter) && self.address_flags()
                    }
                    _ => false,
                }
            }
            _ => true,
        }
    }

    fn address_flags(&mut self) -> bool {
        self.skip(|c| c == 'I' || c == 'M');
        true
    }

    fn substitution(&mut self) -> bool {
        let Some(delimiter) = self.next().filter(|&c| usable_delimiter(c)) else {
            return false;
        };
        if !self.regex(delimiter) || !self.literal(delimiter) {
            return false;
        }
        // `w` (write a file) and `e` (execute) are not listed, so they end the
        // flags and fail the command-terminator check.
        self.skip(|c| matches!(c, 'g' | 'p' | 'i' | 'I' | 'm' | 'M') || c.is_ascii_digit());
        true
    }

    fn transliteration(&mut self) -> bool {
        match self.next().filter(|&c| usable_delimiter(c)) {
            Some(delimiter) => self.literal(delimiter) && self.literal(delimiter),
            None => false,
        }
    }

    /// A regular expression up to `delimiter`. A bracket expression holding
    /// the delimiter is rejected: GNU sed ends the regex there, BSD does not.
    fn regex(&mut self, delimiter: char) -> bool {
        loop {
            match self.next() {
                None | Some('\n') => return false,
                Some(c) if c == delimiter => return true,
                Some('\\') => {
                    if self.next().is_none() {
                        return false;
                    }
                }
                Some('[') => {
                    if !self.bracket(delimiter) {
                        return false;
                    }
                }
                Some(_) => {}
            }
        }
    }

    fn bracket(&mut self, delimiter: char) -> bool {
        if self.peek() == Some('^') {
            self.at += 1;
        }
        if self.peek() == Some(']') {
            self.at += 1;
        }
        loop {
            match self.next() {
                None | Some('\n') => return false,
                Some(c) if c == delimiter => return false,
                Some(']') => return true,
                Some('[') if matches!(self.peek(), Some(':' | '=' | '.')) => {
                    let kind = self.next();
                    loop {
                        match self.next() {
                            None | Some('\n') => return false,
                            Some(c) if c == delimiter => return false,
                            Some(c) if Some(c) == kind && self.peek() == Some(']') => {
                                self.at += 1;
                                break;
                            }
                            Some(_) => {}
                        }
                    }
                }
                Some(_) => {}
            }
        }
    }

    /// A replacement or `y` part up to `delimiter`.
    fn literal(&mut self, delimiter: char) -> bool {
        loop {
            match self.next() {
                None | Some('\n') => return false,
                Some(c) if c == delimiter => return true,
                Some('\\') => {
                    if self.next().is_none() {
                        return false;
                    }
                }
                Some(_) => {}
            }
        }
    }
}

fn usable_delimiter(c: char) -> bool {
    !matches!(c, '\\' | '\n' | '[' | ']')
}
