//! Service-message wording, following Telegram Desktop's
//! `HistoryItem::setServiceMessageByAction` / `prepare*Text` strings
//! (`lng_action_*` in `lang.strings`). Pure: the caller supplies the
//! names it knows through [`ServiceNames`], and gets back a
//! [`ServiceText`] — plain segments, some carrying a link (a user or chat
//! to open, a message to jump to) — that both the history pill and the
//! chat-list preview use.
#![deny(clippy::indexing_slicing)]

use crate::lang::{Key, PluralKey, keys, tr, tr_with, trn, trn_with};
use crate::local_time::{civil_local, day_label, hhmm};
use crate::telegram::envelope::{
    BotAccessReason, ExpiredKind, GiftOrigin, MessageContent, MessageSender, Money, PassportKind,
    ServiceAction, format_payment_price,
};
use std::ops::Range;

mod actions;
mod gifts;
mod wallet;
pub use actions::*;

/// What a clickable segment opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceLink {
    /// The profile of a user or chat.
    Sender(MessageSender),
    /// Scroll to a message in this chat.
    Message(i64),
    /// An external address.
    Url(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seg {
    pub text: String,
    pub link: Option<ServiceLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceText {
    pub segs: Vec<Seg>,
}

impl ServiceText {
    /// The text without links (chat-list preview, search, accessibility).
    pub fn plain(&self) -> String {
        self.segs.iter().map(|seg| seg.text.as_str()).collect()
    }

    /// The text and the byte ranges of its links, for a styled paragraph.
    pub fn spans(&self) -> (String, Vec<(Range<usize>, ServiceLink)>) {
        let mut text = String::new();
        let mut links = Vec::new();
        for seg in &self.segs {
            let start = text.len();
            text.push_str(&seg.text);
            if let Some(link) = &seg.link
                && !seg.text.is_empty()
            {
                links.push((start..text.len(), link.clone()));
            }
        }
        (text, links)
    }
}

/// What a pinned message shows in "X pinned …".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinTarget {
    /// Text excerpt of the pinned message.
    Text(String),
    /// A media phrase ("a photo", "a voice message"…).
    Media(String),
    /// The message is not in the cache: a lookup result, nothing is asked
    /// for here.
    NotLoaded,
}

/// Lookups the wording needs; implemented by the session.
pub trait ServiceNames {
    /// Full display name of a user.
    fn user_name(&self, user_id: i64) -> Option<String>;
    /// Short name (first name) of a user; defaults to the full name.
    fn user_short_name(&self, user_id: i64) -> Option<String> {
        self.user_name(user_id)
    }
    /// Title of a chat.
    fn chat_name(&self, chat_id: i64) -> Option<String>;
    /// The message a pin points at.
    fn pinned(&self, message_id: i64) -> PinTarget;
    /// Title of the game a game-score message answers.
    fn game_title(&self, message_id: i64) -> Option<String>;
}

pub struct ServiceCtx<'a> {
    /// `message.sender_id`.
    pub actor: Option<MessageSender>,
    pub is_outgoing: bool,
    pub my_id: Option<i64>,
    /// A broadcast channel: events are anonymous posts.
    pub is_channel: bool,
    pub is_secret: bool,
    /// The chat the message lives in (the bot, for shared-chat rows).
    pub chat_id: i64,
    /// Unix time, for "today at…" in scheduled-call rows.
    pub now: i64,
    pub names: &'a dyn ServiceNames,
}

#[derive(Default, Clone)]
struct B(Vec<Seg>);

impl B {
    fn t(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        if text.is_empty() {
            return self;
        }
        match self.0.last_mut() {
            Some(Seg {
                link: None,
                text: last,
            }) => last.push_str(&text),
            _ => self.0.push(Seg { text, link: None }),
        }
        self
    }

    fn link(mut self, text: impl Into<String>, link: ServiceLink) -> Self {
        self.0.push(Seg {
            text: text.into(),
            link: Some(link),
        });
        self
    }

    /// Appends another builder's segments.
    fn extend(mut self, other: B) -> Self {
        for seg in other.0 {
            self = match seg.link {
                Some(link) => self.link(seg.text, link),
                None => self.t(seg.text),
            };
        }
        self
    }

    fn done(self) -> ServiceText {
        ServiceText { segs: self.0 }
    }
}

/// A name placeholder inside a sentence: each name gets its own
/// private-use character, so the pack may put it anywhere in the
/// sentence and [`sentence`] still finds it.
fn mark(index: usize) -> String {
    u32::try_from(index)
        .ok()
        .and_then(|index| 0xE000_u32.checked_add(index))
        .and_then(char::from_u32)
        .map_or_else(String::new, |ch| ch.to_string())
}

