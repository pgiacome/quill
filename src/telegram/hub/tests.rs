use super::*;
use crate::diagnostics::MemorySink;
use crate::telegram::envelope::{AuthPayload, AuthorizationState, EnvelopePayload};

/// A hub reading from a channel, so tests decide what `td_receive` returns.
/// A stop's wake sends an empty string, which ends a blocked receive.
fn channel_hub() -> (Arc<TdHub>, Sender<String>, Arc<MemorySink>) {
    let (tx, rx) = mpsc::channel::<String>();
    let source: ReceiveSource = Box::new(move |_| rx.recv().ok());
    let sink = Arc::new(MemorySink::new());
    let hub = TdHub::with_source(source, wake_on(&tx), sink.clone()).expect("hub thread");
    (Arc::new(hub), tx, sink)
}

fn wake_on(tx: &Sender<String>) -> ReceiveWake {
    let tx = tx.clone();
    Box::new(move || {
        let _ = tx.send(String::new());
    })
}

fn no_wake() -> ReceiveWake {
    Box::new(|| {})
}

fn option_update(client: Option<i32>, value: i64) -> String {
    let client = client
        .map(|id| format!(r#","@client_id":{id}"#))
        .unwrap_or_default();
    format!(
        r#"{{"@type":"updateOption","name":"unix_time","value":{{"@type":"optionValueInteger","value":"{value}"}}{client}}}"#
    )
}

#[test]
fn envelopes_go_to_the_client_they_name_in_order() {
    let (hub, tx, sink) = channel_hub();
    let first = hub.bridge(1);
    let second = hub.bridge(2);
    for json in [
        option_update(Some(1), 10),
        option_update(Some(2), 20),
        option_update(Some(3), 30),
        option_update(None, 40),
        option_update(Some(1), 11),
    ] {
        tx.send(json).unwrap();
    }
    let a = first.rx.recv().unwrap();
    let b = first.rx.recv().unwrap();
    let c = second.rx.recv().unwrap();
    assert_eq!(
        (a.client_id, b.client_id, c.client_id),
        (Some(1), Some(1), Some(2))
    );
    assert!(a.seq < b.seq, "one client's envelopes keep their order");
    assert!(matches!(a.envelope.payload, EnvelopePayload::Common(_)));
    // Client 3 is not registered and the update before `b` names no
    // client: both were dropped before `b` was read, never handed to
    // another account.
    assert!(first.rx.try_recv().is_err());
    assert!(second.rx.try_recv().is_err());
    assert_eq!(sink.rendered().matches("unrouted-client").count(), 2);
}

#[test]
fn a_shut_down_bridge_stops_receiving_and_frees_its_route() {
    let (hub, tx, sink) = channel_hub();
    let mut closing = hub.bridge(5);
    let staying = hub.bridge(6);
    assert!(!closing.is_joined());
    closing.shutdown();
    assert!(closing.is_joined());
    assert!(!hub.routes().is_registered(5));
    assert!(hub.routes().is_registered(6));
    tx.send(option_update(Some(5), 1)).unwrap();
    tx.send(option_update(Some(6), 2)).unwrap();
    assert_eq!(
        staying.rx.recv().ok().and_then(|owned| owned.client_id),
        Some(6)
    );
    // Client 5's envelope came first, so it was dropped by now.
    assert!(sink.rendered().contains("unrouted-client"));
    // Shutting down twice is harmless.
    closing.shutdown();
}

#[test]
fn a_dropped_reader_loses_its_route() {
    let (hub, tx, sink) = channel_hub();
    let routes = hub.routes();
    let rx = routes.register(9);
    let after = hub.bridge(10);
    drop(rx);
    tx.send(option_update(Some(9), 1)).unwrap();
    tx.send(option_update(Some(10), 1)).unwrap();
    // The loop handles envelopes in order: client 9's is done by now.
    after.rx.recv().unwrap();
    assert!(!routes.is_registered(9));
    assert!(sink.rendered().contains("client-reader-gone"));
}

#[test]
fn deliver_reports_each_outcome() {
    let routes = Routes::default();
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let seq = AtomicU64::new(0);
    let parse = |json: String| copy_and_parse(&json, &seq, &sink).unwrap();
    let rx = routes.register(4);
    assert_eq!(
        routes.deliver(parse(option_update(Some(4), 1))),
        Delivery::Delivered
    );
    assert_eq!(rx.try_recv().unwrap().client_id, Some(4));
    assert_eq!(
        routes.deliver(parse(option_update(Some(8), 1))),
        Delivery::Unrouted
    );
    assert_eq!(
        routes.deliver(parse(option_update(None, 1))),
        Delivery::Unrouted
    );
    drop(rx);
    assert_eq!(
        routes.deliver(parse(option_update(Some(4), 1))),
        Delivery::ReaderGone
    );
    assert!(!routes.is_registered(4));
}

#[test]
fn the_receive_thread_sleeps_while_no_client_is_registered() {
    // Each call reports whether a client was registered when it came.
    let registered = Arc::new(AtomicBool::new(false));
    let (entered_tx, entered) = mpsc::channel::<bool>();
    let (wake_tx, wake_rx) = mpsc::channel::<()>();
    let seen = registered.clone();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = entered_tx.send(seen.load(Ordering::SeqCst));
        let _ = wake_rx.recv();
        None
    });
    let wake: ReceiveWake = Box::new(move || {
        let _ = wake_tx.send(());
    });
    let hub = Arc::new(TdHub::with_source(source, wake, Arc::new(MemorySink::new())).unwrap());
    registered.store(true, Ordering::SeqCst);
    let mut bridge = hub.bridge(1);
    assert!(entered.recv().unwrap(), "no client, no td_receive");
    bridge.shutdown();
    assert!(hub.stop_within(Duration::MAX));
    // A hub stopped with no client never called td_receive at all.
    let (wake_tx, _wake_rx) = mpsc::channel::<()>();
    let (called_tx, called) = mpsc::channel::<()>();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = called_tx.send(());
        None
    });
    let wake: ReceiveWake = Box::new(move || {
        let _ = wake_tx.send(());
    });
    let idle = TdHub::with_source(source, wake, Arc::new(MemorySink::new())).unwrap();
    assert!(idle.stop_within(Duration::MAX));
    assert!(called.try_recv().is_err());
}

