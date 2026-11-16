//! Screenshot demos: the open forum topic while its first page loads, after
//! it failed, and when it is really empty. The forum has topic tabs, one
//! with a very long name, and the last topic is open so the strip has to
//! scroll it into view (injected, no live Telegram).
#![deny(clippy::indexing_slicing)]

use crate::ui::app::QuillApp;
use crate::ui::screenshot_demo::{DemoSpec, register_demos};
use gpui_kit::*;
use quill::ids::ChatId;
use quill::state::{Session, TopicHistory};
use quill::telegram::client::copy_and_parse;
use std::sync::atomic::Ordering;

const FORUM: i64 = 16;
const OPEN_TOPIC: i32 = 9;
/// The newest seeded message's date: a fixed instant, so the demo's rows
/// read the same on every run (the other demos' messages use it too).
const NEWEST: i64 = 1_790_632_300;

register_demos![
    DemoSpec::chats(
        "ready-topic-loading",
        "screenshot demo — forum topic while its first page loads"
    )
    .setup(QuillApp::demo_topic_loading),
    DemoSpec::chats(
        "ready-topic-failed",
        "screenshot demo — forum topic whose first page failed"
    )
    .setup(QuillApp::demo_topic_failed),
    DemoSpec::chats(
        "ready-topic-empty",
        "screenshot demo — forum topic with no messages"
    )
    .setup(QuillApp::demo_topic_empty),
    DemoSpec::chats("ready-forum-topics", "screenshot demo — forum topic list")
        .setup(QuillApp::demo_forum_topics),
];

/// One seeded topic: its last message (Maya's, or yours) and its counters.
struct Seed {
    id: i32,
    name: &'static str,
    said: &'static str,
    /// Seconds before `NEWEST`.
    ago: i64,
    outgoing: bool,
    unread: i32,
    mentions: i32,
    muted: bool,
    closed: bool,
}

const SEED: Seed = Seed {
    id: 0,
    name: "",
    said: "",
    ago: 0,
    outgoing: false,
    unread: 0,
    mentions: 0,
    muted: false,
    closed: false,
};

fn topic_json(seed: &Seed) -> String {
    let Seed {
        id,
        name,
        said,
        ago,
        outgoing,
        unread,
        mentions,
        muted,
        closed,
    } = *seed;
    let last = 100 + id;
    // Read up to the first unread message, so the count is the server's.
    let read = last - unread;
    let date = NEWEST - ago;
    let (default_mute, mute_for) = if muted {
        (false, 2_000_000_000)
    } else {
        (true, 0)
    };
    format!(
        r#"{{"info":{{"@type":"forumTopicInfo","chat_id":{FORUM},"forum_topic_id":{id},"name":"{name}","icon":{{"@type":"forumTopicIcon","color":16766590,"custom_emoji_id":"0"}},"is_general":false,"is_closed":{closed},"is_hidden":false,"is_outgoing":false}},"last_message":{{"id":{last},"chat_id":{FORUM},"date":{date},"is_outgoing":{outgoing},"sender_id":{{"@type":"messageSenderUser","user_id":81}},"content":{{"@type":"messageText","text":{{"@type":"formattedText","text":"{said}","entities":[]}}}}}},"order":"{id}","is_pinned":false,"unread_count":{unread},"unread_mention_count":{mentions},"last_read_inbox_message_id":{read},"notification_settings":{{"@type":"chatNotificationSettings","use_default_mute_for":{default_mute},"mute_for":{mute_for}}}}}"#
    )
}

