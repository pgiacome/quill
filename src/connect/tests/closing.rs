//! Closing clients without blocking (`connect::closing`): a client that
//! never answers `close` costs one poll, not a wait, and stays until TDLib
//! confirms Closed; one that answers Closed leaves the set; a dead receive
//! loop never counts as Closed; on quit every client shares one deadline.
//!
//! The clients here run `LiveConnect`'s own close code
//! (`begin_close_bridged` / `drain_bridged`) on the recording sender, fed
//! through a real routed `ReceiveBridge` of a test `TdHub`, so only the
//! native library is missing.
use super::super::*;
use super::*;
use crate::calls::engine::RemoteVideoState;
use crate::diagnostics::{DiagnosticSink, MemorySink};
use crate::ids::AccountKey;
use crate::platform::MemorySecretStore;
use crate::state::ActiveCall;
use crate::telegram::client::{ReceiveBridge, copy_and_parse};
use crate::telegram::envelope::{AuthorizationState, CallState};
use crate::telegram::hub::{ReceiveSource, ReceiveWake, TdHub};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CLIENT_ID: i32 = 1;

fn auth_json(state: &str) -> String {
    format!(
        r#"{{"@type":"updateAuthorizationState","authorization_state":{{"@type":"{state}"}},"@client_id":{CLIENT_ID}}}"#
    )
}

/// What `LiveConnect` is, minus the native library: a ready driver on the
/// recording sender and a bridge routed by the hub.
struct TestClient {
    driver: ConnectDriver<Arc<RecordingSender>>,
    bridge: ReceiveBridge,
    finished: Arc<AtomicBool>,
    auth: Arc<Mutex<AuthorizationState>>,
    dir: std::path::PathBuf,
}

impl Closable for TestClient {
    fn begin_close(&mut self) {
        crate::connect::closing::begin_close_bridged(&mut self.driver, &self.bridge);
    }

    fn drain(&mut self, budget: Duration) -> CloseOutcome {
        let progress =
            crate::connect::closing::drain_bridged(&mut self.driver, &self.bridge, budget);
        *self.auth.lock().expect("auth") = self.driver.session.auth.clone();
        progress
    }

    fn finish(&mut self) {
        self.bridge.shutdown();
        self.finished.store(true, Ordering::SeqCst);
    }
}

impl Drop for TestClient {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Handle {
    recorder: Arc<RecordingSender>,
    /// What the hub's `td_receive` returns next.
    tx: Sender<String>,
    /// One message each time the hub's loop asks its source for the next
    /// envelope, which it does only after it handed over the one before.
    entered: Receiver<()>,
    /// Envelopes sent on `tx`, and `entered` messages taken.
    fed: Cell<usize>,
    seen: Cell<usize>,
    hub: Arc<TdHub>,
    finished: Arc<AtomicBool>,
    auth: Arc<Mutex<AuthorizationState>>,
}

impl Handle {
    /// Send `state` and return once the hub has put it on the client's
    /// bridge, so the next poll finds it.
    fn answer(&self, state: &str) {
        self.feed(auth_json(state));
    }

    fn feed(&self, json: String) {
        self.tx.send(json).expect("send");
        self.fed.set(self.fed.get() + 1);
        // The loop enters its source once before the first envelope and
        // once after each one.
        while self.seen.get() <= self.fed.get() {
            self.entered.recv().expect("the hub reads on");
            self.seen.set(self.seen.get() + 1);
        }
    }

    fn sent(&self) -> Vec<String> {
        self.recorder.snapshot()
    }

