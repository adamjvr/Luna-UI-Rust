// SPDX-License-Identifier: MPL-2.0

//! Product-neutral document search mechanics for Luna applications.
//!
//! This crate owns reusable matching, scoped result production, replacement planning, and request
//! identity. It deliberately does not own project inclusion policy, file traversal, language
//! semantics, command naming, editor-panel geometry, or Moth Text product behavior.

use luna_core::{CodedError, ErrorCode};
use regex::{Regex, RegexBuilder};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::ops::Range;

mod async_search;

pub use async_search::{AsyncSearchResponse, AsyncSearchWorker};

/// Default deterministic maximum number of matches returned by one request.
pub const DEFAULT_MAX_MATCHES: usize = 100_000;

/// Stable identity for one search request.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SearchRequestId(u64);

impl SearchRequestId {
    /// Creates a request identity.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric request identity.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Matching interpretation for a search pattern.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SearchMode {
    /// Treat the pattern as ordinary text.
    #[default]
    Literal,
    /// Interpret the pattern using Rust's Unicode-aware regular-expression syntax.
    Regex,
}

/// Complete product-neutral matching specification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchSpec {
    pattern: String,
    mode: SearchMode,
    case_sensitive: bool,
    whole_word: bool,
    scope: Option<Range<usize>>,
    max_matches: usize,
}

impl SearchSpec {
    /// Creates a matching specification.
    #[must_use]
    pub fn new(pattern: impl Into<String>, mode: SearchMode) -> Self {
        Self {
            pattern: pattern.into(),
            mode,
            case_sensitive: false,
            whole_word: false,
            scope: None,
            max_matches: DEFAULT_MAX_MATCHES,
        }
    }

    /// Creates a literal matching specification.
    #[must_use]
    pub fn literal(pattern: impl Into<String>) -> Self {
        Self::new(pattern, SearchMode::Literal)
    }

    /// Creates a regular-expression matching specification.
    #[must_use]
    pub fn regex(pattern: impl Into<String>) -> Self {
        Self::new(pattern, SearchMode::Regex)
    }

    /// Returns the source pattern.
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Returns the matching mode.
    #[must_use]
    pub const fn mode(&self) -> SearchMode {
        self.mode
    }

    /// Returns whether matching distinguishes letter case.
    #[must_use]
    pub const fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Returns whether matches must occupy complete identifier words.
    #[must_use]
    pub const fn whole_word(&self) -> bool {
        self.whole_word
    }

    /// Returns the optional UTF-8 byte scope.
    #[must_use]
    pub fn scope(&self) -> Option<Range<usize>> {
        self.scope.clone()
    }

    /// Returns the maximum number of matches.
    #[must_use]
    pub const fn max_matches(&self) -> usize {
        self.max_matches
    }

    /// Sets case-sensitive matching.
    #[must_use]
    pub const fn with_case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.case_sensitive = case_sensitive;
        self
    }

    /// Sets complete-identifier-word matching.
    #[must_use]
    pub const fn with_whole_word(mut self, whole_word: bool) -> Self {
        self.whole_word = whole_word;
        self
    }

    /// Restricts matching to one UTF-8 byte scope.
    #[must_use]
    pub fn with_scope(mut self, scope: Range<usize>) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Sets a deterministic result cap.
    #[must_use]
    pub const fn with_max_matches(mut self, max_matches: usize) -> Self {
        self.max_matches = max_matches;
        self
    }
}

/// One versioned search request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRequest {
    id: SearchRequestId,
    source_revision: u64,
    spec: SearchSpec,
}

impl SearchRequest {
    /// Creates a request for one source revision.
    #[must_use]
    pub const fn new(id: SearchRequestId, source_revision: u64, spec: SearchSpec) -> Self {
        Self {
            id,
            source_revision,
            spec,
        }
    }

    /// Returns the request identity.
    #[must_use]
    pub const fn id(&self) -> SearchRequestId {
        self.id
    }

