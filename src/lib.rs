#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
//! Generic zero-copy token infrastructure for Logos lexers.
//!
//! The crate owns token storage, source spans, position calculation, and the
//! common scanning loop. A language-specific crate supplies only its Logos
//! token enum and its [`TokenKind`] implementation.

use std::fmt;
use std::ops::Range;

/// A single token produced by a lexer.
///
/// The token borrows its matched text from the original source and stores the
/// corresponding byte range. No token text is copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'src, K> {
    /// The language-specific token kind.
    pub kind: K,
    /// The half-open byte range covered by this token.
    pub span: Range<usize>,
    /// The matched slice of the source text.
    pub text: &'src str,
}

/// The language-specific behavior required by [`tokenize`].
pub trait TokenKind: Copy + Eq + fmt::Debug {
    /// Returns whether this token is whitespace or a comment.
    fn is_trivia(self) -> bool;

    /// Returns a human-readable token name for diagnostics.
    fn description(self) -> &'static str;
}

/// A source position measured in Unicode scalar-value columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    /// The one-based line number.
    pub line: usize,
    /// The one-based column number.
    pub column: usize,
    /// The zero-based byte offset.
    pub offset: usize,
}

impl fmt::Display for SourcePosition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}, column {}", self.line, self.column)
    }
}

/// A half-open source span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    /// The inclusive start position.
    pub start: SourcePosition,
    /// The exclusive end position.
    pub end: SourcePosition,
}

impl SourceSpan {
    /// Creates a span from an inclusive start and exclusive end position.
    #[must_use]
    pub const fn new(start: SourcePosition, end: SourcePosition) -> Self {
        Self { start, end }
    }

    /// Creates a zero-width span at a position.
    #[must_use]
    pub const fn point(position: SourcePosition) -> Self {
        Self::new(position, position)
    }
}

impl fmt::Display for SourceSpan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start.line == self.end.line && self.start.column + 1 == self.end.column {
            write!(formatter, "{}", self.start)
        } else if self.start.line == self.end.line {
            write!(
                formatter,
                "line {}, columns {}-{}",
                self.start.line,
                self.start.column,
                self.end.column.saturating_sub(1)
            )
        } else {
            write!(formatter, "{} to {}", self.start, self.end)
        }
    }
}

/// An error produced while scanning source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenizeError {
    /// The lexer encountered a source range matching no token pattern.
    UnexpectedCharacter {
        /// The invalid source range.
        span: SourceSpan,
    },
    /// The lexer encountered a Unicode character commonly confused with an
    /// ASCII operator.
    InvalidUnicodeCharacter {
        /// The invalid source range.
        span: SourceSpan,
    },
    /// Multiple errors were collected while scanning the source.
    Multiple {
        /// The individual errors in source order.
        errors: Vec<Self>,
    },
}

impl TokenizeError {
    /// Returns the first source span carried by this error, if any.
    #[must_use]
    pub fn source_span(&self) -> Option<SourceSpan> {
        match self {
            Self::UnexpectedCharacter { span } | Self::InvalidUnicodeCharacter { span } => {
                Some(*span)
            }
            Self::Multiple { errors } => errors.iter().find_map(Self::source_span),
        }
    }
}

