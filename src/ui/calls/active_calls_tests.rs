//! The Calls tab's active group calls: the section above the history, its
//! order (the session's) and the Join that asks for a call. The demo app
//! has no call driver, so a non-busy Join leaves the wish for the window
//! that will track the call; a busy one offers the call (or, without a
//! chat to offer, says to end the current call first).
use crate::ui::app::QuillApp;
use crate::ui::chat_list_state::SidebarTab;
use crate::ui::group_call_stream_layout::StreamJoinIntent;
use crate::ui::keybindings::bind_keys;
use crate::ui::screenshot_demo::ScreenshotDemo;
use crate::ui::shell::QuillShell;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, TestAppContext, VisualTestContext, px, size};
use quill::diagnostics::DiagnosticSink;
use quill::lang::{keys, tr};
use quill::state::LiveGroupCall;
use quill::telegram::client::copy_and_parse;
use std::sync::Arc;

fn new_app(cx: &mut TestAppContext) -> (Entity<QuillApp>, VisualTestContext) {
    cx.update(gpui_kit::init);
    cx.update(bind_keys);
    let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot_in = slot.clone();
    let handle = cx.open_window(size(px(1100.), px(800.)), move |window, cx| {
        let view = cx.new(|cx| {
            QuillApp::new_with_demo(
                window,
                cx,
                None,
                Some(ScreenshotDemo::named("ready-calls-history")),
            )
        });
        let shell = cx.new(|_| QuillShell::new(view.clone()));
        *slot_in.borrow_mut() = Some(view);
        Root::new(shell, window, cx)
    });
    let app = slot.borrow().clone().unwrap();
    let vcx = VisualTestContext::from_window(handle.into(), cx);
    (app, vcx)
}

fn frames(vcx: &mut VisualTestContext) {
    vcx.run_until_parked();
    for _ in 0..2 {
        vcx.update(|window, cx| window.render_frame(cx));
    }
}

/// One envelope through the demo session's own parser, as the demos feed
/// their fixtures.
fn feed(app: &mut QuillApp, json: &str) {
    let last = app.session().map_or(0, |session| session.last_seq);
    app.demo_ui
        .seq
        .store(last, std::sync::atomic::Ordering::SeqCst);
    let sink: Arc<dyn DiagnosticSink> = app.demo_ui.sink.clone();
    let Some(owned) = copy_and_parse(json, &app.demo_ui.seq, &sink) else {
        return;
    };
    if let Some(session) = app.demo_session.as_deref_mut() {
        session.apply(owned);
    }
}

/// Two running calls with their chats: a live stream (61, "Design live")
/// and a voice chat (62, "Team standup").
fn seed_two_calls(app: &mut QuillApp) {
    feed(
        app,
        r#"{"@type":"updateNewChat","chat":{"id":51,"title":"Design live",
        "type":{"@type":"chatTypeSupergroup","supergroup_id":51,"is_channel":true},"unread_count":0}}"#,
    );
    feed(
        app,
        r#"{"@type":"updateChatVideoChat","chat_id":51,
        "video_chat":{"@type":"videoChat","group_call_id":61,"has_participants":true,"default_participant_id":null}}"#,
    );
    feed(
        app,
        r#"{"@type":"updateGroupCall","group_call":{"@type":"groupCall","id":61,
        "title":"Design live","is_active":true,"is_video_chat":false,"is_joined":false,
        "is_rtmp_stream":true,"participant_count":12}}"#,
    );
    feed(
        app,
        r#"{"@type":"updateNewChat","chat":{"id":52,"title":"Team standup",
        "type":{"@type":"chatTypeSupergroup","supergroup_id":52,"is_channel":false},"unread_count":0}}"#,
    );
    feed(
        app,
        r#"{"@type":"updateChatVideoChat","chat_id":52,
        "video_chat":{"@type":"videoChat","group_call_id":62,"has_participants":true,"default_participant_id":null}}"#,
    );
    feed(
        app,
        r#"{"@type":"updateGroupCall","group_call":{"@type":"groupCall","id":62,
        "title":"Team standup","is_active":true,"is_video_chat":false,"is_joined":false,
        "is_rtmp_stream":false,"participant_count":3}}"#,
    );
}

/// A joined call: the slot is taken, so any Calls-tab Join is busy.
fn seed_joined_call(app: &mut QuillApp) {
    feed(
        app,
        r#"{"@type":"updateGroupCall","group_call":{"@type":"groupCall","id":77,
        "title":"Standup room","is_active":true,"is_video_chat":false,"is_joined":true,
        "is_rtmp_stream":false,"participant_count":4}}"#,
    );
}

