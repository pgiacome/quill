//! The title, detail and peer of a Stars transaction row, following the
//! wording of Telegram Desktop's credits history rows.

use super::{group_digits, int, sender_of, text_of};
use crate::lang::{Key, keys, tr, trn, trn_decimal};
use crate::telegram::envelope::MessageSender;
use serde_json::Value;

/// Which id of the transaction type names the row's peer.
#[derive(Clone, Copy)]
enum Party {
    None,
    User,
    Chat,
    Owner,
}

impl Party {
    fn of(self, ty: &Value) -> Option<MessageSender> {
        match self {
            Self::None => None,
            Self::User => user(int(ty.get("user_id"))),
            Self::Chat => chat(int(ty.get("chat_id"))),
            Self::Owner => sender_of(ty.get("owner_id")),
        }
    }
}

/// What the row's detail line shows.
#[derive(Clone, Copy)]
enum Detail {
    None,
    Fixed(Key),
    Gift,
    Period,
}

/// Rows whose title is a fixed key. The rest are computed in
/// [`describe_tx_type`].
const FIXED: &[(&str, Key, Detail, Party)] = &[
    (
        "starTransactionTypePremiumBotDeposit",
        keys::Q_MONEY_TX_STARS_TOP_UP,
        Detail::Fixed(keys::Q_MONEY_TX_VIA_PREMIUM_BOT),
        Party::None,
    ),
    (
        "starTransactionTypeAppStoreDeposit",
        keys::Q_MONEY_TX_STARS_TOP_UP,
        Detail::Fixed(keys::Q_MONEY_TX_VIA_APP_STORE),
        Party::None,
    ),
    (
        "starTransactionTypeGooglePlayDeposit",
        keys::Q_MONEY_TX_STARS_TOP_UP,
        Detail::Fixed(keys::Q_MONEY_TX_VIA_GOOGLE_PLAY),
        Party::None,
    ),
    (
        "starTransactionTypeFragmentDeposit",
        keys::Q_MONEY_TX_STARS_TOP_UP,
        Detail::Fixed(keys::Q_MONEY_TX_VIA_FRAGMENT),
        Party::None,
    ),
    (
        "starTransactionTypeUserDeposit",
        keys::Q_MONEY_TX_GIFTED_STARS,
        Detail::None,
        Party::User,
    ),
    (
        "starTransactionTypeGiveawayDeposit",
        keys::Q_MONEY_TX_GIVEAWAY_PRIZE,
        Detail::None,
        Party::Chat,
    ),
    (
        "starTransactionTypeFragmentWithdrawal",
        keys::Q_MONEY_TX_WITHDRAWAL,
        Detail::Fixed(keys::Q_MONEY_TX_VIA_FRAGMENT),
        Party::None,
    ),
    (
        "starTransactionTypeTelegramAdsWithdrawal",
        keys::Q_MONEY_TX_WITHDRAWAL,
        Detail::Fixed(keys::Q_MONEY_TX_TO_TELEGRAM_ADS),
        Party::None,
    ),
    (
        "starTransactionTypeBotPaidMediaPurchase",
        keys::Q_MONEY_TX_PAID_MEDIA,
        Detail::None,
        Party::User,
    ),
    (
        "starTransactionTypeBotPaidMediaSale",
        keys::Q_MONEY_TX_PAID_MEDIA_SALE,
        Detail::None,
        Party::User,
    ),
    (
        "starTransactionTypeChannelPaidMediaPurchase",
        keys::Q_MONEY_TX_PAID_MEDIA,
        Detail::None,
        Party::Chat,
    ),
    (
        "starTransactionTypeChannelPaidMediaSale",
        keys::Q_MONEY_TX_PAID_MEDIA_SALE,
        Detail::None,
        Party::User,
    ),
    (
        "starTransactionTypeBotSubscriptionSale",
        keys::Q_MONEY_TX_SUBSCRIPTION_SALE,
        Detail::Period,
        Party::User,
    ),
    (
        "starTransactionTypeChannelSubscriptionPurchase",
        keys::Q_MONEY_TX_CHANNEL_SUBSCRIPTION,
        Detail::Period,
        Party::Chat,
    ),
    (
        "starTransactionTypeChannelSubscriptionSale",
        keys::Q_MONEY_TX_SUBSCRIPTION_SALE,
        Detail::Period,
        Party::User,
    ),
    (
        "starTransactionTypeGiftAuctionBid",
        keys::Q_MONEY_TX_GIFT_AUCTION_BID,
        Detail::Gift,
        Party::Owner,
    ),
    (
        "starTransactionTypeGiftPurchase",
        keys::Q_MONEY_TX_GIFT,
        Detail::Gift,
        Party::Owner,
    ),
    (
        "starTransactionTypeGiftPurchaseOffer",
        keys::Q_MONEY_TX_GIFT_OFFER,
        Detail::Gift,
        Party::None,
    ),
    (
        "starTransactionTypeGiftTransfer",
        keys::Q_MONEY_TX_GIFT_TRANSFER,
        Detail::Gift,
        Party::Owner,
    ),
    (
        "starTransactionTypeGiftOriginalDetailsDrop",
        keys::Q_MONEY_TX_ORIGINAL_DETAILS_REMOVED,
        Detail::Gift,
        Party::Owner,
    ),
    (
        "starTransactionTypeGiftSale",
        keys::Q_MONEY_TX_GIFT_CONVERTED,
        Detail::Gift,
        Party::User,
    ),
    (
        "starTransactionTypeGiftUpgrade",
        keys::Q_MONEY_TX_GIFT_UPGRADE,
        Detail::Gift,
        Party::User,
    ),
    (
        "starTransactionTypeGiftUpgradePurchase",
        keys::Q_MONEY_TX_GIFT_UPGRADE,
        Detail::Gift,
        Party::Owner,
    ),
    (
        "starTransactionTypeUpgradedGiftPurchase",
        keys::Q_MONEY_TX_COLLECTIBLE_GIFT,
        Detail::Gift,
        Party::User,
    ),
    (
        "starTransactionTypeUpgradedGiftSale",
        keys::Q_MONEY_TX_COLLECTIBLE_GIFT_SALE,
        Detail::Gift,
        Party::User,
    ),
    (
        "starTransactionTypeChannelPaidReactionSend",
        keys::Q_MONEY_TX_REACTION,
        Detail::None,
        Party::Chat,
    ),
    (
        "starTransactionTypeChannelPaidReactionReceive",
        keys::Q_MONEY_TX_REACTION_RECEIVED,
        Detail::None,
        Party::User,
    ),
    (
        "starTransactionTypeAffiliateProgramCommission",
        keys::Q_MONEY_TX_AFFILIATE_COMMISSION,
        Detail::None,
        Party::Chat,
    ),
    (
        "starTransactionTypePaidGroupCallMessageSend",
        keys::Q_MONEY_TX_LIVE_STREAM_MESSAGE,
        Detail::None,
        Party::Chat,
    ),
    (
        "starTransactionTypePaidGroupCallReactionSend",
        keys::Q_MONEY_TX_LIVE_STREAM_MESSAGE,
        Detail::None,
        Party::Chat,
    ),
    (
        "starTransactionTypeSuggestedPostPaymentSend",
        keys::Q_MONEY_TX_SUGGESTED_POST,
        Detail::None,
        Party::Chat,
    ),
    (
        "starTransactionTypeSuggestedPostPaymentReceive",
        keys::Q_MONEY_TX_SUGGESTED_POST,
        Detail::None,
        Party::User,
    ),
    (
        "starTransactionTypeBusinessBotTransferSend",
        keys::Q_MONEY_TX_BUSINESS_BOT_TRANSFER,
        Detail::None,
        Party::User,
    ),
    (
        "starTransactionTypeBusinessBotTransferReceive",
        keys::Q_MONEY_TX_BUSINESS_BOT_TRANSFER_RECEIVED,
        Detail::None,
        Party::User,
    ),
    (
        "starTransactionTypePublicPostSearch",
        keys::Q_MONEY_TX_PUBLIC_POST_SEARCH,
        Detail::None,
        Party::None,
    ),
];