    /// Returns the searched source revision.
    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    /// Returns the matching specification.
    #[must_use]
    pub const fn spec(&self) -> &SearchSpec {
        &self.spec
    }
}

/// One matched UTF-8 byte range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchMatch {
    range: Range<usize>,
}

impl SearchMatch {
    /// Creates one matched range.
    #[must_use]
    pub const fn new(range: Range<usize>) -> Self {
        Self { range }
    }

    /// Returns the matched byte range.
    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
}

/// Complete result for one search request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchResult {
    request_id: SearchRequestId,
    source_revision: u64,
    matches: Vec<SearchMatch>,
    was_truncated: bool,
}

impl SearchResult {
    /// Returns the originating request identity.
    #[must_use]
    pub const fn request_id(&self) -> SearchRequestId {
        self.request_id
    }

    /// Returns the searched source revision.
    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    /// Returns ordered non-overlapping matches.
    #[must_use]
    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    /// Returns copied byte ranges for editor selection and paint integration.
    #[must_use]
    pub fn ranges(&self) -> Vec<Range<usize>> {
        self.matches.iter().map(SearchMatch::range).collect()
    }

    /// Returns whether the request hit its deterministic result cap.
    #[must_use]
    pub const fn was_truncated(&self) -> bool {
        self.was_truncated
    }
}

/// One replacement edit in pre-edit UTF-8 byte coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementEdit {
    range: Range<usize>,
    replacement: String,
}

impl ReplacementEdit {
    /// Creates a replacement edit.
    #[must_use]
    pub fn new(range: Range<usize>, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }

    /// Returns the replaced byte range.
    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    /// Returns the inserted text.
    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

/// Ordered replacement plan produced from one search request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementPlan {
    request_id: SearchRequestId,
    source_revision: u64,
    edits: Vec<ReplacementEdit>,
    was_truncated: bool,
}

impl ReplacementPlan {
    /// Returns the originating request identity.
    #[must_use]
    pub const fn request_id(&self) -> SearchRequestId {
        self.request_id
    }

    /// Returns the searched source revision.
    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    /// Returns edits in ascending pre-edit document order.
    #[must_use]
    pub fn edits(&self) -> &[ReplacementEdit] {
        &self.edits
    }

    /// Returns whether the request hit its deterministic result cap.
    #[must_use]
    pub const fn was_truncated(&self) -> bool {
        self.was_truncated
    }

    /// Applies this plan from the end of the document toward the beginning.
    pub fn apply(&self, text: &str) -> Result<String, SearchError> {
        validate_replacement_edits(text, &self.edits)?;
        let mut result = text.to_owned();
        for edit in self.edits.iter().rev() {
            result.replace_range(edit.range.clone(), &edit.replacement);
        }
        Ok(result)
    }
}

/// Product-neutral matching and replacement-planning boundary.
pub trait SearchProvider {
    /// Executes one search request.
    fn search(&self, text: &str, request: &SearchRequest) -> Result<SearchResult, SearchError>;

    /// Builds a replacement plan for every accepted match.
    fn replacement_plan(
        &self,
        text: &str,
        request: &SearchRequest,
        replacement: &str,
    ) -> Result<ReplacementPlan, SearchError>;
}

/// Unicode-aware literal and regular-expression provider.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RegexSearchProvider;

