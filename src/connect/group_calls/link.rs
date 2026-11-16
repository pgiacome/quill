//! Connect driver: offering the call a voice chat, video chat or live
//! stream link points at (`crate::call_link`).
use super::*;
use crate::call_link::{
    CallLinkConfirm, CallLinkLeaving, CallLinkStage, CallLinkStep, CurrentCall, LeaveKind,
    LeavingCall, OfferMode, OfferedCallLink, PendingCallLink, plan,
};
use crate::ids::ChatId;

impl<S: JsonSender> ConnectDriver<S> {
    /// A call link opened `chat_id`: decide what it does and, when the chat
    /// has a call, fetch it so the join screen shows. The link's hash is kept
    /// for the join. Nothing is joined here: the Join button on the call
    /// screen is tdesktop's `JoinConfirm::Always` box. While another call is
    /// on, the question to leave it is raised instead (`confirmLeaveCurrent`).
    pub fn offer_call_link(
        &mut self,
        chat_id: ChatId,
        invite_hash: &str,
        live_stream: bool,
    ) -> Result<CallLinkStep, ConnectSendError> {
        self.offer_call(
            chat_id,
            Some(invite_hash),
            live_stream,
            OfferMode::OpenScreen,
            None,
        )
    }

    /// The Calls tab's Join on a listed call of `chat_id`: the same offer as
    /// a call link, with no hash, joining the call once it is tracked. While
    /// another call is on (joined, reconnecting, its join in flight, or
    /// `joining`, the UI's own join), the question to leave it is raised and
    /// the join waits in it, so a declined question leaves no join behind.
    pub fn join_listed_call(
        &mut self,
        chat_id: ChatId,
        live_stream: bool,
        joining: Option<i32>,
    ) -> Result<CallLinkStep, ConnectSendError> {
        self.offer_call(chat_id, None, live_stream, OfferMode::Join, joining)
    }

    fn offer_call(
        &mut self,
        chat_id: ChatId,
        invite_hash: Option<&str>,
        live_stream: bool,
        mode: OfferMode,
        joining: Option<i32>,
    ) -> Result<CallLinkStep, ConnectSendError> {
        if !self.chats_path_active() {
            return Err(ConnectSendError::InvalidRequest);
        }
        let Some(chat) = self.session.chats.get(&chat_id.0) else {
            return Ok(CallLinkStep::JustOpen);
        };
        let target_is_channel = chat.kind.is_channel();
        // Joined, reconnecting after a network loss, or with its join in
        // flight: all are in a call, so a different one must ask first.
        let in_call = self.session.group_call_joined_or_joining(joining);
        let current = self
            .session
            .calls
            .active_group_call
            .as_ref()
            .map(|call| CurrentCall {
                group_call_id: call.id,
                in_call,
            });
        let private_call = self.session.calls.active_call.is_some();
        let step = plan(
            &chat.kind,
            chat.video_chat.as_ref().map(|chat| chat.group_call_id),
            current,
            private_call,
        );
        match step {
            CallLinkStep::Offer { group_call_id } => {
                // The hash is kept only once the fetch went out; a failed
                // send leaves no hash behind for a later join of that call.
                self.fetch_group_call(group_call_id)?;
                self.session.calls.call_link_stage = CallLinkStage::Joinable(PendingCallLink {
                    group_call_id,
                    invite_hash: invite_hash.unwrap_or_default().to_string(),
                });
            }
            CallLinkStep::OtherCallOn => {
                let leave = if private_call {
                    LeaveKind::PrivateCall
                } else {
                    let current_is_channel = current
                        .and_then(|call| self.group_call_chat_id(call.group_call_id))
                        .and_then(|id| self.session.chats.get(&id))
                        .is_some_and(|chat| chat.kind.is_channel());
                    LeaveKind::GroupCall { current_is_channel }
                };
                self.session.calls.call_link_stage = CallLinkStage::Confirm(CallLinkConfirm {
                    chat_id: chat_id.0,
                    invite_hash: invite_hash.unwrap_or_default().to_string(),
                    live_stream,
                    target_is_channel,
                    leave,
                    mode,
                });
            }
            _ => {}
        }
        Ok(step)
    }

