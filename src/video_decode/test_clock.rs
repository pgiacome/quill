//! Tests drive the player on a clock they move by hand, and wait on the
//! decode thread's turns instead of on time: each turn it takes is counted
//! and announced, so a test checks its condition, and while it does not
//! hold yet, sleeps until the next turn. The thread's end is announced too,
//! so a condition it can no longer bring about fails instead of hanging.
#![deny(clippy::indexing_slicing)]

use super::{Player, Shared, State, lock};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The decode thread's turns so far, and whether it ended.
#[derive(Debug, Default)]
pub(super) struct Turns {
    taken: u64,
    exited: bool,
}

impl Shared {
    /// The decode thread took a turn (it holds the state lock).
    pub(super) fn count_turn(&self, state: &mut State) {
        state.turns.taken = state.turns.taken.wrapping_add(1);
        self.progress.notify_all();
    }

    /// The decode thread ended; its last report is in.
    pub(super) fn mark_exited(&self) {
        lock(&self.state).turns.exited = true;
        self.progress.notify_all();
    }
}

/// A test's handle on a player's clock and decode thread.
pub(super) struct Progress(Arc<Shared>);

impl Player {
    /// The handle, with the clock stopped where it is: from now on only
    /// [`Progress::advance`] moves it.
    pub(super) fn progress(&self) -> Progress {
        let progress = Progress(self.shared.clone());
        progress.freeze();
        progress
    }
}

impl Progress {
    /// Stop the clock at the current time (once; later calls keep it).
    pub(super) fn freeze(&self) {
        lock(&self.0.manual_now).get_or_insert_with(Instant::now);
        self.wake();
    }

    pub(super) fn now(&self) -> Instant {
        self.0.now()
    }

    /// Move the clock on by `by`, and wake the decode thread to it.
    pub(super) fn advance(&self, by: Duration) {
        if let Some(at) = lock(&self.0.manual_now).as_mut() {
            *at += by;
        }
        self.wake();
    }

    /// Under the state lock, so a thread about to sleep is either before
    /// its check or already asleep.
    fn wake(&self) {
        let _state = lock(&self.0.state);
        self.0.wake.notify_all();
    }

    fn taken(&self) -> u64 {
        lock(&self.0.state).turns.taken
    }

    /// Sleep until the thread takes a turn after `seen`; false once it ended.
    fn wait_turn(&self, seen: u64) -> bool {
        let state = lock(&self.0.state);
        let state = self
            .0
            .progress
            .wait_while(state, |state| {
                state.turns.taken == seen && !state.turns.exited
            })
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.turns.taken != seen
    }

    /// Wait for the decode thread to bring `done` about.
    pub(super) fn wait_until(&self, mut done: impl FnMut() -> bool) {
        loop {
            let seen = self.taken();
            if done() {
                return;
            }
            assert!(
                self.wait_turn(seen) || done(),
                "the decode thread ended first"
            );
        }
    }

    /// Play on in `step`s of the clock until `done`, at most a minute of it.
    pub(super) fn play_until(&self, step: Duration, mut done: impl FnMut() -> bool) {
        let mut played = Duration::ZERO;
        loop {
            let seen = self.taken();
            if done() {
                return;
            }
            assert!(
                played < Duration::from_secs(60),
                "not done after {played:?}"
            );
            self.advance(step);
            played += step;
            assert!(
                self.wait_turn(seen) || done(),
                "the decode thread ended first"
            );
        }
    }

    /// Pull `count` samples of decoded sound (not underrun silence) from
    /// `tap`, waiting on the decode thread between misses.
    pub(super) fn pull_sound(&self, tap: &mut super::AudioTap, count: usize) {
        let mut pulled = 0;
        while pulled < count {
            let seen = self.taken();
            tap.next_sample();
            if tap.sounding() {
                pulled += 1;
            } else {
                assert!(self.wait_turn(seen), "the decode thread ended first");
            }
        }
    }
}
