// SPDX-License-Identifier: MPL-2.0

use super::{WorkloadResult, invariant, require, usize_to_u64};
use crate::report::WorkloadReport;
use luna_document_services::{MemoryTextFileService, TextFileService, WritePrecondition};
use luna_documents::{CloseRequirement, DocumentRegistry, FileIdentity, OpenFileOutcome};
use luna_text::EditableText;
use std::path::PathBuf;

const WORKLOAD: &str = "document_lifecycle";
const MAXIMUM_OPEN_DOCUMENTS: u64 = 1;

pub(super) fn run(cycles: u32) -> WorkloadResult<WorkloadReport> {
    let files = MemoryTextFileService::new("/luna-m8-3/documents")?;
    let mut registry = DocumentRegistry::new();
    let mut bytes_written = 0_u64;
    let mut maximum_open_documents = 0_u64;

    for cycle in 0..cycles {
        let path = PathBuf::from(format!("document-{cycle:04}.txt"));
        let initial = format!("Luna M8.3 document cycle {cycle}\n");
        files.insert_utf8(&path, &initial)?;

        let loaded = files.load_utf8(&path)?;
        let storage_snapshot = loaded.snapshot();
        let identity = FileIdentity::from_canonical_path(loaded.identity().path().to_path_buf())?;
        let document_id = match registry.register_file(
            identity,
            format!("document-{cycle:04}.txt"),
            0,
            Some(storage_snapshot),
        ) {
            OpenFileOutcome::Opened(document_id) => document_id,
            OpenFileOutcome::AlreadyOpen(document_id) => {
                return Err(Box::new(invariant(
                    WORKLOAD,
                    format!("fresh path reopened existing document {document_id}"),
                )));
            }
        };

        maximum_open_documents = maximum_open_documents.max(usize_to_u64(registry.records().len()));
        require(
            WORKLOAD,
            registry.records().len() == 1,
            "registry must contain exactly one open document",
        )?;

        let mut editor = EditableText::new(loaded.into_text());
        editor.set_caret(editor.document().end_location());
        let suffix = format!("edited-and-saved-{cycle:04}\n");
        let edit = editor.insert_text(&suffix);
        require(
            WORKLOAD,
            edit.did_change,
            "editor insertion must change the buffer",
        )?;

        let written = files.write_utf8_atomic(
            &path,
            editor.document().text(),
            WritePrecondition::Matches(storage_snapshot),
        )?;
        let edit_revision = editor.edit_revision();
        let record = registry
            .get_mut(document_id)
            .ok_or(invariant(WORKLOAD, "open document disappeared before save"))?;
        record.mark_saved(edit_revision, Some(written.snapshot()));
        require(
            WORKLOAD,
            record.close_requirement(edit_revision) == CloseRequirement::Safe,
            "saved document must be safe to close",
        )?;

        let stored = files
            .bytes(&path)?
            .ok_or(invariant(WORKLOAD, "saved memory file disappeared"))?;
        require(
            WORKLOAD,
            stored.as_slice() == editor.document().text().as_bytes(),
            "saved bytes must equal the editor buffer",
        )?;
        bytes_written = bytes_written.saturating_add(usize_to_u64(stored.len()));

        require(
            WORKLOAD,
            registry.remove(document_id).is_some(),
            "close cycle must remove the document record",
        )?;
        require(
            WORKLOAD,
            files.remove_file(&path)?,
            "close cycle must remove the temporary backing file",
        )?;
    }

    require(
        WORKLOAD,
        registry.records().is_empty(),
        "document registry must be empty after all cycles",
    )?;
    require(
        WORKLOAD,
        maximum_open_documents <= MAXIMUM_OPEN_DOCUMENTS,
        "document high-water mark exceeded one open record",
    )?;

    let mut report = WorkloadReport::new(WORKLOAD);
    report.record("cycles", u64::from(cycles));
    report.record("opens", u64::from(cycles));
    report.record("edits", u64::from(cycles));
    report.record("saves", u64::from(cycles));
    report.record("closes", u64::from(cycles));
    report.record("bytes_written", bytes_written);
    report.record("maximum_open_documents", maximum_open_documents);
    report.limit("maximum_open_documents", MAXIMUM_OPEN_DOCUMENTS);
    Ok(report)
}
