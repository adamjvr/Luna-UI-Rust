// SPDX-License-Identifier: MPL-2.0

//! Qualification of Luna's file lifecycle from an independent downstream product boundary.

use luna_document_services::{
    DocumentDialogService, FileServiceErrorKind, MemoryTextFileService, SaveConflictChoice,
    ScriptedDialogService, TextFileService, WritePrecondition,
};
use luna_documents::{DocumentRegistry, OpenFileOutcome};
use std::error::Error;
use std::io;
use std::path::Path;

#[test]
fn downstream_product_file_lifecycle_is_safe_and_reproducible() -> Result<(), Box<dyn Error>> {
    let files = MemoryTextFileService::new("/moth-qualification")?;
    let path = Path::new("/moth-qualification/source.txt");
    files.insert_utf8(path, "first\n")?;

    let loaded = files.load_utf8(path)?;
    let mut documents = DocumentRegistry::new();
    let document_id = match documents.register_file(
        loaded.identity().clone(),
        "source.txt",
        1,
        Some(loaded.snapshot()),
    ) {
        OpenFileOutcome::Opened(id) => id,
        OpenFileOutcome::AlreadyOpen(_) => {
            return Err(
                io::Error::other("first file registration was unexpectedly duplicate").into(),
            );
        }
    };

    assert_eq!(
        documents.register_file(
            loaded.identity().clone(),
            "source.txt",
            1,
            Some(loaded.snapshot()),
        ),
        OpenFileOutcome::AlreadyOpen(document_id)
    );

    let written = files.write_utf8_atomic(
        path,
        "second\n",
        WritePrecondition::Matches(loaded.snapshot()),
    )?;
    let record = documents
        .get_mut(document_id)
        .ok_or_else(|| io::Error::other("registered document disappeared"))?;
    record.mark_saved(2, Some(written.snapshot()));

    files.insert_utf8(path, "external\n")?;
    let conflict = match files.write_utf8_atomic(
        path,
        "third\n",
        WritePrecondition::Matches(written.snapshot()),
    ) {
        Ok(_) => return Err(io::Error::other("stale write unexpectedly succeeded").into()),
        Err(error) => error,
    };
    assert_eq!(conflict.kind(), FileServiceErrorKind::Conflict);

    let reloaded = files.load_utf8(path)?;
    assert_eq!(reloaded.text(), "external\n");

    let mut dialogs = ScriptedDialogService::default();
    dialogs.push_save_conflict(SaveConflictChoice::Reload);
    assert_eq!(
        dialogs.resolve_save_conflict_for_product("Moth Text", "source.txt", path)?,
        SaveConflictChoice::Reload
    );

    let overwritten = files.write_utf8_atomic(path, "third\n", WritePrecondition::Any)?;
    let record = documents
        .get_mut(document_id)
        .ok_or_else(|| io::Error::other("registered document disappeared after overwrite"))?;
    record.mark_saved(3, Some(overwritten.snapshot()));
    assert_eq!(files.load_utf8(path)?.text(), "third\n");

    println!("downstream_product_file_lifecycle=passed");
    Ok(())
}