#[test]
fn a_dead_receive_thread_marks_every_bridge_stopped() {
    let (tx, rx) = mpsc::channel::<()>();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = rx.recv();
        panic!("receive source failed");
    });
    let hub = Arc::new(TdHub::with_source(source, no_wake(), Arc::new(MemorySink::new())).unwrap());
    let first = hub.bridge(1);
    let second = hub.bridge(2);
    assert!(!first.stopped_unexpectedly());
    // The loop survives single panics; a source that keeps failing (the
    // closed channel panics at once from now on) ends it.
    tx.send(()).unwrap();
    drop(tx);
    hub.join_ended_loop();
    assert!(first.stopped_unexpectedly());
    assert!(second.stopped_unexpectedly());
    assert!(hub.stopped_unexpectedly());
}

#[test]
fn one_panicking_envelope_does_not_end_the_receive_thread() {
    let (tx, rx) = mpsc::channel::<String>();
    let mut panicked = false;
    let source: ReceiveSource = Box::new(move |_| {
        let json = rx.recv().ok()?;
        if !panicked {
            panicked = true;
            panic!("one envelope failed to parse");
        }
        Some(json)
    });
    let hub = TdHub::with_source(source, wake_on(&tx), Arc::new(MemorySink::new())).unwrap();
    let hub = Arc::new(hub);
    let bridge = hub.bridge(1);
    tx.send(option_update(Some(1), 1)).unwrap();
    tx.send(option_update(Some(1), 2)).unwrap();
    // The first envelope is lost with its panic; the second one arrives.
    let owned = bridge.rx.recv().unwrap();
    assert_eq!(owned.client_id, Some(1));
    assert!(!bridge.stopped_unexpectedly());
    assert!(!hub.stopped_unexpectedly());
}

#[test]
fn a_stopped_shared_hub_is_replaced_but_a_running_one_is_kept() {
    assert!(matches!(reusable(None), Reuse::None));
    let (live, _tx, _sink) = channel_hub();
    assert!(matches!(reusable(Some(&live)), Reuse::Running(_)));

    let (dying_tx, dying_rx) = mpsc::channel::<()>();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = dying_rx.recv();
        panic!("receive source failed");
    });
    let dead =
        Arc::new(TdHub::with_source(source, no_wake(), Arc::new(MemorySink::new())).unwrap());
    // The loop only reads while a client is registered.
    let _bridge = dead.bridge(1);
    dying_tx.send(()).unwrap();
    drop(dying_tx);
    dead.join_ended_loop();
    assert!(dead.stopped_unexpectedly());
    assert!(matches!(reusable(Some(&dead)), Reuse::Stopped(..)));
}

