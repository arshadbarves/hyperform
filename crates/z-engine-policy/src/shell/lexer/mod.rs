//! Quote-aware shell lexer. Splits a command line into simple commands at
//! `;`, `&`, `&&`, `|`, `||`, `|&`, newlines and parentheses, and records
//! words, redirects, heredocs, and substitutions. Nothing is executed or
//! expanded. Input it cannot follow clears `ok` so callers fail closed, while
//! the segments seen so far stay available to deny rules.

mod nested;
mod quoting;
mod redirect;
mod scanner;
mod words;

pub(crate) use scanner::lex;

#[cfg(test)]
mod tests;
