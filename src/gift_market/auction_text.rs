//! The wording of the auction boxes and the chat-list bar (tdesktop
//! `boxes/star_gift_auction_box.cpp`, `lng_auction_*`).

use super::{AuctionPhase, AuctionRound, AuctionState};
use crate::lang::{keys, tr, tr_with, trn};
use crate::premium_hub::group_digits;

/// Seconds from `now` until `date`, never negative.
pub fn seconds_left(date: i32, now: i64) -> i64 {
    (i64::from(date) - now).max(0)
}

/// Whole minutes left, rounded up (tdesktop `MinutesLeftTillValue`).
pub fn minutes_left(date: i32, now: i64) -> i64 {
    (seconds_left(date, now) + 59) / 60
}

/// tdesktop `NiceCountdownText`: "1:05:09" or "5:09".
pub fn nice_countdown(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let minutes = seconds / 60;
    let hours = minutes / 60;
    if hours > 0 {
        format!("{hours}:{:02}:{:02}", minutes % 60, seconds % 60)
    } else {
        format!("{minutes}:{:02}", seconds % 60)
    }
}

/// "★ 1,250" for the minimum-bid block; 1M and up in short form
/// (tdesktop `FormatCountToShort`).
pub fn short_stars(count: i64) -> String {
    if count >= 1_000_000 {
        let tenths = count / 100_000;
        if tenths % 10 == 0 || count >= 10_000_000 {
            format!("{}M", count / 1_000_000)
        } else {
            format!("{}.{}M", tenths / 10, tenths % 10)
        }
    } else {
        group_digits(count)
    }
}

/// The two lines of the info box's button (tdesktop
/// `SetAuctionButtonCountdownText`); `place` is the send box's variant.
pub fn join_button(state: &AuctionState, now: i64, place: bool) -> (String, String) {
    let preview = !state.started(now);
    let left = if preview {
        seconds_left(state.start_date(), now)
    } else {
        minutes_left(state.end_date(), now)
    };
    if state.finished(now) || (!preview && left <= 0) {
        return (tr(keys::Q_MONEY_AUCTION_OK).into(), String::new());
    }
    if preview {
        return (
            tr(keys::Q_MONEY_AUCTION_PLACE_EARLY_BID).into(),
            tr_with(
                keys::Q_MONEY_AUCTION_STARTS_IN,
                &[("time", &nice_countdown(left))],
            ),
        );
    }
    let title = if place {
        tr(keys::Q_MONEY_AUCTION_PLACE_BID)
    } else {
        tr(keys::Q_MONEY_AUCTION_JOIN_AUCTION)
    };
    let (hours, minutes) = (left / 60, left % 60);
    let time = if hours > 0 {
        tr_with(
            keys::Q_MONEY_AUCTION_TIME_HM,
            &[
                ("hours", &hours.to_string()),
                ("minutes", &minutes.to_string()),
            ],
        )
    } else {
        tr_with(
            keys::Q_MONEY_AUCTION_TIME_M,
            &[("minutes", &minutes.to_string())],
        )
    };
    (
        title.into(),
        tr_with(keys::Q_MONEY_AUCTION_TIME_LEFT, &[("time", &time)]),
    )
}

/// The bid box title (`lng_auction_bid_title` / `_early`).
pub fn bid_title(started: bool) -> &'static str {
    if started {
        tr(keys::Q_MONEY_AUCTION_PLACE_BID)
    } else {
        tr(keys::Q_MONEY_AUCTION_PLACE_EARLY_BID)
    }
}

/// "Top 5 bidders will win" (`lng_auction_bid_subtitle`).
pub fn bid_subtitle(per_round: i32) -> String {
    trn(keys::Q_MONEY_AUCTION_WILL_WIN, per_round)
}

/// The toast after a bid: title and text.
pub fn bid_done_toast(increased: bool, per_round: i32) -> (&'static str, String) {
    (
        if increased {
            tr(keys::Q_MONEY_AUCTION_BID_INCREASED)
        } else {
            tr(keys::Q_MONEY_AUCTION_BID_PLACED)
        },
        tr_with(
            keys::Q_MONEY_AUCTION_ROLLOVER,
            &[("count", &group_digits(i64::from(per_round)))],
        ),
    )
}

