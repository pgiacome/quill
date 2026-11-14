//! Driver tests: calls nobody joined are listed from their details and
//! from the chats that report them, never taken into the call window
//! just because an update arrived.
use super::*;
use crate::state::GroupCallKind;
use std::sync::atomic::AtomicU64;

fn feed(
    driver: &mut ConnectDriver<Arc<RecordingSender>>,
    seq: &AtomicU64,
    sink: &Arc<dyn DiagnosticSink>,
    json: &str,
) {
    driver
        .ingest(copy_and_parse(json, seq, sink).unwrap())
        .unwrap();
}

#[test]
fn an_unjoined_update_feeds_the_list_and_not_the_slot() {
    let (dir, _recorder, mut driver, seq) = group_call_test_driver();
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{
        "@type":"updateGroupCall",
        "group_call":{"@type":"groupCall","id":41,"title":"Town hall",
        "is_active":true,"is_video_chat":false,"is_joined":false,
        "is_rtmp_stream":true,"participant_count":12}
    }"#,
    );
    let calls = driver.session.live_group_calls();
    let call = calls.iter().find(|c| c.id == 41).expect("listed");
    assert_eq!(call.kind, Some(GroupCallKind::LiveStream));
    assert_eq!(call.participant_count, 12);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_joined_update_leaves_the_list_and_takes_the_slot() {
    let (dir, _recorder, mut driver, seq) = group_call_test_driver();
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateNewChat","chat":{"id":51,"title":"Town hall",
        "type":{"@type":"chatTypeSupergroup","supergroup_id":51,"is_channel":false},"unread_count":0}}"#,
    );
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateChatVideoChat","chat_id":51,
        "video_chat":{"@type":"videoChat","group_call_id":41,"has_participants":true,"default_participant_id":null}}"#,
    );
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{
        "@type":"updateGroupCall",
        "group_call":{"@type":"groupCall","id":41,"title":"Town hall",
        "is_active":true,"is_video_chat":false,"is_joined":true,
        "is_rtmp_stream":false,"participant_count":12}
    }"#,
    );
    assert_eq!(
        driver.session.calls.active_group_call.as_ref().map(|c| c.id),
        Some(41)
    );
    assert!(
        !driver.session.live_group_calls().iter().any(|c| c.id == 41),
        "a call in the slot is not listed beside it"
    );
    assert_eq!(driver.session.chat_id_for_group_call(41), Some(51));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_ended_call_leaves_the_list() {
    let (dir, _recorder, mut driver, seq) = group_call_test_driver();
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateNewChat","chat":{"id":51,"title":"Town hall",
        "type":{"@type":"chatTypeSupergroup","supergroup_id":51,"is_channel":false},"unread_count":0}}"#,
    );
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateChatVideoChat","chat_id":51,
        "video_chat":{"@type":"videoChat","group_call_id":41,"has_participants":true,"default_participant_id":null}}"#,
    );
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{
        "@type":"updateGroupCall",
        "group_call":{"@type":"groupCall","id":41,"title":"Town hall",
        "is_active":true,"is_video_chat":true,"is_joined":false,
        "is_rtmp_stream":false,"participant_count":5}
    }"#,
    );
    assert!(driver.session.live_group_calls().iter().any(|c| c.id == 41));

    // The chat emptied, but the call is still running: it stays listed.
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateChatVideoChat","chat_id":51,
        "video_chat":{"@type":"videoChat","group_call_id":41,"has_participants":false,"default_participant_id":null}}"#,
    );
    assert!(
        driver.session.calls.group_call_details.contains_key(&41),
        "has_participants:false must not delete the call"
    );

    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{
        "@type":"updateGroupCall",
        "group_call":{"@type":"groupCall","id":41,"title":"Town hall",
        "is_active":false,"is_video_chat":true,"is_joined":false,
        "is_rtmp_stream":false,"participant_count":0}
    }"#,
    );
    assert!(!driver.session.calls.group_call_details.contains_key(&41));
    assert!(!driver.session.live_group_calls().iter().any(|c| c.id == 41));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_chat_seeds_a_row_when_no_update_arrived() {
    let (dir, _recorder, mut driver, seq) = group_call_test_driver();
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateNewChat","chat":{"id":52,"title":"Studio",
        "type":{"@type":"chatTypeSupergroup","supergroup_id":52,"is_channel":false},"unread_count":0}}"#,
    );
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateChatVideoChat","chat_id":52,
        "video_chat":{"@type":"videoChat","group_call_id":42,"has_participants":true,"default_participant_id":null}}"#,
    );
    let calls = driver.session.live_group_calls();
    let call = calls.iter().find(|c| c.id == 42).expect("seeded from chat");
    assert_eq!(call.kind, None);
    assert_eq!(call.participant_count, 0);
    assert_eq!(call.title, "Studio");
    assert_eq!(call.chat_id, Some(52));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn zero_group_call_id_removes_details_but_no_participants_does_not() {
    let (dir, _recorder, mut driver, seq) = group_call_test_driver();
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateNewChat","chat":{"id":51,"title":"Town hall",
        "type":{"@type":"chatTypeSupergroup","supergroup_id":51,"is_channel":false},"unread_count":0}}"#,
    );
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateChatVideoChat","chat_id":51,
        "video_chat":{"@type":"videoChat","group_call_id":41,"has_participants":true,"default_participant_id":null}}"#,
    );
    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{
        "@type":"updateGroupCall",
        "group_call":{"@type":"groupCall","id":41,"title":"Town hall",
        "is_active":true,"is_video_chat":false,"is_joined":false,
        "is_rtmp_stream":false,"participant_count":3}
    }"#,
    );

    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateChatVideoChat","chat_id":51,
        "video_chat":{"@type":"videoChat","group_call_id":41,"has_participants":false,"default_participant_id":null}}"#,
    );
    assert!(
        driver.session.calls.group_call_details.contains_key(&41),
        "an empty chat keeps the running call"
    );

    feed(
        &mut driver,
        &seq,
        &sink,
        r#"{"@type":"updateChatVideoChat","chat_id":51,
        "video_chat":{"@type":"videoChat","group_call_id":0,"has_participants":false,"default_participant_id":null}}"#,
    );
    assert!(!driver.session.calls.group_call_details.contains_key(&41));
    let _ = std::fs::remove_dir_all(dir);
}