/// Sweep 06-065: a hub replacing a stopped one takes over its routes, so a
/// client that was open keeps receiving, and its bridge reads running.
#[test]
fn a_replacement_hub_keeps_the_open_clients_routed() {
    let (old_tx, old_rx) = mpsc::channel::<String>();
    let source: ReceiveSource = Box::new(move |_| match old_rx.recv() {
        Ok(json) => Some(json),
        Err(_) => panic!("receive source failed"),
    });
    let dead =
        Arc::new(TdHub::with_source(source, no_wake(), Arc::new(MemorySink::new())).unwrap());
    let bridge = dead.bridge(1);
    // The old hub delivers a few envelopes, so the client's session has seen
    // a high seq before the hub dies.
    let sink = Arc::new(MemorySink::new());
    let mut session = crate::state::Session::new(crate::ids::AccountKey::primary(), sink);
    for n in 0..3 {
        old_tx.send(option_update(Some(1), n)).unwrap();
        let owned = bridge.rx.recv().expect("the old hub delivers");
        session.apply(owned);
    }
    let seen = session.last_seq;
    assert!(seen > 0);
    drop(old_tx);
    dead.join_ended_loop();
    assert!(bridge.stopped_unexpectedly());

    let (tx, rx) = mpsc::channel::<String>();
    let source: ReceiveSource = Box::new(move |_| rx.recv().ok());
    let replacement = Arc::new(
        TdHub::with_source_replacing(source, wake_on(&tx), Arc::new(MemorySink::new()), &dead)
            .unwrap(),
    );
    assert!(!bridge.stopped_unexpectedly(), "running again");
    tx.send(option_update(Some(1), 5)).unwrap();
    let owned = bridge
        .rx
        .recv()
        .expect("the old client's envelope is routed on the new hub");
    assert_eq!(owned.client_id, Some(1));
    assert!(!replacement.stopped_unexpectedly());
    // The numbering continues, so the surviving session applies it.
    assert!(owned.seq > seen, "seq continues past the old hub's");
    let revision = session.revision;
    session.apply(owned);
    assert_eq!(
        session.last_seq,
        seen + 1,
        "applied, not out-of-order-ignored"
    );
    assert!(session.revision > revision);
}

#[test]
fn a_requested_shutdown_is_not_a_failure() {
    let (hub, _tx, _sink) = channel_hub();
    let bridge = hub.bridge(1);
    hub.shutdown();
    assert!(!bridge.stopped_unexpectedly());
}

/// Three real TDLib clients on the shared hub each get only their own
/// envelopes. No network and no account: `getAuthorizationState` before
/// `setTdlibParameters`. Run with
/// `QUILL_TDJSON_PATH=<libtdjson> cargo test --no-default-features live_clients -- --ignored`.
#[test]
#[ignore = "needs libtdjson (QUILL_TDJSON_PATH)"]
fn live_clients_get_only_their_own_envelopes() {
    let Ok(hub) = TdHub::shared(Arc::new(MemorySink::new())) else {
        eprintln!("libtdjson not found; skipped");
        return;
    };
    let api = hub.api().expect("live hub").clone();
    let ids: Vec<i32> = (0..3).map(|_| api.create_client_id()).collect();
    let bridges: Vec<ReceiveBridge> = ids.iter().map(|&id| hub.bridge(id)).collect();
    for (n, &id) in ids.iter().enumerate() {
        let request = format!(
            r#"{{"@type":"getAuthorizationState","@extra":"{}"}}"#,
            100 + n
        );
        api.send(id, &request).unwrap();
    }
    for (n, (bridge, &id)) in bridges.iter().zip(&ids).enumerate() {
        let wanted = crate::ids::RequestId(100 + n as u64);
        // TDLib answers every request, so the blocking receive ends.
        loop {
            let owned = bridge
                .rx
                .recv()
                .unwrap_or_else(|_| panic!("client {id} got its answer"));
            assert_eq!(owned.client_id, Some(id), "another client's envelope");
            if owned.envelope.extra == Some(wanted) {
                break;
            }
        }
    }
    // Close every client before the process exits: TDLib's static
    // destructors racing live clients crash at exit now and then (3 in 60
    // runs left open, 0 in 60 closed), which is why `LiveConnect` always
    // closes its client.
    for (bridge, &id) in bridges.iter().zip(&ids) {
        api.send(id, r#"{"@type":"close"}"#).unwrap();
        // A closed client always reports Closed last.
        loop {
            let owned = bridge
                .rx
                .recv()
                .unwrap_or_else(|_| panic!("client {id} closed"));
            if matches!(
                owned.envelope.payload,
                EnvelopePayload::Auth(AuthPayload::UpdateAuthorizationState(
                    AuthorizationState::Closed
                ))
            ) {
                break;
            }
        }
    }
}

#[test]
fn a_stop_ends_the_receive_thread_with_or_without_clients() {
    let (idle, _tx, _sink) = channel_hub();
    assert!(idle.stop_within(Duration::MAX));

    let (busy, _tx, _sink) = channel_hub();
    let bridge = busy.bridge(1);
    assert!(busy.stop_within(Duration::MAX));
    assert!(!bridge.stopped_unexpectedly());
}

#[test]
fn a_stop_wakes_a_receive_that_is_blocked() {
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (wake_tx, wake_rx) = mpsc::channel::<()>();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = entered_tx.send(());
        let _ = wake_rx.recv();
        None
    });
    let wake: ReceiveWake = Box::new(move || {
        let _ = wake_tx.send(());
    });
    let hub = TdHub::with_source(source, wake, Arc::new(MemorySink::new())).unwrap();
    let hub = Arc::new(hub);
    let _client = hub.bridge(1);
    entered_rx.recv().unwrap();
    assert!(hub.stop_within(Duration::MAX));
}

