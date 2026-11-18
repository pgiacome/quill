//! The words of the "About this giveaway" box (tdesktop
//! `boxes/gift_premium_box.cpp` `GiveawayInfoBox`, strings `lng_prizes_*`).
use crate::lang::format::unmarked;
use crate::lang::{keys, tr, tr_with, trn, trn_with};
use crate::telegram::envelope::{
    GiveawayInfoData, GiveawayParticipantStatus, GiveawayPrize, GiveawaySummary,
};

/// What the wording needs from the app.
pub struct GiveawayTextContext<'a> {
    /// Name of the boosted channel or group.
    pub first_channel: String,
    /// The boosted chat is a supergroup.
    pub group: bool,
    /// Name of an administered chat, by id.
    pub chat_title: &'a dyn Fn(i64) -> String,
    /// "October 6" (tdesktop `langDayOfMonthFull`).
    pub day: &'a dyn Fn(i32) -> String,
    /// "October 6 at 14:05" (tdesktop `langDateTime`).
    pub date_time: &'a dyn Fn(i32) -> String,
}

/// The box, as plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AboutText {
    pub title: &'static str,
    /// The highlighted line about the current user's prize.
    pub result: Option<String>,
    /// Whether `result` announces a win.
    pub won: bool,
    /// tdesktop marked text: `**bold**` runs (`lang::format::split_bold`).
    pub paragraphs: Vec<String>,
    /// Shown in a box of its own when the prizes were refunded.
    pub refund_notice: Option<String>,
}