/// The rows of the auction info table: label and value.
pub fn info_rows(
    state: &AuctionState,
    now: i64,
    date: impl Fn(i32) -> String,
) -> Vec<(String, String)> {
    let preview = !state.started(now);
    let finished = state.finished(now);
    let mut rows = vec![
        (
            if preview {
                tr(keys::Q_MONEY_AUCTION_STARTS)
            } else if finished {
                tr(keys::Q_MONEY_AUCTION_FIRST_SALE)
            } else {
                tr(keys::Q_MONEY_AUCTION_STARTED)
            }
            .to_string(),
            date(state.start_date()),
        ),
        (
            if finished {
                tr(keys::Q_MONEY_AUCTION_LAST_SALE)
            } else {
                tr(keys::Q_MONEY_AUCTION_ENDS)
            }
            .to_string(),
            date(state.end_date()),
        ),
    ];
    match &state.phase {
        AuctionPhase::Active(active) if preview => {
            rows.push((
                tr(keys::Q_MONEY_AUCTION_QUANTITY).into(),
                group_digits(i64::from(state.total())),
            ));
            rows.push((
                tr(keys::Q_MONEY_AUCTION_ROUNDS).into(),
                group_digits(i64::from(active.total_rounds)),
            ));
            for (index, round) in active.rounds.iter().enumerate() {
                let next = active
                    .rounds
                    .get(index + 1)
                    .map_or(active.total_rounds.saturating_add(1), |next| next.number);
                rows.push(round_row(round, next));
            }
        }
        AuctionPhase::Active(active) => {
            rows.push((
                tr(keys::Q_MONEY_AUCTION_CURRENT_ROUND).into(),
                tr_with(
                    keys::Q_MONEY_AUCTION_OF,
                    &[
                        ("have", &group_digits(i64::from(active.round))),
                        ("total", &group_digits(i64::from(active.total_rounds))),
                    ],
                ),
            ));
            rows.push((
                tr(keys::Q_MONEY_AUCTION_AVAILABILITY).into(),
                tr_with(
                    keys::Q_MONEY_AUCTION_OF_LEFT,
                    &[
                        ("have", &group_digits(i64::from(active.left))),
                        ("total", &group_digits(i64::from(state.total()))),
                    ],
                ),
            ));
        }
        AuctionPhase::Finished(finished) => {
            if finished.average_price > 0 {
                rows.push((
                    tr(keys::Q_MONEY_AUCTION_AVERAGE_PRICE).into(),
                    format!("★{}", group_digits(finished.average_price)),
                ));
            }
        }
    }
    rows
}

/// "Round 1" / "Rounds 2-5" and "10 minutes each" (+ late-bid extension).
fn round_row(round: &AuctionRound, next_number: i32) -> (String, String) {
    let exact = next_number == round.number.saturating_add(1);
    let label = if exact {
        tr_with(
            keys::Q_MONEY_AUCTION_ROUND,
            &[("number", &round.number.to_string())],
        )
    } else {
        tr_with(
            keys::Q_MONEY_AUCTION_ROUNDS_RANGE,
            &[
                ("from", &round.number.to_string()),
                ("to", &next_number.saturating_sub(1).to_string()),
            ],
        )
    };
    let duration = duration_text(round.duration, !exact);
    let value = if round.top_winner_count > 0 && round.extend_time > 0 {
        tr_with(
            keys::Q_MONEY_AUCTION_LATE_BIDS,
            &[
                ("duration", &duration),
                ("extend", &duration_text(round.extend_time, false)),
                ("count", &round.top_winner_count.to_string()),
            ],
        )
    } else {
        duration
    };
    (label, value)
}

/// "10 minutes", or "10 minutes each" for a range of rounds.
fn duration_text(seconds: i32, each: bool) -> String {
    let text = if seconds % 3600 == 0 {
        trn(keys::HOURS, seconds / 3600)
    } else if seconds % 60 == 0 {
        trn(keys::MINUTES, seconds / 60)
    } else {
        trn(keys::SECONDS, seconds)
    };
    if each {
        format!("{text}{}", tr(keys::Q_MONEY_AUCTION_EACH))
    } else {
        text
    }
}