    /// The viewer confirmed: leave the current call, then offer the link's
    /// call once the old one is gone. A leave that cannot be sent drops the
    /// link: the call stays, and nothing is offered later.
    pub fn confirm_call_link_leave(&mut self) -> Result<(), ConnectSendError> {
        let Some(confirm) = self.session.calls.call_link_stage.take_confirm() else {
            return Ok(());
        };
        let call = match confirm.leave {
            LeaveKind::PrivateCall => self.session.calls.active_call.as_ref().map(|call| call.id),
            LeaveKind::GroupCall { .. } => self
                .session
                .calls
                .active_group_call
                .as_ref()
                .map(|call| call.id),
        };
        // The call may have ended while the question was open.
        let Some(call) = call else {
            return self.offer_confirmed_link(confirm);
        };
        let leaving = match confirm.leave {
            // A hang-up TDLib was not sent leaves the call up: no link
            // waits for it to end.
            LeaveKind::PrivateCall => LeavingCall::Private {
                call_id: call,
                request: self.discard_call()?,
            },
            // The group call is dropped here whether or not the leave went
            // out, as the call screen's own Leave does.
            LeaveKind::GroupCall { .. } => {
                let _ = self.leave_group_call();
                self.drop_group_call_locally();
                LeavingCall::Group(call)
            }
        };
        self.session.calls.call_link_stage = CallLinkStage::Leaving(CallLinkLeaving {
            link: confirm,
            leaving,
        });
        self.maybe_offer_pending_call_link()
    }

    /// The viewer declined: the link does nothing more.
    pub fn cancel_call_link_leave(&mut self) {
        self.session.calls.call_link_stage.take_confirm();
    }

    /// Offer a confirmed link's call once no call is on. Called from
    /// `ingest`, like the swap prompt's queued accept. A call other than the
    /// one being left taking its place drops the link.
    pub(crate) fn maybe_offer_pending_call_link(&mut self) -> Result<(), ConnectSendError> {
        let CallLinkStage::Leaving(waiting) = &self.session.calls.call_link_stage else {
            return Ok(());
        };
        let private = self.session.calls.active_call.as_ref().map(|call| call.id);
        let group = self
            .session
            .calls
            .active_group_call
            .as_ref()
            .filter(|call| call.is_joined)
            .map(|call| call.id);
        if private.is_some() || group.is_some() {
            let same = match waiting.leaving {
                LeavingCall::Private { call_id, .. } => private == Some(call_id) && group.is_none(),
                LeavingCall::Group(id) => group == Some(id) && private.is_none(),
            };
            if !same {
                self.session.calls.call_link_stage = CallLinkStage::Idle;
            }
            return Ok(());
        }
        let CallLinkStage::Leaving(CallLinkLeaving { link, .. }) =
            std::mem::take(&mut self.session.calls.call_link_stage)
        else {
            return Ok(());
        };
        self.offer_confirmed_link(link)
    }

    fn offer_confirmed_link(&mut self, link: CallLinkConfirm) -> Result<(), ConnectSendError> {
        // The old call is gone: nothing the UI joins blocks the offer now.
        let step = self.offer_call(
            ChatId(link.chat_id),
            Some(&link.invite_hash),
            link.live_stream,
            link.mode,
            None,
        )?;
        if let CallLinkStep::Offer { group_call_id } = step
            && let CallLinkStage::Joinable(pending) =
                std::mem::take(&mut self.session.calls.call_link_stage)
        {
            self.session.calls.call_link_stage = CallLinkStage::Offered {
                offered: OfferedCallLink {
                    group_call_id,
                    live_stream: link.live_stream,
                    mode: link.mode,
                },
                link: pending,
            };
        }
        Ok(())
    }
}