/// Rebuilds a translated sentence: each [`mark`] becomes the segments of
/// its name (links kept), in whatever order the translation puts them.
fn sentence(template: &str, parts: &[B]) -> B {
    let mut out = B::default();
    let mut plain = String::new();
    for ch in template.chars() {
        let slot = (ch as u32)
            .checked_sub(0xE000)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| parts.get(index));
        match slot {
            Some(part) => {
                if !plain.is_empty() {
                    out = out.t(std::mem::take(&mut plain));
                }
                out = out.extend(part.clone());
            }
            None => plain.push(ch),
        }
    }
    if !plain.is_empty() {
        out = out.t(plain);
    }
    out
}

fn marks(parts: &[(&str, B)]) -> (Vec<String>, Vec<B>) {
    let marks: Vec<String> = (0..parts.len()).map(mark).collect();
    let bodies: Vec<B> = parts.iter().map(|(_, body)| body.clone()).collect();
    (marks, bodies)
}

/// `key` with its `{tag}` placeholders filled by `parts`, each keeping its
/// link. A name the translation drops simply disappears.
fn phrase(key: Key, parts: &[(&str, B)]) -> B {
    let (marks, bodies) = marks(parts);
    let tags: Vec<(&str, &str)> = parts
        .iter()
        .zip(&marks)
        .map(|((tag, _), mark)| (*tag, mark.as_str()))
        .collect();
    sentence(&tr_with(key, &tags), &bodies)
}

/// [`phrase`] with a plural form chosen by `count` (`{count}` filled in).
fn phrase_plural(key: PluralKey, count: i64, parts: &[(&str, B)]) -> B {
    phrase_plural_count(key, count, &count.to_string(), parts)
}

/// [`phrase_plural`] with `{count}` written as `shown` (a fractional Gram
/// amount, say) while the form still follows `count`.
fn phrase_plural_count(key: PluralKey, count: i64, shown: &str, parts: &[(&str, B)]) -> B {
    let (marks, bodies) = marks(parts);
    let mut tags: Vec<(&str, &str)> = parts
        .iter()
        .zip(&marks)
        .map(|((tag, _), mark)| (*tag, mark.as_str()))
        .collect();
    if !parts.iter().any(|(tag, _)| *tag == "count") {
        tags.push(("count", shown));
    }
    sentence(&trn_with(key, count, &tags), &bodies)
}

/// A text-only `key`, its `{tag}` placeholders filled.
fn line(key: Key, tags: &[(&str, &str)]) -> ServiceText {
    B::default().t(tr_with(key, tags)).done()
}

/// "A, B and C": the segments joined with the pack's list words
/// (`lng_action_*_and_one` / `_and_last`).
fn join_segments(items: &[B], and_one: Key, and_last: Key) -> B {
    let Some((first, rest)) = items.split_first() else {
        return B::default().t(someone());
    };
    let mut out = first.clone();
    for (index, item) in rest.iter().enumerate() {
        let key = if index + 1 == rest.len() {
            and_last
        } else {
            and_one
        };
        out = phrase(key, &[("accumulated", out), ("user", item.clone())]);
    }
    out
}

/// `Ui::FormatTTL`.
fn format_ttl(secs: i32) -> String {
    let secs = i64::from(secs);
    if secs < 86_400 {
        trn(keys::HOURS, secs / 3600)
    } else if secs < 86_400 * 7 {
        trn(keys::DAYS, secs / 86_400)
    } else if secs < 86_400 * 31 {
        let days = secs / 86_400;
        if secs % 7 == 0 {
            trn(keys::WEEKS, days / 7)
        } else {
            format!(
                "{} {}",
                trn(keys::WEEKS, days / 7),
                trn(keys::DAYS, days % 7)
            )
        }
    } else if secs <= 86_400 * 31 * 11 {
        trn(keys::MONTHS, secs / (86_400 * 31))
    } else {
        trn(
            keys::YEARS,
            (secs as f64 / (86_400.0 * 365.0)).round() as i64,
        )
    }
}

/// The duration of a finished video chat (`prepareGroupCall`).
fn call_duration(secs: i32) -> String {
    let secs = i64::from(secs);
    let (days, hours, minutes) = (secs / 86_400, secs / 3600, secs / 60);
    if days > 1 {
        trn(keys::DAYS, days)
    } else if hours > 1 {
        trn(keys::HOURS, hours)
    } else if minutes > 1 {
        trn(keys::MINUTES, minutes)
    } else {
        trn(keys::SECONDS, secs)
    }
}

