//! Screenshot demos of the Calls tab. `ready-calls-history` has a page of
//! call history and more on the server: the spinner row at the end, where a
//! Load more button used to be. `ready-calls-active` puts a live stream and
//! a voice chat above the same history. Injected, no live Telegram, so no
//! next page arrives.
#![deny(clippy::indexing_slicing)]

use crate::ui::app::QuillApp;
use crate::ui::chat_list_state::SidebarTab;
use crate::ui::screenshot_demo::{DemoSpec, register_demos};
use gpui_kit::*;
use quill::diagnostics::DiagnosticSink;
use quill::state::RequestPurpose;
use quill::telegram::client::copy_and_parse;
use std::sync::Arc;
use std::sync::atomic::Ordering;

register_demos![
    DemoSpec::chats(
        "ready-calls-history",
        "screenshot demo — call history loading more"
    )
    .setup(QuillApp::demo_calls_history),
    DemoSpec::chats(
        "ready-calls-active",
        "screenshot demo — active group calls above recent calls"
    )
    .setup(QuillApp::demo_calls_active),
];

/// Two running calls with their chats: a live stream (61, "Design live",
/// 1,280 viewers) and a voice chat (62, "Team standup", 4 people). Fed
/// through the session's own parser, as the app receives them.
const ACTIVE_CALLS: [&str; 6] = [
    r#"{"@type":"updateNewChat","chat":{"id":51,"title":"Design live","type":{"@type":"chatTypeSupergroup","supergroup_id":51,"is_channel":true},"unread_count":0}}"#,
    r#"{"@type":"updateChatVideoChat","chat_id":51,"video_chat":{"@type":"videoChat","group_call_id":61,"has_participants":true,"default_participant_id":null}}"#,
    r#"{"@type":"updateGroupCall","group_call":{"@type":"groupCall","id":61,"title":"Design live","is_active":true,"is_video_chat":false,"is_joined":false,"is_rtmp_stream":true,"participant_count":1280}}"#,
    r#"{"@type":"updateNewChat","chat":{"id":52,"title":"Team standup","type":{"@type":"chatTypeSupergroup","supergroup_id":52,"is_channel":false},"unread_count":0}}"#,
    r#"{"@type":"updateChatVideoChat","chat_id":52,"video_chat":{"@type":"videoChat","group_call_id":62,"has_participants":true,"default_participant_id":null}}"#,
    r#"{"@type":"updateGroupCall","group_call":{"@type":"groupCall","id":62,"title":"Team standup","is_active":true,"is_video_chat":false,"is_joined":false,"is_rtmp_stream":false,"participant_count":4}}"#,
];

impl QuillApp {
    fn demo_calls_history(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.demo_calls_tab(false, cx);
    }

    fn demo_calls_active(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.demo_calls_tab(true, cx);
    }

    fn demo_calls_tab(&mut self, active_calls: bool, cx: &mut Context<Self>) {
        let Some(session) = self.demo_session.as_mut() else {
            return;
        };
        self.demo_ui.seq.store(session.last_seq, Ordering::SeqCst);
        let sink: Arc<dyn DiagnosticSink> = self.demo_ui.sink.clone();
        if active_calls {
            for json in ACTIVE_CALLS {
                if let Some(owned) = copy_and_parse(json, &self.demo_ui.seq, &sink) {
                    session.apply(owned);
                }
            }
        }
        let extra = session.request(RequestPurpose::SearchCallMessages, None);
        // Recent calls only take the page they wait for.
        session.calls.recent_calls.page_sent(extra);
        let calls: Vec<String> = (0..14i64)
            .map(|n| {
                let reason = if n % 3 == 1 {
                    "callDiscardReasonMissed"
                } else {
                    "callDiscardReasonHungUp"
                };
                format!(
                    r#"{{"@type":"message","id":{id},"chat_id":{chat},"is_outgoing":{out},"date":{date},"content":{{"@type":"messageCall","unique_id":{id},"is_video":{video},"discard_reason":{{"@type":"{reason}"}},"duration":{dur}}}}}"#,
                    id = 900 + n,
                    chat = 11 + n % 2,
                    out = n % 2 == 0,
                    date = 1_760_000_000 - n * 40_000,
                    video = n % 4 == 0,
                    dur = if n % 3 == 1 { 0 } else { 60 + n * 37 },
                )
            })
            .collect();
        let json = format!(
            r#"{{"@type":"foundMessages","@extra":"{}","total_count":40,"next_offset":"page2","messages":[{}]}}"#,
            extra.0,
            calls.join(","),
        );
        if let Some(owned) = copy_and_parse(&json, &self.demo_ui.seq, &sink) {
            session.apply(owned);
        }
        self.chat_list.tab = SidebarTab::Calls;
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::QuillApp;
    use crate::ui::screenshot_demo::ScreenshotDemo;
    use crate::ui::shell::QuillShell;
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, VisualTestContext, px, size};
    use quill::ids::ChatId;
    use quill::state::GroupCallKind;

    /// The screenshot cannot be taken on every platform, so the fixture is
    /// checked here: both running calls reach the session and the Calls tab
    /// as rows, the stream first, above the Recent calls row.
    #[gpui_kit::test]
    fn the_active_calls_demo_draws_a_stream_and_a_voice_chat(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(crate::ui::keybindings::bind_keys);
        let handle = cx.open_window(size(px(1100.), px(800.)), move |window, cx| {
            let view = cx.new(|cx| {
                QuillApp::new_with_demo(
                    window,
                    cx,
                    None,
                    Some(ScreenshotDemo::named("ready-calls-active")),
                )
            });
            let shell = cx.new(|_| QuillShell::new(view));
            Root::new(shell, window, cx)
        });
        let mut vcx = VisualTestContext::from_window(handle.into(), cx);
        vcx.run_until_parked();
        for _ in 0..2 {
            vcx.update(|window, cx| window.render_frame(cx));
        }

        let app = vcx.update(|window, cx| {
            let root = window.root::<Root>().flatten().expect("window root");
            let shell = root
                .read(cx)
                .view()
                .clone()
                .downcast::<QuillShell>()
                .expect("the shell is the root view");
            shell.read(cx).app.clone()
        });
        let listed = app.read_with(&vcx, |app, _| app.session().unwrap().live_group_calls());
        assert_eq!(
            listed.iter().map(|call| call.id).collect::<Vec<_>>(),
            vec![61, 62],
            "the stream first, then the voice chat"
        );
        assert_eq!(
            listed
                .iter()
                .map(|call| call
                    .details
                    .as_ref()
                    .map(|details| (details.kind, details.participant_count)))
                .collect::<Vec<_>>(),
            vec![
                Some((GroupCallKind::LiveStream, 1280)),
                Some((GroupCallKind::VoiceChat, 4))
            ]
        );
        assert_eq!(
            listed.iter().map(|call| call.chat_id).collect::<Vec<_>>(),
            vec![ChatId(51), ChatId(52)]
        );

        for id in [61u64, 62u64] {
            assert!(
                vcx.update(|window, _| window.try_find(("active-call-row", id)).is_some()),
                "row {id} is drawn"
            );
        }
        let first_row = vcx
            .update(|window, _| window.try_find(("active-call-row", 61u64)))
            .expect("the first active call row is drawn");
        let recent = vcx
            .update(|window, _| window.try_find("calls-clear-all"))
            .expect("the Recent calls row is drawn");
        assert!(
            first_row.bounds().origin.y < recent.bounds().origin.y,
            "the active calls section is above Recent calls"
        );
    }
}
