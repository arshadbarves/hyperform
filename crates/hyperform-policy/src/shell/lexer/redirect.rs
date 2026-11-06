//! Redirection operators and their targets.

use super::super::syntax::{Redirect, RedirectKind, Segment};
use super::scanner::{Heredoc, Lexer};
use super::words::{is_descriptor, names_descriptor};

#[derive(Debug, Clone, Copy)]
enum Operator {
    Read,
    Write,
    DuplicateOut,
    DuplicateIn,
    Heredoc { strip_tabs: bool },
    HereString,
}

impl Lexer {
    /// A redirection starting at `<`, `>` or `&>`.
    pub(super) fn redirect(&mut self, segment: &mut Segment) {
        if self.word_end == Some(self.pos) && segment.words.last().is_some_and(names_descriptor) {
            segment.words.pop();
        }
        let operator = self.operator();
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.pos += 1;
        }
        let Some(target) = self.word() else {
            self.ok = false;
            return;
        };
        let kind = match operator {
            Operator::Read => RedirectKind::Read,
            Operator::Write => RedirectKind::Write,
            Operator::DuplicateOut | Operator::DuplicateIn if is_descriptor(&target) => {
                RedirectKind::Duplicate
            }
            Operator::DuplicateOut => RedirectKind::Write,
            Operator::DuplicateIn => RedirectKind::Read,
            Operator::Heredoc { strip_tabs } => {
                self.heredocs.push(Heredoc {
                    delimiter: target.text.clone(),
                    strip_tabs,
                    expands: !target.quoted,
                });
                RedirectKind::Inline
            }
            Operator::HereString => RedirectKind::Inline,
        };
        segment.redirects.push(Redirect { kind, target });
    }

    fn operator(&mut self) -> Operator {
        match self.bump() {
            Some('&') => {
                self.pos += 1;
                if self.peek() == Some('>') {
                    self.pos += 1;
                }
                Operator::Write
            }
            Some('>') => match self.peek() {
                Some('>' | '|') => {
                    self.pos += 1;
                    Operator::Write
                }
                Some('&') => {
                    self.pos += 1;
                    Operator::DuplicateOut
                }
                _ => Operator::Write,
            },
            _ => match self.peek() {
                Some('<') => {
                    self.pos += 1;
                    match self.peek() {
                        Some('<') => {
                            self.pos += 1;
                            Operator::HereString
                        }
                        Some('-') => {
                            self.pos += 1;
                            Operator::Heredoc { strip_tabs: true }
                        }
                        _ => Operator::Heredoc { strip_tabs: false },
                    }
                }
                Some('>') => {
                    self.pos += 1;
                    Operator::Write
                }
                Some('&') => {
                    self.pos += 1;
                    Operator::DuplicateIn
                }
                _ => Operator::Read,
            },
        }
    }
}