fn seed_forum(session: &mut Session, app: &QuillApp) {
    let sink = app.demo_ui.sink.clone();
    let seq = &app.demo_ui.seq;
    seq.store(session.last_seq, Ordering::SeqCst);
    let dyn_sink: std::sync::Arc<dyn quill::diagnostics::DiagnosticSink> = sink;
    for json in [
        format!(
            r#"{{"@type":"updateNewChat","chat":{{"id":{FORUM},"title":"Dev Forum","type":{{"@type":"chatTypeSupergroup","supergroup_id":{FORUM},"is_channel":false}},"unread_count":0}}}}"#
        ),
        format!(
            r#"{{"@type":"updateSupergroup","supergroup":{{"@type":"supergroup","id":{FORUM},"is_forum":true,"has_forum_tabs":true}}}}"#
        ),
        r#"{"@type":"updateUser","user":{"id":81,"first_name":"Maya","last_name":"","type":{"@type":"userTypeRegular"}}}"#.to_owned(),
    ] {
        if let Some(owned) = copy_and_parse(&json, seq, &dyn_sink) {
            session.apply(owned);
        }
    }
    let seeds = [
        Seed {
            id: 2,
            name: "Announcements",
            said: "Rules are up for the quarter",
            ago: 600,
            closed: true,
            ..SEED
        },
        Seed {
            id: 3,
            name: "Release planning and roadmap discussion for the next quarter",
            said: "Push the milestone to Friday?",
            ago: 300,
            unread: 3,
            ..SEED
        },
        Seed {
            id: 4,
            name: "Design",
            said: "New icon set is ready",
            ago: 1_800,
            ..SEED
        },
        Seed {
            id: 5,
            name: "Backend",
            said: "Rate limits look stable",
            ago: 2_400,
            unread: 1,
            mentions: 1,
            ..SEED
        },
        Seed {
            id: 6,
            name: "Frontend",
            said: "Fixed the truncation bug",
            ago: 3_600,
            outgoing: true,
            ..SEED
        },
        Seed {
            id: 7,
            name: "Off topic",
            said: "Coffee machine is fixed",
            ago: 7_200,
            unread: 8,
            muted: true,
            ..SEED
        },
        Seed {
            id: 8,
            name: "Questions and answers",
            said: "Thread about topics is open",
            ago: 9_000,
            ..SEED
        },
        Seed {
            id: OPEN_TOPIC,
            name: "Build infrastructure",
            said: "Cache hit rate went up",
            ago: 12_000,
            ..SEED
        },
    ];
    let topics: Vec<String> = seeds.iter().map(topic_json).collect();
    let extra = session.ask_forum_topics(ChatId(FORUM));
    let answer = format!(
        r#"{{"@type":"forumTopics","@extra":"{}","total_count":{},"topics":[{}],"next_offset_date":0,"next_offset_message_id":0,"next_offset_forum_topic_id":0}}"#,
        extra.0,
        seeds.len(),
        topics.join(",")
    );
    if let Some(owned) = copy_and_parse(&answer, seq, &dyn_sink) {
        session.apply(owned);
    }
    session.open_chat(ChatId(FORUM));
    session.select_topic(ChatId(FORUM), OPEN_TOPIC);
}

impl QuillApp {
    fn demo_topic_session(&mut self, then: impl FnOnce(&mut Session)) {
        // `seed_forum` reads the demo sink and counter off the app while the
        // session is borrowed, so take the session out for the duration.
        if let Some(mut session) = self.demo_session.take() {
            seed_forum(&mut session, self);
            then(&mut session);
            self.demo_session = Some(session);
        }
    }

    fn demo_topic_loading(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.demo_topic_session(|_| {});
    }

    /// The topic list itself: the forum column instead of the topic tabs,
    /// with the seeded rows. `has_forum_tabs` would draw the tabs instead.
    fn demo_forum_topics(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.demo_topic_session(|session| {
            session.threads.forum_tabs_supergroups.remove(&FORUM);
            session.deselect_topic();
        });
    }

    fn demo_topic_failed(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.demo_topic_session(|session| {
            session
                .threads
                .topic_histories
                .entry((FORUM, OPEN_TOPIC))
                .or_default()
                .older = quill::state::OlderEnd::Failed;
        });
    }

    fn demo_topic_empty(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.demo_topic_session(|session| {
            let history = TopicHistory {
                older: quill::state::OlderEnd::Complete,
                ..TopicHistory::default()
            };
            session
                .threads
                .topic_histories
                .insert((FORUM, OPEN_TOPIC), history);
        });
    }
}