/// "{amount} is the average sale price for {gift} gifts."
pub fn average_tooltip(average: i64, title: &str) -> String {
    tr_with(
        keys::Q_MONEY_AUCTION_AVERAGE_TOOLTIP,
        &[("average", &group_digits(average)), ("title", title)],
    )
}

/// "3 items bought" (`lng_auction_bought`), without the emoji and arrow.
pub fn bought_link(count: i32) -> String {
    trn(keys::Q_MONEY_AUCTION_BOUGHT, count)
}

/// "3 Items Bought" (`lng_auction_bought_title`).
pub fn bought_title(count: i32) -> String {
    trn(keys::Q_MONEY_AUCTION_BOUGHT_TITLE, count)
}

/// "View 12 Variants" (`lng_auction_preview_variants`).
pub fn variants_link(count: i32) -> String {
    tr_with(
        keys::Q_MONEY_AUCTION_VIEW_COUNT,
        &[("variants", &trn(keys::Q_MONEY_AUCTION_VARIANT, count))],
    )
}

/// The about box's three feature rows: title and text.
pub fn about_features(per_round: i32, rounds: i32) -> [(String, String); 3] {
    let per = i64::from(per_round);
    let gifts = trn(keys::Q_MONEY_AUCTION_GIFT_IS, per);
    let round_count = trn(keys::Q_MONEY_AUCTION_ROUND_COUNT, i64::from(rounds));
    let bidders = trn(keys::Q_MONEY_AUCTION_TOP_BIDDERS_LOWER, per);
    [
        (
            trn(keys::Q_MONEY_AUCTION_TOP_BIDDERS_TITLE, per),
            tr_with(
                keys::Q_MONEY_AUCTION_DROPPED,
                &[
                    ("gifts", &gifts),
                    ("rounds", &round_count),
                    ("bidders", &bidders),
                ],
            ),
        ),
        (
            tr(keys::Q_MONEY_AUCTION_CARRYOVER_TITLE).into(),
            tr_with(
                keys::Q_MONEY_AUCTION_CARRYOVER_TEXT,
                &[("count", &group_digits(per))],
            ),
        ),
        (
            tr(keys::Q_MONEY_AUCTION_MISSED_TITLE).into(),
            tr(keys::Q_MONEY_AUCTION_MISSED_TEXT).into(),
        ),
    ]
}

/// The send box's about text for an auction gift
/// (`lng_auction_about_top_short`), without the "Learn more" link.
pub fn send_about(per_round: i32) -> String {
    let per = i64::from(per_round);
    tr_with(
        keys::Q_MONEY_AUCTION_SEND_ABOUT,
        &[
            ("gifts", &trn(keys::Q_MONEY_AUCTION_GIFT_IS, per)),
            (
                "bidders",
                &trn(keys::Q_MONEY_AUCTION_TOP_BIDDERS_LOWER, per),
            ),
        ],
    )
}

/// The chat-list bar: title, text, whether the text is red, and the
/// button (countdown for one auction, "View" for several).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionBar {
    pub title: String,
    pub text: String,
    pub outbid: bool,
    pub button: String,
}

