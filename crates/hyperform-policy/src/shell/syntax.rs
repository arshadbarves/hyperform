//! Shell syntax produced by the lexer: words, redirects, and segments
//! (simple commands between control operators).

/// One word after quote removal. `text` is the literal value only when
/// [`Word::expands`] is false; the flags record what the shell would still
/// expand at run time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Word {
    pub text: String,
    /// Some part was quoted or backslash-escaped.
    pub quoted: bool,
    /// Unquoted `*`, `?` or `[`: pathname expansion.
    pub glob: bool,
    /// Unquoted `{a,b}` or `{1..3}`: brace expansion.
    pub brace: bool,
    /// `$name`, `${..}`, `$(..)`, backticks, or arithmetic.
    pub param: bool,
    /// Unquoted leading `~`: tilde expansion.
    pub tilde: bool,
    /// `NAME=value` or `NAME+=value` (an assignment in command position).
    pub assignment: bool,
}

impl Word {
    /// The run-time value may differ from `text`.
    pub fn expands(&self) -> bool {
        self.glob || self.brace || self.param || self.tilde
    }

    /// The run-time value may become several words or unexpected flags.
    pub fn splits(&self) -> bool {
        self.glob || self.brace || self.param
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RedirectKind {
    /// `<` or `<&file`: reads a file.
    Read,
    /// `>`, `>>`, `>|`, `&>`, `&>>`, `<>` or `>&file`: opens a file for writing.
    Write,
    /// `N>&M`, `N<&M` or `N>&-`: duplicates or closes a descriptor.
    Duplicate,
    /// `<<`, `<<-` or `<<<`: inline input.
    Inline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Redirect {
    pub kind: RedirectKind,
    pub target: Word,
}

/// Write targets that are streams, not files.
const STREAMS: &[&str] = &["/dev/null", "/dev/stdout", "/dev/stderr"];

impl Redirect {
    pub fn writes_file(&self) -> bool {
        self.kind == RedirectKind::Write
            && (self.target.expands() || !STREAMS.contains(&self.target.text.as_str()))
    }
}

/// Reserved words and grouping that can precede the command name.
const LEADING_KEYWORDS: &[&str] = &[
    "!", "{", "}", "if", "then", "else", "elif", "fi", "while", "until", "do", "done", "esac",
    "time",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Segment {
    pub words: Vec<Word>,
    pub redirects: Vec<Redirect>,
}

impl Segment {
    /// The words from the command name on (`then rm x` gives `rm x`).
    pub fn command(&self) -> &[Word] {
        let mut words = self.words.as_slice();
        while let Some((first, rest)) = words.split_first() {
            if first.quoted || !LEADING_KEYWORDS.contains(&first.text.as_str()) {
                break;
            }
            words = match rest.split_first() {
                Some((flag, after)) if first.text == "time" && flag.text == "-p" => after,
                _ => rest,
            };
        }
        words
    }

    pub fn argv(&self) -> Vec<String> {
        self.command()
            .iter()
            .map(|word| word.text.clone())
            .collect()
    }

    pub fn write_targets(&self) -> impl Iterator<Item = &Word> {
        self.redirects
            .iter()
            .filter(|redirect| redirect.writes_file())
            .map(|redirect| &redirect.target)
    }
}