impl SearchProvider for RegexSearchProvider {
    fn search(&self, text: &str, request: &SearchRequest) -> Result<SearchResult, SearchError> {
        if request.spec.pattern.is_empty() || request.spec.max_matches == 0 {
            return Ok(SearchResult {
                request_id: request.id,
                source_revision: request.source_revision,
                matches: Vec::new(),
                was_truncated: false,
            });
        }

        let scope = validated_scope(text, &request.spec)?;
        let matcher = compile(&request.spec)?;
        let haystack = text
            .get(scope.clone())
            .ok_or_else(|| invalid_scope_error(&scope, text.len()))?;
        let mut matches = Vec::new();
        let mut was_truncated = false;

        for found in matcher.find_iter(haystack) {
            let range =
                scope.start.saturating_add(found.start())..scope.start.saturating_add(found.end());
            if request.spec.whole_word && !has_identifier_boundaries(text, &range) {
                continue;
            }
            if matches.len() >= request.spec.max_matches {
                was_truncated = true;
                break;
            }
            matches.push(SearchMatch::new(range));
        }

        Ok(SearchResult {
            request_id: request.id,
            source_revision: request.source_revision,
            matches,
            was_truncated,
        })
    }

    fn replacement_plan(
        &self,
        text: &str,
        request: &SearchRequest,
        replacement: &str,
    ) -> Result<ReplacementPlan, SearchError> {
        if request.spec.pattern.is_empty() || request.spec.max_matches == 0 {
            return Ok(ReplacementPlan {
                request_id: request.id,
                source_revision: request.source_revision,
                edits: Vec::new(),
                was_truncated: false,
            });
        }

        let scope = validated_scope(text, &request.spec)?;
        let matcher = compile(&request.spec)?;
        let haystack = text
            .get(scope.clone())
            .ok_or_else(|| invalid_scope_error(&scope, text.len()))?;
        let mut edits = Vec::new();
        let mut was_truncated = false;

        for captures in matcher.captures_iter(haystack) {
            let Some(found) = captures.get(0) else {
                continue;
            };
            let range =
                scope.start.saturating_add(found.start())..scope.start.saturating_add(found.end());
            if request.spec.whole_word && !has_identifier_boundaries(text, &range) {
                continue;
            }
            if edits.len() >= request.spec.max_matches {
                was_truncated = true;
                break;
            }

            let inserted = match request.spec.mode {
                SearchMode::Literal => replacement.to_owned(),
                SearchMode::Regex => {
                    let mut expanded = String::new();
                    captures.expand(replacement, &mut expanded);
                    expanded
                }
            };
            edits.push(ReplacementEdit::new(range, inserted));
        }

        validate_replacement_edits(text, &edits)?;
        Ok(ReplacementPlan {
            request_id: request.id,
            source_revision: request.source_revision,
            edits,
            was_truncated,
        })
    }
}

/// Tracks the latest asynchronous search request and rejects stale results.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchCoordinator {
    next_request_id: u64,
    active: Option<(SearchRequestId, u64)>,
}

impl SearchCoordinator {
    /// Creates a coordinator whose first request identity is one.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_request_id: 1,
            active: None,
        }
    }

    /// Begins and records the latest request.
    pub fn begin(&mut self, source_revision: u64, spec: SearchSpec) -> SearchRequest {
        let id = SearchRequestId::new(self.next_request_id);
        self.next_request_id = self.next_request_id.checked_add(1).unwrap_or(1);
        self.active = Some((id, source_revision));
        SearchRequest::new(id, source_revision, spec)
    }

    /// Cancels acceptance of the active request.
    pub fn cancel(&mut self) {
        self.active = None;
    }

    /// Returns whether a request identity and revision are still active.
    #[must_use]
    pub fn accepts_identity(&self, request_id: SearchRequestId, source_revision: u64) -> bool {
        self.active == Some((request_id, source_revision))
    }

    /// Returns whether a result belongs to the latest active request and revision.
    #[must_use]
    pub fn accepts(&self, result: &SearchResult) -> bool {
        self.accepts_identity(result.request_id, result.source_revision)
    }
}

