//! The Calls tab's active group calls: the section above the history, its
//! rows, and what the window does with the offer a Join gets back. The
//! demo app has no call driver; the offer itself is tested at the driver
//! (`connect::tests::group_calls::unjoined`).
use crate::ui::app::QuillApp;
use crate::ui::call_link_ui::offered_call_intent;
use crate::ui::chat_list_state::SidebarTab;
use crate::ui::group_call_stream_layout::StreamJoinIntent;
use crate::ui::keybindings::bind_keys;
use crate::ui::screenshot_demo::ScreenshotDemo;
use crate::ui::shell::QuillShell;
use gpui_kit::component::Root;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{AppContext, Entity, TestAppContext, VisualTestContext, px, size};
use quill::call_link::{BAD_LINK_NOTE, CallLinkStep, OfferMode};
use quill::connect::ConnectSendError;
use quill::diagnostics::DiagnosticSink;
use quill::ids::ChatId;
use quill::lang::{keys, tr, trn_decimal};
use quill::state::{GroupCallDetails, GroupCallKind, LiveGroupCall};
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
/// and a voice chat (62) whose call title differs from its chat's.
fn seed_two_calls(app: &mut QuillApp) {
    for json in [
        r#"{"@type":"updateNewChat","chat":{"id":51,"title":"Design live",
        "type":{"@type":"chatTypeSupergroup","supergroup_id":51,"is_channel":true},"unread_count":0}}"#,
        r#"{"@type":"updateChatVideoChat","chat_id":51,
        "video_chat":{"@type":"videoChat","group_call_id":61,"has_participants":true,"default_participant_id":null}}"#,
        r#"{"@type":"updateGroupCall","group_call":{"@type":"groupCall","id":61,
        "title":"Design live","is_active":true,"is_video_chat":false,"is_joined":false,
        "is_rtmp_stream":true,"participant_count":12}}"#,
        r#"{"@type":"updateNewChat","chat":{"id":52,"title":"Team standup",
        "type":{"@type":"chatTypeSupergroup","supergroup_id":52,"is_channel":false},"unread_count":0}}"#,
        r#"{"@type":"updateChatVideoChat","chat_id":52,
        "video_chat":{"@type":"videoChat","group_call_id":62,"has_participants":true,"default_participant_id":null}}"#,
        r#"{"@type":"updateGroupCall","group_call":{"@type":"groupCall","id":62,
        "title":"Old call title","is_active":true,"is_video_chat":false,"is_joined":false,
        "is_rtmp_stream":false,"participant_count":3}}"#,
    ] {
        feed(app, json);
    }
}

fn wish(app: &Entity<QuillApp>, vcx: &VisualTestContext) -> Option<StreamJoinIntent> {
    app.read_with(vcx, |app, _| {
        app.group_call_screen
            .stream_wish()
            .map(|(intent, _)| intent)
    })
}

fn note(app: &Entity<QuillApp>, vcx: &VisualTestContext) -> String {
    app.read_with(vcx, |app, _| {
        app.screen.connection.status_note().to_string()
    })
}

