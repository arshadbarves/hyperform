//! The lexer state, control operators, and word boundaries.

use super::super::syntax::{Segment, Word};
use super::words::plain;

/// Longer input is still lexed for deny rules but reported as unparsed.
const MAX_LEN: usize = 10_000;
/// Substitutions nested deeper than this are skipped instead of lexed.
pub(super) const MAX_DEPTH: usize = 32;

/// Lexer output. `nested` holds the commands inside `$(..)`, backticks,
/// `<(..)`, `>(..)` and expanding heredoc bodies, at any depth.
#[derive(Debug, Default)]
pub(crate) struct Lexed {
    pub segments: Vec<Segment>,
    pub nested: Vec<Segment>,
    pub ok: bool,
    pub dynamic: bool,
}

pub(crate) fn lex(input: &str) -> Lexed {
    let mut lexer = Lexer::new(input, 0);
    lexer.ok = input.len() <= MAX_LEN;
    let segments = lexer.list(false);
    Lexed {
        segments,
        nested: lexer.nested,
        ok: lexer.ok,
        dynamic: lexer.dynamic,
    }
}

#[derive(Debug)]
pub(super) struct Heredoc {
    pub(super) delimiter: String,
    pub(super) strip_tabs: bool,
    pub(super) expands: bool,
}

#[derive(Debug)]
pub(super) struct Lexer {
    pub(super) chars: Vec<char>,
    pub(super) pos: usize,
    pub(super) depth: usize,
    pub(super) ok: bool,
    pub(super) dynamic: bool,
    pub(super) nested: Vec<Segment>,
    pub(super) heredocs: Vec<Heredoc>,
    /// Position right after the last word, to spot `2>` descriptor prefixes.
    pub(super) word_end: Option<usize>,
}

impl Lexer {
    pub(super) fn new(input: &str, depth: usize) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            depth,
            ok: true,
            dynamic: false,
            nested: Vec::new(),
            heredocs: Vec::new(),
            word_end: None,
        }
    }

    pub(super) fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    pub(super) fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    pub(super) fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    /// Segments up to the end of input or, inside a substitution, up to and
    /// including its closing parenthesis.
    pub(super) fn list(&mut self, in_substitution: bool) -> Vec<Segment> {
        let mut segments = Vec::new();
        let mut current = Segment::default();
        let mut groups = 0usize;
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' => self.pos += 1,
                '\n' => {
                    self.pos += 1;
                    finish(&mut segments, &mut current);
                    self.heredoc_bodies();
                }
                '#' => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.pos += 1;
                    }
                }
                '&' if self.peek_at(1) == Some('>') => self.redirect(&mut current),
                ';' | '&' | '|' => {
                    self.pos += 1;
                    finish(&mut segments, &mut current);
                }
                '(' => {
                    self.pos += 1;
                    groups += 1;
                    finish(&mut segments, &mut current);
                }
                ')' => {
                    self.pos += 1;
                    finish(&mut segments, &mut current);
                    if groups > 0 {
                        groups -= 1;
                    } else if in_substitution {
                        return segments;
                    } else {
                        self.ok = false;
                    }
                }
                '<' | '>' if self.peek_at(1) != Some('(') => self.redirect(&mut current),
                _ => {
                    if let Some(word) = self.word() {
                        current.words.push(word);
                        self.word_end = Some(self.pos);
                    }
                }
            }
        }
        finish(&mut segments, &mut current);
        if in_substitution || !self.heredocs.is_empty() {
            self.ok = false;
            self.heredocs.clear();
        }
        segments
    }

    /// One word; `None` when only a line continuation was consumed.
    pub(super) fn word(&mut self) -> Option<Word> {
        let mut word = Word::default();
        let mut started = false;
        let mut brace_at = None;
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | '\n' | ';' | '&' | '|' | '(' | ')' => break,
                '<' | '>' if self.peek_at(1) != Some('(') => break,
                '<' | '>' => {
                    self.pos += 2;
                    self.substitution(&mut word, c);
                }
                '\'' => {
                    self.pos += 1;
                    self.single_quoted(&mut word);
                }
                '"' => {
                    self.pos += 1;
                    self.double_quoted(&mut word);
                }
                '\\' => {
                    self.pos += 1;
                    match self.bump() {
                        Some('\n') => continue,
                        Some(escaped) => {
                            word.text.push(escaped);
                            word.quoted = true;
                        }
                        None => word.text.push('\\'),
                    }
                }
                '$' => self.dollar(&mut word, false),
                '`' => {
                    self.pos += 1;
                    self.backtick(&mut word);
                }
                _ => {
                    self.pos += 1;
                    plain(&mut word, c, started, &mut brace_at);
                }
            }
            started = true;
        }
        // Words reach programs as C strings, so a decoded NUL ends the word.
        if let Some(nul) = word.text.find('\0') {
            word.text.truncate(nul);
        }
        started.then_some(word)
    }
}

fn finish(segments: &mut Vec<Segment>, current: &mut Segment) {
    if !current.words.is_empty() || !current.redirects.is_empty() {
        segments.push(std::mem::take(current));
    }
}