impl fmt::Display for TokenizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedCharacter { span } => write!(formatter, "unexpected input at {span}"),
            Self::InvalidUnicodeCharacter { span } => {
                write!(formatter, "invalid Unicode character at {span}")
            }
            Self::Multiple { errors } => {
                write!(formatter, "{} tokenization errors: ", errors.len())?;
                for (index, error) in errors.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str("; ")?;
                    }
                    write!(formatter, "{error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for TokenizeError {}

/// Tokenizes source text with a Logos-generated lexer.
///
/// Whitespace and comments are retained so callers can construct a lossless
/// syntax tree. All successful tokens borrow directly from `source`.
///
/// # Errors
///
/// Returns [`TokenizeError`] when one or more source ranges match no token
/// pattern.
pub fn tokenize<'src, K>(source: &'src str) -> Result<Vec<Token<'src, K>>, TokenizeError>
where
    K: logos::Logos<'src, Source = str, Error = ()> + TokenKind,
    K::Extras: Default,
{
    let mut lexer = K::lexer(source);
    let mut tokens = Vec::new();
    let mut errors = Vec::new();

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        if let Ok(kind) = result {
            tokens.push(Token {
                kind,
                span: span.clone(),
                text: &source[span],
            });
        } else {
            let source_span = offsets_to_span(source, span.start, span.end);
            let error = match source[span.clone()].chars().next() {
                Some(character) if is_confusable_operator_char(character) => {
                    TokenizeError::InvalidUnicodeCharacter { span: source_span }
                }
                _ => TokenizeError::UnexpectedCharacter { span: source_span },
            };
            errors.push(error);
        }
    }

    match errors.len() {
        0 => Ok(tokens),
        1 => Err(errors.remove(0)),
        _ => Err(TokenizeError::Multiple { errors }),
    }
}

/// Converts a byte offset to a one-based line and Unicode scalar-value column.
#[must_use]
pub fn offset_to_position(source: &str, offset: usize) -> SourcePosition {
    let before = &source[..offset.min(source.len())];
    let line = before
        .chars()
        .filter(|&character| character == '\n')
        .count()
        + 1;
    let last_newline = before.rfind('\n').map_or(0, |position| position + 1);
    let column = before[last_newline..].chars().count() + 1;
    SourcePosition {
        line,
        column,
        offset,
    }
}

/// Converts a byte-offset range to a [`SourceSpan`].
#[must_use]
pub fn offsets_to_span(source: &str, start: usize, end: usize) -> SourceSpan {
    SourceSpan::new(
        offset_to_position(source, start),
        offset_to_position(source, end),
    )
}

const fn is_confusable_operator_char(character: char) -> bool {
    matches!(
        character,
        '\u{2010}'
            | '\u{2011}'
            | '\u{2012}'
            | '\u{2013}'
            | '\u{2014}'
            | '\u{2015}'
            | '\u{2212}'
            | '\u{FE58}'
            | '\u{FE63}'
            | '\u{FF0D}'
    )
}

#[cfg(test)]
mod tests {
    use logos::Logos;

    use super::*;

    #[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
    enum Kind {
        #[token("word")]
        Word,
        #[regex(r"[ \t\n]+")]
        Whitespace,
    }

    impl TokenKind for Kind {
        fn is_trivia(self) -> bool {
            matches!(self, Self::Whitespace)
        }

        fn description(self) -> &'static str {
            match self {
                Self::Word => "word",
                Self::Whitespace => "whitespace",
            }
        }
    }

    #[test]
    fn tokenizes_borrowed_text_and_spans() {
        let source = "word word";
        let tokens = tokenize::<Kind>(source).expect("source should tokenize");
        assert_eq!(tokens[0].text, "word");
        assert_eq!(tokens[0].span, 0..4);
        assert_eq!(tokens[2].span, 5..9);
    }

    #[test]
    fn reports_multiple_errors() {
        let error = tokenize::<Kind>("? word !").expect_err("invalid characters must fail");
        let TokenizeError::Multiple { errors } = error else {
            panic!("expected multiple errors");
        };
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn classifies_confusable_operator_characters() {
        let error = tokenize::<Kind>("−").expect_err("invalid Unicode must fail");
        assert!(matches!(
            error,
            TokenizeError::InvalidUnicodeCharacter { .. }
        ));
    }

    #[test]
    fn calculates_unicode_columns() {
        assert_eq!(
            offset_to_position("é\nword", 3),
            SourcePosition {
                line: 2,
                column: 1,
                offset: 3,
            }
        );
    }
}
