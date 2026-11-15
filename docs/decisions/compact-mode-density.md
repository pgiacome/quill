# Compact mode: one density value and one table of sizes

Contributed in #1463 (Settings → Appearance → Compact mode). Telegram Desktop
has no density switch (it only scales the whole interface), so this is a
Quill feature; Telegram Desktop is still the reference for the spacing it
keeps.

## Shape

- `AppearancePrefs::density: quill::density::Density` (`Standard` /
  `Compact`, `#[serde(default)]`). A bool would have needed a migration for a
  third step.
- `quill::density::DensityMetrics` holds every size that changes with the
  density (`STANDARD` and `COMPACT`). Render sites read `self.density()`;
  none branch on the density. `chat_row_height_px(has_tags, lines, &metrics)`
  is the one row-height formula, used by the virtual list, the renderer, the
  pinned-row drag and keyboard paging alike. Every Standard value equals the
  literal it replaced; the loading skeleton row now has the real row's shape.
- A change of any setting that changes row heights (density, preview lines,
  bubbles, adaptive width: `RowLayout`) runs one step,
  `appearance_layout_changed`: it ends a pinned drag, notes the chat list's
  top row (`chat_list/keep_top.rs`), resets the tab strips' reveal, and
  re-measures every window's history around its anchor.
- The noted top row is put back by the next build of the list. It belongs to
  the view it was noted in: `ChatListUi::show_view` (another folder or
  category) and `leave_rows` (another tab, the search) drop it, so it is never
  applied to another list.
- The title-bar height is a kit theme value (`Theme::title_bar_height`, see
  `third_party/gpui-component/QUILL-CHANGES.md`), so the kit `TitleBar`, the
  dialog dismiss band, sheets and notifications follow it. macOS traffic
  lights are re-centred per window (`window_frame.rs`, `LIGHTS_ON`), only on
  macOS: `Window::set_traffic_light_position` does not exist elsewhere.

## Spacing kept on purpose

Between sender runs Telegram Desktop adds `msgMargin` top 6px over an attached
message's `msgMarginTopAttached` 0px (`ui/chat/chat.style`). Compact keeps a
4px `sender_gap` at the start of a run so runs stay apart in busy groups;
within a run the bubbles' own inset is the gap.