#[test]
fn what_ends_the_receive_of_a_stopped_hub_is_not_delivered() {
    let (tx, rx) = mpsc::channel::<String>();
    let (entered_tx, entered) = mpsc::channel();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = entered_tx.send(());
        rx.recv().ok()
    });
    let wake: ReceiveWake = Box::new(move || {
        let _ = tx.send(
            r#"{"@type":"error","code":400,"message":"Invalid TDLib instance specified"}"#.into(),
        );
    });
    let sink = Arc::new(MemorySink::new());
    let hub = Arc::new(TdHub::with_source(source, wake, sink.clone()).unwrap());
    let _client = hub.bridge(1);
    entered.recv().unwrap();
    assert!(hub.stop_within(Duration::MAX));
    assert!(sink.snapshot().is_empty());
}

#[test]
fn a_stop_gives_up_on_a_receive_that_does_not_return() {
    let (release, released) = mpsc::channel::<()>();
    let (entered_tx, entered) = mpsc::channel();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = entered_tx.send(());
        let _ = released.recv();
        None
    });
    let hub = Arc::new(TdHub::with_source(source, no_wake(), Arc::new(MemorySink::new())).unwrap());
    let _client = hub.bridge(1);
    entered.recv().unwrap();
    assert!(!hub.stop_within(Duration::ZERO));
    // The timed-out stop kept the handle: the receive returning lets a
    // later stop join the thread.
    drop(release);
    assert!(hub.stop_within(Duration::MAX));
    assert!(hub.run.stop_requested());
}

#[test]
#[ignore = "QUILL_TDJSON_PATH=<libtdjson> cargo test --lib blocked_receive -- --ignored"]
fn a_send_to_an_unmade_client_ends_a_blocked_receive() {
    let api = Arc::new(TdJson::load_default().expect("libtdjson"));
    let receive = api.clone();
    let (entering_tx, entering_rx) = mpsc::channel::<()>();
    let thread = std::thread::spawn(move || {
        let _ = entering_tx.send(());
        receive.receive(60.0)
    });
    entering_rx.recv().unwrap();
    api.wake_receive();
    let answer = thread.join().unwrap().expect("an answer, not the timeout");
    assert!(answer.contains("Invalid TDLib instance"), "{answer}");
}

#[test]
fn each_route_wakes_only_its_own_poll_loop_and_a_switch_swaps_them() {
    static ON_SCREEN: crate::wake::Signal = crate::wake::Signal::new();
    static BACKGROUND: crate::wake::Signal = crate::wake::Signal::new();
    let routes = Routes::default();
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let seq = AtomicU64::new(0);
    let parse = |json: String| copy_and_parse(&json, &seq, &sink).unwrap();
    let _first = routes.register(1);
    let _second = routes.register(2);
    routes.wake_with(1, &ON_SCREEN);
    routes.wake_with(2, &BACKGROUND);
    let epochs = || (ON_SCREEN.epoch(), BACKGROUND.epoch());
    let before = epochs();
    routes.deliver(parse(option_update(Some(2), 1)));
    assert_eq!(epochs(), (before.0, before.1 + 1));
    routes.deliver(parse(option_update(Some(1), 1)));
    assert_eq!(epochs(), (before.0 + 1, before.1 + 1));
    // Account switch: client 2 comes on screen, client 1 goes behind.
    routes.wake_with(2, &ON_SCREEN);
    routes.wake_with(1, &BACKGROUND);
    routes.deliver(parse(option_update(Some(2), 2)));
    routes.deliver(parse(option_update(Some(2), 3)));
    assert_eq!(epochs(), (before.0 + 3, before.1 + 1));
    routes.deliver(parse(option_update(Some(1), 2)));
    assert_eq!(epochs(), (before.0 + 3, before.1 + 2));
}

