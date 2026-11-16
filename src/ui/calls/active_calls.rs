//! The Calls tab's active group calls: the running calls above the recent
//! history, one row each, and the Join that offers the call the way a call
//! link does, joining it once the window tracks it
//! ([`OfferMode::Join`]).
#![deny(clippy::indexing_slicing)]

use super::*;
use crate::ui::bidi_line::name_line;
use crate::ui::chat_row::{CHAT_ROW_AVATAR, chat_avatar};
use crate::ui::pressable::PressableDiv;
use crate::ui::status_note::StatusNote;
use gpui_kit::assets::IconName;
use quill::call_link::OfferMode;
use quill::lang::{keys, tr, trn_decimal};
use quill::state::{GroupCallKind, LiveGroupCall};

impl QuillApp {
    /// The section above Recent calls: the running group calls the call
    /// window is not in, already ordered by `Session::live_group_calls`.
    pub(super) fn active_calls_section(
        &self,
        calls: &[LiveGroupCall],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows: Vec<AnyElement> = calls
            .iter()
            .map(|call| self.active_call_row(call, cx).into_any_element())
            .collect();
        div()
            .id("active-calls")
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .px_1()
                    .child(tr(keys::Q_OPS_ACTIVE_CALLS)),
            )
            .children(rows)
            .child(div().h_px().bg(cx.theme().border).my_1())
    }

    /// One running call: its chat (photo, name), what it is and who is in
    /// it, and Join. The row and the button both join; the row does not
    /// open the chat.
    fn active_call_row(&self, call: &LiveGroupCall, cx: &mut Context<Self>) -> impl IntoElement {
        let name = call.name.clone();
        let status = status_line(call);
        let spoken = if status.is_empty() {
            name.clone()
        } else {
            format!("{name}, {status}")
        };
        let photo = self
            .session()
            .and_then(|session| session.chat_photo_path(call.chat_id));
        let photo = photo.and_then(|path| {
            quill::local_path::sandboxed_display_path(path, &self.media_display_roots())
        });
        let icon = match call.details.as_ref().map(|details| details.kind) {
            Some(GroupCallKind::LiveStream | GroupCallKind::VideoChat) => IconName::Video,
            Some(GroupCallKind::VoiceChat) | None => IconName::Phone,
        };
        let row_call = call.clone();
        let button_call = call.clone();
        div()
            .id(("active-call-row", call.id as u64))
            .px_2()
            .py_2()
            .rounded_md()
            .flex()
            .items_center()
            .gap_3()
            .cursor_pointer()
            .pressable(cx.theme())
            .role(gpui_kit::Role::Button)
            .aria_label(spoken)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.join_listed_group_call(&row_call, cx);
            }))
            .child(chat_avatar(&name, photo.as_deref(), CHAT_ROW_AVATAR))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .min_w_0()
                            .font_semibold()
                            .overflow_hidden()
                            .child(name_line(&name)),
                    )
                    .when(!status.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(div().truncate().child(status)),
                        )
                    }),
            )
            .child(
                Button::new(("active-call-join", call.id as u64))
                    .icon(icon)
                    .label(tr(keys::GROUP_CALL_JOIN))
                    .ghost()
                    .accessibility_label(format!("{} {name}", tr(keys::GROUP_CALL_JOIN)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.join_listed_group_call(&button_call, cx);
                    })),
            )
            .test_support()
    }

    /// Join a running call from the Calls tab: the call link's offer for
    /// the row's chat, joining the call once it is tracked. While another
    /// call is on, the leave question comes first and the join waits in it.
    /// Without the call engine nothing is joined, as from the chat header.
    pub(super) fn join_listed_group_call(&mut self, call: &LiveGroupCall, cx: &mut Context<Self>) {
        if self.refuse_without_call_engine(cx) {
            return;
        }
        let live_stream = is_live_stream(call);
        let joining = self.group_call_screen.joining_call();
        let Some(live) = self.live.as_mut() else {
            self.set_note(StatusNote::info(tr(keys::Q_OPS_CALLS_NEED_A_SIGNED_IN)));
            cx.notify();
            return;
        };
        let step = live
            .driver
            .join_listed_call(call.chat_id, live_stream, joining);
        self.show_call_offer(step, live_stream, OfferMode::Join, cx);
    }
}

fn is_live_stream(call: &LiveGroupCall) -> bool {
    call.details
        .as_ref()
        .is_some_and(|details| details.kind == GroupCallKind::LiveStream)
}

/// What kind of call a row lists ("Live stream", "Video chat", "Voice
/// chat"); a call known only from `chat.video_chat` has no kind.
pub(super) fn kind_label(kind: GroupCallKind) -> &'static str {
    tr(match kind {
        GroupCallKind::LiveStream => keys::Q_OPS_LIVE_STREAM,
        GroupCallKind::VideoChat => keys::Q_OPS_VIDEO_CHAT,
        GroupCallKind::VoiceChat => keys::Q_OPS_VOICE_CHAT,
    })
}

/// The row's second line: the kind, then " • " and who is in the call when
/// the count is known. Streams count viewers; the others participants; the
/// count is grouped ("1,200") like the rest of the app. Nothing at all for a
/// call known only from `chat.video_chat`.
pub(super) fn status_line(call: &LiveGroupCall) -> String {
    let Some(details) = call.details.as_ref() else {
        return String::new();
    };
    let kind = kind_label(details.kind);
    if details.participant_count <= 0 {
        return kind.to_string();
    }
    let count = i64::from(details.participant_count);
    let people = if details.kind == GroupCallKind::LiveStream {
        trn_decimal(keys::GROUP_CALL_RTMP_VIEWERS, count)
    } else {
        trn_decimal(keys::Q_OPS_CALL_PARTICIPANTS, count)
    };
    format!("{kind} • {people}")
}