/// The text for `summary` (the giveaway card) and `info` (the answer to
/// `getGiveawayInfo`).
pub fn about_text(
    summary: &GiveawaySummary,
    info: &GiveawayInfoData,
    ctx: &GiveawayTextContext<'_>,
) -> AboutText {
    let finished = info.is_finished();
    let (refunded, winner_count, activated, gift_code, won_stars) = match info {
        GiveawayInfoData::Completed {
            was_refunded,
            winner_count,
            activation_count,
            gift_code,
            won_star_count,
            ..
        } => (
            *was_refunded,
            *winner_count,
            *activation_count,
            !gift_code.0.is_empty(),
            *won_star_count,
        ),
        GiveawayInfoData::Ongoing { .. } => (false, 0, 0, false, 0),
    };
    let ch = &unmarked(&ctx.first_channel);
    let group = ctx.group;
    let many = summary.additional_chat_count > 0;
    let quantity = i64::from(summary.quantity());
    let count = if winner_count > 0 {
        i64::from(winner_count)
    } else {
        quantity
    };

    let (result, won) = if gift_code {
        (
            Some(tr_with(keys::PRIZES_YOU_WON, &[("cup", "\u{1F3C6}")])),
            true,
        )
    } else if won_stars > 0 {
        let amount = trn(keys::PRIZES_YOU_WON_CREDITS_AMOUNT, won_stars);
        (
            Some(tr_with(
                keys::PRIZES_YOU_WON_CREDITS,
                &[("amount", &amount), ("cup", "\u{1F3C6}")],
            )),
            true,
        )
    } else if matches!(info, GiveawayInfoData::Completed { .. }) && !refunded {
        (Some(tr(keys::PRIZES_YOU_DIDNT).to_string()), false)
    } else {
        (None, false)
    };

    let admins = match summary.prize {
        GiveawayPrize::Stars { star_count } => {
            let amount = trn(keys::PRIZES_CREDITS_ADMINS_AMOUNT, star_count);
            let key = if group {
                keys::PRIZES_CREDITS_ADMINS_GROUP
            } else {
                keys::PRIZES_CREDITS_ADMINS
            };
            tr_with(key, &[("channel", ch), ("amount", &amount)])
        }
        GiveawayPrize::Premium { month_count } => {
            let duration = trn(keys::PREMIUM_GIFT_DURATION_MONTHS, month_count);
            let key = if group {
                keys::PRIZES_ADMINS_GROUP
            } else {
                keys::PRIZES_ADMINS
            };
            trn_with(key, quantity, &[("channel", ch), ("duration", &duration)])
        }
    };
    let mut intro = tr_with(
        if finished {
            keys::PRIZES_END_TEXT
        } else {
            keys::PRIZES_HOW_TEXT
        },
        &[("admins", &admins)],
    );

    let winners = if summary.only_new_members {
        let start = (ctx.date_time)(match info {
            GiveawayInfoData::Ongoing { creation_date, .. }
            | GiveawayInfoData::Completed { creation_date, .. } => *creation_date,
        });
        let key = if many {
            keys::PRIZES_WINNERS_NEW_OF_MANY
        } else {
            keys::PRIZES_WINNERS_NEW_OF_ONE
        };
        trn_with(key, count, &[("channel", ch), ("start_date", &start)])
    } else if many {
        let key = if group {
            keys::PRIZES_WINNERS_ALL_OF_MANY_GROUP
        } else {
            keys::PRIZES_WINNERS_ALL_OF_MANY
        };
        trn_with(key, count, &[("channel", ch)])
    } else {
        let key = if group {
            keys::PRIZES_WINNERS_ALL_OF_ONE_GROUP
        } else {
            keys::PRIZES_WINNERS_ALL_OF_ONE
        };
        trn_with(key, count, &[("channel", ch)])
    };

    let mut paragraphs = Vec::new();
    if !summary.prize_description.is_empty() {
        let key = if group {
            keys::PRIZES_ADDITIONAL_ADDED_GROUP
        } else {
            keys::PRIZES_ADDITIONAL_ADDED
        };
        let line = trn_with(
            key,
            count,
            &[
                ("channel", ch),
                ("prize", &unmarked(&summary.prize_description)),
            ],
        );
        intro.push_str("\n\n");
        intro.push_str(&line);
    }
    let until = (ctx.day)(summary.selection_date);
    let line = tr_with(
        if finished {
            keys::PRIZES_END_WHEN_FINISH
        } else {
            keys::PRIZES_HOW_WHEN_FINISH
        },
        &[("date", &until), ("winners", &winners)],
    );
    intro.push_str("\n\n");
    intro.push_str(&line);
    if activated > 0 {
        intro.push(' ');
        intro.push_str(&trn(keys::PRIZES_END_ACTIVATED, activated));
    }
    paragraphs.push(intro);

    // An unfinished giveaway is always `Ongoing`.
    if !gift_code
        && !finished
        && !refunded
        && let GiveawayInfoData::Ongoing { status, .. } = info
    {
        {
            let line = match status {
                GiveawayParticipantStatus::Administrator { chat_id } => {
                    let name = unmarked(&(ctx.chat_title)(*chat_id));
                    tr_with(
                        if group {
                            keys::PRIZES_HOW_NO_ADMIN_GROUP
                        } else {
                            keys::PRIZES_HOW_NO_ADMIN
                        },
                        &[("channel", &name)],
                    )
                }
                GiveawayParticipantStatus::AlreadyWasMember { joined_chat_date } => {
                    let date = (ctx.date_time)(*joined_chat_date);
                    tr_with(
                        if group {
                            keys::PRIZES_HOW_NO_JOINED_GROUP
                        } else {
                            keys::PRIZES_HOW_NO_JOINED
                        },
                        &[("date", &date)],
                    )
                }
                GiveawayParticipantStatus::DisallowedCountry { .. } => {
                    tr(keys::PRIZES_HOW_NO_COUNTRY).to_string()
                }
                GiveawayParticipantStatus::Participating => tr_with(
                    if many {
                        keys::PRIZES_HOW_YES_JOINED_MANY
                    } else {
                        keys::PRIZES_HOW_YES_JOINED_ONE
                    },
                    &[("channel", ch)],
                ),
                GiveawayParticipantStatus::Eligible => tr_with(
                    if many {
                        keys::PRIZES_HOW_PARTICIPATE_MANY
                    } else {
                        keys::PRIZES_HOW_PARTICIPATE_ONE
                    },
                    &[("channel", ch), ("date", &until)],
                ),
            };
            paragraphs.push(line);
        }
    }

    AboutText {
        title: tr(if finished {
            keys::PRIZES_END_TITLE
        } else {
            keys::PRIZES_HOW_TITLE
        }),
        result,
        won,
        paragraphs,
        refund_notice: refunded.then(|| {
            tr(if group {
                keys::PRIZES_CANCELLED_GROUP
            } else {
                keys::PRIZES_CANCELLED
            })
            .to_string()
        }),
    }
}
