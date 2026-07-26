// SPDX-License-Identifier: MPL-2.0

use luna_theme::Rgba8;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::ops::Range;

/// One semantic syntax range supplied by a language service or lightweight scanner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxSpan {
    /// UTF-8 byte range in the document.
    pub range: Range<usize>,
    /// Product-neutral scope name such as `comment.line` or `keyword.control`.
    pub scope: String,
}

impl SyntaxSpan {
    /// Creates one syntax span.
    #[must_use]
    pub fn new(range: Range<usize>, scope: impl Into<String>) -> Self {
        Self {
            range,
            scope: scope.into(),
        }
    }
}

/// Immutable syntax-provider result for one document revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxSnapshot {
    /// Document revision used to produce these spans.
    pub document_revision: u64,
    /// Sorted non-overlapping syntax spans.
    pub spans: Vec<SyntaxSpan>,
}

impl SyntaxSnapshot {
    /// Creates and validates a syntax snapshot.
    pub fn new(
        text: &str,
        document_revision: u64,
        spans: impl IntoIterator<Item = SyntaxSpan>,
    ) -> Result<Self, SyntaxError> {
        let mut spans = spans.into_iter().collect::<Vec<_>>();
        spans.sort_by_key(|span| (span.range.start, span.range.end));
        validate_spans(text, &spans)?;
        Ok(Self {
            document_revision,
            spans,
        })
    }
}

/// Product-neutral syntax source.
pub trait SyntaxProvider: Send {
    /// Produces syntax scopes for one complete UTF-8 document revision.
    fn snapshot(
        &mut self,
        text: &str,
        document_revision: u64,
    ) -> Result<SyntaxSnapshot, SyntaxError>;
}

/// Lightweight deterministic scanner used by Luna's proof application.
///
/// This is not a Rust parser. It demonstrates the provider boundary with comments, quoted strings,
/// numbers, configured keywords, and leading-uppercase type-like identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeywordSyntaxProvider {
    keywords: Vec<String>,
    line_comment: String,
}

impl KeywordSyntaxProvider {
    /// Creates a scanner from a keyword list and line-comment prefix.
    #[must_use]
    pub fn new(
        keywords: impl IntoIterator<Item = impl Into<String>>,
        line_comment: impl Into<String>,
    ) -> Self {
        let mut keywords = keywords.into_iter().map(Into::into).collect::<Vec<_>>();
        keywords.sort();
        keywords.dedup();
        Self {
            keywords,
            line_comment: line_comment.into(),
        }
    }

    /// Practical Rust-like demonstration scanner.
    #[must_use]
    pub fn rust_demo() -> Self {
        Self::new(
            [
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
                "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
                "super", "trait", "true", "type", "unsafe", "use", "where", "while",
            ],
            "//",
        )
    }

    fn is_keyword(&self, identifier: &str) -> bool {
        self.keywords
            .binary_search_by(|keyword| keyword.as_str().cmp(identifier))
            .is_ok()
    }
}