/// tdesktop `ActiveAuctionsTitle` / `ActiveAuctionsState` /
/// `ActiveAuctionsButton`; `None` without active auctions.
pub fn auction_bar(list: &[AuctionState], now: i64) -> Option<AuctionBar> {
    let first = list.first()?;
    let winning = |state: &AuctionState| {
        let position = state.my_position();
        (position <= state.per_round()).then_some(position)
    };
    if list.len() == 1 {
        let (text, outbid) = match winning(first) {
            Some(1) => (tr(keys::Q_MONEY_AUCTION_WINNING_FIRST).to_string(), false),
            Some(n) => (
                tr_with(
                    keys::Q_MONEY_AUCTION_WINNING_PLACE,
                    &[("place", &n.to_string())],
                ),
                false,
            ),
            None => (tr(keys::Q_MONEY_AUCTION_OUTBID).to_string(), true),
        };
        return Some(AuctionBar {
            title: tr(keys::Q_MONEY_AUCTION_ACTIVE).into(),
            text,
            outbid,
            button: nice_countdown(seconds_left(first.next_round_at(), now)),
        });
    }
    let outbid = list.iter().filter(|state| winning(state).is_none()).count();
    let text = if outbid == list.len() {
        tr(keys::Q_MONEY_AUCTION_OUTBID_ALL).to_string()
    } else if outbid > 0 {
        tr_with(
            keys::Q_MONEY_AUCTION_OUTBID_SOME,
            &[("count", &outbid.to_string())],
        )
    } else {
        tr(keys::Q_MONEY_AUCTION_WINNING_ALL).to_string()
    };
    Some(AuctionBar {
        title: trn(keys::Q_MONEY_AUCTION_ACTIVE_TITLE, list.len() as i64),
        text,
        outbid: outbid > 0,
        button: tr(keys::Q_MONEY_AUCTION_VIEW).into(),
    })
}

/// One row of the "Active Auctions" list: "Round 2 of 10" and "Your bid
/// ★500 is ranked #3" / "Your bid ★500 is outbid".
pub fn active_row(state: &AuctionState) -> (String, String, bool) {
    let (round, total) = state
        .active()
        .map_or((0, 0), |active| (active.round, active.total_rounds));
    let position = state.my_position();
    let outbid = position > state.per_round();
    let stars = format!("★{}", group_digits(state.my_stars()));
    let text = if outbid {
        tr_with(keys::Q_MONEY_AUCTION_MY_BID_OUTBID, &[("stars", &stars)])
    } else {
        tr_with(
            keys::Q_MONEY_AUCTION_MY_BID_RANKED,
            &[("stars", &stars), ("position", &position.to_string())],
        )
    };
    (
        tr_with(
            keys::Q_MONEY_AUCTION_ROUND_OF,
            &[("round", &round.to_string()), ("total", &total.to_string())],
        ),
        text,
        outbid,
    )
}

/// The tile's badge and price line for an auction gift (tdesktop
/// `lng_gift_stars_auction*`): badge "auction" or "soon", line "Join" or
/// "View".
pub fn auction_tile(start_date: i32, sold_out: bool, now: i64) -> (&'static str, &'static str) {
    let upcoming = i64::from(start_date) > now;
    let badge = if upcoming {
        tr(keys::Q_MONEY_AUCTION_BADGE_SOON)
    } else {
        tr(keys::Q_MONEY_AUCTION_BADGE_AUCTION)
    };
    let line = if sold_out || upcoming {
        tr(keys::Q_MONEY_AUCTION_VIEW)
    } else {
        tr(keys::Q_MONEY_AUCTION_JOIN)
    };
    (badge, line)
}

/// The link preview button (tdesktop `history_view_web_page.cpp`).
pub fn preview_button(start_date: i32, end_date: i32, now: i64) -> &'static str {
    if i64::from(end_date) <= now {
        tr(keys::Q_MONEY_AUCTION_VIEW_RESULTS)
    } else if i64::from(start_date) > now {
        tr(keys::Q_MONEY_AUCTION_VIEW)
    } else {
        tr(keys::Q_MONEY_AUCTION_JOIN)
    }
}

/// The "Change Recipient" text.
pub fn change_recipient_text(from: Option<&str>, to: Option<&str>) -> String {
    let already = match from {
        None => tr(keys::Q_MONEY_AUCTION_ALREADY_BID_SELF).to_string(),
        Some(name) => tr_with(keys::Q_MONEY_AUCTION_ALREADY_BID_FOR, &[("name", name)]),
    };
    let change = match to {
        None => tr(keys::Q_MONEY_AUCTION_CHANGE_TO_SELF).to_string(),
        Some(name) => tr_with(keys::Q_MONEY_AUCTION_CHANGE_TO, &[("name", name)]),
    };
    format!("{already} {change}")
}
