// SPDX-License-Identifier: MPL-2.0

use super::{WorkloadResult, require, usize_to_u64};
use crate::report::WorkloadReport;
use luna_session::{
    MemorySessionStore, SessionDocument, SessionDocumentSource, SessionDocumentView,
    SessionPaneNode, SessionPaneTab, SessionPaneTree, SessionState, SessionStore, SessionWorkspace,
};
use std::path::PathBuf;

const WORKLOAD: &str = "session_round_trips";
const MAXIMUM_DOCUMENTS: u64 = 1;
const MAXIMUM_VIEWS: u64 = 1;
const MAXIMUM_PANE_LEAVES: u64 = 1;

pub(super) fn run(cycles: u32) -> WorkloadResult<WorkloadReport> {
    let store = MemorySessionStore::default();
    let mut maximum_document_bytes = 0_u64;
    let mut total_serialized_document_bytes = 0_u64;

    for cycle in 0..cycles {
        let initial_text = format!("M8.3 session cycle {cycle}\ninitial state\n");
        let initial = state(cycle, initial_text);
        store.save(&initial)?;
        let loaded = store.load()?;
        require(
            WORKLOAD,
            loaded == initial,
            "first session save/load round-trip changed state",
        )?;

        let mut updated = loaded;
        let suffix = format!("restored-and-resaved-{cycle:04}\n");
        let document = updated
            .documents
            .first_mut()
            .ok_or(super::invariant(WORKLOAD, "session lost its document"))?;
        document.text.push_str(&suffix);
        document.is_dirty = cycle.is_multiple_of(2);
        let updated_text_length = document.text.len();
        let document_bytes = usize_to_u64(updated_text_length);
        maximum_document_bytes = maximum_document_bytes.max(document_bytes);
        total_serialized_document_bytes =
            total_serialized_document_bytes.saturating_add(document_bytes);

        let view = updated
            .views
            .first_mut()
            .ok_or(super::invariant(WORKLOAD, "session lost its document view"))?;
        view.caret_byte = updated_text_length;
        view.selection_anchor_byte = Some(0);
        view.selection_focus_byte = Some(updated_text_length);
        view.scroll_y = i32::try_from(cycle % 400).unwrap_or(i32::MAX);

        store.save(&updated)?;
        let restored = store.load()?;
        require(
            WORKLOAD,
            restored == updated,
            "second session save/load round-trip changed state",
        )?;
        require(
            WORKLOAD,
            restored.documents.len() == usize::try_from(MAXIMUM_DOCUMENTS).unwrap_or(usize::MAX),
            "session document count changed",
        )?;
        require(
            WORKLOAD,
            restored.views.len() == usize::try_from(MAXIMUM_VIEWS).unwrap_or(usize::MAX),
            "session view count changed",
        )?;
        let pane_leaves = restored
            .pane_tree
            .as_ref()
            .map_or(0, |tree| pane_leaf_count(&tree.root));
        require(
            WORKLOAD,
            pane_leaves == MAXIMUM_PANE_LEAVES,
            "session pane-tree leaf count changed",
        )?;
    }

    let mut report = WorkloadReport::new(WORKLOAD);
    report.record("cycles", u64::from(cycles));
    report.record("saves", u64::from(cycles).saturating_mul(2));
    report.record("loads", u64::from(cycles).saturating_mul(2));
    report.record("maximum_documents", MAXIMUM_DOCUMENTS);
    report.record("maximum_views", MAXIMUM_VIEWS);
    report.record("maximum_pane_leaves", MAXIMUM_PANE_LEAVES);
    report.record("maximum_document_bytes", maximum_document_bytes);
    report.record(
        "total_serialized_document_bytes",
        total_serialized_document_bytes,
    );
    report.limit("maximum_documents", MAXIMUM_DOCUMENTS);
    report.limit("maximum_views", MAXIMUM_VIEWS);
    report.limit("maximum_pane_leaves", MAXIMUM_PANE_LEAVES);
    Ok(report)
}

fn state(cycle: u32, text: String) -> SessionState {
    let workspace_root = PathBuf::from("/luna-m8-3/workspace");
    let source_root = workspace_root.join("src");
    let document_key = 1_u64;
    let view_key = 1_u64;
    let pane_key = 1_u64;
    let text_length = text.len();

    SessionState {
        recent_files: Vec::new(),
        workspace: Some(SessionWorkspace {
            root: workspace_root,
            expanded_paths: vec![source_root.clone()],
            selected_path: Some(source_root.join(format!("cycle-{cycle:04}.rs"))),
        }),
        documents: vec![SessionDocument {
            document_key,
            source: SessionDocumentSource::Virtual("m8.3.long-session".to_owned()),
            title: "M8.3 Long Session".to_owned(),
            text,
            is_dirty: true,
            storage_snapshot: None,
        }],
        views: vec![SessionDocumentView {
            view_key,
            document_key,
            caret_byte: text_length,
            selection_anchor_byte: Some(0),
            selection_focus_byte: Some(text_length),
            scroll_x: 0,
            scroll_y: i32::try_from(cycle % 200).unwrap_or(i32::MAX),
        }],
        pane_tree: Some(SessionPaneTree {
            focused_pane_key: pane_key,
            root: SessionPaneNode::Leaf {
                pane_key,
                tabs: vec![SessionPaneTab {
                    view_key,
                    is_pinned: cycle.is_multiple_of(2),
                    is_preview: false,
                }],
                active_view_key: view_key,
                tab_scroll_offset: 0,
            },
        }),
    }
}

fn pane_leaf_count(node: &SessionPaneNode) -> u64 {
    match node {
        SessionPaneNode::Leaf { .. } => 1,
        SessionPaneNode::Split { first, second, .. } => {
            pane_leaf_count(first).saturating_add(pane_leaf_count(second))
        }
    }
}