/// Search compilation, scope, or replacement validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchError {
    /// A regular-expression pattern could not be compiled.
    InvalidPattern {
        /// Provider diagnostic.
        message: String,
    },
    /// A requested scope is outside the source or splits a UTF-8 scalar value.
    InvalidScope {
        /// Requested start offset.
        start: usize,
        /// Requested end offset.
        end: usize,
        /// Source byte length.
        text_len: usize,
    },
    /// A replacement edit is outside the source or splits a UTF-8 scalar value.
    InvalidReplacementRange {
        /// Requested start offset.
        start: usize,
        /// Requested end offset.
        end: usize,
        /// Source byte length.
        text_len: usize,
    },
    /// Replacement edits overlap in pre-edit coordinates.
    OverlappingReplacementRanges {
        /// End of the preceding edit.
        previous_end: usize,
        /// Start of the following edit.
        next_start: usize,
    },
}

impl Display for SearchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPattern { message } => {
                write!(formatter, "invalid regular expression: {message}")
            }
            Self::InvalidScope {
                start,
                end,
                text_len,
            } => write!(
                formatter,
                "search scope {start}..{end} is invalid for {text_len} UTF-8 bytes"
            ),
            Self::InvalidReplacementRange {
                start,
                end,
                text_len,
            } => write!(
                formatter,
                "replacement range {start}..{end} is invalid for {text_len} UTF-8 bytes"
            ),
            Self::OverlappingReplacementRanges {
                previous_end,
                next_start,
            } => write!(
                formatter,
                "replacement ranges overlap at {previous_end} and {next_start}"
            ),
        }
    }
}

impl Error for SearchError {}

impl CodedError for SearchError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::InvalidPattern { .. } => "search.pattern.invalid",
            Self::InvalidScope { .. } => "search.scope.invalid",
            Self::InvalidReplacementRange { .. } => "search.replacement.range_invalid",
            Self::OverlappingReplacementRanges { .. } => "search.replacement.ranges_overlap",
        })
    }
}

fn compile(spec: &SearchSpec) -> Result<Regex, SearchError> {
    let pattern = match spec.mode {
        SearchMode::Literal => regex::escape(&spec.pattern),
        SearchMode::Regex => spec.pattern.clone(),
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!spec.case_sensitive)
        .build()
        .map_err(|error| SearchError::InvalidPattern {
            message: error.to_string(),
        })
}

fn validated_scope(text: &str, spec: &SearchSpec) -> Result<Range<usize>, SearchError> {
    let scope = spec.scope.clone().unwrap_or(0..text.len());
    if scope.start > scope.end
        || scope.end > text.len()
        || !text.is_char_boundary(scope.start)
        || !text.is_char_boundary(scope.end)
    {
        return Err(invalid_scope_error(&scope, text.len()));
    }
    Ok(scope)
}

fn invalid_scope_error(scope: &Range<usize>, text_len: usize) -> SearchError {
    SearchError::InvalidScope {
        start: scope.start,
        end: scope.end,
        text_len,
    }
}

fn has_identifier_boundaries(text: &str, range: &Range<usize>) -> bool {
    let before_is_word = text
        .get(..range.start)
        .and_then(|prefix| prefix.chars().next_back())
        .is_some_and(is_identifier_character);
    let after_is_word = text
        .get(range.end..)
        .and_then(|suffix| suffix.chars().next())
        .is_some_and(is_identifier_character);
    !before_is_word && !after_is_word
}

