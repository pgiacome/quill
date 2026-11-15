//! Methods moved out of `groups_forum.rs` to keep files under 1000 lines.

use super::*;
use crate::ui::history_state::ForumPeek;
use crate::ui::status_note::StatusNote;
use quill::lang::{keys, tr, tr_with, trn};
use quill::local_time::{CivilTime, chat_list_stamp, civil_local, now_unix};
use quill::telegram::envelope::MessageSender;

impl QuillApp {
    /// Phase 5.1: the topic list for a forum supergroup — one chat-list-like
    /// row per topic: the round icon, the name with its state chips and the
    /// timestamp, then the last message's sender and preview with the unread
    /// badge at the trailing edge. Selecting a row opens the per-topic
    /// history.
    pub(in crate::ui) fn forum_topics_pane(
        &self,
        open: Option<ChatId>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let session = self.session();
        let topics: Vec<ForumTopic> = open
            .map(|id| {
                session
                    .map(|s| s.ordered_forum_topics(id))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let selected_topic = session.and_then(|s| s.open_forum_topic());
        let mut list = div()
            .id("forum-topics")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .px_3()
            .pt_2()
            .gap_1();
        if topics.is_empty() {
            return pane_placeholder(
                tr(keys::Q_PEOPLE_NO_TOPICS_YET),
                tr(keys::Q_PEOPLE_THE_TOPIC_LIST_ARRIVES_VIA),
                cx,
            )
            .into_any_element();
        }
        list = list.child(
            div()
                .id("forum-topics-count")
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(if topics.len() == 1 {
                    tr(keys::Q_PEOPLE_1_TOPIC).to_owned()
                } else {
                    trn(keys::Q_PEOPLE_TOPICS_COUNT, topics.len() as i64)
                }),
        );
        let m = self.density();
        // The row's round icon: 42 px, as tall as a chat row's photo, so the
        // compact density still gives the row the two text lines' height.
        let avatar = 42.;
        let row_h = m.row_base.max(avatar + 12.);
        let now = civil_local(now_unix());
        for topic in topics {
            let topic_id = topic.forum_topic_id;
            let unread = topic.unread_count;
            let name = if topic.name.is_empty() {
                tr_with(
                    keys::Q_PEOPLE_TOPIC,
                    &[("forum_topic_id", &topic.forum_topic_id.to_string())],
                )
            } else {
                topic.name.clone()
            };
            // A topic draft replaces the last-message preview (red
            // "Draft:" prefix), as a chat row's does.
            let draft = topic.draft_preview();
            let preview = match &draft {
                Some(draft) => draft.text.clone(),
                None => topic.last_message_preview.clone(),
            };
            let draft_reply = draft.map(|draft| draft.reply);
            let sender = if draft_reply.is_some() {
                None
            } else {
                self.forum_topic_sender(&topic)
            };
            let stamp = topic_stamp(topic.last_message_date, &now);
            let extras = open
                .map(|id| self.topic_extras(id, &topic))
                .unwrap_or_default();
            let can_pin = open.is_some_and(|id| {
                self.session().is_some_and(|s| s.chat_can_manage_topics(id)) && !topic.is_general
            });
            let pinned = topic.is_pinned;
            let owner = cx.entity().downgrade();
            let chat_for_menu = open.unwrap_or(ChatId(0));
            let muted = cx.theme().muted_foreground;
            // The second line: "Sender: preview", or just the draft. The
            // unread badge trails it, under the timestamp.
            let line = topic_line(sender.as_deref(), &preview);
            list = list.child(
                div()
                    .id(("forum-topic-row", topic_id as u64))
                    .w_full()
                    .h(px(row_h))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap(px(m.row_gap))
                    .rounded_md()
                    .overflow_hidden()
                    .role(gpui_kit::Role::Button)
                    .aria_label(name.clone())
                    .tab_index(0)
                    .cursor_pointer()
                    .pressable(cx.theme())
                    .test_support()
                    .bg(if selected_topic == Some(topic_id) {
                        cx.theme().accent
                    } else {
                        cx.theme().sidebar
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_topic_ui(topic_id, cx);
                    }))
                    .context_menu(move |menu, _, _| {
                        let mut menu = menu;
                        if can_pin {
                            let pin_owner = owner.clone();
                            menu = menu.item(
                                PopupMenuItem::new(if pinned {
                                    tr(keys::Q_PEOPLE_UNPIN)
                                } else {
                                    tr(keys::CONTEXT_PIN_MSG)
                                })
                                .icon(if pinned {
                                    IconName::PinOff
                                } else {
                                    IconName::Pin
                                })
                                .on_click(move |_, _, cx| {
                                    let _ = pin_owner.update(cx, |this, cx| {
                                        let action = if pinned {
                                            ForumTopicAction::Unpin
                                        } else {
                                            ForumTopicAction::Pin
                                        };
                                        this.forum_topic_action(
                                            chat_for_menu,
                                            topic_id,
                                            action,
                                            cx,
                                        );
                                    });
                                }),
                            );
                        }
                        super::forum_extras::add_topic_extras(
                            menu,
                            extras,
                            owner.clone(),
                            chat_for_menu,
                            topic_id,
                        )
                    })
                    .child(self.forum_topic_avatar(&topic, avatar, cx))
                    .child(
                        super::super::chat_row::row_text_lines(m)
                            .overflow_hidden()
                            .child(
                                // First line: the name (truncating) and the
                                // state chips in the flexible part, the
                                // timestamp held at the trailing edge.
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .w_full()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .id(("forum-topic-name", topic_id as u64))
                                                    .test_support()
                                                    .min_w_0()
                                                    .truncate()
                                                    .font_medium()
                                                    .child(name),
                                            )
                                            .when(
                                                topic.is_general
                                                    && !topic.name.eq_ignore_ascii_case("general"),
                                                |this| {
                                                    this.child(
                                                        div()
                                                            .flex_none()
                                                            .text_xs()
                                                            .text_color(muted)
                                                            .child(tr(keys::Q_PEOPLE_GENERAL)),
                                                    )
                                                },
                                            )
                                            .when(topic.is_pinned, |this| {
                                                this.child(
                                                    div()
                                                        .flex_none()
                                                        .text_xs()
                                                        .text_color(muted)
                                                        .child(tr(keys::Q_PEOPLE_PINNED)),
                                                )
                                            })
                                            .when(topic.is_closed, |this| {
                                                this.child(
                                                    div()
                                                        .flex_none()
                                                        .text_xs()
                                                        .text_color(muted)
                                                        .child(tr(keys::Q_PEOPLE_CLOSED)),
                                                )
                                            }),
                                    )
                                    .when_some(stamp, |this, stamp| {
                                        this.child(
                                            div()
                                                .id(("forum-topic-stamp", topic_id as u64))
                                                .test_support()
                                                .flex_none()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(stamp),
                                        )
                                    }),
                            )
                            .child(
                                // Second line: sender and preview, then the
                                // unread badge at the trailing edge.
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .w_full()
                                    .text_xs()
                                    .text_color(muted)
                                    .when_some(draft_reply, |this, reply| {
                                        this.child(super::super::chat_row::draft_prefix(
                                            reply,
                                            line.is_empty(),
                                        ))
                                    })
                                    .child(
                                        div()
                                            .id(("forum-topic-preview", topic_id as u64))
                                            .test_support()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .child(line),
                                    )
                                    .when(unread > 0, |this| {
                                        this.child(topic_unread_pill(unread, topic_id))
                                    }),
                            ),
                    ),
            );
        }
        list.into_any_element()
    }

    /// The topic row's round icon: the colored letter/# circle from the
    /// subsection tabs, with a downloaded custom-emoji icon drawn inside it.
    pub(in crate::ui) fn forum_topic_avatar(
        &self,
        topic: &ForumTopic,
        size: f32,
        cx: &App,
    ) -> AnyElement {
        if !topic.is_general
            && topic.icon_custom_emoji_id > 0
            && let Some(path) = self.ai_emoji_still(topic.icon_custom_emoji_id)
        {
            let color: Hsla = if topic.icon_color > 0 {
                rgb(topic.icon_color as u32).into()
            } else {
                accent().into()
            };
            let glyph = size * 0.66;
            return div()
                .size(px(size))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(color)
                .child(
                    img(path)
                        .w(px(glyph))
                        .h(px(glyph))
                        .object_fit(ObjectFit::Contain),
                )
                .into_any_element();
        }
        super::subsection_tabs::topic_icon(topic, size, cx)
    }

    /// The sender the topic row's preview leads with: "You" for an outgoing
    /// last message, otherwise the user's or chat's name; `None` when the
    /// sender is not cached (the preview then stands alone).
    pub(in crate::ui) fn forum_topic_sender(&self, topic: &ForumTopic) -> Option<String> {
        if topic.last_message_is_outgoing {
            return Some(tr(keys::FROM_YOU).to_owned());
        }
        let session = self.session()?;
        match topic.last_message_sender? {
            MessageSender::User { user_id } => {
                session.user(user_id).map(|user| user.display_name())
            }
            MessageSender::Chat { chat_id } => {
                session.chats.get(&chat_id).map(|chat| chat.title.clone())
            }
        }
    }

    /// Whether the user is peeking at the chat list over the open forum's
    /// topics.
    pub(in crate::ui) fn forum_peeking(&self) -> bool {
        let open = self.session().and_then(|s| s.open_chat_id());
        self.history.forum_peek.is_for(open)
    }

    /// Where the forum topic column sits in a window of this width: a pure
    /// function of the session, the width and the user's peek.
    pub(in crate::ui) fn forum_column_at(&self, width: f32) -> quill::state::ForumColumn {
        self.session()
            .map_or(quill::state::ForumColumn::Hidden, |s| {
                s.forum_column(width, self.forum_peeking())
            })
    }

    /// `forum_column_at` for the width of `window`.
    pub(in crate::ui) fn forum_column_layout(&self, window: &Window) -> quill::state::ForumColumn {
        self.forum_column_at(f32::from(window.viewport_size().width))
    }

    /// The forum's topic list as its own column (tdesktop shows the topics
    /// where the chat list was): the forum's name and topic count on top,
    /// then the topics. Where the topics replace the chat list a "Chats"
    /// button brings it back; filling a one-column window, a back button
    /// leaves the forum.
    pub(in crate::ui) fn forum_column_view(
        &self,
        open: Option<ChatId>,
        pane: crate::ui::shell_layout::ForumPane,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::ui::shell_layout::ForumPane;
        let title = open
            .and_then(|id| self.session().and_then(|s| s.chats.get(&id.0)))
            .map(|chat| chat.title.clone())
            .unwrap_or_default();
        let count = open
            .and_then(|id| self.session().map(|s| s.ordered_forum_topics(id).len()))
            .unwrap_or(0);
        div()
            .id("forum-column")
            .role(gpui_kit::Role::Group)
            .a11y_tag(quill::agent::ui_driver::redact::REGION_FORUM)
            .flex()
            .flex_col()
            .map(|this| {
                if pane == ForumPane::Fill {
                    this.flex_1().min_w_0()
                } else {
                    this.flex_none().w(px(quill::state::FORUM_COLUMN_WIDTH))
                }
            })
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .when(pane == ForumPane::Fill, |this| {
                        this.child(
                            Button::new("forum-column-back")
                                .icon(gpui_kit::assets::IconName::ChevronLeft)
                                .ghost()
                                .tooltip(tr(keys::Q_CHAT_BACK))
                                .accessibility_label(tr(keys::Q_CHAT_BACK))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.back_to_chat_list(window, cx);
                                })),
                        )
                    })
                    .when(pane == ForumPane::Column { chats_button: true }, |this| {
                        this.child(
                            Button::new("forum-column-chats")
                                .icon(gpui_kit::assets::IconName::ChevronLeft)
                                .ghost()
                                .tooltip(tr(keys::Q_PEOPLE_SHOW_CHATS))
                                .accessibility_label(tr(keys::Q_PEOPLE_SHOW_CHATS))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(forum) = open {
                                        this.history.forum_peek = ForumPeek::Chats { forum };
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(div().font_semibold().truncate().child(title))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if count == 1 {
                                        tr(keys::Q_PEOPLE_1_TOPIC).to_owned()
                                    } else {
                                        trn(keys::Q_PEOPLE_TOPICS_COUNT, count as i64)
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("forum-column-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(self.forum_topics_pane(open, cx)),
            )
            .into_any_element()
    }

    /// Phase 5.1: select a forum topic (live: `searchChatMessages` with
    /// `topic_id`; demo: the seeded session takes it directly).
    pub(in crate::ui) fn select_topic_ui(&mut self, forum_topic_id: i32, cx: &mut Context<Self>) {
        // The composer belongs to the slot being left (the chat's "All"
        // view or the previous topic): save it there, then restore the new
        // topic's draft once the switch is done.
        self.flush_leaving_draft(cx);
        let open_chat = self.session().and_then(|s| s.open_chat_id());
        let mut opened = true;
        if let Some(live) = self.live.as_mut() {
            let result = live.driver.select_topic(forum_topic_id);
            opened = result.is_ok();
            self.set_note(match result {
                Ok(_) => StatusNote::silent(tr(keys::Q_PEOPLE_TOPIC_SELECTED)),
                Err(_) => StatusNote::error(tr(keys::Q_PEOPLE_COULD_NOT_OPEN_TOPIC)),
            });
        } else if let Some(session) = self.demo_session.as_mut()
            && let Some(chat_id) = session.open_chat_id()
        {
            session.select_topic(chat_id, forum_topic_id);
        }
        // An opened topic lands at its bottom, never where it was left; a
        // refused open changes nothing, so it forgets nothing either.
        if opened {
            if let Some(chat_id) = open_chat {
                self.history
                    .scroll_memory
                    .forget((chat_id.0, Some(forum_topic_id), 0));
            }
            self.request_draft_restore();
        }
        cx.notify();
    }

    /// Phase 5.1: leave the topic view, back to the forum's topic list.
    pub(in crate::ui) fn deselect_topic_ui(&mut self, cx: &mut Context<Self>) {
        self.flush_leaving_draft(cx);
        if let Some(live) = self.live.as_mut() {
            live.driver.deselect_topic();
        } else if let Some(session) = self.demo_session.as_mut() {
            session.deselect_topic();
        }
        self.request_draft_restore();
        self.set_note(StatusNote::silent(tr(keys::Q_PEOPLE_BACK_TO_TOPICS)));
        cx.notify();
    }
}

/// The unread counter on the topic row's second line: the chat rows' pill,
/// held to the trailing edge so it never covers the preview text.
fn topic_unread_pill(count: i32, topic_id: i32) -> AnyElement {
    div()
        .id(("forum-topic-unread", topic_id as u64))
        .test_support()
        .flex_none()
        .h(px(20.))
        .min_w(px(20.))
        .px(px(6.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(Hsla::from(accent_strong()))
        .text_color(text_on_fill())
        .text_xs()
        .font_semibold()
        .child(super::super::chat_row::compact_count(count))
        .into_any_element()
}

/// The topic row's second line: the sender (or "You"), then the last
/// message's preview, separated by ": ". A sender whose preview is empty
/// still names who wrote last.
fn topic_line(sender: Option<&str>, preview: &str) -> String {
    match sender {
        Some(sender) if !preview.is_empty() => format!("{sender}: {preview}"),
        Some(sender) => sender.to_owned(),
        None => preview.to_owned(),
    }
}

/// The topic row's timestamp: the chat-list stamp of the last message, or
/// nothing when the topic has no messages yet.
fn topic_stamp(date: i32, now: &CivilTime) -> Option<String> {
    (date > 0).then(|| chat_list_stamp(&civil_local(i64::from(date)), now))
}

#[cfg(test)]
mod tests {
    use super::{topic_line, topic_stamp};
    use quill::local_time::{CivilTime, civil_local, hhmm};

    #[test]
    fn the_topic_line_leads_with_the_sender() {
        assert_eq!(topic_line(Some("You"), "ship it"), "You: ship it");
        assert_eq!(topic_line(Some("Maya"), "hello"), "Maya: hello");
        assert_eq!(topic_line(Some("Maya"), ""), "Maya");
        assert_eq!(topic_line(None, "just the text"), "just the text");
        assert_eq!(topic_line(None, ""), "");
    }

    #[test]
    fn a_topic_without_messages_has_no_stamp() {
        let date = 1_700_000_000_i32;
        let now: CivilTime = civil_local(i64::from(date));
        // The same instant seen by its own stamp is the clock time.
        assert_eq!(topic_stamp(date, &now), Some(hhmm(&now)));
        assert_eq!(topic_stamp(0, &now), None);
    }
}
