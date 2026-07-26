// SPDX-License-Identifier: MPL-2.0

//! Product-neutral asynchronous completion request and delivery contracts.
//!
//! Providers may compute on any thread, but responses are drained and validated by the application
//! on its UI thread. Request identities and document revisions prevent canceled or stale work from
//! replacing a newer completion list.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender};

/// Monotonic identity for one completion request.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompletionRequestId(u64);

impl CompletionRequestId {
    /// Returns the underlying application-local value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Explicit byte range replaced when a completion candidate is accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionReplacementRange {
    /// Inclusive UTF-8 byte start.
    pub start_byte: usize,
    /// Exclusive UTF-8 byte end.
    pub end_byte: usize,
}

impl CompletionReplacementRange {
    /// Creates a validated replacement range.
    #[must_use]
    pub const fn new(start_byte: usize, end_byte: usize) -> Option<Self> {
        if start_byte <= end_byte {
            Some(Self {
                start_byte,
                end_byte,
            })
        } else {
            None
        }
    }
}

/// Immutable context supplied to a completion provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionRequest {
    /// Unique request identity.
    pub id: CompletionRequestId,
    /// Application-defined document-view key.
    pub view_key: u64,
    /// Shared document edit revision at request time.
    pub document_revision: u64,
    /// Caret UTF-8 byte offset at request time.
    pub caret_byte: usize,
    /// Identifier or query prefix selected by application policy.
    pub prefix: String,
}

/// One provider-owned completion candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionCandidate {
    /// Stable provider-owned candidate identity.
    pub id: String,
    /// Primary user-visible label.
    pub label: String,
    /// Secondary detail such as type or source.
    pub detail: String,
    /// Optional longer documentation projected by richer clients.
    pub documentation: Option<String>,
    /// UTF-8 text inserted on acceptance.
    pub insert_text: String,
    /// Exact range replaced on acceptance.
    pub replacement: CompletionReplacementRange,
}

/// Completion result delivered back to the UI thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionResponse {
    /// Request identity being answered.
    pub request_id: CompletionRequestId,
    /// Document revision observed by the provider.
    pub document_revision: u64,
    /// Ordered candidates.
    pub candidates: Vec<CompletionCandidate>,
}

/// Cloneable response sink that providers may move to worker threads.
#[derive(Clone, Debug)]
pub struct CompletionResponseSender {
    sender: Sender<CompletionResponse>,
}

impl CompletionResponseSender {
    /// Delivers one provider response.
    ///
    /// Returns the original response when the receiving coordinator has been dropped.
    pub fn send(&self, response: CompletionResponse) -> Result<(), CompletionResponse> {
        self.sender.send(response).map_err(|error| error.0)
    }
}

/// Provider boundary. Implementations may answer immediately or from background work.
pub trait CompletionProvider {
    /// Starts work for one immutable request.
    fn request(&mut self, request: CompletionRequest, responses: CompletionResponseSender);

    /// Cancels provider work for an obsolete request when supported.
    fn cancel(&mut self, request_id: CompletionRequestId);
}

/// UI-thread coordinator that assigns request IDs and rejects stale delivery.
#[derive(Debug)]
pub struct CompletionCoordinator {
    next_request_id: u64,
    active: Option<(CompletionRequestId, u64, u64)>,
    sender: CompletionResponseSender,
    receiver: Receiver<CompletionResponse>,
}

impl Default for CompletionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionCoordinator {
    /// Creates an empty coordinator and response channel.
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            next_request_id: 1,
            active: None,
            sender: CompletionResponseSender { sender },
            receiver,
        }
    }

    /// Starts a request, canceling the previously active provider request.
    pub fn begin<P: CompletionProvider + ?Sized>(
        &mut self,
        provider: &mut P,
        view_key: u64,
        document_revision: u64,
        caret_byte: usize,
        prefix: impl Into<String>,
    ) -> CompletionRequestId {
        if let Some((previous, _, _)) = self.active.take() {
            provider.cancel(previous);
        }
        let id = CompletionRequestId(self.next_request_id);
        self.next_request_id = self.next_request_id.saturating_add(1).max(1);
        self.active = Some((id, view_key, document_revision));
        provider.request(
            CompletionRequest {
                id,
                view_key,
                document_revision,
                caret_byte,
                prefix: prefix.into(),
            },
            self.sender.clone(),
        );
        id
    }

    /// Cancels the active request and drains already queued obsolete responses.
    pub fn cancel<P: CompletionProvider + ?Sized>(&mut self, provider: &mut P) {
        if let Some((request_id, _, _)) = self.active.take() {
            provider.cancel(request_id);
        }
        while self.receiver.try_recv().is_ok() {}
    }

    /// Returns the newest response matching the active request and current document revision.
    ///
    /// Obsolete, canceled, and out-of-order responses are consumed and discarded.
    #[must_use]
    pub fn drain_latest(
        &mut self,
        current_view_key: u64,
        current_document_revision: u64,
    ) -> Option<CompletionResponse> {
        let active = self.active;
        let mut accepted = None;
        while let Ok(response) = self.receiver.try_recv() {
            if active
                == Some((
                    response.request_id,
                    current_view_key,
                    response.document_revision,
                ))
                && response.document_revision == current_document_revision
            {
                accepted = Some(response);
            }
        }
        accepted
    }

    /// Returns the active request identity, when work is outstanding.
    #[must_use]
    pub const fn active_request(&self) -> Option<CompletionRequestId> {
        match self.active {
            Some((request_id, _, _)) => Some(request_id),
            None => None,
        }
    }
}