    fn finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    fn auth(&self) -> AuthorizationState {
        self.auth.lock().expect("auth").clone()
    }
}

/// What a test hub reads from: the channel the test feeds, and the signal
/// its source gives each time the loop asks for the next envelope.
struct Feed {
    tx: Sender<String>,
    entered: Receiver<()>,
}

/// A hub reading from a channel the test feeds. A stop's wake sends an
/// empty string, which ends a blocked receive.
fn channel_hub() -> (Arc<TdHub>, Feed) {
    let (tx, rx) = mpsc::channel::<String>();
    let (entered_tx, entered) = mpsc::channel();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = entered_tx.send(());
        rx.recv().ok()
    });
    let wake_tx = tx.clone();
    let wake: ReceiveWake = Box::new(move || {
        let _ = wake_tx.send(String::new());
    });
    let hub = TdHub::with_source(source, wake, Arc::new(MemorySink::new())).expect("hub thread");
    (Arc::new(hub), Feed { tx, entered })
}

/// A hub whose receive loop hands over what the test sends, then dies once
/// the channel closes (its source keeps panicking).
fn dying_hub() -> (Arc<TdHub>, Feed) {
    let (tx, rx) = mpsc::channel::<String>();
    let (entered_tx, entered) = mpsc::channel();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = entered_tx.send(());
        match rx.recv() {
            Ok(json) => Some(json),
            Err(_) => panic!("receive source failed"),
        }
    });
    let hub = TdHub::with_source(source, Box::new(|| {}), Arc::new(MemorySink::new()))
        .expect("hub thread");
    (Arc::new(hub), Feed { tx, entered })
}

fn client_on(hub: Arc<TdHub>, feed: Feed) -> (Box<dyn Closable>, Handle) {
    let (client, handle) = test_client(hub, feed);
    (Box::new(client), handle)
}

fn test_client(hub: Arc<TdHub>, feed: Feed) -> (TestClient, Handle) {
    let store = MemorySecretStore::new();
    let (dir, prepared) = prepared_tmp(&store);
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let recorder = Arc::new(RecordingSender::new());
    let seq = AtomicU64::new(0);
    let driver = ready_driver(&recorder, prepared, &sink, &seq);
    let bridge = hub.bridge(CLIENT_ID);
    // The hub numbers envelopes for every client (`seq`); the driver
    // already applied seq 1 (Ready) and drops anything not newer, so use
    // up the hub's first number.
    let Feed { tx, entered } = feed;
    tx.send(format!(
        r#"{{"@type":"updateOption","name":"unix_time","value":{{"@type":"optionValueInteger","value":"1"}},"@client_id":{CLIENT_ID}}}"#
    ))
    .expect("send");
    bridge.rx.recv().expect("the hub hands it over");
    let finished = Arc::new(AtomicBool::new(false));
    let auth = Arc::new(Mutex::new(driver.session.auth.clone()));
    let client = TestClient {
        driver,
        bridge,
        finished: finished.clone(),
        auth: auth.clone(),
        dir,
    };
    let handle = Handle {
        recorder,
        tx,
        entered,
        fed: Cell::new(1),
        seen: Cell::new(0),
        hub,
        finished,
        auth,
    };
    (client, handle)
}

fn client() -> (Box<dyn Closable>, Handle) {
    let (hub, feed) = channel_hub();
    client_on(hub, feed)
}

fn key(n: u32) -> AccountKey {
    AccountKey::new(&format!("account-{n}")).expect("account key")
}

/// Time moves only when a quit sleeps; each sleep first runs `on_sleep`
/// (an answer arriving while the quit waits).
struct ManualClock<F: FnMut()> {
    now: Instant,
    slept: Duration,
    on_sleep: F,
}

fn manual_clock() -> ManualClock<impl FnMut()> {
    ManualClock {
        now: Instant::now(),
        slept: Duration::ZERO,
        on_sleep: || {},
    }
}

impl<F: FnMut()> crate::connect::closing::QuitClock for ManualClock<F> {
    fn now(&self) -> Instant {
        self.now
    }
    fn sleep(&mut self, step: Duration) {
        (self.on_sleep)();
        self.now += step;
        self.slept += step;
    }
}

