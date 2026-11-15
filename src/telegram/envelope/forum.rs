use super::*;
use serde_json::Value;

/// One `forumTopic` (TDLib 1.8.67, `schema/td_api.tl:3968` + `forumTopicInfo`
/// at 3953). Only the fields the topic list / topic view need are kept;
/// fields no feature reads (such as `is_name_implicit`,
/// `unread_poll_vote_count`) are dropped, as documented in the Phase 5.1
/// DECISIONS entry. The topic row reads `icon` (custom-emoji topic icons
/// included) and the `last_message` date and sender. Slice G2 added
/// `is_hidden` back (`forumTopicInfo.is_hidden`, schema line 3953)
/// for the General topic Hide/Show action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopic {
    pub forum_topic_id: i32,
    pub name: String,
    pub is_general: bool,
    pub is_closed: bool,
    pub is_pinned: bool,
    /// Slice G2: `forumTopicInfo.is_hidden` (schema 1.8.67, line 3953)
    /// — "True, if the topic is hidden above the topic list and
    /// closed; for General topic only". Drives the Hide/Show action.
    pub is_hidden: bool,
    pub unread_count: i32,
    /// Schema `forumTopic.order` — topics sort by order descending.
    pub order: i64,
    /// Cheap preview of `forumTopic.last_message` via
    /// `MessageContent::preview`; empty when there is no last message.
    pub last_message_preview: String,
    /// `forumTopic.last_message.date` (Unix time), 0 when there is no last
    /// message — the topic row's timestamp.
    pub last_message_date: i32,
    /// `forumTopic.last_message.sender_id`, `None` when there is no last
    /// message — the sender the topic row's preview leads with.
    pub last_message_sender: Option<MessageSender>,
    /// `forumTopic.last_message.is_outgoing` — the preview says "You".
    pub last_message_is_outgoing: bool,
    /// Subsection tabs: `forumTopicIcon.color` (RGB, schema 1.8.67 line
    /// 3939) — the round topic icon's fill.
    pub icon_color: i32,
    /// Subsection tabs: `forumTopicIcon.custom_emoji_id` (0 = none). Kept
    /// so the icon can be upgraded later; the tabs draw the letter icon.
    pub icon_custom_emoji_id: i64,
    /// Subsection tabs: `forumTopic.last_message.id` (0 = none) — the
    /// "Mark as read" target and the unread bookkeeping anchor.
    pub last_message_id: i64,
    /// Subsection tabs: `forumTopic.last_read_inbox_message_id`.
    pub last_read_inbox_message_id: i64,
    /// Subsection tabs: `forumTopic.notification_settings` — drives the
    /// tab menu's Mute / Unmute (`setForumTopicNotificationSettings`).
    pub notification_settings: ChatNotificationSettings,
    /// `forumTopic.unread_mention_count` — gates "Mark all mentions as read".
    pub unread_mention_count: i32,
    /// `forumTopic.unread_reaction_count` — gates "Read all reactions".
    pub unread_reaction_count: i32,
    /// `forumTopicInfo.creation_date` (Unix time, 0 = unknown).
    pub creation_date: i32,
    /// `forumTopicInfo.creator_id`.
    pub creator: Option<MessageSender>,
    /// `forumTopicInfo.is_outgoing` — the current user created the topic.
    pub is_outgoing: bool,
    /// `forumTopic.draft_message` (schema 1.8.68, line 4278): the topic's
    /// own text draft, shown as "Draft:" in the topic row and restored
    /// when the topic opens (tdesktop `ForumTopic::cloudDraft`).
    pub draft: Option<ChatDraft>,
}

/// Subsection tabs: the `forumTopicInfo` fields that
/// `updateForumTopicInfo` refreshes (schema 1.8.67, lines 3953 / 10652).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopicInfoUpdate {
    pub chat_id: i64,
    pub forum_topic_id: i32,
    pub name: String,
    pub icon_color: i32,
    pub icon_custom_emoji_id: i64,
    pub is_general: bool,
    pub is_closed: bool,
    pub is_hidden: bool,
    pub creation_date: i32,
    pub creator: Option<MessageSender>,
    pub is_outgoing: bool,
}

/// Subsection tabs: `updateForumTopic` (schema 1.8.67, line 10665).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopicUpdate {
    pub chat_id: i64,
    pub forum_topic_id: i32,
    pub is_pinned: bool,
    pub last_read_inbox_message_id: i64,
    pub notification_settings: ChatNotificationSettings,
    pub unread_mention_count: i32,
    pub unread_reaction_count: i32,
    /// `updateForumTopic.draft_message` (schema 1.8.68, line 11028).
    pub draft: Option<ChatDraft>,
}