fn is_identifier_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn validate_replacement_edits(text: &str, edits: &[ReplacementEdit]) -> Result<(), SearchError> {
    let mut previous_end = 0;
    for (index, edit) in edits.iter().enumerate() {
        if edit.range.start > edit.range.end
            || edit.range.end > text.len()
            || !text.is_char_boundary(edit.range.start)
            || !text.is_char_boundary(edit.range.end)
        {
            return Err(SearchError::InvalidReplacementRange {
                start: edit.range.start,
                end: edit.range.end,
                text_len: text.len(),
            });
        }
        if index > 0 && edit.range.start < previous_end {
            return Err(SearchError::OverlappingReplacementRanges {
                previous_end,
                next_start: edit.range.start,
            });
        }
        previous_end = edit.range.end;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        RegexSearchProvider, SearchCoordinator, SearchError, SearchProvider, SearchRequest,
        SearchRequestId, SearchSpec,
    };
    use luna_core::CodedError;
    use std::error::Error;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn literal_matches_are_non_overlapping() -> TestResult {
        let request = SearchRequest::new(
            SearchRequestId::new(1),
            4,
            SearchSpec::literal("aa").with_case_sensitive(true),
        );
        let result = RegexSearchProvider.search("aaaa", &request)?;
        assert_eq!(result.ranges(), vec![0..2, 2..4]);
        Ok(())
    }

    #[test]
    fn whole_word_matching_skips_identifier_substrings() -> TestResult {
        let request = SearchRequest::new(
            SearchRequestId::new(2),
            5,
            SearchSpec::literal("alpha").with_whole_word(true),
        );
        let result = RegexSearchProvider.search("Alpha alpha alphabet alpha_ alpha", &request)?;
        assert_eq!(result.ranges(), vec![0..5, 6..11, 28..33]);
        Ok(())
    }

    #[test]
    fn scoped_matching_returns_document_coordinates() -> TestResult {
        let request = SearchRequest::new(
            SearchRequestId::new(3),
            6,
            SearchSpec::literal("one")
                .with_case_sensitive(true)
                .with_scope(4..15),
        );
        let result = RegexSearchProvider.search("one one two one", &request)?;
        assert_eq!(result.ranges(), vec![4..7, 12..15]);
        Ok(())
    }

    #[test]
    fn regex_replacement_expands_numbered_and_named_captures() -> TestResult {
        let request = SearchRequest::new(
            SearchRequestId::new(4),
            7,
            SearchSpec::regex(r"(?P<left>[a-z]+)=(\d+)").with_case_sensitive(true),
        );
        let plan =
            RegexSearchProvider.replacement_plan("alpha=12 beta=34", &request, "${left}[$2]")?;
        assert_eq!(plan.apply("alpha=12 beta=34")?, "alpha[12] beta[34]");
        Ok(())
    }

    #[test]
    fn literal_replacement_does_not_expand_dollar_syntax() -> TestResult {
        let request = SearchRequest::new(
            SearchRequestId::new(5),
            8,
            SearchSpec::literal("x").with_case_sensitive(true),
        );
        let plan = RegexSearchProvider.replacement_plan("x+x", &request, "$1")?;
        assert_eq!(plan.apply("x+x")?, "$1+$1");
        Ok(())
    }

    #[test]
    fn invalid_regex_exposes_a_stable_error_code() {
        let request = SearchRequest::new(SearchRequestId::new(6), 9, SearchSpec::regex("("));
        let error = RegexSearchProvider
            .search("text", &request)
            .err()
            .unwrap_or(SearchError::InvalidPattern {
                message: "missing error".to_owned(),
            });
        assert_eq!(error.error_code().as_str(), "search.pattern.invalid");
    }

    #[test]
    fn deterministic_match_cap_reports_truncation() -> TestResult {
        let request = SearchRequest::new(
            SearchRequestId::new(7),
            10,
            SearchSpec::literal("a")
                .with_case_sensitive(true)
                .with_max_matches(2),
        );
        let result = RegexSearchProvider.search("aaaa", &request)?;
        assert_eq!(result.ranges(), vec![0..1, 1..2]);
        assert!(result.was_truncated());
        Ok(())
    }

    #[test]
    fn coordinator_rejects_stale_results() -> TestResult {
        let mut coordinator = SearchCoordinator::new();
        let first = coordinator.begin(10, SearchSpec::literal("alpha"));
        let second = coordinator.begin(11, SearchSpec::literal("beta"));
        let first_result = RegexSearchProvider.search("alpha beta", &first)?;
        let second_result = RegexSearchProvider.search("alpha beta", &second)?;
        assert!(!coordinator.accepts(&first_result));
        assert!(coordinator.accepts(&second_result));
        Ok(())
    }
}