#[test]
fn push_sends_offline_then_close_and_returns_at_once() {
    let (client, handle) = client();
    let mut set = ClosingSet::new();
    set.push(key(1), client, false);
    let sent = handle.sent();
    let close = sent
        .iter()
        .position(|json| json.contains("\"@type\":\"close\""));
    let offline = sent
        .iter()
        .position(|json| json.contains("\"online\"") && json.contains("false"));
    assert!(close.is_some(), "close sent: {sent:?}");
    assert!(
        offline.is_some() && offline < close,
        "offline first: {sent:?}"
    );
    assert!(set.contains(&key(1)));
    assert!(!set.contains(&key(2)));
}

#[test]
fn a_client_that_never_answers_stays_until_closed_without_waiting() {
    let (client, handle) = client();
    let mut set = ClosingSet::new();
    let now = Instant::now();
    set.push(key(1), client, false);
    assert!(set.poll(now).is_empty());
    // Long past the grace period it is still drained, never dropped: its
    // database may still be open.
    assert!(set.poll(now + CLOSE_GRACE * 10).is_empty());
    assert!(set.may_be_open(&key(1)));
    assert!(!handle.finished());
    handle.answer("authorizationStateClosed");
    let done = set.poll(now);
    assert_eq!(
        done,
        vec![ClosedClient {
            key: key(1),
            restart: false,
            confirmed: true
        }]
    );
    assert!(!set.may_be_open(&key(1)));
    assert!(handle.finished());
}

#[test]
fn a_client_that_answers_closed_leaves_the_set() {
    let (client, handle) = client();
    let mut set = ClosingSet::new();
    set.push(key(2), client, true);
    handle.answer("authorizationStateClosing");
    assert!(set.poll(Instant::now()).is_empty(), "Closing is not Closed");
    assert!(matches!(handle.auth(), AuthorizationState::Closing));
    assert!(set.contains(&key(2)));
    handle.answer("authorizationStateClosed");
    let done = set.poll(Instant::now());
    assert_eq!(
        done,
        vec![ClosedClient {
            key: key(2),
            restart: true,
            confirmed: true
        }]
    );
    assert!(!set.contains(&key(2)));
    assert!(handle.finished());
    // Its route is gone from the hub.
    assert!(!handle.hub.routes().is_registered(CLIENT_ID));
}

#[test]
fn a_dead_receive_loop_is_never_taken_for_closed() {
    let (hub, feed) = dying_hub();
    let (client, handle) = client_on(hub, feed);
    let mut set = ClosingSet::new();
    set.push(key(1), client, true);
    drop(handle.tx);
    handle.hub.join_ended_loop();
    assert!(handle.hub.stopped_unexpectedly());
    let done = set.poll(Instant::now());
    assert_eq!(
        done,
        vec![ClosedClient {
            key: key(1),
            restart: true,
            confirmed: false
        }]
    );
    assert!(set.is_empty(), "nothing more to drain");
    assert!(set.unconfirmed(&key(1)));
    assert!(
        set.may_be_open(&key(1)),
        "TDLib may still be closing it: never reopen or delete"
    );
    assert_eq!(set.keys(), vec![key(1)]);
}

#[test]
fn closed_queued_before_the_receive_loop_died_still_counts() {
    let (hub, feed) = dying_hub();
    let (client, handle) = client_on(hub, feed);
    let mut set = ClosingSet::new();
    set.push(key(1), client, false);
    handle.answer("authorizationStateClosed");
    drop(handle.tx);
    handle.hub.join_ended_loop();
    assert!(handle.hub.stopped_unexpectedly());
    let done = set.poll(Instant::now());
    assert_eq!(done.len(), 1);
    assert!(done.iter().all(|closed| closed.confirmed));
    assert!(!set.may_be_open(&key(1)));
}

