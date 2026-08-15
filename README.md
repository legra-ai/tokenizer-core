# tokenizer-core

[![Crates.io](https://img.shields.io/crates/v/tokenizer-core.svg)](https://crates.io/crates/tokenizer-core)
[![Downloads](https://img.shields.io/crates/d/tokenizer-core.svg)](https://crates.io/crates/tokenizer-core)
[![Documentation](https://docs.rs/tokenizer-core/badge.svg)](https://docs.rs/tokenizer-core)
[![CI/CD](https://github.com/legra-ai/tokenizer-core/actions/workflows/ci.yml/badge.svg)](https://github.com/legra-ai/tokenizer-core/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/legra-ai/tokenizer-core#license)

Generic zero-copy token infrastructure for [`logos`](https://crates.io/crates/logos)-based lexers.

## Why this crate?

Language-specific lexers usually repeat the same mechanics: run a Logos
lexer, preserve the original text, track byte spans, calculate source
positions, retain trivia, and report malformed input. `tokenizer-core` keeps
those mechanics in one small crate while leaving token definitions and grammar
rules to the language crate.

The crate provides:

- `Token<'src, K>` with borrowed text and a byte span;
- `TokenKind` for language-specific trivia and diagnostic names;
- `tokenize` for a common Logos scanning loop;
- `SourcePosition` and `SourceSpan` for source diagnostics;
- typed single and multiple tokenization errors.

## Example

```rust
use logos::Logos;
use tokenizer_core::{tokenize, TokenKind};

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

let tokens = tokenize::<Kind>("word word")?;
assert_eq!(tokens[0].text, "word");
assert_eq!(tokens[2].span, 5..9);
# Ok::<(), tokenizer_core::TokenizeError>(())
```

`Token` borrows from the input, so the source string must outlive the tokens.
The scanning loop retains whitespace and comments; callers can filter trivia
for semantic parsing while keeping every source fragment available for a
lossless syntax tree.

## Design boundary

This crate does not define a language grammar, AST, parser recovery policy, or
diagnostic format. It owns only reusable lexical infrastructure. A language
crate supplies its `Logos` token enum and implements `TokenKind`.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
