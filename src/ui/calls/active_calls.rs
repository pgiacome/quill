//! The Calls tab's active group calls: the running calls above the recent
//! history, one row each, and the Join that asks for the call (the window
//! joins it once it is tracked, [`StreamJoinIntent::Join`]).
#![deny(clippy::indexing_slicing)]

use super::*;
use crate::ui::bidi_line::name_line;
use crate::ui::chat_row::{CHAT_ROW_AVATAR, chat_avatar};
use crate::ui::group_call_stream_layout::StreamJoinIntent;
use crate::ui::pressable::PressableDiv;
use crate::ui::status_note::StatusNote;
use crate::ui::user_error::user_error_for;
use gpui_kit::assets::IconName;
use quill::ids::ChatId;
use quill::lang::{keys, tr, trn_decimal};
use quill::state::{GroupCallKind, LiveGroupCall, Session};

/// Whether a listed call's Join has a call to replace: a 1:1 call, or a
/// group call that is joined, reconnecting after a network loss, or whose
/// join is in flight. `joining` is the call the call window is joining
/// right now (its UI-side mark; the transport covers the rest).
pub(super) fn listed_join_is_busy(session: &Session, joining: Option<i32>) -> bool {
    session.calls.active_call.is_some() || session.group_call_joined_or_joining(joining)
}

/// The row's name: the loaded chat's title, or the call's own title when
/// the chat is not loaded yet.
pub(super) fn row_name(call: &LiveGroupCall, session: Option<&Session>) -> String {
    call.chat_id
        .and_then(|chat_id| session.and_then(|session| session.chats.get(&chat_id)))
        .map(|chat| chat.title.clone())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| call.title.clone())
}

impl QuillApp {
    /// The section above Recent calls: the unjoined running group calls (and
    /// chat-seeded bare rows) the call window does not own. The list is
    /// already ordered by `Session::live_group_calls`.
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
        let name = row_name(call, self.session());
        let status = status_line(call);
        let spoken = if status.is_empty() {
            name.clone()
        } else {
            format!("{name}, {status}")
        };
        let photo = call.chat_id.and_then(|chat_id| {
            self.session()
                .and_then(|session| session.chat_photo_path(ChatId(chat_id)))
        });
        let photo = photo.and_then(|path| {
            quill::local_path::sandboxed_display_path(path, &self.media_display_roots())
        });
        let icon = match call.kind {
            Some(GroupCallKind::LiveStream | GroupCallKind::VideoChat) => IconName::Video,
            _ => IconName::Phone,
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

    /// Join a running call from the Calls tab. Busy (a 1:1 call, or a group
    /// call that is joined, reconnecting, or joining right now): the offered
    /// call waits for the old one to end, as a call link does; without a
    /// chat there is nothing to offer, so the note asks to end the call
    /// first. Otherwise the wish names the call and, with a live driver,
    /// its `getGroupCall` asks for it.
    pub(super) fn join_listed_group_call(&mut self, call: &LiveGroupCall, cx: &mut Context<Self>) {
        let joining = self.group_call_screen.joining_call();
        let busy = self
            .session()
            .is_some_and(|session| listed_join_is_busy(session, joining));
        if busy {
            if let Some(chat) = call.chat_id {
                self.offer_call_link(
                    ChatId(chat),
                    "",
                    call.kind == Some(GroupCallKind::LiveStream),
                    true,
                    cx,
                );
            } else {
                self.set_note(StatusNote::error(tr(keys::Q_OPS_END_CURRENT_CALL_FIRST)));
            }
            cx.notify();
            return;
        }
        self.note_stream_join_intent(StreamJoinIntent::Join(call.id));
        if let Some(live) = self.live.as_mut()
            && let Err(err) = live.driver.fetch_group_call(call.id)
        {
            self.group_call_screen.clear_stream_wish();
            self.set_note(StatusNote::error(user_error_for(
                keys::Q_NOTES_SOMETHING_WENT_WRONG,
                &err,
            )));
        }
        cx.notify();
    }
}

/// What kind of call a row lists ("Live stream", "Video chat", "Voice
/// Chat"); a call known only from `chat.video_chat` has no kind.
pub(super) fn kind_label(kind: Option<GroupCallKind>) -> &'static str {
    match kind {
        Some(GroupCallKind::LiveStream) => tr(keys::VIEW_BUTTON_VOICE_CHAT_CHANNEL),
        Some(GroupCallKind::VideoChat) => tr(keys::MENU_START_GROUP_CALL_OPTIONS),
        Some(GroupCallKind::VoiceChat) => tr(keys::Q_AV_VOICE_CHAT_2),
        // Only `chat.video_chat` is known: no kind to name.
        None => "",
    }
}

/// The row's second line: the kind, then " • " and who is in the call when
/// the count is known. Streams count viewers; the others participants; the
/// count is grouped ("1,200") like the rest of the app. Nothing at all for a
/// call known only from `chat.video_chat`.
pub(super) fn status_line(call: &LiveGroupCall) -> String {
    let kind = kind_label(call.kind);
    let people = (call.participant_count > 0).then(|| {
        let count = i64::from(call.participant_count);
        if call.kind == Some(GroupCallKind::LiveStream) {
            trn_decimal(keys::GROUP_CALL_RTMP_VIEWERS, count)
        } else {
            trn_decimal(keys::Q_OPS_CALL_PARTICIPANTS, count)
        }
    });
    match (kind, people) {
        ("", None) => String::new(),
        ("", Some(people)) => people,
        (kind, None) => kind.to_string(),
        (kind, Some(people)) => format!("{kind} • {people}"),
    }
}