#[test]
fn cancelling_restarts_keeps_an_account_from_starting_again() {
    let mut set = ClosingSet::new();
    let (a, a_handle) = client();
    let (b, b_handle) = client();
    set.push(key(3), a, true);
    set.push(key(4), b, true);
    set.cancel_restart(&key(3));
    a_handle.answer("authorizationStateClosed");
    b_handle.answer("authorizationStateClosed");
    let mut done = set.poll(Instant::now());
    done.sort_by(|x, y| x.key.0.cmp(&y.key.0));
    assert_eq!(
        done.iter().map(|closed| closed.restart).collect::<Vec<_>>(),
        vec![false, true]
    );
    let (c, c_handle) = client();
    set.push(key(5), c, true);
    set.cancel_restarts();
    c_handle.answer("authorizationStateClosed");
    let done = set.poll(Instant::now());
    assert_eq!(done.len(), 1);
    assert!(done.iter().all(|closed| !closed.restart));
}

#[test]
fn quit_waits_for_everyone_together_within_one_deadline() {
    let mut set = ClosingSet::new();
    let mut handles = Vec::new();
    for n in 1..=4 {
        let (client, handle) = client();
        set.push(key(n), client, false);
        handles.push(handle);
    }
    let shared = Duration::from_millis(300);
    let mut clock = manual_clock();
    let unconfirmed = set.close_all_on(clock.now + shared, &mut clock);
    assert_eq!(unconfirmed, 4);
    assert!(set.is_empty());
    // One deadline for all four, not 4 x 300 ms.
    assert_eq!(clock.slept, shared, "waited the shared deadline once");
    assert!(handles.iter().all(Handle::finished));
    // Cut off, not confirmed: still may be open for this process.
    assert!((1..=4).all(|n| set.may_be_open(&key(n))));
}

#[test]
fn quit_returns_as_soon_as_every_client_is_closed() {
    let mut set = ClosingSet::new();
    let (a, a_handle) = client();
    let (b, b_handle) = client();
    set.push(key(1), a, false);
    set.push(key(2), b, false);
    a_handle.answer("authorizationStateClosed");
    // `b` answers while the quit waits its first step.
    let mut answered = false;
    let mut clock = ManualClock {
        now: Instant::now(),
        slept: Duration::ZERO,
        on_sleep: || {
            if !std::mem::replace(&mut answered, true) {
                b_handle.answer("authorizationStateClosed");
            }
        },
    };
    let deadline = clock.now + Duration::from_secs(5);
    let unconfirmed = set.close_all_on(deadline, &mut clock);
    assert_eq!(unconfirmed, 0);
    assert_eq!(
        clock.slept,
        Duration::from_millis(5),
        "one step, not the deadline"
    );
    assert!(a_handle.finished() && b_handle.finished());
    assert!(matches!(b_handle.auth(), AuthorizationState::Closed));
    assert!(!set.may_be_open(&key(1)) && !set.may_be_open(&key(2)));
}

#[test]
fn a_closed_client_gets_no_requests_and_leaves_on_the_first_poll() {
    let (mut client, handle) = client();
    handle.answer("authorizationStateClosed");
    // Reach Closed before it is handed over.
    assert_eq!(client.drain(Duration::ZERO), CloseOutcome::Closed);
    let before = handle.sent().len();
    let mut set = ClosingSet::new();
    set.push(key(1), client, false);
    assert_eq!(
        handle.sent().len(),
        before,
        "nothing sent to a Closed client"
    );
    assert_eq!(set.poll(Instant::now()).len(), 1);
}

#[test]
fn a_shut_down_bridge_is_sent_nothing_and_never_counts_as_closed() {
    let (hub, feed) = channel_hub();
    let (mut client, handle) = client_on(hub, feed);
    client.finish();
    let before = handle.sent().len();
    let mut set = ClosingSet::new();
    set.push(key(1), client, false);
    assert_eq!(handle.sent().len(), before);
    let done = set.poll(Instant::now());
    assert_eq!(done.len(), 1);
    assert!(done.iter().all(|closed| !closed.confirmed));
    assert!(set.may_be_open(&key(1)));
}

#[test]
fn dropping_the_set_finishes_its_clients_without_waiting_for_them() {
    let (client, handle) = client();
    let mut set = ClosingSet::new();
    set.push(key(1), client, false);
    // The client never answers; the drop lets it go instead of waiting.
    drop(set);
    assert!(handle.finished());
}

const CALL_ID: i32 = 5;

