# Active group calls in the Calls tab

The Calls tab lists every unjoined running group call above Recent calls —
every running call the call window does not own. A joined call is not listed
there: it is in the call window, whose slot holds one call at a time. Opening
the tab never joins or opens a call: the list is a read of live state, and
joining stays a press on a row.

## Two sources, merged on read

`Session::live_group_calls()` (`src/state/session_calls.rs`) builds the list
from:

- `CallsState::group_call_details` — the details `updateGroupCall` sent for
  each active, not-joined, non-live-story call (title, kind, participant
  count). This is what makes the list survive a fresh start when no chat
  update is replayed.
- Every chat whose `video_chat.has_participants` is true and whose
  `group_call_id` is not already cached. This seeds a row when only the chat
  knows about the call: the title comes from the chat, `kind: None` and
  `participant_count: 0`, because the chat does not prove either, and the row
  shows no kind or count rather than guessing.

The chat id the row needs (avatar, busy Join) is resolved on read from the
chat whose `video_chat.group_call_id` matches. Sorting also happens on read,
in one order: `LiveStream` first, then `VideoChat`, then `VoiceChat`, then
calls known only from a chat (`kind: None`), each group by title; participant
counts never reorder rows.

## The slot rule: only an answer may open a call

`CallsState::active_group_call` is the one call the call window owns. An
`updateGroupCall` may take that slot only as
`group_call_update_may_take_slot` allows:

- a joined update may take it, unless that call was just left (a late update
  must not undo a leave);
- the call already in the slot may always update it;
- a call the person is in is never replaced by an unjoined one;
- an unjoined call may take a slot that holds no joined call only when the
  update is the answer to the person's own `getGroupCall` for that id. That
  includes replacing a different unjoined call already tracked, but never a
  joined one.

The last rule is why the answer's own `PendingRequest` is the intent.
`group_call_update_may_take_slot` does the comparison; the pending request
reaches it from `apply_calls_payload` and must be
`CallsPurpose::GetGroupCall { group_call_id }` for the same id. That is an
answer matched to the request that asked, not a purpose lookup, and it needs
no "wanted" flag kept in step by hand (see
docs/contributing/state-machines.md). Every intentional open already has that
request out — the chat header, a call link, `createVideoChat`'s queued fetch,
a scheduled call — so it still opens. A random `updateGroupCall` for a call
nobody joined only feeds the list; an update racing the answer waits for the
answer, which follows at once.

`StreamJoinIntent::Join(call_id)` carries the same intent for the Calls tab's
Join: the row asks the driver for the call, and the answer to that
`getGroupCall` is what takes the slot. Nothing opens until the person pressed
Join.

## Removal rules

An entry leaves `group_call_details` when:

- `is_active` is false (the call ended);
- `is_joined` is true (the window owns it now, so it is not listed beside the
  slot);
- `is_live_story` is true (a watched story has its own `story_stream` slot;
  any other live story is not tracked at all);
- `updateChatVideoChat` reports `group_call_id == 0` (the chat no longer
  points at a call), which removes the old id;
- `leave_group_call_local` drops the tracked call after the person leaves or
  ends it.

`has_participants: false` alone never removes details. A just-created or
empty stream may legitimately have no participants yet, and the call is still
running. A chat with `has_participants: false` only stops seeding a row; it
does not delete what `updateGroupCall` said.

`live_group_calls()` finally hides the call in the window slot only while it
is *joined* (and hides a call in `story_stream` always). A call being opened
stays listed until it is joined, so the list does not blink it out between
the answer and the join.

## The busy Join carries `join_when_offered`

The Calls tab's Join (`ui/calls/active_calls.rs`) checks whether a 1:1 call
or a joined group call is on. Busy, it does not silently do nothing: it takes
the call-link leave confirmation the app already has
(`offer_call_link_joining`, `src/connect/group_calls/link.rs`) for the row's
chat, with `join_when_offered: true`. The flag waits in the confirmation
(`CallLinkConfirm`), so a declined question leaves no join behind; once the
old call ends, the offer joins the call itself instead of only opening its
screen for the call screen's Join. A row without a chat id cannot be offered
this way and says the current call must end first.

## Verified

- `connect::tests::group_calls`: an unjoined update feeds the list and takes
  no slot; its own `getGroupCall` answer takes the slot; a racing update
  waits; a joined update takes the slot and leaves the list; `is_active:
  false` and `group_call_id == 0` remove details; `has_participants: false`
  does not.
- `ui::calls`: the section is drawn above the Recent calls row, in the
  session's order (`LiveStream` first); the `ready-calls-active` fixture
  yields ids 61 and 62, kinds `LiveStream` then `VoiceChat`, counts 1,280 and
  4, and chats 51 and 52; Join sets the intent; a busy Join offers the call;
  a confirmed offer notes the join.
- Screenshot demo `ready-calls-active`: one live stream and one voice chat
  above the history rows, injected, no network (the capture itself needs a
  platform whose GPUI readback is implemented).
