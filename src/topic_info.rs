//! Rows of the topic and reply-thread info cards (tdesktop shows the same
//! facts in the topic profile and the replies section header). Pure
//! formatting; names and dates come in already resolved.
#![deny(clippy::indexing_slicing)]

use crate::lang::{keys, tr, trn};
use crate::telegram::envelope::ForumTopic;

/// One label/value line of an info card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoRow {
    pub label: &'static str,
    pub value: String,
}

fn row(label: &'static str, value: impl Into<String>) -> InfoRow {
    InfoRow {
        label,
        value: value.into(),
    }
}

/// Info rows for a forum topic. `creator` is the resolved name of
/// `forumTopicInfo.creator_id` and `created` its formatted creation date.
pub fn topic_info_rows(
    topic: &ForumTopic,
    creator: Option<&str>,
    created: Option<&str>,
) -> Vec<InfoRow> {
    let mut rows = Vec::new();
    let created_by = tr(keys::Q_PEOPLE_TOPIC_INFO_CREATED_BY);
    if topic.is_outgoing {
        rows.push(row(created_by, tr(keys::FROM_YOU)));
    } else if let Some(name) = creator.filter(|name| !name.is_empty()) {
        rows.push(row(created_by, name));
    }
    if let Some(date) = created.filter(|date| !date.is_empty()) {
        rows.push(row(tr(keys::Q_PEOPLE_TOPIC_INFO_CREATED), date));
    }
    let mut status = Vec::new();
    if topic.is_general {
        status.push(tr(keys::Q_PEOPLE_GENERAL));
    }
    if topic.is_pinned {
        status.push(tr(keys::SR_CHAT_PINNED));
    }
    if topic.is_closed {
        status.push(tr(keys::Q_PEOPLE_CLOSED));
    }
    if topic.is_hidden {
        status.push(tr(keys::COMMUNITY_HIDDEN_STATUS));
    }
    if !status.is_empty() {
        rows.push(row(tr(keys::Q_PEOPLE_TOPIC_INFO_STATUS), status.join(", ")));
    }
    if topic.unread_count > 0 {
        rows.push(row(
            tr(keys::SR_CHAT_COLUMN_UNREAD),
            trn(keys::FORUM_MESSAGES, topic.unread_count),
        ));
    }
    if topic.unread_mention_count > 0 {
        rows.push(row(
            tr(keys::Q_PEOPLE_TOPIC_INFO_MENTIONS),
            trn(
                keys::Q_PEOPLE_TOPIC_INFO_MENTIONS_COUNT,
                topic.unread_mention_count,
            ),
        ));
    }
    rows
}

/// Info rows for a reply thread. `comments` is true for a channel post's
/// discussion; `started_by` and `started` describe the root message.
pub fn thread_info_rows(
    comments: bool,
    reply_count: i32,
    unread_count: i32,
    started_by: Option<&str>,
    started: Option<&str>,
) -> Vec<InfoRow> {
    let mut rows = Vec::new();
    let (title, header) = if comments {
        (keys::COMMENTS_HEADER_NONE, keys::COMMENTS_HEADER)
    } else {
        (keys::REPLIES_HEADER_NONE, keys::REPLIES_HEADER)
    };
    rows.push(row(tr(title), trn(header, reply_count.max(0))));
    if unread_count > 0 {
        rows.push(row(
            tr(keys::SR_CHAT_COLUMN_UNREAD),
            trn(header, unread_count),
        ));
    }
    if let Some(name) = started_by.filter(|name| !name.is_empty()) {
        rows.push(row(
            tr(if comments {
                keys::Q_PEOPLE_TOPIC_INFO_POSTED_BY
            } else {
                keys::Q_PEOPLE_TOPIC_INFO_STARTED_BY
            }),
            name,
        ));
    }
    if let Some(date) = started.filter(|date| !date.is_empty()) {
        rows.push(row(
            tr(if comments {
                keys::Q_PEOPLE_TOPIC_INFO_POSTED
            } else {
                keys::Q_PEOPLE_TOPIC_INFO_STARTED
            }),
            date,
        ));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::{InfoRow, thread_info_rows, topic_info_rows};
    use crate::telegram::envelope::{ChatNotificationSettings, ForumTopic};

    fn topic() -> ForumTopic {
        ForumTopic {
            forum_topic_id: 3,
            name: "Releases".into(),
            is_general: false,
            is_closed: false,
            is_pinned: false,
            is_hidden: false,
            unread_count: 0,
            order: 0,
            last_message_preview: String::new(),
            last_message: None,
            icon_color: 0,
            icon_custom_emoji_id: 0,
            last_message_id: 0,
            last_read_inbox_message_id: 0,
            notification_settings: ChatNotificationSettings::default(),
            unread_mention_count: 0,
            unread_reaction_count: 0,
            creation_date: 0,
            creator: None,
            is_outgoing: false,
            draft: None,
        }
    }

    fn values(rows: &[InfoRow]) -> Vec<(&'static str, &str)> {
        rows.iter().map(|r| (r.label, r.value.as_str())).collect()
    }

    #[test]
    fn plain_topic_without_data_has_no_rows() {
        assert!(topic_info_rows(&topic(), None, None).is_empty());
    }

    #[test]
    fn topic_rows_cover_creator_date_status_and_counts() {
        let mut t = topic();
        t.is_pinned = true;
        t.is_closed = true;
        t.unread_count = 1;
        t.unread_mention_count = 2;
        let rows = topic_info_rows(&t, Some("Dana"), Some("3 March 2026, 10:00"));
        assert_eq!(
            values(&rows),
            vec![
                ("Created by", "Dana"),
                ("Created", "3 March 2026, 10:00"),
                ("Status", "Pinned, Closed"),
                ("Unread", "1 message"),
                ("Mentions", "2 mentions"),
            ]
        );
        t.is_outgoing = true;
        assert_eq!(
            topic_info_rows(&t, Some("Dana"), None)[0].value,
            "You",
            "own topics say You even when a name is known"
        );
    }

    #[test]
    fn thread_rows_word_comments_and_replies() {
        let rows = thread_info_rows(true, 1, 0, Some("News"), Some("1 May 2026, 09:00"));
        assert_eq!(
            values(&rows),
            vec![
                ("Comments", "1 comment"),
                ("Posted by", "News"),
                ("Posted", "1 May 2026, 09:00"),
            ]
        );
        let rows = thread_info_rows(false, 5, 2, None, None);
        assert_eq!(
            values(&rows),
            vec![("Replies", "5 replies"), ("Unread", "2 replies")]
        );
    }
}