#[gpui_kit::test]
fn the_section_orders_streams_first_and_join_sets_the_intent(cx: &mut TestAppContext) {
    let (app, mut vcx) = new_app(cx);
    app.update_in(&mut vcx, |app, _, cx| {
        seed_two_calls(app);
        app.chat_list.tab = SidebarTab::Calls;
        app.notify_sidebar(cx);
    });
    frames(&mut vcx);

    // `live_group_calls` orders: LiveStream, VideoChat, VoiceChat, None.
    let listed = app.read_with(&vcx, |app, _| app.session().unwrap().live_group_calls());
    assert_eq!(
        listed.iter().map(|call| call.id).collect::<Vec<_>>(),
        vec![61, 62]
    );
    assert_eq!(listed[0].chat_id, Some(51));
    assert_eq!(listed[1].chat_id, Some(52));

    // The section is drawn above the history, one row and one Join per call.
    assert!(
        vcx.update(|window, _| window.try_find("calls-clear-all").is_some()),
        "the Calls tab is drawn"
    );
    for id in [61u64, 62u64] {
        assert!(
            vcx.update(|window, _| window.try_find(("active-call-join", id)).is_some()),
            "row {id} has Join"
        );
        assert!(
            vcx.update(|window, _| window.try_find(("active-call-row", id)).is_some()),
            "row {id} is drawn"
        );
    }

    // Not busy: the wish names the call; the window joins once it is tracked.
    // The button, the row's body and a listed row all ask for the same call.
    vcx.update(|window, cx| window.click(("active-call-join", 61u64), cx));
    frames(&mut vcx);
    app.read_with(&vcx, |app, _| {
        assert_eq!(
            app.group_call_screen
                .stream_wish()
                .map(|(intent, _)| intent),
            Some(StreamJoinIntent::Join(61)),
            "the Join button asks for the call"
        );
    });
    app.update_in(&mut vcx, |app, _, _| {
        app.group_call_screen.clear_stream_wish();
    });
    vcx.update(|window, cx| window.click(("active-call-row", 61u64), cx));
    frames(&mut vcx);
    app.read_with(&vcx, |app, _| {
        assert_eq!(
            app.group_call_screen
                .stream_wish()
                .map(|(intent, _)| intent),
            Some(StreamJoinIntent::Join(61)),
            "the row body joins instead of opening the chat"
        );
    });
    app.update_in(&mut vcx, |app, _, cx| {
        let call = app.session().unwrap().live_group_calls()[0].clone();
        app.join_listed_group_call(&call, cx);
    });
    app.read_with(&vcx, |app, _| {
        assert_eq!(
            app.group_call_screen
                .stream_wish()
                .map(|(intent, _)| intent),
            Some(StreamJoinIntent::Join(61))
        );
    });
}

#[gpui_kit::test]
fn a_busy_join_opens_the_leave_confirmation(cx: &mut TestAppContext) {
    let (app, mut vcx) = new_app(cx);
    app.update_in(&mut vcx, |app, _, _| {
        seed_two_calls(app);
        seed_joined_call(app);
    });
    let listed = app.read_with(&vcx, |app, _| app.session().unwrap().live_group_calls());
    assert_eq!(
        listed.iter().map(|call| call.id).collect::<Vec<_>>(),
        vec![61, 62]
    );

    // With a chat, the call is offered rather than joined: live, that is
    // `offer_call_link`'s leave confirmation; the demo has no call driver,
    // so the offer is its note and nothing is joined.
    app.update_in(&mut vcx, |app, _, cx| {
        app.join_listed_group_call(&listed[0], cx);
    });
    app.read_with(&vcx, |app, _| {
        assert_eq!(
            app.screen.connection.status_note(),
            tr(keys::Q_OPS_CALLS_NEED_A_SIGNED_IN),
            "the offer path runs (a live driver would confirm leaving)"
        );
        assert!(
            app.group_call_screen.stream_wish().is_none(),
            "a busy Join never asks for the call"
        );
    });

    // No chat to offer: the note asks to end the current call first.
    app.update_in(&mut vcx, |app, _, cx| {
        let no_chat = LiveGroupCall {
            id: 63,
            chat_id: None,
            title: "Mystery".to_string(),
            kind: None,
            participant_count: 0,
        };
        app.demo_session
            .as_deref_mut()
            .unwrap()
            .calls
            .group_call_details
            .insert(no_chat.id, no_chat.clone());
        app.join_listed_group_call(&no_chat, cx);
    });
    app.read_with(&vcx, |app, _| {
        assert_eq!(
            app.screen.connection.status_note(),
            tr(keys::Q_OPS_END_CURRENT_CALL_FIRST)
        );
        assert!(app.group_call_screen.stream_wish().is_none());
    });
}
