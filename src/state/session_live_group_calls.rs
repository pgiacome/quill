//! The Calls tab's list of running group calls: what `updateGroupCall` said
//! about calls nobody here joined, joined to the chats whose `video_chat`
//! points at them. The call window's own call is left to the window.
#![deny(clippy::indexing_slicing)]

use super::*;
use crate::ids::ChatId;

impl Session {
    /// Remember what `updateGroupCall` said about a running call nobody
    /// here joined. A joined, ended or live-story call has no entry: the
    /// window owns the first, and the others are not listed.
    pub(crate) fn note_live_group_call(&mut self, group_call: &ParsedGroupCall) {
        if !group_call.is_active || group_call.is_joined || group_call.is_live_story {
            self.calls.group_call_details.remove(&group_call.id);
            return;
        }
        let kind = if group_call.is_rtmp_stream {
            GroupCallKind::LiveStream
        } else if group_call.is_video_chat {
            GroupCallKind::VideoChat
        } else {
            GroupCallKind::VoiceChat
        };
        self.calls.group_call_details.insert(
            group_call.id,
            GroupCallDetails {
                title: group_call.title.clone(),
                kind,
                participant_count: group_call.participant_count,
            },
        );
    }

    /// The chat whose `video_chat` points at `group_call_id`, if any
    /// (`groupCall` carries no chat id).
    pub fn chat_for_group_call(&self, group_call_id: i32) -> Option<ChatId> {
        if group_call_id == 0 {
            return None;
        }
        self.chats
            .values()
            .find(|chat| {
                chat.video_chat
                    .as_ref()
                    .is_some_and(|call| call.group_call_id == group_call_id)
            })
            .map(|chat| chat.id)
    }

    /// Every running group call the Calls tab lists, in one pass over the
    /// chats: each chat whose `video_chat` points at a call that has
    /// participants or that an update described. The call the window is in
    /// (joined, reconnecting, or joining) and the watched live story are
    /// left to the window. Streams come first, then video chats, voice
    /// chats, and calls known only from their chat; each group by the
    /// name the row shows, then by id.
    pub fn live_group_calls(&self) -> Vec<LiveGroupCall> {
        let window_call = self
            .calls
            .active_group_call
            .as_ref()
            .filter(|_| self.group_call_joined_or_joining(None))
            .map(|call| call.id);
        let story_call = self.calls.story_stream.as_ref().map(|call| call.id);
        let in_window = |id: i32| window_call == Some(id) || story_call == Some(id);
        let mut calls: Vec<LiveGroupCall> = self
            .chats
            .values()
            .filter_map(|chat| {
                let video_chat = chat.video_chat.as_ref()?;
                let id = video_chat.group_call_id;
                if id == 0 || in_window(id) {
                    return None;
                }
                let details = self.calls.group_call_details.get(&id).cloned();
                if details.is_none() && !video_chat.has_participants {
                    return None;
                }
                let name = if chat.title.is_empty() {
                    details
                        .as_ref()
                        .map(|details| details.title.clone())
                        .unwrap_or_default()
                } else {
                    chat.title.clone()
                };
                Some(LiveGroupCall {
                    id,
                    chat_id: chat.id,
                    name,
                    details,
                })
            })
            .collect();
        calls.sort_by_cached_key(|call| {
            let rank = match call.details.as_ref().map(|details| details.kind) {
                Some(GroupCallKind::LiveStream) => 0u8,
                Some(GroupCallKind::VideoChat) => 1,
                Some(GroupCallKind::VoiceChat) => 2,
                None => 3,
            };
            (rank, call.name.to_lowercase(), call.id)
        });
        calls
    }
}