fn grams(cents: i64) -> String {
    let (whole, frac) = (cents / 100, cents % 100);
    if frac == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{frac:02}")
            .trim_end_matches('0')
            .to_string()
    }
}

/// The plural form a Gram count already written as text selects. A
/// fractional count reads as `other`, which the `few` form covers in
/// Russian and `other` in English.
fn gram_form(text: &str) -> i64 {
    if text.contains('.') {
        2
    } else {
        text.parse().unwrap_or(2)
    }
}

/// "1 Gram" / "2.5 Grams".
fn grams_cost(cents: i64) -> String {
    let text = grams(cents);
    trn_with(
        keys::ACTION_GIFT_FOR_TON,
        gram_form(&text),
        &[("count", &text)],
    )
}

/// "1 Gram" / "2.5 Grams" for a Gram count already formatted as text.
fn gram_amount(text: &str) -> String {
    trn_with(
        keys::ACTION_GRAM_TRANSFER_AMOUNT,
        gram_form(text),
        &[("amount", text)],
    )
}

fn group_digits(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if value < 0 {
        out.insert(0, '-');
    }
    out
}

/// `AmountAndStarCurrency`.
fn amount(money: &Money) -> String {
    match money.currency.as_str() {
        "XTR" => format!("\u{2B50}{}", group_digits(money.amount)),
        "TON" => format!("{} TON", grams(money.amount)),
        currency => format_payment_price(currency, money.amount),
    }
}

/// `lng_action_gift_for_stars` and friends: "N Stars" / "N Grams".
fn cost(money: &Money) -> String {
    match money.currency.as_str() {
        "XTR" => stars_cost(money.amount),
        "TON" => grams_cost(money.amount),
        _ => amount(money),
    }
}

fn stars_cost(stars: i64) -> String {
    trn(keys::ACTION_GIFT_FOR_STARS, stars)
}

fn distance(meters: i32) -> String {
    if meters >= 1000 {
        let km = f64::from(10 * (meters / 10)) / 1000.0;
        let text = format!("{km:.2}");
        let text = text.trim_end_matches('0').trim_end_matches('.');
        trn_with(
            keys::ACTION_PROXIMITY_DISTANCE_KM,
            gram_form(text),
            &[("count", text)],
        )
    } else {
        trn(keys::ACTION_PROXIMITY_DISTANCE_M, i64::from(meters))
    }
}