fn active_call(is_outgoing: bool, state: CallState) -> ActiveCall {
    ActiveCall {
        id: CALL_ID,
        user_id: 11,
        is_outgoing,
        is_video: false,
        progress: crate::state::CallProgress::first(state, Instant::now()),
        action: crate::state::CallAction::Idle,
        started_at: Instant::now(),
        transport: Default::default(),
        signaling_queue: Vec::new(),
        signaling_dropped: 0,
        muted: false,
        video: crate::state::LocalVideo::Off,
        remote_video: RemoteVideoState::Inactive,
        remote_screen: RemoteVideoState::Inactive,
        remote_audio_muted: false,
        remote_battery_low: false,
        signal_bars: None,
        media_note: None,
    }
}

fn call_discarded_json() -> String {
    format!(
        r#"{{"@type":"updateCall","call":{{"@type":"call","id":{CALL_ID},"user_id":11,"is_outgoing":true,"is_video":false,"state":{{"@type":"callStateDiscarded","reason":{{"@type":"callDiscardReasonHungUp"}},"need_rating":false,"need_debug_information":false,"need_log":false}}}},"@client_id":{CLIENT_ID}}}"#
    )
}

struct CallingDriver {
    recorder: Arc<RecordingSender>,
    driver: ConnectDriver<Arc<RecordingSender>>,
    dir: std::path::PathBuf,
}

impl Drop for CallingDriver {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn calling_driver() -> CallingDriver {
    let store = MemorySecretStore::new();
    let (dir, prepared) = prepared_tmp(&store);
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let recorder = Arc::new(RecordingSender::new());
    let seq = AtomicU64::new(0);
    let driver = ready_driver(&recorder, prepared, &sink, &seq);
    recorder.sent.lock().expect("sent").clear();
    CallingDriver {
        recorder,
        driver,
        dir,
    }
}

impl CallingDriver {
    fn with_call(mut self, is_outgoing: bool, state: CallState) -> Self {
        self.driver.session.calls.active_call = Some(active_call(is_outgoing, state));
        self
    }

    fn with_group_call(mut self, is_joined: bool) -> Self {
        let mut call = tracked_group_call(false, false, false);
        call.is_joined = is_joined;
        self.driver.session.calls.active_group_call = Some(call);
        self
    }

    fn sent(&self) -> Vec<String> {
        self.recorder
            .snapshot()
            .iter()
            .map(|json| {
                let value: serde_json::Value = serde_json::from_str(json).expect("request json");
                value["@type"].as_str().unwrap_or_default().to_string()
            })
            .collect()
    }