fn icon_fields(info: &Value) -> (i32, i64) {
    let icon = info.get("icon");
    let color = icon
        .and_then(|i| i.get("color"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .sat_i32();
    let custom_emoji_id = int53_or_zero(icon.and_then(|i| i.get("custom_emoji_id")));
    (color, custom_emoji_id)
}

/// Subsection tabs: parse a `forumTopicInfo` object.
pub(crate) fn parse_forum_topic_info(info: &Value) -> Option<ForumTopicInfoUpdate> {
    let flag = |name: &str| info.get(name).and_then(Value::as_bool).unwrap_or(false);
    let (icon_color, icon_custom_emoji_id) = icon_fields(info);
    Some(ForumTopicInfoUpdate {
        chat_id: info.get("chat_id").and_then(Value::as_i64)?,
        forum_topic_id: info.get("forum_topic_id")?.as_i64()?.sat_i32(),
        name: json_field_str(info, "name"),
        icon_color,
        icon_custom_emoji_id,
        is_general: flag("is_general"),
        is_closed: flag("is_closed"),
        is_hidden: flag("is_hidden"),
        creation_date: info
            .get("creation_date")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .sat_i32(),
        creator: parse_message_sender(info.get("creator_id")).ok(),
        is_outgoing: flag("is_outgoing"),
    })
}

/// Subsection tabs: parse an `updateForumTopic` object.
pub(crate) fn parse_forum_topic_update(value: &Value) -> Option<ForumTopicUpdate> {
    Some(ForumTopicUpdate {
        chat_id: value.get("chat_id").and_then(Value::as_i64)?,
        forum_topic_id: value.get("forum_topic_id")?.as_i64()?.sat_i32(),
        is_pinned: value
            .get("is_pinned")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        last_read_inbox_message_id: int53_or_zero(value.get("last_read_inbox_message_id")),
        notification_settings: parse_chat_notification_settings(value.get("notification_settings")),
        unread_mention_count: int53_or_zero(value.get("unread_mention_count")).sat_i32(),
        unread_reaction_count: int53_or_zero(value.get("unread_reaction_count")).sat_i32(),
        draft: parse_chat_draft(value.get("draft_message")),
    })
}

/// Parse one `forumTopic` object. Returns `None` when `info` is missing or
/// malformed (the row is skipped, matching the lenient message parsing).
pub(crate) fn parse_forum_topic(value: &Value) -> Option<ForumTopic> {
    let info = value.get("info")?;
    let forum_topic_id = info.get("forum_topic_id")?.as_i64()?.sat_i32();
    let name = json_field_str(info, "name");
    let is_general = info
        .get("is_general")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let is_closed = info
        .get("is_closed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let is_hidden = info
        .get("is_hidden")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let is_pinned = value
        .get("is_pinned")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let unread_count = value
        .get("unread_count")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .sat_i32();
    let order = int53_or_zero(value.get("order"));
    let last_message = value
        .get("last_message")
        .and_then(|m| parse_message(m).ok());
    let last_message_id = last_message.as_ref().map_or(0, |m| m.id.0);
    let last_message_date = last_message.as_ref().map_or(0, |m| m.date);
    let last_message_sender = last_message.as_ref().and_then(|m| m.sender);
    let last_message_is_outgoing = last_message.as_ref().is_some_and(|m| m.is_outgoing);
    let last_message_preview = last_message
        .map(|m| effective_content(&m.content, m.ephemeral.as_ref()).preview())
        .unwrap_or_default();
    let (icon_color, icon_custom_emoji_id) = icon_fields(info);
    Some(ForumTopic {
        forum_topic_id,
        name,
        is_general,
        is_closed,
        is_hidden,
        is_pinned,
        unread_count,
        order,
        last_message_preview,
        last_message_date,
        last_message_sender,
        last_message_is_outgoing,
        icon_color,
        icon_custom_emoji_id,
        last_message_id,
        last_read_inbox_message_id: int53_or_zero(value.get("last_read_inbox_message_id")),
        notification_settings: parse_chat_notification_settings(value.get("notification_settings")),
        unread_mention_count: int53_or_zero(value.get("unread_mention_count")).sat_i32(),
        unread_reaction_count: int53_or_zero(value.get("unread_reaction_count")).sat_i32(),
        creation_date: info
            .get("creation_date")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .sat_i32(),
        creator: parse_message_sender(info.get("creator_id")).ok(),
        is_outgoing: info
            .get("is_outgoing")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        draft: parse_chat_draft(value.get("draft_message")),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_forum_topic;
    use crate::telegram::envelope::MessageSender;
    use serde_json::{Value, json};

    fn topic(last_message: Value) -> Value {
        json!({
            "info": {
                "@type": "forumTopicInfo",
                "chat_id": 16,
                "forum_topic_id": 7,
                "name": "Releases",
                "icon": {"@type": "forumTopicIcon", "color": 7322096, "custom_emoji_id": "0"},
                "is_general": false,
                "is_closed": false,
                "is_hidden": false
            },
            "last_message": last_message,
            "order": "7",
            "is_pinned": false,
            "unread_count": 0,
            "last_read_inbox_message_id": 0,
            "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}
        })
    }

    fn message(sender_id: Value, is_outgoing: bool) -> Value {
        json!({
            "id": 50,
            "chat_id": 16,
            "date": 1_700_000_000,
            "is_outgoing": is_outgoing,
            "sender_id": sender_id,
            "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "ship it", "entities": []}}
        })
    }

    #[test]
    fn forum_topic_last_message_keeps_date_sender_and_flags() {
        let parsed = parse_forum_topic(&topic(message(
            json!({"@type": "messageSenderUser", "user_id": 5}),
            false,
        )))
        .expect("topic parses");
        assert_eq!(parsed.last_message_preview, "ship it");
        assert_eq!(parsed.last_message_date, 1_700_000_000);
        assert_eq!(
            parsed.last_message_sender,
            Some(MessageSender::User { user_id: 5 })
        );
        assert!(!parsed.last_message_is_outgoing);

        let own = parse_forum_topic(&topic(message(
            json!({"@type": "messageSenderChat", "chat_id": 77}),
            true,
        )))
        .expect("topic parses");
        assert_eq!(
            own.last_message_sender,
            Some(MessageSender::Chat { chat_id: 77 })
        );
        assert!(own.last_message_is_outgoing);
    }

    #[test]
    fn forum_topic_without_last_message_has_no_date_or_sender() {
        let parsed = parse_forum_topic(&topic(Value::Null)).expect("topic parses");
        assert_eq!(parsed.last_message_preview, "");
        assert_eq!(parsed.last_message_date, 0);
        assert_eq!(parsed.last_message_sender, None);
        assert!(!parsed.last_message_is_outgoing);
    }
}
