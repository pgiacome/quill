# gpui-kit 0.10.0

gpui-kit, gpui-base and gpui-component go from `=0.9.1` to `=0.10.0`. GPUI
stays on the gpui-pre 0.3.12 snapshot: every 0.10.0 crate pins `=0.3.12`, so
the gpui-pre 0.3.13 on crates.io can't be taken until a Kit release moves to it.
The optional, unbuilt gpui-fast crates go to 0.1.16. Only `gpui-base` and
`gpui-component` were re-vendored.

## What tdesktop does

Nothing relevant. This is Quill's own dependency policy
(`docs/dependency-updates.md`).

## What changed

### Vendored crates

Commit `2ef7b0d98` adds the unmodified 0.10.0 registry copies (without
`.cargo-ok`, `.cargo_vcs_info.json`, `Cargo.lock` and `Cargo.toml.orig`). The
next commit replays Quill's diff against the 0.9.1 registry copies.

| Crate | What had to change |
|---|---|
| gpui-base | Upstream 0.10 swaps Left and Right in a right-to-left layout (`arrows_mirrored`). Quill's input engine already moves the caret in visual order (`step_horizontally`), so Left goes left on screen in any paragraph. The swap would mirror a right-to-left layout twice, so it is left out and `left`/`right` call Quill's visual steps. Upstream's `buffer_line == target_row` check in `line_and_position_for_offset` is kept, with Quill's line-end affinity. Upstream's fallback that still selects a line when a triple click lands beside it is kept, on Quill's `run.geo()`. The tab cell clip in `text_wrapper.rs` is upstream's, on Quill's row tops. |
| gpui-component | `TitleBar` takes upstream's start-edge padding and keeps Quill's themed height. `MessageScrollerState::scroll_to_item_offset` is gone: 0.10's `scroll_to_offset` (#3922) does the same, and Quill's callers use it. |

### Kit 0.10 features Quill now uses

- **Spelling underlines are Textarea text decorations** (`TextareaState::set_decorations`,
  #3970). Before, a canvas over the composer asked the input for each word's
  rectangles and painted wavy lines over them every frame. Now the composer
  render hands the kit the misspelled ranges with a red wavy `UnderlineStyle`
  (`sync_spellcheck_decorations`), and the kit draws them as part of the text runs.
  The underline follows wrapping, mixed-direction words and scrolling
  without extra work, and the kit moves each range across an edit until the
  background re-check lands. The kit ignores an unchanged list, so a render
  costs one comparison. The spelling state (`Underlines`) is still the only source; the
  decorations are derived from it. The words-with-an-emoji wash stays a canvas:
  it has a two-colour gradient and a glimmer that a text style can't draw.
- **Reduced motion is an explicit preference** (`set_reduce_motion_preference`,
  #3881). The kit reads the OS setting on its own and, on Linux, follows the
  portal live. A plain `App::set_reduce_motion` is a value the kit may
  overwrite when it equals what the kit last wrote, so a reduction from Power
  Saving or the battery could come undone on an OS change. Quill now hands its
  answer as `Reduce` or `Allow` (`reduce_motion_probe::apply`).
- **Japanese kit strings.** A Japanese language pack now sets the kit's `ja`
  locale (added in #3976), so dialogs, pickers and the date picker read in Japanese.

### Looked at, no use in Quill

- Global right-to-left layout (#3886): tdesktop's code never turns on
  `style::RightToLeft` for a language, and Quill's composer already lays out each paragraph
  in its own direction (`composer_rtl.rs`).
- Match highlighting in Select, Combobox and Command, `Checkbox::indeterminate`,
  `TableRow::id`/`hover`, `DataTable::scrollbar_mode`, `SidebarGroup::suffix`,
  the NumberInput stepper, `Diff::split_ratio`, tooltip anchor offsets: Quill
  doesn't use these components, or has no mixed-state checkbox.
- `context_menu_offset`: the composer's spelling menu reads the word at the caret,
  which the right click has already moved. That is the same word.
- `TextView::link`: the only `TextView` is What's New, whose notes are Quill's
  own release notes; their links should open in the browser, as they do.
- Textarea `set_tokens`: the composer's custom emoji tokens change only with
  edits, which already carry them.

These fixes reach Quill without any change: the caret moves and deletes by
grapheme cluster, IME candidates stay on their row, the caret shows after a long
paste, triple click selects a line, tooltips wrap within `max_w`, a stacked
dialog is offset only by open dialogs, Windows scrollbars follow the system,
input text, labels and disabled state reach screen readers, and a title-bar click
that moves a pixel still counts.

### Menus

`popup_menu.rs` opens a submenu on the end side (right in a left-to-right
layout) and flips only when it would overflow there. Items lay out with
`flex_row_dir`, and the chevron points left in right-to-left layouts. Quill's layout is left to right, so its
menus behave as before. The submenu safe-triangle work patches the same files
and is carried across this re-vendor.

### Other updates in this sweep

- `cargo update` (as of the 3-day publish cooldown): annotate-snippets, cc,
  errno, libloading, mio, naga, rust-embed, rustls-webpki, rusty-opus 1.1.0,
  taffy, unicode-properties, ureq, wgpu 30.0.2, zbus 6.3, zruntime. Releases
  younger than three days (bumpalo, ctor, link-section, lz4_flex, nix 0.32,
  notify-types, zip 9.6, rusty-opus 1.1.1) arrive with the next sweep.
- object 0.41 (Mach-O reading for the launch checks and `settle-probe`) and
  resvg 0.50.
- Held, rechecked 2026-11-16: moxcms 0.9 (image 0.25.10 and jxl-oxide 0.12.6
  still ask for 0.8), x11rb 0.14 (gpui-pre-linux, even 0.3.13, still asks for
  0.13), zbus 6 for GPUI (ksni 0.3.7 and accesskit_unix 0.24.1 still ask for 5).
  windows-future 0.100 is newly held: it pairs with windows-core 0.100, and
  Quill's `windows` 0.62 returns the 0.3 operation types.
- TDLib 1.8.70, FFmpeg, ThorVG, Rust 1.100 and Actions have no open watch issue.

## How verified

- Gate: fmt, both clippies, core and UI tests, license list.
- A UI test checks that misspellings become wavy decorations over exactly their
  ranges, and that the Check spelling switch removes them.
- Demo captures of the composer with misspelled Latin and Hebrew words, and the
  main screens in light and dark.