impl SyntaxProvider for KeywordSyntaxProvider {
    fn snapshot(
        &mut self,
        text: &str,
        document_revision: u64,
    ) -> Result<SyntaxSnapshot, SyntaxError> {
        let mut spans = Vec::new();
        let bytes = text.as_bytes();
        let mut index = 0_usize;
        while index < bytes.len() {
            if !self.line_comment.is_empty()
                && text
                    .get(index..)
                    .is_some_and(|remaining| remaining.starts_with(&self.line_comment))
            {
                let end = text
                    .get(index..)
                    .and_then(|remaining| remaining.find('\n'))
                    .map_or(text.len(), |relative| index.saturating_add(relative));
                spans.push(SyntaxSpan::new(index..end, "comment.line"));
                index = end;
                continue;
            }
            let Some(character) = text
                .get(index..)
                .and_then(|remaining| remaining.chars().next())
            else {
                break;
            };
            if character == '"' {
                let start = index;
                index = index.saturating_add(character.len_utf8());
                let mut escaped = false;
                while index < text.len() {
                    let Some(next) = text
                        .get(index..)
                        .and_then(|remaining| remaining.chars().next())
                    else {
                        break;
                    };
                    index = index.saturating_add(next.len_utf8());
                    if escaped {
                        escaped = false;
                    } else if next == '\\' {
                        escaped = true;
                    } else if next == '"' {
                        break;
                    }
                }
                spans.push(SyntaxSpan::new(start..index, "string.quoted.double"));
                continue;
            }
            if character.is_ascii_digit() {
                let start = index;
                index = index.saturating_add(character.len_utf8());
                while index < text.len() {
                    let Some(next) = text
                        .get(index..)
                        .and_then(|remaining| remaining.chars().next())
                    else {
                        break;
                    };
                    if !(next.is_ascii_alphanumeric() || matches!(next, '_' | '.')) {
                        break;
                    }
                    index = index.saturating_add(next.len_utf8());
                }
                spans.push(SyntaxSpan::new(start..index, "constant.numeric"));
                continue;
            }
            if character == '_' || character.is_alphabetic() {
                let start = index;
                index = index.saturating_add(character.len_utf8());
                while index < text.len() {
                    let Some(next) = text
                        .get(index..)
                        .and_then(|remaining| remaining.chars().next())
                    else {
                        break;
                    };
                    if !(next == '_' || next.is_alphanumeric()) {
                        break;
                    }
                    index = index.saturating_add(next.len_utf8());
                }
                let identifier = text.get(start..index).unwrap_or_default();
                if self.is_keyword(identifier) {
                    spans.push(SyntaxSpan::new(start..index, "keyword.control"));
                } else if identifier.chars().next().is_some_and(char::is_uppercase) {
                    spans.push(SyntaxSpan::new(start..index, "entity.name.type"));
                }
                continue;
            }
            index = index.saturating_add(character.len_utf8());
        }
        SyntaxSnapshot::new(text, document_revision, spans)
    }
}

/// Visual attributes resolved for a syntax scope.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SyntaxStyle {
    /// Optional foreground override.
    pub foreground: Option<Rgba8>,
    /// Optional background override.
    pub background: Option<Rgba8>,
    /// Whether a renderer may request bold text.
    pub bold: bool,
    /// Whether a renderer may request italic text.
    pub italic: bool,
    /// Whether a renderer may underline the range.
    pub underline: bool,
}

impl SyntaxStyle {
    /// Creates a style containing only a foreground color.
    #[must_use]
    pub const fn foreground(color: Rgba8) -> Self {
        Self {
            foreground: Some(color),
            background: None,
            bold: false,
            italic: false,
            underline: false,
        }
    }

    /// Merges fields from `overlay` over this style.
    #[must_use]
    pub const fn merged(self, overlay: Self) -> Self {
        Self {
            foreground: match overlay.foreground {
                Some(value) => Some(value),
                None => self.foreground,
            },
            background: match overlay.background {
                Some(value) => Some(value),
                None => self.background,
            },
            bold: self.bold || overlay.bold,
            italic: self.italic || overlay.italic,
            underline: self.underline || overlay.underline,
        }
    }
}

/// One Sublime-compatible scope selector and its visual attributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRule {
    /// Comma-separated selector alternatives, stored individually.
    pub selectors: Vec<String>,
    /// Style applied when one selector matches.
    pub style: SyntaxStyle,
}

impl SyntaxRule {
    /// Creates a rule from one or more selector strings.
    #[must_use]
    pub fn new(selectors: impl IntoIterator<Item = impl Into<String>>, style: SyntaxStyle) -> Self {
        Self {
            selectors: selectors.into_iter().map(Into::into).collect(),
            style,
        }
    }
}

/// Product-neutral syntax palette imported from or exported to an editor theme format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTheme {
    /// Human-readable scheme name.
    pub name: String,
    /// Default editor background.
    pub background: Rgba8,
    /// Default editor foreground.
    pub foreground: Rgba8,
    /// Caret color.
    pub caret: Rgba8,
    /// Selection color.
    pub selection: Rgba8,
    /// Ordered scope rules.
    pub rules: Vec<SyntaxRule>,
}

impl SyntaxTheme {
    /// Resolves a scope using the longest matching selector prefix.
    #[must_use]
    pub fn style_for_scope(&self, scope: &str) -> SyntaxStyle {
        let mut result = SyntaxStyle::foreground(self.foreground);
        let mut best_specificity = 0_usize;
        for rule in &self.rules {
            for selector in &rule.selectors {
                let selector = selector.trim();
                if selector_matches(selector, scope) && selector.len() >= best_specificity {
                    best_specificity = selector.len();
                    result = SyntaxStyle::foreground(self.foreground).merged(rule.style);
                }
            }
        }
        result
    }