    fn feed(&mut self, json: &str) {
        let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
        let seq = AtomicU64::new(100);
        let update = copy_and_parse(json, &seq, &sink).expect("update");
        self.driver.ingest(update).expect("ingest");
    }
}

fn the_hold_is_over() -> Instant {
    crate::connect::closing::deadline_after(crate::connect::closing::HANG_UP_HOLD * 2)
}

#[test]
fn closing_hangs_up_a_running_call_and_sends_close_behind_the_terminal_update() {
    let mut client = calling_driver().with_call(true, CallState::Ready(Default::default()));
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(client.sent(), ["discardCall", "setOption"]);
    assert!(
        !client.driver.chats_path_active(),
        "an account on its way out starts nothing while close is held"
    );

    let before_the_hold_ends = Instant::now();
    crate::connect::closing::release_close(&mut client.driver, before_the_hold_ends);
    assert!(
        !client.sent().iter().any(|sent| sent == "close"),
        "close waits for the hang-up"
    );

    client.feed(&call_discarded_json());
    crate::connect::closing::release_close(&mut client.driver, before_the_hold_ends);
    assert_eq!(client.sent().last().map(String::as_str), Some("close"));
}

#[test]
fn what_a_stopped_export_left_queued_goes_out_ahead_of_close() {
    let mut client = calling_driver();
    let (export, worker) = crate::export::ChatExport::scripted();
    export.cancel();
    client.driver.session.settings.chat_export = Some(export);
    for request in ["cancelDownloadFile", "deleteFile"] {
        let request = serde_json::json!({"@type": request, "file_id": 11});
        worker.send(request).expect("queued");
    }
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(
        client.sent(),
        ["cancelDownloadFile", "deleteFile", "setOption", "close"]
    );
}

#[test]
fn a_hang_up_nobody_answers_holds_close_back_only_so_long() {
    let mut client = calling_driver().with_call(false, CallState::Ready(Default::default()));
    crate::connect::closing::begin_close(&mut client.driver);
    crate::connect::closing::release_close(&mut client.driver, the_hold_is_over());
    assert_eq!(client.sent(), ["discardCall", "setOption", "close"]);

    crate::connect::closing::release_close(&mut client.driver, the_hold_is_over());
    assert_eq!(client.sent().len(), 3, "close goes out once");
}

#[test]
fn a_call_still_ringing_in_is_left_to_ring() {
    let mut client = calling_driver().with_call(
        false,
        CallState::Pending {
            is_created: true,
            is_received: true,
        },
    );
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(client.sent(), ["setOption", "close"]);
}

#[test]
fn a_call_answered_but_not_yet_moved_on_is_hung_up() {
    let mut client = calling_driver().with_call(
        false,
        CallState::Pending {
            is_created: true,
            is_received: true,
        },
    );
    client
        .driver
        .set_call_engine(Box::new(crate::calls::engine::MockEngine::new()));
    client.driver.accept_call().expect("accept");
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(client.sent(), ["acceptCall", "discardCall", "setOption"]);
}

#[test]
fn a_call_whose_answer_failed_is_left_to_ring() {
    let mut client = calling_driver().with_call(
        false,
        CallState::Pending {
            is_created: true,
            is_received: true,
        },
    );
    client
        .driver
        .set_call_engine(Box::new(crate::calls::engine::MockEngine::new()));
    let extra = client.driver.accept_call().expect("accept");
    client.feed(&format!(
        r#"{{"@type":"error","code":400,"message":"CALL_PROTOCOL_FLAGS_INVALID","@extra":"{}","@client_id":{CLIENT_ID}}}"#,
        extra.0
    ));
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(client.sent(), ["acceptCall", "setOption", "close"]);
}

/// r2-14, 02-032: a second click on Accept while the first `acceptCall` is
/// out sends nothing and returns the first request.
#[test]
fn a_second_accept_click_sends_one_accept() {
    let mut client = calling_driver().with_call(
        false,
        CallState::Pending {
            is_created: true,
            is_received: true,
        },
    );
    client
        .driver
        .set_call_engine(Box::new(crate::calls::engine::MockEngine::new()));
    let first = client.driver.accept_call().expect("accept");
    let second = client.driver.accept_call().expect("accept again");
    assert_eq!(first, second);
    assert_eq!(client.sent(), ["acceptCall"]);
}

#[test]
fn a_late_answer_failure_for_an_earlier_call_still_hangs_up_the_answered_one() {
    let ringing = CallState::Pending {
        is_created: true,
        is_received: true,
    };
    let mut client = calling_driver().with_call(false, ringing.clone());
    client
        .driver
        .set_call_engine(Box::new(crate::calls::engine::MockEngine::new()));
    let first = client.driver.accept_call().expect("accept");
    let mut next = active_call(false, ringing);
    next.id = CALL_ID + 1;
    client.driver.session.calls.active_call = Some(next);
    client.driver.accept_call().expect("accept");
    client.feed(&format!(
        r#"{{"@type":"error","code":400,"message":"CALL_ALREADY_DECLINED","@extra":"{}","@client_id":{CLIENT_ID}}}"#,
        first.0
    ));
    assert_eq!(
        client.driver.session.calls.error, None,
        "not this call's note"
    );
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(
        client.sent(),
        ["acceptCall", "acceptCall", "discardCall", "setOption"]
    );
}

#[test]
fn an_outgoing_call_still_ringing_is_hung_up() {
    let mut client = calling_driver().with_call(
        true,
        CallState::Pending {
            is_created: true,
            is_received: false,
        },
    );
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(client.sent(), ["discardCall", "setOption"]);
}

#[test]
fn a_call_already_hanging_up_gets_no_second_discard_but_still_holds_close() {
    let mut client = calling_driver().with_call(true, CallState::HangingUp);
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(client.sent(), ["setOption"]);
    crate::connect::closing::release_close(&mut client.driver, the_hold_is_over());
    assert_eq!(client.sent(), ["setOption", "close"]);
}

#[test]
fn closing_leaves_a_joined_group_call_and_sends_close_behind_the_answer() {
    let mut client = calling_driver().with_group_call(true);
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(client.sent(), ["leaveGroupCall", "setOption"]);

    let leave = client
        .recorder
        .snapshot()
        .into_iter()
        .next()
        .expect("the leave request");
    let leave: serde_json::Value = serde_json::from_str(&leave).expect("request json");
    let extra = leave["@extra"].as_str().expect("request id").to_string();
    client.feed(&format!(r#"{{"@type":"ok","@extra":"{extra}"}}"#));
    assert!(
        client.driver.session.calls.active_group_call.is_none(),
        "the answer drops the tracked call"
    );
    crate::connect::closing::release_close(&mut client.driver, Instant::now());
    assert_eq!(client.sent().last().map(String::as_str), Some("close"));
}

#[test]
fn a_group_call_only_looked_at_is_not_left() {
    let mut client = calling_driver().with_group_call(false);
    crate::connect::closing::begin_close(&mut client.driver);
    assert_eq!(client.sent(), ["setOption", "close"]);
}

#[test]
fn a_closing_client_sends_close_from_its_poll_once_its_call_ended() {
    let (hub, feed) = dying_hub();
    let (mut client, handle) = test_client(hub, feed);
    client.driver.session.calls.active_call =
        Some(active_call(true, CallState::Ready(Default::default())));
    let mut set = ClosingSet::new();
    set.push(key(1), Box::new(client), false);
    let recorder = handle.recorder.clone();
    let sent_close = || {
        recorder
            .snapshot()
            .iter()
            .any(|json| json.contains("\"@type\":\"close\""))
    };
    assert!(
        handle
            .sent()
            .iter()
            .any(|json| json.contains("discardCall"))
    );
    assert!(!sent_close(), "close waits for the hang-up");

    handle.feed(call_discarded_json());
    drop(handle.tx);
    handle.hub.join_ended_loop();
    // A poll applies at least one waiting envelope, and may stop there when
    // its budget ran out (a preempted thread): the second one then finds
    // the queue dry.
    let mut finished = set.poll(Instant::now());
    if finished.is_empty() {
        finished = set.poll(Instant::now());
    }
    assert_eq!(finished.len(), 1);
    assert!(
        finished.iter().all(|closed| !closed.confirmed),
        "the test's receive loop ends with no Closed"
    );
    assert!(
        sent_close(),
        "the poll sent close: {:?}",
        recorder.snapshot()
    );
}

#[test]
fn a_running_call_or_joined_voice_chat_is_a_call_in_progress() {
    assert!(!calling_driver().driver.call_in_progress());
    assert!(
        calling_driver()
            .with_call(true, CallState::Ready(Default::default()))
            .driver
            .call_in_progress()
    );
    assert!(
        !calling_driver()
            .with_call(true, CallState::HangingUp)
            .driver
            .call_in_progress()
    );
    let ringing = CallState::Pending {
        is_created: true,
        is_received: true,
    };
    assert!(
        !calling_driver()
            .with_call(false, ringing)
            .driver
            .call_in_progress(),
        "a call ringing in can be answered elsewhere"
    );
    assert!(
        calling_driver()
            .with_group_call(true)
            .driver
            .call_in_progress()
    );
    assert!(
        !calling_driver()
            .with_group_call(false)
            .driver
            .call_in_progress()
    );
}
