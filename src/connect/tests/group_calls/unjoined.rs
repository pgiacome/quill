//! Driver tests: running calls nobody here joined are listed from the chats
//! that point at them and what updates said about them, and are taken into
//! the call window only by the answer to the call the person asked for.
use super::*;
use crate::call_link::{CallLinkStage, CallLinkStep, OfferMode};
use crate::ids::ChatId;
use crate::state::{GroupCallKind, LiveGroupCall};
use std::sync::atomic::AtomicU64;

type Driver = ConnectDriver<Arc<RecordingSender>>;

struct Harness {
    dir: std::path::PathBuf,
    recorder: Arc<RecordingSender>,
    driver: Driver,
    seq: AtomicU64,
    sink: Arc<dyn DiagnosticSink>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn harness() -> Harness {
    let (dir, recorder, mut driver, seq) = group_call_test_driver();
    driver.set_call_engine(Box::new(MockEngine::new()));
    Harness {
        dir,
        recorder,
        driver,
        seq,
        sink: Arc::new(MemorySink::new()),
    }
}

impl Harness {
    fn feed(&mut self, json: &str) {
        ingest_call_json(&mut self.driver, &self.seq, &self.sink, json);
    }

    /// A group chat `chat_id` whose `video_chat` points at `call`.
    fn chat_with_call(&mut self, chat_id: i64, title: &str, call: i32, has_participants: bool) {
        self.feed(&format!(
            r#"{{"@type":"updateNewChat","chat":{{"id":{chat_id},"title":"{title}","type":{{"@type":"chatTypeSupergroup","supergroup_id":{chat_id},"is_channel":false}},"unread_count":0}}}}"#
        ));
        self.video_chat(chat_id, call, has_participants);
    }

    fn video_chat(&mut self, chat_id: i64, call: i32, has_participants: bool) {
        self.feed(&format!(
            r#"{{"@type":"updateChatVideoChat","chat_id":{chat_id},"video_chat":{{"@type":"videoChat","group_call_id":{call},"has_participants":{has_participants},"default_participant_id":null}}}}"#
        ));
    }

    /// A plain `updateGroupCall`, with no request behind it.
    fn update(&mut self, call: &str) {
        self.feed(&format!(
            r#"{{"@type":"updateGroupCall","group_call":{call}}}"#
        ));
    }

    /// The person asked for the call, and TDLib answered.
    fn ask_and_answer(&mut self, call: &str) {
        answer_group_call_fetch(&mut self.driver, &self.seq, &self.sink, call);
    }

    fn tracked(&self) -> Option<i32> {
        self.driver
            .session
            .calls
            .active_group_call
            .as_ref()
            .map(|call| call.id)
    }

    fn listed(&self) -> Vec<LiveGroupCall> {
        self.driver.session.live_group_calls()
    }

    fn listed_ids(&self) -> Vec<i32> {
        self.listed().iter().map(|call| call.id).collect()
    }