    /// Resolves every syntax span to concrete visual attributes.
    #[must_use]
    pub fn resolve(&self, snapshot: &SyntaxSnapshot) -> Vec<ResolvedSyntaxSpan> {
        snapshot
            .spans
            .iter()
            .map(|span| ResolvedSyntaxSpan {
                range: span.range.clone(),
                scope: span.scope.clone(),
                style: self.style_for_scope(&span.scope),
            })
            .collect()
    }
}

/// Concrete styled syntax span ready for shaping or rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSyntaxSpan {
    /// UTF-8 byte range.
    pub range: Range<usize>,
    /// Source semantic scope.
    pub scope: String,
    /// Resolved visual attributes.
    pub style: SyntaxStyle,
}

/// Syntax validation or provider failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxError {
    /// A span exceeded document bounds.
    OutOfBounds(Range<usize>),
    /// A span endpoint was not a UTF-8 character boundary.
    InvalidUtf8Boundary(Range<usize>),
    /// Two spans overlapped.
    OverlappingSpans {
        /// Earlier range.
        previous: Range<usize>,
        /// Later overlapping range.
        next: Range<usize>,
    },
}

impl Display for SyntaxError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds(range) => {
                write!(formatter, "syntax span {range:?} exceeds the document")
            }
            Self::InvalidUtf8Boundary(range) => {
                write!(formatter, "syntax span {range:?} splits a UTF-8 code point")
            }
            Self::OverlappingSpans { previous, next } => {
                write!(formatter, "syntax spans overlap: {previous:?} and {next:?}")
            }
        }
    }
}

impl Error for SyntaxError {}

fn validate_spans(text: &str, spans: &[SyntaxSpan]) -> Result<(), SyntaxError> {
    let mut previous: Option<&SyntaxSpan> = None;
    for span in spans {
        if span.range.start > span.range.end || span.range.end > text.len() {
            return Err(SyntaxError::OutOfBounds(span.range.clone()));
        }
        if !text.is_char_boundary(span.range.start) || !text.is_char_boundary(span.range.end) {
            return Err(SyntaxError::InvalidUtf8Boundary(span.range.clone()));
        }
        if let Some(previous) = previous
            && span.range.start < previous.range.end
        {
            return Err(SyntaxError::OverlappingSpans {
                previous: previous.range.clone(),
                next: span.range.clone(),
            });
        }
        previous = Some(span);
    }
    Ok(())
}

fn selector_matches(selector: &str, scope: &str) -> bool {
    if selector.is_empty() {
        return false;
    }
    scope == selector
        || scope
            .strip_prefix(selector)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::{KeywordSyntaxProvider, SyntaxProvider, SyntaxRule, SyntaxStyle, SyntaxTheme};
    use luna_theme::Rgba8;
    use std::error::Error;

    #[test]
    fn demo_scanner_reports_keywords_comments_strings_numbers_and_types()
    -> Result<(), Box<dyn Error>> {
        let text = "pub struct Cat { value: 42, name: \"Milo\" } // pet";
        let snapshot = KeywordSyntaxProvider::rust_demo().snapshot(text, 7)?;
        let scopes = snapshot
            .spans
            .iter()
            .map(|span| span.scope.as_str())
            .collect::<Vec<_>>();
        assert!(scopes.contains(&"keyword.control"));
        assert!(scopes.contains(&"entity.name.type"));
        assert!(scopes.contains(&"constant.numeric"));
        assert!(scopes.contains(&"string.quoted.double"));
        assert!(scopes.contains(&"comment.line"));
        Ok(())
    }

    #[test]
    fn longest_scope_selector_wins() {
        let theme = SyntaxTheme {
            name: "Test".to_owned(),
            background: Rgba8::opaque(0, 0, 0),
            foreground: Rgba8::opaque(200, 200, 200),
            caret: Rgba8::opaque(255, 255, 255),
            selection: Rgba8::opaque(30, 30, 30),
            rules: vec![
                SyntaxRule::new(
                    ["comment"],
                    SyntaxStyle::foreground(Rgba8::opaque(100, 100, 100)),
                ),
                SyntaxRule::new(
                    ["comment.line"],
                    SyntaxStyle::foreground(Rgba8::opaque(0, 200, 0)),
                ),
            ],
        };
        assert_eq!(
            theme
                .style_for_scope("comment.line.double-slash")
                .foreground,
            Some(Rgba8::opaque(0, 200, 0))
        );
    }
}
