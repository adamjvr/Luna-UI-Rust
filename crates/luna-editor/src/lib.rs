// SPDX-License-Identifier: MPL-2.0

//! Product-neutral editor mechanics shared by Luna applications.
//!
//! This crate deliberately stops below product policy and language services. It supplies stable
//! byte selections, simultaneous edits, bounded transaction history, IME composition state,
//! syntax-span contracts, Sublime color-scheme import, and deterministic behavior fixtures.

mod history;
mod ime;
mod parity;
mod selection;
mod sublime;
mod syntax;

pub use history::{EditGroup, EditHistory, HistorySnapshot};
pub use ime::{ImeCommit, ImeComposition};
pub use parity::{EditorParityFixture, ParityError, ParityOperation, ParityResult};
pub use selection::{ByteEdit, ByteSelection, MultiEditResult, SelectionError, SelectionSet};
pub use sublime::{ColorSchemeError, SublimeColorSchemeAdapter};
pub use syntax::{
    KeywordSyntaxProvider, ResolvedSyntaxSpan, SyntaxError, SyntaxProvider, SyntaxRule,
    SyntaxSnapshot, SyntaxSpan, SyntaxStyle, SyntaxTheme,
};
