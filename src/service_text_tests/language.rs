//! Service wording follows the language pack: a Russian pack (the shipped
//! `qll_*` overlay plus the `lng_*` values Telegram provides) changes the
//! sentences while the links stay on the same names.
use super::*;
use crate::lang::parse::parse_strings;
use crate::lang::{PackValue, Snapshot, install, isolate_thread};

fn ru(one: &str, few: &str, many: &str, other: &str) -> PackValue {
    PackValue::Pluralized([
        String::new(),
        one.into(),
        String::new(),
        few.into(),
        many.into(),
        other.into(),
    ])
}

fn ordinary(text: &str) -> PackValue {
    PackValue::Ordinary(text.into())
}

/// Installs a Russian pack for the rest of the test and puts English back
/// at the end.
fn with_russian(test: impl FnOnce()) {
    isolate_thread();
    let mut pack = Snapshot::new("ru", "ru", false);
    let overlay = parse_strings(include_str!("../../lang/ru/service.strings"))
        .expect("the Russian service overlay parses");
    pack.layer_entries(&overlay);
    pack.layer([
        (
            "lng_action_user_joined",
            &ordinary("{from} вступил(а) в группу"),
        ),
        ("lng_action_add_user", &ordinary("{from} добавил(а) {user}")),
        (
            "lng_action_add_users_many",
            &ordinary("{from} добавил(а) {users}"),
        ),
        (
            "lng_action_add_users_and_one",
            &ordinary("{accumulated}, {user}"),
        ),
        (
            "lng_action_add_users_and_last",
            &ordinary("{accumulated} и {user}"),
        ),
        (
            "lng_action_pinned_message",
            &ordinary("{from} закрепил(а) сообщение «{text}»"),
        ),
        (
            "lng_action_changed_title",
            &ordinary("{from} изменил(а) название группы на «{title}»"),
        ),
        (
            "lng_action_changed_photo",
            &ordinary("{from} обновил(а) фото группы"),
        ),
        (
            "lng_action_ttl_changed",
            &ordinary("{from} установил(а) автоудаление сообщений через {duration}"),
        ),
        (
            "lng_hours",
            &ru(
                "{count} час",
                "{count} часа",
                "{count} часов",
                "{count} часа",
            ),
        ),
        (
            "lng_action_gift_received",
            &ordinary("{user} отправил(а) вам подарок за {cost}"),
        ),
        (
            "lng_action_gift_for_stars",
            &ru(
                "{count} звезда",
                "{count} звезды",
                "{count} звёзд",
                "{count} звезды",
            ),
        ),
        (
            "lng_action_group_call_scheduled_group",
            &ordinary("{from} запланировал(а) видеозвонок на {date}"),
        ),
        ("lng_group_call_starts_today", &ordinary("сегодня в {time}")),
        (
            "lng_action_proximity_reached_you",
            &ordinary("{from} теперь в {distance} от вас"),
        ),
        (
            "lng_action_proximity_distance_m",
            &ru(
                "{count} метр",
                "{count} метра",
                "{count} метров",
                "{count} метра",
            ),
        ),
        (
            "lng_replies_discussion_started",
            &ordinary("Обсуждение начато"),
        ),
        (
            "lng_replies_no_comments",
            &ordinary("Комментариев пока нет…"),
        ),
    ]);
    install(Some(pack));
    test();
    install(None);
}

#[test]
fn russian_join_add_and_pin_keep_their_links() {
    with_russian(|| {
        assert_eq!(
            say_action(ServiceAction::AddMembers { users: vec![DANA] }, BY_DANA),
            "Dana Cole вступил(а) в группу"
        );
        assert_eq!(
            say(
                r#"{"@type":"messageChatAddMembers","member_user_ids":[2,3]}"#,
                BY_DANA
            ),
            "Dana Cole добавил(а) Omar Haddad и Lea Stern"
        );
        let text = run(r#"{"@type":"messagePinMessage","message_id":100}"#, BY_DANA);
        let (plain, links) = text.spans();
        assert_eq!(
            plain,
            "Dana Cole закрепил(а) сообщение «Lunch at noon, b\u{2026}»"
        );
        assert_eq!(
            links.len(),
            2,
            "the actor and the excerpt keep links: {plain}"
        );
        assert_eq!(
            links
                .iter()
                .map(|(range, link)| (&plain[range.clone()], link))
                .collect::<Vec<_>>(),
            vec![
                (
                    "Dana Cole",
                    &ServiceLink::Sender(MessageSender::User { user_id: DANA })
                ),
                ("Lunch at noon, b\u{2026}", &ServiceLink::Message(100)),
            ]
        );
    });
}

#[test]
fn russian_title_photo_and_ttl_follow_the_pack() {
    with_russian(|| {
        assert_eq!(
            say(
                r#"{"@type":"messageChatChangeTitle","title":"Brunch Club"}"#,
                BY_DANA
            ),
            "Dana Cole изменил(а) название группы на «Brunch Club»"
        );
        assert_eq!(
            say(
                r#"{"@type":"messageChatChangePhoto","photo":{"@type":"chatPhoto","sizes":[]}}"#,
                BY_DANA
            ),
            "Dana Cole обновил(а) фото группы"
        );
        assert_eq!(
            say(
                r#"{"@type":"messageChatSetMessageAutoDeleteTime","message_auto_delete_time":7200,"from_user_id":1}"#,
                BY_DANA
            ),
            "Dana Cole установил(а) автоудаление сообщений через 2 часа"
        );
    });
}

#[test]
fn russian_gift_call_and_proximity_are_plural_and_reordered() {
    with_russian(|| {
        let gift = run(
            r#"{"@type":"messageGift","gift":{"@type":"gift","id":"1","star_count":100},"sender_id":{"@type":"messageSenderUser","user_id":1},"receiver_id":{"@type":"messageSenderUser","user_id":9},"prepaid_upgrade_star_count":0}"#,
            BY_DANA,
        );
        assert_eq!(gift.plain(), "Dana отправил(а) вам подарок за 100 звёзд");
        let schedule = say(
            r#"{"@type":"messageVideoChatScheduled","group_call_id":5,"start_date":1800000060}"#,
            BY_DANA,
        );
        assert!(
            schedule.starts_with("Dana Cole запланировал(а) видеозвонок на сегодня в "),
            "{schedule}"
        );
        assert_eq!(
            say(
                r#"{"@type":"messageProximityAlertTriggered","traveler_id":{"@type":"messageSenderUser","user_id":1},"watcher_id":{"@type":"messageSenderUser","user_id":9},"distance":250}"#,
                BY_DANA
            ),
            "Dana Cole теперь в 250 метров от вас"
        );
    });
}

#[test]
fn russian_thread_divider_follows_the_pack() {
    with_russian(|| {
        assert_eq!(
            say_action(
                ServiceAction::ThreadDivider {
                    with_comments: true
                },
                BY_DANA
            ),
            "Обсуждение начато"
        );
        assert_eq!(
            say_action(
                ServiceAction::ThreadDivider {
                    with_comments: false
                },
                BY_DANA
            ),
            "Комментариев пока нет…"
        );
    });
}

#[test]
fn english_comes_back_when_the_pack_is_removed() {
    with_russian(|| {
        assert_eq!(
            say_action(ServiceAction::AddMembers { users: vec![DANA] }, BY_DANA),
            "Dana Cole вступил(а) в группу"
        );
    });
    assert_eq!(
        say_action(ServiceAction::AddMembers { users: vec![DANA] }, BY_DANA),
        "Dana Cole joined the group"
    );
}