fn user(id: i64) -> Option<MessageSender> {
    (id != 0).then_some(MessageSender::User { user_id: id })
}

fn chat(id: i64) -> Option<MessageSender> {
    (id != 0).then_some(MessageSender::Chat { chat_id: id })
}

fn months(count: i64) -> String {
    match count {
        n if n > 0 && n % 12 == 0 => trn(keys::YEARS, n / 12),
        n => trn(keys::MONTHS, n.max(1)),
    }
}

fn period_label(seconds: i64) -> String {
    match seconds {
        2_592_000 => tr(keys::Q_MONEY_TX_MONTHLY).to_string(),
        s if s > 0 && s % 86_400 == 0 => trn(keys::Q_MONEY_TX_EVERY_DAYS, s / 86_400),
        _ => String::new(),
    }
}

/// The bought product's title, or `fallback` when the bot gave none.
fn product_title(value: &Value, fallback: Key) -> String {
    let title = value
        .get("product_info")
        .map(|info| text_of(info, "title"))
        .unwrap_or_default();
    if title.is_empty() {
        tr(fallback).to_string()
    } else {
        title
    }
}

fn gift_name(value: &Value) -> String {
    let Some(gift) = value.get("gift") else {
        return String::new();
    };
    if gift.get("@type").and_then(Value::as_str) == Some("upgradedGift") {
        let title = text_of(gift, "title");
        let number = int(gift.get("number"));
        return if number > 0 {
            format!("{title} #{}", group_digits(number))
        } else {
            title
        };
    }
    let stars = int(gift.get("star_count"));
    if stars > 0 {
        trn_decimal(keys::Q_MONEY_TX_STARS, stars)
    } else {
        String::new()
    }
}