    fn sent(&self, type_name: &str) -> usize {
        self.recorder
            .snapshot()
            .iter()
            .filter(|json| json.contains(&format!(r#""@type":"{type_name}""#)))
            .count()
    }
}

/// A `groupCall` object: `kind` is "voice", "video" or "stream".
fn call(id: i32, title: &str, kind: &str, joined: bool, count: i32) -> String {
    format!(
        r#"{{"@type":"groupCall","id":{id},"title":"{title}","is_active":true,"is_video_chat":{},"is_rtmp_stream":{},"is_joined":{joined},"participant_count":{count}}}"#,
        kind == "video",
        kind == "stream"
    )
}

fn ended(id: i32) -> String {
    format!(r#"{{"@type":"groupCall","id":{id},"title":"","is_active":false,"is_joined":false}}"#)
}

#[test]
fn an_unjoined_update_is_listed_with_its_chat_and_details() {
    let mut h = harness();
    h.chat_with_call(51, "Town hall", 41, true);
    h.update(&call(41, "", "stream", false, 12));
    let listed = h.listed();
    let row = listed.first().expect("listed");
    assert_eq!(row.id, 41);
    assert_eq!(row.chat_id, ChatId(51));
    assert_eq!(row.name, "Town hall");
    let details = row.details.as_ref().expect("the update's details");
    assert_eq!(details.kind, GroupCallKind::LiveStream);
    assert_eq!(details.participant_count, 12);
    assert_eq!(h.tracked(), None, "an update is listed, never opened");
}

#[test]
fn a_call_with_no_chat_pointing_at_it_is_not_listed() {
    let mut h = harness();
    h.update(&call(41, "Town hall", "voice", false, 3));
    assert!(h.listed().is_empty(), "nothing to offer without its chat");
    // The chat arrives: the details already known show at once.
    h.chat_with_call(51, "Town hall", 41, false);
    assert_eq!(h.listed_ids(), vec![41]);
}

#[test]
fn a_chat_with_participants_seeds_a_row_before_any_update() {
    let mut h = harness();
    h.chat_with_call(52, "Studio", 42, true);
    let listed = h.listed();
    let row = listed.first().expect("seeded from the chat");
    assert_eq!((row.id, row.chat_id), (42, ChatId(52)));
    assert_eq!(row.details, None, "kind and count are not guessed");
    // An empty call nobody described is not listed.
    h.video_chat(52, 42, false);
    assert!(h.listed().is_empty());
}

#[test]
fn the_answer_to_the_persons_own_fetch_takes_the_slot_and_leaving_keeps_the_row() {
    let mut h = harness();
    h.chat_with_call(51, "Town hall", 41, true);
    h.ask_and_answer(&call(41, "", "voice", false, 3));
    assert_eq!(h.tracked(), Some(41));
    assert_eq!(h.listed_ids(), vec![41], "only looked at: still listed");
    // Closing the window: the call still runs, so its row keeps its details.
    h.driver.session.leave_group_call_local();
    assert_eq!(h.tracked(), None);
    let listed = h.listed();
    assert_eq!(
        listed
            .first()
            .and_then(|row| row.details.as_ref())
            .map(|details| details.kind),
        Some(GroupCallKind::VoiceChat)
    );
}

#[test]
fn an_update_racing_the_answer_waits_for_the_answer() {
    let mut h = harness();
    h.update(&call(41, "Town hall", "voice", false, 3));
    assert_eq!(h.tracked(), None);
    h.ask_and_answer(&call(41, "Town hall", "voice", false, 3));
    assert_eq!(h.tracked(), Some(41));
}

#[test]
fn an_update_for_another_call_does_not_replace_the_one_asked_for() {
    let mut h = harness();
    h.ask_and_answer(&call(41, "Town hall", "voice", false, 3));
    h.update(&call(42, "Studio", "video", false, 5));
    assert_eq!(h.tracked(), Some(41));
}

#[test]
fn only_the_last_ask_takes_the_slot() {
    let mut h = harness();
    let first = h.driver.fetch_group_call(41).expect("fetch sends");
    let second = h.driver.fetch_group_call(42).expect("fetch sends");
    let answer = |id: i32, extra: crate::ids::RequestId| {
        call(id, "", "voice", false, 3).replacen('{', &format!(r#"{{"@extra":"{}","#, extra.0), 1)
    };
    // The first answer is late: the person has asked for another call.
    h.feed(&answer(41, first));
    assert_eq!(h.tracked(), None);
    h.feed(&answer(42, second));
    assert_eq!(h.tracked(), Some(42));
}

#[test]
fn an_answer_after_leaving_does_not_reopen_the_window() {
    let mut h = harness();
    let extra = h.driver.fetch_group_call(41).expect("fetch sends");
    h.driver.session.leave_group_call_local();
    h.feed(&call(41, "", "voice", false, 3).replacen(
        '{',
        &format!(r#"{{"@extra":"{}","#, extra.0),
        1,
    ));
    assert_eq!(h.tracked(), None);
}

/// Network loss: TDLib reports the call you are in as not joined, needing
/// a rejoin. It is still your call: another call's answer does not take
/// its place, and it is not listed with a Join beside the window.
#[test]
fn a_reconnecting_call_is_not_replaced_or_listed() {
    let mut h = harness();
    h.chat_with_call(51, "Town hall", 41, true);
    h.chat_with_call(52, "Studio", 42, true);
    h.update(&call(41, "", "voice", true, 3));
    h.update(&call(41, "", "voice", false, 3).replace(
        r#""is_joined":false"#,
        r#""is_joined":false,"need_rejoin":true"#,
    ));
    assert!(h.driver.session.group_call_joined_or_joining(None));
    h.ask_and_answer(&call(42, "", "video", false, 5));
    assert_eq!(
        h.tracked(),
        Some(41),
        "the reconnecting call keeps the slot"
    );
    assert_eq!(h.listed_ids(), vec![42], "your own call has no Join row");
}

#[test]
fn a_joined_update_leaves_the_list_and_takes_the_slot() {
    let mut h = harness();
    h.chat_with_call(51, "Town hall", 41, true);
    h.update(&call(41, "", "voice", false, 12));
    h.update(&call(41, "", "voice", true, 12));
    assert_eq!(h.tracked(), Some(41));
    assert!(
        h.listed().is_empty(),
        "the window's call is not listed beside it"
    );
}

#[test]
fn an_ended_call_leaves_the_list() {
    let mut h = harness();
    h.chat_with_call(51, "Town hall", 41, true);
    h.update(&call(41, "", "video", false, 5));
    // The chat emptied, but the call still runs: it stays listed.
    h.video_chat(51, 41, false);
    assert_eq!(h.listed_ids(), vec![41]);
    h.update(&ended(41));
    assert!(h.listed().is_empty());
}

/// The chat moved straight from one call to the next (one ended and
/// another started while the app was away): the old call's details go,
/// and only the new call is listed.
#[test]
fn a_chat_moving_to_another_call_drops_the_old_one() {
    let mut h = harness();
    h.chat_with_call(51, "Town hall", 41, true);
    h.update(&call(41, "", "video", false, 5));
    h.video_chat(51, 43, true);
    assert_eq!(h.listed_ids(), vec![43]);
    // Back at the old call: its details went, so nothing is guessed.
    h.video_chat(51, 41, true);
    let listed = h.listed();
    let row = listed.first().expect("listed again");
    assert_eq!(row.id, 41);
    assert_eq!(row.details, None);
    h.video_chat(51, 0, false);
    assert!(h.listed().is_empty());
}

#[test]
fn the_list_orders_by_kind_then_by_the_name_it_shows() {
    let mut h = harness();
    // Call titles that sort the other way round from the chat titles: the
    // row shows the chat's, so the chat's decides.
    h.chat_with_call(51, "Zeta", 41, true);
    h.chat_with_call(52, "alpha", 42, true);
    h.chat_with_call(53, "Mid", 43, true);
    h.chat_with_call(54, "Seeded", 44, true);
    h.update(&call(41, "A call", "voice", false, 1));
    h.update(&call(42, "Z call", "voice", false, 1));
    h.update(&call(43, "", "stream", false, 1));
    assert_eq!(h.listed_ids(), vec![43, 42, 41, 44]);
}

#[test]
fn a_listed_join_offers_the_call() {
    let mut h = harness();
    h.chat_with_call(51, "Town hall", 41, true);
    let step = h
        .driver
        .join_listed_call(ChatId(51), false, None)
        .expect("offers");
    assert_eq!(step, CallLinkStep::Offer { group_call_id: 41 });
    assert_eq!(h.sent("getGroupCall"), 1);
    assert_eq!(h.driver.session.calls.call_link_stage.hash_for(41), "");
}

#[test]
fn a_listed_join_while_in_a_call_asks_first_and_carries_the_join() {
    let mut h = harness();
    h.chat_with_call(51, "Town hall", 41, true);
    h.chat_with_call(52, "Studio", 42, true);
    h.update(&call(42, "", "video", true, 4));
    let step = h
        .driver
        .join_listed_call(ChatId(51), false, None)
        .expect("asks");
    assert_eq!(step, CallLinkStep::OtherCallOn);
    assert_eq!(
        h.sent("getGroupCall"),
        0,
        "nothing is fetched before the answer"
    );
    let ask = h
        .driver
        .session
        .calls
        .call_link_stage
        .confirm()
        .cloned()
        .expect("the leave question");
    assert_eq!(ask.chat_id, 51);
    assert_eq!(ask.mode, OfferMode::Join);
    h.driver.confirm_call_link_leave().expect("leaves");
    assert!(matches!(
        &h.driver.session.calls.call_link_stage,
        CallLinkStage::Offered { offered, .. }
            if offered.group_call_id == 41 && offered.mode == OfferMode::Join
    ));
    assert_eq!(h.sent("getGroupCall"), 1, "the new call is asked for");
}

/// The UI's own join of a tracked call (before TDLib hears of it) is a
/// call you are in: one listed Join decision, made by the driver.
#[test]
fn a_listed_join_during_the_uis_join_asks_first() {
    let mut h = harness();
    h.chat_with_call(51, "Town hall", 41, true);
    h.chat_with_call(52, "Studio", 42, true);
    h.ask_and_answer(&call(42, "", "video", false, 4));
    let before = h.sent("getGroupCall");
    let step = h
        .driver
        .join_listed_call(ChatId(51), false, Some(42))
        .expect("asks");
    assert_eq!(step, CallLinkStep::OtherCallOn);
    assert_eq!(h.sent("getGroupCall"), before);
    assert_eq!(h.tracked(), Some(42));
}

/// A basic group's call is listed like any other, and its Join offers it
/// (a call link would only open such a chat).
#[test]
fn a_basic_groups_call_is_offered_from_the_list() {
    let mut h = harness();
    h.feed(
        r#"{"@type":"updateNewChat","chat":{"id":61,"title":"Family","type":{"@type":"chatTypeBasicGroup","basic_group_id":61},"unread_count":0}}"#,
    );
    h.video_chat(61, 71, true);
    assert_eq!(h.listed_ids(), vec![71]);
    let step = h
        .driver
        .join_listed_call(ChatId(61), false, None)
        .expect("offers");
    assert_eq!(step, CallLinkStep::Offer { group_call_id: 71 });
    assert_eq!(h.sent("getGroupCall"), 1);
}