#[gpui_kit::test]
fn the_section_lists_running_calls_above_the_history(cx: &mut TestAppContext) {
    let (app, mut vcx) = new_app(cx);
    app.update_in(&mut vcx, |app, _, cx| {
        seed_two_calls(app);
        app.chat_list.tab = SidebarTab::Calls;
        app.notify_sidebar(cx);
    });
    frames(&mut vcx);

    let listed = app.read_with(&vcx, |app, _| app.session().unwrap().live_group_calls());
    assert_eq!(
        listed
            .iter()
            .map(|call| (call.id, call.chat_id, call.name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (61, ChatId(51), "Design live"),
            (62, ChatId(52), "Team standup")
        ],
        "the stream first; a row shows its chat's title, not the call's"
    );
    for id in [61u64, 62u64] {
        assert!(
            vcx.update(|window, _| window.try_find(("active-call-join", id)).is_some()),
            "row {id} has Join"
        );
    }
    assert!(
        vcx.update(|window, _| window.try_find_by_label("Join Team standup").is_some()),
        "the Join label names the call"
    );
    let first_row = vcx
        .update(|window, _| window.try_find(("active-call-row", 61u64)))
        .expect("the first row is drawn");
    let recent = vcx
        .update(|window, _| window.try_find("calls-clear-all"))
        .expect("the Recent calls row is drawn");
    assert!(first_row.bounds().origin.y < recent.bounds().origin.y);
}

/// The demo has no call driver: Join says so and asks for nothing, the
/// button and the row's body alike.
#[gpui_kit::test]
fn join_without_a_driver_says_calls_need_an_account(cx: &mut TestAppContext) {
    let (app, mut vcx) = new_app(cx);
    app.update_in(&mut vcx, |app, _, cx| {
        seed_two_calls(app);
        app.chat_list.tab = SidebarTab::Calls;
        app.notify_sidebar(cx);
    });
    frames(&mut vcx);
    for target in ["active-call-join", "active-call-row"] {
        vcx.update(|window, cx| window.click((target, 61u64), cx));
        frames(&mut vcx);
        assert_eq!(note(&app, &vcx), tr(keys::Q_OPS_CALLS_NEED_A_SIGNED_IN));
        assert_eq!(wish(&app, &vcx), None, "{target} asks for nothing");
    }
}

/// What the window does with the step a listed Join gets back from the
/// driver: an offer is joined once tracked, whatever its kind; a call
/// that cannot be offered says something went wrong, not that a link is
/// bad (there was no link).
#[gpui_kit::test]
fn a_listed_offer_joins_and_its_failures_say_something_went_wrong(cx: &mut TestAppContext) {
    let (app, mut vcx) = new_app(cx);
    app.update_in(&mut vcx, |app, _, cx| {
        app.show_call_offer(
            Ok(CallLinkStep::Offer { group_call_id: 62 }),
            false,
            OfferMode::Join,
            cx,
        );
    });
    assert_eq!(wish(&app, &vcx), Some(StreamJoinIntent::Join(62)));
    assert_eq!(note(&app, &vcx), tr(keys::Q_OPS_OPENING_THE_VOICE_CHAT));

    for step in [
        Ok(CallLinkStep::NoCall),
        Err(ConnectSendError::InvalidRequest),
    ] {
        app.update_in(&mut vcx, |app, _, cx| {
            app.group_call_screen.clear_stream_wish();
            app.show_call_offer(step, false, OfferMode::Join, cx);
        });
        let shown = note(&app, &vcx);
        assert!(
            shown.starts_with(tr(keys::Q_NOTES_SOMETHING_WENT_WRONG)),
            "a listed call's failure: {shown}"
        );
        assert_eq!(wish(&app, &vcx), None);
    }

    // A link's offer keeps its own notes and only opens the call.
    app.update_in(&mut vcx, |app, _, cx| {
        app.show_call_offer(Ok(CallLinkStep::NoCall), false, OfferMode::OpenScreen, cx);
    });
    assert_eq!(note(&app, &vcx), BAD_LINK_NOTE);
    app.update_in(&mut vcx, |app, _, cx| {
        app.show_call_offer(
            Ok(CallLinkStep::Offer { group_call_id: 62 }),
            false,
            OfferMode::OpenScreen,
            cx,
        );
    });
    assert_eq!(wish(&app, &vcx), Some(StreamJoinIntent::Call(62)));
}

/// The row says only what the list knows: a call known only from its
/// chat has no kind and no count, so its second line is empty.
#[test]
fn the_row_status_shows_no_kind_or_count_it_does_not_know() {
    let seeded = LiveGroupCall {
        id: 63,
        chat_id: ChatId(53),
        name: "Mystery".to_string(),
        details: None,
    };
    assert_eq!(super::active_calls::status_line(&seeded), "");

    let with = |kind, participant_count| LiveGroupCall {
        details: Some(GroupCallDetails {
            title: String::new(),
            kind,
            participant_count,
        }),
        ..seeded.clone()
    };
    assert_eq!(
        super::active_calls::status_line(&with(GroupCallKind::LiveStream, 12)),
        format!(
            "{} • {}",
            tr(keys::Q_OPS_LIVE_STREAM),
            trn_decimal(keys::GROUP_CALL_RTMP_VIEWERS, 12)
        )
    );
    assert_eq!(
        super::active_calls::status_line(&with(GroupCallKind::VideoChat, 3)),
        format!(
            "{} • {}",
            tr(keys::Q_OPS_VIDEO_CHAT),
            trn_decimal(keys::Q_OPS_CALL_PARTICIPANTS, 3)
        )
    );
    assert_eq!(
        super::active_calls::status_line(&with(GroupCallKind::VoiceChat, 0)),
        tr(keys::Q_OPS_VOICE_CHAT),
        "the kind alone, no count at zero"
    );
}

/// A confirmed Calls-tab offer joins whatever the call's kind; a link's
/// offer keeps the call screen's Join for a non-stream call.
#[test]
fn an_offer_joins_only_for_the_calls_tab() {
    assert_eq!(
        offered_call_intent(61, OfferMode::Join),
        StreamJoinIntent::Join(61)
    );
    assert_eq!(
        offered_call_intent(61, OfferMode::OpenScreen),
        StreamJoinIntent::Call(61)
    );
}

/// No running calls: the Calls tab draws the history with no Active calls
/// header above it.
#[gpui_kit::test]
fn an_empty_list_draws_no_header(cx: &mut TestAppContext) {
    let (_app, mut vcx) = new_app(cx);
    frames(&mut vcx);
    assert!(
        vcx.update(|window, _| window.try_find("calls-clear-all").is_some()),
        "the Calls tab is drawn"
    );
    assert!(
        vcx.update(|window, _| window.try_find("active-calls").is_none()),
        "no Active calls section without calls"
    );
}