impl ServiceCtx<'_> {
    fn is_me_user(&self, user_id: i64) -> bool {
        self.my_id == Some(user_id)
    }

    fn actor_is_me(&self) -> bool {
        self.is_outgoing
            || matches!(self.actor, Some(MessageSender::User { user_id }) if self.is_me_user(user_id))
    }

    fn user(&self, user_id: i64) -> B {
        let name = self.names.user_name(user_id).unwrap_or_else(someone_string);
        B::default().link(name, ServiceLink::Sender(MessageSender::User { user_id }))
    }

    fn user_short(&self, user_id: i64) -> B {
        let name = self
            .names
            .user_short_name(user_id)
            .unwrap_or_else(someone_string);
        B::default().link(name, ServiceLink::Sender(MessageSender::User { user_id }))
    }

    fn chat(&self, chat_id: i64) -> B {
        let name = self
            .names
            .chat_name(chat_id)
            .unwrap_or_else(|| tr(keys::Q_SERVICE_A_CHAT).to_string());
        B::default().link(name, ServiceLink::Sender(MessageSender::Chat { chat_id }))
    }

    fn peer(&self, sender: MessageSender) -> B {
        match sender {
            MessageSender::User { user_id } => self.user(user_id),
            MessageSender::Chat { chat_id } => self.chat(chat_id),
        }
    }

    /// `{from}`: the acting member.
    fn from(&self) -> B {
        match self.actor {
            Some(actor) => {
                let mut out = self.peer(actor);
                if self.actor_is_me()
                    && let Some(seg) = out.0.first_mut()
                    && (seg.text == someone() || seg.text == tr(keys::Q_SERVICE_A_CHAT))
                {
                    seg.text = tr(keys::FROM_YOU).to_string();
                }
                out
            }
            None if self.is_outgoing => B::default().t(tr(keys::FROM_YOU)),
            None => B::default().t(someone()),
        }
    }

    /// A list "A, B and C" of user links, joined the way tdesktop's
    /// `lng_action_*_users_and_one` / `_and_last` keys say.
    fn users(&self, ids: &[i64]) -> B {
        self.join_users(
            ids,
            keys::ACTION_ADD_USERS_AND_ONE,
            keys::ACTION_ADD_USERS_AND_LAST,
        )
    }

    fn join_users(&self, ids: &[i64], and_one: Key, and_last: Key) -> B {
        let links: Vec<B> = ids.iter().map(|id| self.user(*id)).collect();
        join_segments(&links, and_one, and_last)
    }

    /// The other side of a private chat when the actor is me (the row's
    /// chat), else the actor.
    fn peer_of_gift(&self) -> B {
        if self.actor_is_me() {
            self.chat(self.chat_id)
        } else if let Some(MessageSender::User { user_id }) = self.actor {
            self.user_short(user_id)
        } else {
            self.from()
        }
    }

    /// "{user} sent you a gift for {cost}" / "You sent a gift for {cost}".
    fn gift_line(&self, cost: &str) -> ServiceText {
        let cost = B::default().t(cost);
        if self.actor_is_me() {
            phrase(keys::ACTION_GIFT_SENT, &[("cost", cost)]).done()
        } else {
            phrase(
                keys::ACTION_GIFT_RECEIVED,
                &[("user", self.peer_of_gift()), ("cost", cost)],
            )
            .done()
        }
    }

    fn gift_line_between(&self, gifter: i64, cost: &str) -> ServiceText {
        let cost = B::default().t(cost);
        if self.is_me_user(gifter) || self.actor_is_me() {
            phrase(keys::ACTION_GIFT_SENT, &[("cost", cost)]).done()
        } else {
            phrase(
                keys::ACTION_GIFT_RECEIVED,
                &[("user", self.user_short(gifter)), ("cost", cost)],
            )
            .done()
        }
    }

    /// "{channel} also included …": the prize wording naming the giveaway's
    /// chat.
    fn channel_prize(&self, key: Key, chat_id: i64) -> ServiceText {
        phrase(key, &[("channel", self.chat(chat_id))]).done()
    }

    fn format_schedule(&self, start: i32) -> String {
        let scheduled = civil_local(i64::from(start));
        let now = civil_local(self.now);
        let time = hhmm(&scheduled);
        let ahead = scheduled.day_number() - now.day_number();
        match ahead {
            0 => tr_with(keys::GROUP_CALL_STARTS_TODAY, &[("time", &time)]),
            1 => tr_with(keys::GROUP_CALL_STARTS_TOMORROW, &[("time", &time)]),
            _ => tr_with(
                keys::GROUP_CALL_STARTS_DATE,
                &[("date", &day_label(&scheduled, &now)), ("time", &time)],
            ),
        }
    }
}

/// The "Someone" fallback for a name the session does not know.
fn someone() -> &'static str {
    tr(keys::Q_CHAT_SOMEONE)
}

fn someone_string() -> String {
    someone().to_string()
}

fn pin_text(target: PinTarget) -> (String, bool) {
    match target {
        PinTarget::Text(text) => {
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            let mut chars = text.chars();
            let head: String = chars.by_ref().take(16).collect();
            // tdesktop cuts at 16 characters and only adds the ellipsis
            // when more than 5 remain.
            let rest = chars.count();
            if rest > 5 {
                (format!("{head}\u{2026}"), false)
            } else {
                (text, false)
            }
        }
        PinTarget::Media(phrase) => (phrase, true),
        PinTarget::NotLoaded => (tr(keys::Q_SERVICE_PINNED_MESSAGE).into(), true),
    }
}

/// The media phrase a pinned (or replied-to) message gets in service
/// text (`pinnedTextSubstring`), or `None` for text messages.
pub fn pinned_media_phrase(content: &MessageContent) -> Option<String> {
    Some(match content {
        MessageContent::Photo(_) => tr(keys::ACTION_PINNED_MEDIA_PHOTO).into(),
        MessageContent::Video(_) => tr(keys::ACTION_PINNED_MEDIA_VIDEO).into(),
        MessageContent::Audio(_) => tr(keys::ACTION_PINNED_MEDIA_AUDIO).into(),
        MessageContent::VoiceNote(_) => tr(keys::ACTION_PINNED_MEDIA_VOICE).into(),
        MessageContent::VideoNote(_) => tr(keys::ACTION_PINNED_MEDIA_VIDEO_MESSAGE).into(),
        MessageContent::Document(_) => tr(keys::ACTION_PINNED_MEDIA_FILE).into(),
        MessageContent::Animation(_) => tr(keys::ACTION_PINNED_MEDIA_GIF).into(),
        MessageContent::Contact(_) => tr(keys::ACTION_PINNED_MEDIA_CONTACT).into(),
        MessageContent::Location(_) | MessageContent::Venue(_) => {
            tr(keys::ACTION_PINNED_MEDIA_LOCATION).into()
        }
        MessageContent::Sticker(sticker) => {
            if sticker.emoji.is_empty() {
                tr(keys::ACTION_PINNED_MEDIA_STICKER).into()
            } else {
                tr_with(
                    keys::ACTION_PINNED_MEDIA_EMOJI_STICKER,
                    &[("emoji", &sticker.emoji)],
                )
            }
        }
        MessageContent::Game(game) => {
            tr_with(keys::ACTION_PINNED_MEDIA_GAME, &[("game", &game.title)])
        }
        MessageContent::Action(action) if matches!(**action, ServiceAction::Story { .. }) => {
            tr(keys::ACTION_PINNED_MEDIA_STORY).into()
        }
        _ => return None,
    })
}

