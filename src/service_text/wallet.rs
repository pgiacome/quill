//! Service wording of the TON wallet messages, following the official iOS
//! app (`Notification.GramTransfer`, `Notification.GramTransferYou`,
//! `Notification.GramTransferUnknown`) in the keys Telegram Desktop uses
//! for Gram transfers (`lng_action_gram_transfer_*`).
use super::*;
use crate::ton_wallet::{SERVICE_NOTIFICATIONS_CHAT_ID, TonWalletTransfer, format_gram_amount};

/// "Dana sent you 5 Grams", "You sent Dana 5 Grams", and in the service
/// chat "Someone sent you 5 Grams". The name links to the profile.
pub(super) fn transfer_line(transfer: &TonWalletTransfer, ctx: &ServiceCtx<'_>) -> ServiceText {
    let amount = B::from(gram_amount(transfer.amount));
    if transfer.is_received() {
        if ctx.chat_id == SERVICE_NOTIFICATIONS_CHAT_ID {
            return phrase(
                keys::ACTION_GRAM_TRANSFER_RECEIVED,
                &[("user", B::from(someone())), ("amount", amount)],
            )
            .done();
        }
        // The sender by short name; else, as the iOS app, the chat's
        // other side.
        let sender = match (transfer.sender_user_id, ctx.actor) {
            (0, Some(MessageSender::User { user_id })) => user_id,
            (id, _) => id,
        };
        let mut peer = if sender != 0 && ctx.names.user_short_name(sender).is_some() {
            ctx.user_short(sender)
        } else {
            conversation_peer(ctx)
        };
        if peer.0.is_empty() {
            peer = B::from(someone());
        }
        phrase(
            keys::ACTION_GRAM_TRANSFER_RECEIVED,
            &[("user", peer), ("amount", amount)],
        )
        .done()
    } else {
        let peer = conversation_peer(ctx);
        if peer.0.is_empty() {
            phrase(
                keys::ACTION_GRAM_TRANSFER_SENT_UNKNOWN,
                &[("amount", amount)],
            )
            .done()
        } else {
            phrase(
                keys::ACTION_GRAM_TRANSFER_SENT,
                &[("user", peer), ("amount", amount)],
            )
            .done()
        }
    }
}

/// The other side of the chat the transfer message is in (a private
/// chat's id is the user's id), by its short name; empty when unknown.
fn conversation_peer(ctx: &ServiceCtx<'_>) -> B {
    let user = ctx
        .names
        .user_short_name(ctx.chat_id)
        .filter(|name| !name.trim().is_empty())
        .map(|name| {
            (
                name,
                MessageSender::User {
                    user_id: ctx.chat_id,
                },
            )
        });
    let peer = user.or_else(|| {
        ctx.names
            .chat_name(ctx.chat_id)
            .filter(|name| !name.trim().is_empty())
            .map(|name| {
                (
                    name,
                    MessageSender::Chat {
                        chat_id: ctx.chat_id,
                    },
                )
            })
    });
    match peer {
        Some((name, sender)) => B::default().link(name, ServiceLink::Sender(sender)),
        None => B::default(),
    }
}

/// "1 Gram" / "1,000 Grams" / "2.5 Grams" for an amount in nanograms.
fn gram_amount(nanos: i64) -> String {
    let text = format_gram_amount(nanos);
    let whole = i64::try_from(nanos.unsigned_abs() / 1_000_000_000).unwrap_or(i64::MAX);
    // The formatter writes a decimal point only when it shows a fraction.
    let fractional = text.contains('.');
    let shown = Shown::new(text, whole, fractional);
    trn_with(
        keys::ACTION_GRAM_TRANSFER_AMOUNT,
        shown.form,
        &[("amount", &shown.text)],
    )
}

#[cfg(test)]
mod tests {
    use super::gram_amount;
    use crate::lang::{PackValue, Snapshot, install, isolate_thread};

    /// The plural form follows the amount, not a parse of "1,000".
    #[test]
    fn gram_amounts_take_the_form_of_their_number() {
        isolate_thread();
        let mut pack = Snapshot::new("ru", "ru", false);
        let forms = PackValue::Pluralized([
            String::new(),
            "{amount} грам".into(),
            String::new(),
            "{amount} грама".into(),
            "{amount} грамов".into(),
            "{amount} грама".into(),
        ]);
        pack.layer([("lng_action_gram_transfer_amount", &forms)]);
        install(Some(pack));
        assert_eq!(gram_amount(1_000_000_000), "1 грам");
        assert_eq!(gram_amount(1_000 * 1_000_000_000), "1,000 грамов");
        assert_eq!(gram_amount(2_500_000_000), "2.5 грама");
        install(None);
    }
}
