//! Sweep fixes (s4-chat-list) that need a send that fails.
use super::super::*;
use super::*;
use crate::diagnostics::{DiagnosticSink, MemorySink};
use crate::platform::MemorySecretStore;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

#[test]
fn the_sync_switch_stays_put_when_the_prefs_cannot_be_saved() {
    let (_dir, prepared) = prepared_tmp(&MemorySecretStore::new());
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let recorder = Arc::new(RecordingSender::new());
    let seq = AtomicU64::new(0);
    let mut driver = ready_driver(&recorder, prepared, &sink, &seq);
    assert!(driver.session.settings.contact_prefs.sync_enabled);
    // The file's place is taken by a directory: the save fails.
    let file = driver.paths.root.join("contacts_prefs.json");
    std::fs::create_dir_all(&file).unwrap();
    assert!(driver.set_contact_sync(false).is_err());
    assert!(driver.session.settings.contact_prefs.sync_enabled);
    std::fs::remove_dir_all(&file).unwrap();
    driver.set_contact_sync(false).unwrap();
    assert!(!driver.session.settings.contact_prefs.sync_enabled);
}
