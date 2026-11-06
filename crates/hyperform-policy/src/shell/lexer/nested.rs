//! Nested commands: `$(..)`, `<(..)`, `>(..)`, backticks, and heredoc
//! bodies. Their segments go to `nested` and mark the command dynamic.

use super::super::syntax::Word;
use super::scanner::{Lexer, MAX_DEPTH};

impl Lexer {
    /// `$(..)`, `<(..)` or `>(..)`, entered after the opening parenthesis.
    pub(super) fn substitution(&mut self, word: &mut Word, sigil: char) {
        self.dynamic = true;
        word.param = true;
        let start = self.pos;
        if self.depth >= MAX_DEPTH {
            self.ok = false;
            self.skip_to_close();
        } else {
            self.depth += 1;
            let outer = std::mem::take(&mut self.heredocs);
            let segments = self.list(true);
            self.ok &= self.heredocs.is_empty();
            self.heredocs = outer;
            self.depth -= 1;
            self.nested.extend(segments);
        }
        word.text.push(sigil);
        word.text.push('(');
        let end = self.pos.min(self.chars.len());
        word.text.extend(&self.chars[start..end]);
    }

    fn skip_to_close(&mut self) {
        let mut open = 0usize;
        while let Some(c) = self.bump() {
            match c {
                '\\' => {
                    self.bump();
                }
                '\'' => while self.bump().is_some_and(|q| q != '\'') {},
                '(' => open += 1,
                ')' if open == 0 => return,
                ')' => open -= 1,
                _ => {}
            }
        }
    }

    /// `` `..` ``, entered after the opening backtick.
    pub(super) fn backtick(&mut self, word: &mut Word) {
        self.dynamic = true;
        word.param = true;
        let mut inner = String::new();
        loop {
            match self.bump() {
                Some('`') => break,
                Some('\\') => match self.bump() {
                    Some(c @ ('`' | '\\' | '$')) => inner.push(c),
                    Some(c) => {
                        inner.push('\\');
                        inner.push(c);
                    }
                    None => {
                        self.ok = false;
                        break;
                    }
                },
                Some(c) => inner.push(c),
                None => {
                    self.ok = false;
                    break;
                }
            }
        }
        word.text.push('`');
        word.text.push_str(&inner);
        word.text.push('`');
        if self.depth >= MAX_DEPTH {
            self.ok = false;
            return;
        }
        let mut sub = Lexer::new(&inner, self.depth + 1);
        let segments = sub.list(false);
        self.nested.extend(segments);
        self.absorb(sub);
    }

    /// Reads the bodies of heredocs opened on the line that just ended.
    pub(super) fn heredoc_bodies(&mut self) {
        for doc in std::mem::take(&mut self.heredocs) {
            let mut body = String::new();
            let mut closed = false;
            while self.pos < self.chars.len() {
                let end = self.chars[self.pos..]
                    .iter()
                    .position(|&c| c == '\n')
                    .map_or(self.chars.len(), |offset| self.pos + offset);
                let line: String = self.chars[self.pos..end].iter().collect();
                self.pos = (end + 1).min(self.chars.len());
                let candidate = if doc.strip_tabs {
                    line.trim_start_matches('\t')
                } else {
                    line.as_str()
                };
                if candidate == doc.delimiter {
                    closed = true;
                    break;
                }
                body.push_str(&line);
                body.push('\n');
            }
            self.ok &= closed;
            if doc.expands {
                self.body_expansions(&body);
            }
        }
    }

    /// Command substitutions inside an unquoted heredoc body.
    fn body_expansions(&mut self, body: &str) {
        if self.depth >= MAX_DEPTH {
            self.ok = false;
            return;
        }
        let mut sub = Lexer::new(body, self.depth + 1);
        let mut sink = Word::default();
        while let Some(c) = sub.peek() {
            match c {
                '\\' => sub.pos = (sub.pos + 2).min(sub.chars.len()),
                '$' => sub.dollar(&mut sink, true),
                '`' => {
                    sub.pos += 1;
                    sub.backtick(&mut sink);
                }
                _ => sub.pos += 1,
            }
        }
        self.absorb(sub);
    }

    fn absorb(&mut self, sub: Lexer) {
        self.ok &= sub.ok;
        self.dynamic |= sub.dynamic;
        self.nested.extend(sub.nested);
    }
}
