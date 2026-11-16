# Active group calls in the Calls tab

The Calls tab lists the running group calls above Recent calls, every one
the call window is not in. Opening the tab never joins or opens a call: the
list is a read of live state, and joining stays a press on a row.

## The list is read from the chats

`Session::live_group_calls()` (`src/state/session_live_group_calls.rs`)
makes one pass over the chats. A chat is listed when its
`video_chat.group_call_id` points at a call and either:

- `CallsState::group_call_details` has what `updateGroupCall` said about
  that call (its own title, kind and participant count), or
- the chat reports `has_participants`, and no update described the call yet.
  The row then shows no kind or count rather than guessing.

Each row is a `LiveGroupCall { id, chat_id, name, details }`. A call no chat
points at is not listed: there is nothing to offer it through, and its call
title is usually empty. Because the chat decides, a call the chat moved off
(it ended, or another started while the app was away) drops out at once,
whatever the details map still holds.

The row shows the chat's title (the call's own only when the chat has
none), and the list is sorted by that name: live streams first, then video
chats, voice chats, and calls known only from their chat; each group by the
lowercased name, then by call id. The key is computed once per row
(`sort_by_cached_key`).

The call the window is in (joined, reconnecting after a network loss, or
with its join in flight: `group_call_joined_or_joining`) and the watched
live story are not listed. A call the window only shows (looked at, not
joined) stays listed, so its Join still works.

## What writes the details

Only updates do. An entry is written for an active, not-joined call that is
not a live story, and removed when:

- `is_active` is false (the call ended);
- `is_joined` is true (you are in it, and the window owns it);
- `is_live_story` is true;
- `updateChatVideoChat` moves the chat off the call (`group_call_id` 0 or
  another call).

`has_participants: false` never removes details: an empty call still runs.
Leaving a call does not remove them either: the call may still run, and
`updateGroupCall` says when it ends.

## Only the call the person asked for opens the window

`CallsState::active_group_call` is the one call the window holds. A call
nobody joined takes that slot only as the answer to the `getGroupCall` the
person asked for last. The driver's `fetch_group_call` records that request
in `CallsState::wanted_group_call`, and `group_call_update_may_take_slot`
compares the answer's own `RequestId` with it. So:

- a plain `updateGroupCall` for a call nobody joined only feeds the list;
- pressing Join on A and then on B lets only B's answer in;
- leaving the call forgets the request, so a late answer opens nothing;
- a call you are in (joined, reconnecting, or joining) is never replaced by
  another call's answer.

A live story's call is fetched with `send_get_group_call`, which does not
touch `wanted_group_call`: it goes to its own `story_stream` slot.

## Join is the call link's offer

The Calls tab's Join (`ui/calls/active_calls.rs`) first refuses without the
call engine, as the chat header does. It then calls the driver's
`join_listed_call` for the row's chat, the same offer a call link makes
(`src/connect/group_calls/link.rs`) with no hash and `OfferMode::Join`. The
driver alone decides whether another call is on, counting the UI's own join
in flight (`joining`). Free, it fetches the call, and the window joins it
once it is tracked (`StreamJoinIntent::Join`). Busy, the leave question
comes first; the mode waits in it (`CallLinkConfirm`), so a declined
question leaves no join behind, and once the old call ends the offered call
is joined (`offered_call_intent`).

A listed call that cannot be offered (it ended between the list and the
press, or the request could not be sent) says something went wrong, not
that a link is bad.

## Follow-up

tdesktop treats any call in a broadcast channel as a stream
(`peer->isBroadcast()`); the kind here is `LiveStream` only for
`is_rtmp_stream`, matching the window's existing `uses_stream_layout`.

## Verified

- `connect::tests::group_calls::unjoined`: listing from chats and details,
  seeding, a chat moving to another call, ended calls, sort by the shown
  name, the slot gate (racing update, another call's update, the last ask
  wins, an answer after leaving, a reconnecting call kept and not listed),
  and the listed Join (offer, busy question carrying the join, the UI's own
  join counted as busy).
- `ui::calls::active_calls_tests`: the section above Recent calls, Join
  without a driver, what the window does with each offer step, the row's
  second line.
- Screenshot demo `ready-calls-active`.