/// Deterministic immediate provider used by tests, demos, and local fallback completion.
#[derive(Clone, Debug, Default)]
pub struct ScriptedCompletionProvider {
    responses: BTreeMap<String, Vec<CompletionCandidate>>,
    canceled: Vec<CompletionRequestId>,
}

impl ScriptedCompletionProvider {
    /// Creates a provider from prefix-to-candidate responses.
    #[must_use]
    pub fn new(responses: BTreeMap<String, Vec<CompletionCandidate>>) -> Self {
        Self {
            responses,
            canceled: Vec::new(),
        }
    }

    /// Returns canceled request identities in delivery order.
    #[must_use]
    pub fn canceled_requests(&self) -> &[CompletionRequestId] {
        &self.canceled
    }
}

impl CompletionProvider for ScriptedCompletionProvider {
    fn request(&mut self, request: CompletionRequest, responses: CompletionResponseSender) {
        let candidates = self
            .responses
            .get(&request.prefix)
            .cloned()
            .unwrap_or_default();
        let _ = responses.send(CompletionResponse {
            request_id: request.id,
            document_revision: request.document_revision,
            candidates,
        });
    }

    fn cancel(&mut self, request_id: CompletionRequestId) {
        self.canceled.push(request_id);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompletionCandidate, CompletionCoordinator, CompletionProvider, CompletionReplacementRange,
        CompletionRequest, CompletionRequestId, CompletionResponse, CompletionResponseSender,
        ScriptedCompletionProvider,
    };
    use std::collections::BTreeMap;

    fn candidate(label: &str) -> CompletionCandidate {
        CompletionCandidate {
            id: label.to_owned(),
            label: label.to_owned(),
            detail: "local".to_owned(),
            documentation: Some(format!("Documentation for {label}")),
            insert_text: label.to_owned(),
            replacement: CompletionReplacementRange::new(2, 4).unwrap_or(
                CompletionReplacementRange {
                    start_byte: 2,
                    end_byte: 4,
                },
            ),
        }
    }

    #[test]
    fn immediate_provider_delivers_matching_revision() {
        let mut responses = BTreeMap::new();
        responses.insert("pr".to_owned(), vec![candidate("print")]);
        let mut provider = ScriptedCompletionProvider::new(responses);
        let mut coordinator = CompletionCoordinator::new();
        let request = coordinator.begin(&mut provider, 9, 4, 12, "pr");
        let response = coordinator.drain_latest(9, 4);
        assert_eq!(
            response.as_ref().map(|value| value.request_id),
            Some(request)
        );
        assert_eq!(
            response
                .as_ref()
                .and_then(|value| value.candidates.first())
                .map(|value| value.label.as_str()),
            Some("print")
        );
    }

    #[derive(Debug, Default)]
    struct DelayedProvider {
        pending: Vec<(CompletionRequest, CompletionResponseSender)>,
        canceled: Vec<CompletionRequestId>,
    }

    impl CompletionProvider for DelayedProvider {
        fn request(&mut self, request: CompletionRequest, responses: CompletionResponseSender) {
            self.pending.push((request, responses));
        }

        fn cancel(&mut self, request_id: CompletionRequestId) {
            self.canceled.push(request_id);
        }
    }

    #[test]
    fn stale_and_out_of_order_responses_are_rejected() {
        let mut provider = DelayedProvider::default();
        let mut coordinator = CompletionCoordinator::new();
        let first = coordinator.begin(&mut provider, 1, 1, 2, "a");
        let second = coordinator.begin(&mut provider, 1, 2, 3, "ab");
        assert_eq!(provider.canceled, vec![first]);

        for (request, sender) in provider.pending.drain(..) {
            let _ = sender.send(CompletionResponse {
                request_id: request.id,
                document_revision: request.document_revision,
                candidates: vec![candidate(&request.prefix)],
            });
        }
        let response = coordinator.drain_latest(1, 2);
        assert_eq!(
            response.as_ref().map(|value| value.request_id),
            Some(second)
        );
        assert_eq!(response.map(|value| value.candidates.len()), Some(1));
    }

    #[test]
    fn revision_change_discards_even_the_active_request() {
        let mut provider = DelayedProvider::default();
        let mut coordinator = CompletionCoordinator::new();
        let _ = coordinator.begin(&mut provider, 1, 5, 4, "name");
        if let Some((request, sender)) = provider.pending.pop() {
            let _ = sender.send(CompletionResponse {
                request_id: request.id,
                document_revision: request.document_revision,
                candidates: vec![candidate("namespace")],
            });
        }
        assert!(coordinator.drain_latest(1, 6).is_none());
    }

    #[test]
    fn response_for_previous_view_is_rejected() {
        let mut responses = BTreeMap::new();
        responses.insert("pr".to_owned(), vec![candidate("print")]);
        let mut provider = ScriptedCompletionProvider::new(responses);
        let mut coordinator = CompletionCoordinator::new();
        let _ = coordinator.begin(&mut provider, 9, 4, 12, "pr");
        assert!(coordinator.drain_latest(10, 4).is_none());
    }
}