fn auth_state(state: &str) -> String {
    format!(
        r#"{{"@type":"updateAuthorizationState","authorization_state":{{"@type":"{state}"}},"@client_id":1}}"#
    )
}

#[test]
fn the_launch_chat_page_answer_ends_the_launch_wait() {
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let seq = AtomicU64::new(0);
    let gate = LaunchChats::new();
    gate.claim(1);
    let see = |json: String| gate.observe(&json, &copy_and_parse(&json, &seq, &sink).unwrap());
    see(auth_state("authorizationStateWaitTdlibParameters"));
    see(auth_state("authorizationStateReady"));
    see(option_update(Some(1), 1));
    see(r#"{"@type":"ok","@extra":7,"@client_id":1}"#.to_owned());
    assert!(!gate.is_in());
    see(format!(
        r#"{{"@type":"ok","@extra":"{LAUNCH_CHATS_EXTRA}","@client_id":1}}"#
    ));
    assert!(gate.is_in());
    assert_eq!(gate.wait(Duration::ZERO), LaunchWait::Came);
}

#[test]
fn a_signed_out_account_ends_the_launch_wait() {
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let seq = AtomicU64::new(0);
    let gate = LaunchChats::new();
    gate.claim(1);
    let json = auth_state("authorizationStateWaitPhoneNumber");
    gate.observe(&json, &copy_and_parse(&json, &seq, &sink).unwrap());
    assert!(gate.is_in());
}

#[test]
fn the_launch_waits_only_for_a_page_tdlib_was_asked_for() {
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let seq = AtomicU64::new(0);
    let starting = LaunchChats::new();
    assert_eq!(starting.wait(Duration::ZERO), LaunchWait::Skipped);
    assert!(starting.is_in());
    let asked = LaunchChats::new();
    asked.claim(1);
    let json = auth_state("authorizationStateReady");
    asked.observe(&json, &copy_and_parse(&json, &seq, &sink).unwrap());
    assert_eq!(asked.wait(Duration::ZERO), LaunchWait::Capped);
    assert_eq!(asked.wait(Duration::ZERO), LaunchWait::Came);
}

#[test]
fn another_accounts_states_do_not_move_the_launch_wait() {
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let seq = AtomicU64::new(0);
    let gate = LaunchChats::new();
    gate.claim(2);
    // Client 1 (a background account) is ready first, and signs out later.
    let json = auth_state("authorizationStateReady");
    gate.observe(&json, &copy_and_parse(&json, &seq, &sink).unwrap());
    assert_eq!(gate.wait(Duration::ZERO), LaunchWait::Skipped);
}

#[test]
fn a_new_claim_starts_the_launch_wait_over_for_its_client() {
    let sink: Arc<dyn DiagnosticSink> = Arc::new(MemorySink::new());
    let seq = AtomicU64::new(0);
    let gate = LaunchChats::new();
    gate.claim(2);
    let json = auth_state("authorizationStateReady");
    gate.observe(&json, &copy_and_parse(&json, &seq, &sink).unwrap());
    // Client 1 ready first changed nothing; now it takes the screen.
    gate.claim(1);
    assert!(!gate.is_in());
    gate.observe(&json, &copy_and_parse(&json, &seq, &sink).unwrap());
    assert_eq!(gate.wait(Duration::ZERO), LaunchWait::Capped);
}

#[test]
fn a_replacement_loop_reports_running_without_the_old_hub_touching_it() {
    let (tx, rx) = mpsc::channel::<()>();
    let source: ReceiveSource = Box::new(move |_| {
        let _ = rx.recv();
        panic!("receive source failed");
    });
    let dead =
        Arc::new(TdHub::with_source(source, no_wake(), Arc::new(MemorySink::new())).unwrap());
    let bridge = dead.bridge(1);
    tx.send(()).unwrap();
    drop(tx);
    // The loop gives up after its run of panics.
    dead.join_ended_loop();
    assert!(bridge.stopped_unexpectedly());
    let (live_tx, live_rx) = mpsc::channel::<String>();
    let source: ReceiveSource = Box::new(move |_| live_rx.recv().ok());
    let replacement = TdHub::with_source_replacing(
        source,
        wake_on(&live_tx),
        Arc::new(MemorySink::new()),
        &dead,
    )
    .unwrap();
    assert!(!bridge.stopped_unexpectedly());
    // The dead hub being dropped later stops only its own (ended) loop.
    drop(dead);
    assert!(!bridge.stopped_unexpectedly());
    assert!(!replacement.stopped_unexpectedly());
}