fn messages(ty: &Value) -> String {
    trn_decimal(
        keys::Q_MONEY_TX_MESSAGES,
        int(ty.get("message_count")).max(1),
    )
}

/// Map a `StarTransactionType` to (title, detail, peer), following the
/// wording of Telegram Desktop's credits history rows.
pub fn describe_tx_type(ty: &Value) -> (String, String, Option<MessageSender>) {
    let kind = ty.get("@type").and_then(Value::as_str).unwrap_or("");
    if let Some((_, title, detail, party)) = FIXED.iter().find(|(name, ..)| *name == kind) {
        let detail = match detail {
            Detail::None => String::new(),
            Detail::Fixed(key) => tr(*key).to_string(),
            Detail::Gift => gift_name(ty),
            Detail::Period => period_label(int(ty.get("subscription_period"))),
        };
        return (tr(*title).to_string(), detail, party.of(ty));
    }
    let buyer = || user(int(ty.get("user_id")));
    match kind {
        "starTransactionTypeTelegramApiUsage" => (
            tr(keys::Q_MONEY_TX_PAID_API_USAGE).to_string(),
            trn_decimal(keys::Q_MONEY_TX_REQUESTS, int(ty.get("request_count"))),
            None,
        ),
        "starTransactionTypeBotInvoicePurchase" => (
            product_title(ty, keys::Q_MONEY_TX_PURCHASE),
            String::new(),
            buyer(),
        ),
        "starTransactionTypeBotInvoiceSale" => (
            product_title(ty, keys::Q_MONEY_TX_SALE),
            tr(keys::Q_MONEY_TX_SALE).to_string(),
            buyer(),
        ),
        "starTransactionTypeBotSubscriptionPurchase" => (
            product_title(ty, keys::Q_MONEY_TX_SUBSCRIPTION),
            period_label(int(ty.get("subscription_period"))),
            buyer(),
        ),
        "starTransactionTypePaidMessageSend" => (
            tr(keys::Q_MONEY_TX_PAID_MESSAGE).to_string(),
            messages(ty),
            chat(int(ty.get("chat_id"))),
        ),
        "starTransactionTypePaidMessageReceive" => (
            tr(keys::Q_MONEY_TX_PAID_MESSAGES_RECEIVED).to_string(),
            messages(ty),
            sender_of(ty.get("sender_id")),
        ),
        "starTransactionTypePaidGroupCallMessageReceive"
        | "starTransactionTypePaidGroupCallReactionReceive" => (
            tr(keys::Q_MONEY_TX_LIVE_STREAM_MESSAGE_RECEIVED).to_string(),
            String::new(),
            sender_of(ty.get("sender_id")),
        ),
        "starTransactionTypePremiumPurchase" => (
            tr(keys::Q_MONEY_TX_TELEGRAM_PREMIUM).to_string(),
            months(int(ty.get("month_count"))),
            buyer(),
        ),
        _ => (
            tr(keys::Q_MONEY_TX_TRANSACTION).to_string(),
            String::new(),
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::describe_tx_type;
    use serde_json::json;

    #[test]
    fn counts_of_a_thousand_or_more_are_grouped() {
        let gift = json!({
            "@type": "starTransactionTypeGiftPurchase",
            "gift": {"@type": "gift", "star_count": 12345},
        });
        assert_eq!(describe_tx_type(&gift).1, "12,345 Stars");
        let api = json!({"@type": "starTransactionTypeTelegramApiUsage", "request_count": 1500});
        assert_eq!(describe_tx_type(&api).1, "1,500 requests");
    }

    #[test]
    fn a_fixed_row_names_its_peer() {
        let deposit = json!({"@type": "starTransactionTypeUserDeposit", "user_id": 7});
        let (title, detail, peer) = describe_tx_type(&deposit);
        assert_eq!(title, "Gifted Stars");
        assert!(detail.is_empty());
        assert!(peer.is_some());
    }
}
