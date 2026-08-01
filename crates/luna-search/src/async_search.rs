// SPDX-License-Identifier: MPL-2.0

//! Background execution for product-neutral document search.

use crate::{
    RegexSearchProvider, SearchError, SearchProvider, SearchRequest, SearchRequestId, SearchResult,
};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct SearchJob {
    text: Arc<str>,
    request: SearchRequest,
}

/// Completed background search with request identity and elapsed worker time.
#[derive(Debug)]
pub struct AsyncSearchResponse {
    request_id: SearchRequestId,
    source_revision: u64,
    outcome: Result<SearchResult, SearchError>,
    elapsed: Duration,
}

impl AsyncSearchResponse {
    /// Returns the originating request identity.
    #[must_use]
    pub const fn request_id(&self) -> SearchRequestId {
        self.request_id
    }

    /// Returns the searched document revision.
    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    /// Returns the worker execution duration.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Consumes the response and returns its search outcome.
    pub fn into_outcome(self) -> Result<SearchResult, SearchError> {
        self.outcome
    }
}

/// One background search worker that coalesces queued requests to the newest job.
///
/// A running regular expression is allowed to finish, but the UI can reject its result by request
/// identity and document revision. Before starting another search, the worker drains queued jobs and
/// retains only the newest request, preventing obsolete query bursts from building an unbounded
/// backlog.
#[derive(Debug)]
pub struct AsyncSearchWorker {
    jobs: Sender<SearchJob>,
    responses: Receiver<AsyncSearchResponse>,
}

impl AsyncSearchWorker {
    /// Starts the background worker thread.
    pub fn new() -> std::io::Result<Self> {
        let (job_sender, job_receiver) = mpsc::channel::<SearchJob>();
        let (response_sender, response_receiver) = mpsc::channel::<AsyncSearchResponse>();
        thread::Builder::new()
            .name("luna-search-worker".to_owned())
            .spawn(move || run_worker(job_receiver, response_sender))?;
        Ok(Self {
            jobs: job_sender,
            responses: response_receiver,
        })
    }

    /// Queues a versioned source snapshot for background matching.
    ///
    /// Returns `false` only when the worker has already stopped.
    pub fn submit(&self, text: Arc<str>, request: SearchRequest) -> bool {
        self.jobs.send(SearchJob { text, request }).is_ok()
    }

    /// Drains completed responses and returns only the newest available completion.
    pub fn try_recv_latest(&self) -> Option<AsyncSearchResponse> {
        let mut latest = None;
        loop {
            match self.responses.try_recv() {
                Ok(response) => latest = Some(response),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return latest,
            }
        }
    }
}

fn run_worker(jobs: Receiver<SearchJob>, responses: Sender<AsyncSearchResponse>) {
    while let Ok(mut job) = jobs.recv() {
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        let started = Instant::now();
        let request_id = job.request.id();
        let source_revision = job.request.source_revision();
        let outcome = RegexSearchProvider.search(&job.text, &job.request);
        let response = AsyncSearchResponse {
            request_id,
            source_revision,
            outcome,
            elapsed: started.elapsed(),
        };
        if responses.send(response).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AsyncSearchWorker;
    use crate::{SearchMode, SearchRequest, SearchRequestId, SearchSpec};
    use std::error::Error;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    type TestResult = Result<(), Box<dyn Error>>;

    fn receive(worker: &AsyncSearchWorker) -> Option<super::AsyncSearchResponse> {
        for _ in 0..200 {
            if let Some(response) = worker.try_recv_latest() {
                return Some(response);
            }
            thread::sleep(Duration::from_millis(1));
        }
        None
    }

    #[test]
    fn worker_returns_request_identity_revision_and_ranges() -> TestResult {
        let worker = AsyncSearchWorker::new()?;
        let request = SearchRequest::new(
            SearchRequestId::new(41),
            17,
            SearchSpec::new("beta", SearchMode::Literal),
        );
        assert!(worker.submit(Arc::from("alpha beta beta"), request));
        let response = receive(&worker)
            .ok_or_else(|| std::io::Error::other("background search did not finish"))?;
        assert_eq!(response.request_id().value(), 41);
        assert_eq!(response.source_revision(), 17);
        assert_eq!(response.into_outcome()?.ranges(), vec![6..10, 11..15]);
        Ok(())
    }

    #[test]
    fn worker_reports_invalid_regex_without_panicking() -> TestResult {
        let worker = AsyncSearchWorker::new()?;
        let request = SearchRequest::new(
            SearchRequestId::new(42),
            18,
            SearchSpec::new("(", SearchMode::Regex),
        );
        assert!(worker.submit(Arc::from("alpha"), request));
        let response = receive(&worker)
            .ok_or_else(|| std::io::Error::other("background search did not finish"))?;
        assert!(response.into_outcome().is_err());
        Ok(())
    }
}