/// The wording of any service-like content, or `None` for content that
/// is an ordinary message.
pub fn render(content: &MessageContent, ctx: &ServiceCtx<'_>) -> Option<ServiceText> {
    match content {
        MessageContent::Action(action) => Some(render_action(action, ctx)),
        MessageContent::ChatTtlChanged { secs } => Some(render_ttl(*secs, ctx)),
        MessageContent::ScreenshotTaken => Some(if ctx.actor_is_me() {
            B::default().t(tr(keys::ACTION_YOU_TOOK_SCREENSHOT)).done()
        } else {
            phrase(keys::ACTION_TOOK_SCREENSHOT, &[("from", ctx.from())]).done()
        }),
        _ => None,
    }
}

fn render_ttl(secs: i32, ctx: &ServiceCtx<'_>) -> ServiceText {
    if secs <= 0 {
        return if ctx.is_channel {
            B::default().t(tr(keys::ACTION_TTL_REMOVED_CHANNEL)).done()
        } else if ctx.actor_is_me() {
            B::default().t(tr(keys::ACTION_TTL_REMOVED_YOU)).done()
        } else {
            phrase(keys::ACTION_TTL_REMOVED, &[("from", ctx.from())]).done()
        };
    }
    let duration = if secs == 5 {
        trn(keys::SECONDS, 5)
    } else {
        format_ttl(secs)
    };
    if ctx.is_channel {
        line(keys::ACTION_TTL_CHANGED_CHANNEL, &[("duration", &duration)])
    } else if ctx.actor_is_me() {
        line(keys::ACTION_TTL_CHANGED_YOU, &[("duration", &duration)])
    } else {
        phrase(
            keys::ACTION_TTL_CHANGED,
            &[("from", ctx.from()), ("duration", B::default().t(duration))],
        )
        .done()
    }
}

struct NoNames;

impl ServiceNames for NoNames {
    fn user_name(&self, _: i64) -> Option<String> {
        None
    }
    fn chat_name(&self, _: i64) -> Option<String> {
        None
    }
    fn pinned(&self, _: i64) -> PinTarget {
        PinTarget::NotLoaded
    }
    fn game_title(&self, _: i64) -> Option<String> {
        None
    }
}

/// Wording without any known names: the fallback of
/// [`MessageContent::preview`] for callers that have no session. Chat-list
/// rows built from a session use [`render`] with real names instead.
/// The Day / Month / Year table of a suggested-birthday card
/// (tdesktop `BirthdayTable`): the year column only shows when it is set.
pub fn birthday_table(day: i32, month: i32, year: i32) -> Vec<(&'static str, String)> {
    let mut rows = vec![
        (tr(keys::Q_PEOPLE_DAY), day.to_string()),
        (
            tr(keys::Q_PEOPLE_MONTH),
            crate::local_time::month_name_nominative(month.clamp(1, 12) as u8).to_string(),
        ),
    ];
    if year > 0 {
        rows.push((tr(keys::Q_PEOPLE_YEAR), year.to_string()));
    }
    rows
}

/// The date a suggested birthday card hands to the birthday form; `None`
/// for a date the form would reject.
pub fn birthday_form_parts(day: i32, month: i32, year: i32) -> Option<(u8, u8, Option<i32>)> {
    let valid = (1..=31).contains(&day) && (1..=12).contains(&month);
    valid.then(|| (day as u8, month as u8, (year > 0).then_some(year)))
}

pub fn action_preview(action: &ServiceAction) -> String {
    let ctx = ServiceCtx {
        actor: None,
        is_outgoing: false,
        my_id: None,
        is_channel: false,
        is_secret: false,
        chat_id: 0,
        now: 0,
        names: &NoNames,
    };
    render_action(action, &ctx).plain()
}
