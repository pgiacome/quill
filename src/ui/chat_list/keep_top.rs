//! The chat list keeps its top row through a change to how tall its rows are
//! (density, preview lines). The kit list scrolls by pixels: scrolled to row
//! 100 of 64px rows (6,400px down), 40px rows would show row 160 there.
#![deny(clippy::indexing_slicing)]

use super::*;
use crate::ui::chat_list_keys::ListRow;

/// The row at the top of the chat list and how far into it the list was
/// scrolled, as a share of its height. Taken before the rows change height
/// and put back by the next build of the list (`ChatListUi::keep_top`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct ListTop {
    index: usize,
    /// The row by identity; `None` for the stories strip, kept by place.
    row: Option<ListRow>,
    into: f32,
}

/// The row under the top edge when the list is scrolled `scrolled` pixels
/// down over rows `heights` tall, and how far into it as a share of its
/// height.
fn top_at(heights: impl IntoIterator<Item = f32>, scrolled: f32) -> Option<(usize, f32)> {
    let mut start = 0.;
    for (index, height) in heights.into_iter().enumerate() {
        if height > 0. && scrolled < start + height {
            return Some((index, ((scrolled - start) / height).clamp(0., 1.)));
        }
        start += height;
    }
    None
}

/// How far down to scroll for row `index` to sit `into` of its height above
/// the top edge.
fn scroll_to(heights: impl IntoIterator<Item = f32>, index: usize, into: f32) -> Option<f32> {
    let mut start = 0.;
    for (i, height) in heights.into_iter().enumerate() {
        if i == index {
            return Some(start + into * height);
        }
        start += height;
    }
    None
}

fn heights(items: &[ChatListItem]) -> impl Iterator<Item = f32> + '_ {
    items.iter().map(|item| f32::from(item.height()))
}

impl QuillApp {
    /// Note the row at the top of the chat list before its rows change
    /// height. A list at its top stays there with nothing to note.
    pub(in crate::ui) fn keep_chat_list_top(&mut self) {
        let scrolled = -f32::from(self.chat_list.scroll.offset().y);
        let items = &self.chat_list.items;
        self.chat_list.keep_top = (scrolled > 0.)
            .then(|| top_at(heights(items), scrolled))
            .flatten()
            .and_then(|(index, into)| {
                Some(ListTop {
                    index,
                    row: items.get(index)?.list_row(),
                    into,
                })
            });
    }

    /// With the rows just built (in their new heights), before the list lays
    /// them out: scroll so the noted row is on top again. A row that left
    /// the list leaves the scroll where it is.
    pub(super) fn restore_chat_list_top(&mut self, items: &[ChatListItem]) {
        let Some(top) = self.chat_list.keep_top.take() else {
            return;
        };
        let index = match top.row {
            Some(row) => items.iter().position(|item| item.list_row() == Some(row)),
            None => items
                .get(top.index)
                .is_some_and(|item| item.list_row().is_none())
                .then_some(top.index),
        };
        let Some(scrolled) = index.and_then(|index| scroll_to(heights(items), index, top.into))
        else {
            return;
        };
        let offset = self.chat_list.scroll.offset();
        self.chat_list
            .scroll
            .set_offset(point(offset.x, px(-scrolled)));
    }
}

#[cfg(test)]
mod tests {
    use super::{scroll_to, top_at};

    #[test]
    fn the_top_row_and_how_far_into_it() {
        let rows = [64., 64., 64.];
        assert_eq!(top_at(rows, 0.), Some((0, 0.)));
        assert_eq!(top_at(rows, 96.), Some((1, 0.5)));
        assert_eq!(top_at(rows, 128.), Some((2, 0.)));
        assert_eq!(top_at(rows, 500.), None);
    }

    #[test]
    fn the_same_row_stays_on_top_in_shorter_rows() {
        let standard = vec![64.; 200];
        let compact = vec![40.; 200];
        let (index, into) = top_at(standard.iter().copied(), 6_400. + 16.).unwrap_or_default();
        assert_eq!((index, into), (100, 0.25));
        assert_eq!(
            scroll_to(compact.iter().copied(), index, into),
            Some(4_010.)
        );
        // And back.
        assert_eq!(scroll_to(standard, index, into), Some(6_416.));
    }

    #[test]
    fn mixed_heights_count_each_row_above() {
        // Stories strip, a tagged row, two plain rows.
        let rows = [80., 88., 64., 64.];
        assert_eq!(top_at(rows, 80. + 88. + 32.), Some((2, 0.5)));
        assert_eq!(
            scroll_to([80., 64., 40., 40.], 2, 0.5),
            Some(80. + 64. + 20.)
        );
        assert_eq!(scroll_to(rows, 9, 0.), None);
    }
}
