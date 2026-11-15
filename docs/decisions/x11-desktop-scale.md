# X11: follow the desktop's scale, not the monitor's pixel density

Branch `fix/x11-desktop-scale`. No parity item.

## The problem

On Xfce (Ubuntu 24.04, X11, a 13.3 inch 1920x1080 laptop panel) Quill
opened with everything 1.75 times larger than the rest of the desktop. Its
maximized window was 1097 logical pixels wide on a 1920 pixel screen, next
to GTK, Qt and Electron applications that all drew at 1.0. Lowering
Settings > Appearance > Interface scale to its minimum (75%) did not get
back to the desktop's size. Starting Quill with `GPUI_X11_SCALE_FACTOR=1`
did.

The cause is in the vendored GPUI X11 backend
(`third_party/gpui-pre-linux/src/linux/x11/client.rs`, `get_scale_factor`).
It looked at, in order:

1. `GPUI_X11_SCALE_FACTOR`,
2. `Xft.dpi` in the X resource database,
3. the monitor's physical size from RandR (pixels per millimetre against
   96 DPI, rounded to twelfths).

GNOME and KDE write `Xft.dpi`, so step 2 answers there. Xfce, MATE, LXQt
and others do not write it while their DPI setting is at its default, so
step 3 answered with the panel's density: 1920 pixels across 293 mm is
166 DPI, which is 1.75. The same guess gives about 1.67 on a 14 inch and
1.5 on a 15.6 inch 1920x1080 panel, so most laptops on those desktops got
an oversized window. The desktop itself had said, through XSETTINGS, that
it wanted no scaling (`Gdk/WindowScalingFactor` 1, `Xft/DPI` unset), and
upstream's code carried a `TODO: Use scale factor from XSettings here`.

## Telegram Desktop

Telegram Desktop derives its default interface scale from Qt's logical DPI
and device pixel ratio of the primary screen. On X11 Qt takes those from
the desktop's settings (`Xft.dpi`, XSETTINGS), not from the monitor's
physical size, so following the desktop is the same rule. (Stated from
knowledge of Qt and Telegram Desktop; their sources were not re-read for
this change.)

## What changed

All of it is in our fork under `third_party/gpui-pre-linux`
(`QUILL-CHANGES.md` has the entry).

- `get_scale_factor` now asks the XSETTINGS manager between steps 2 and 3:
  the owner of the `_XSETTINGS_S<screen>` selection, its
  `_XSETTINGS_SETTINGS` property. When a manager answers, its scale is
  used and RandR is not consulted.
- `src/linux/x11/xsettings_scale.rs` (new, std-only) parses the property
  and computes the scale the way GTK 3 does: `Gdk/WindowScalingFactor`
  (1 when absent or out of range) times the text DPI over 96, where the
  text DPI is `Gdk/UnscaledDPI`, else `Xft/DPI`, else 96. Values that are
  not positive mean "not set". `Gdk/UnscaledDPI` comes first because GNOME
  and Xfce publish `Xft/DPI` already multiplied by the window scale, and
  using that would apply the window scale twice.
- A manager that publishes none of the three keys still answers with 1.0:
  a desktop is running and asks for no scaling.

What stays the same:

- `GPUI_X11_SCALE_FACTOR` and `Xft.dpi` keep their priority, so GNOME, KDE
  and anyone who set either see no change.
- With no XSETTINGS manager (a bare window manager) the RandR guess is
  still used. Nothing there says what the user wants, and on a high-density
  monitor a guess is better than a tiny window.
- Wayland is untouched; the compositor gives the scale.
- The scale is read once at startup, like `Xft.dpi`. Changing the desktop's
  scale takes effect the next time Quill starts.

Nothing is stored. The scale is detected at every start, so a new install
needs no first-run step, and moving to another monitor or changing the
desktop's scale is picked up without resetting a saved value. The
Interface scale setting still applies on top, for people who want Quill
larger or smaller than the desktop.

## Never crash

The property is written by another process. The parser reads through a
bounds-checked cursor (no indexing, no `unwrap`), refuses an unknown byte
order or setting type, a truncated property and a scale outside 0.5 to 20,
and the caller then falls through to the RandR guess as before. Every X
request error on the way (no selection owner, the owner going away) also
falls through.

## Tests

`scripts/test-x11-scale.sh` (in CI's `linux-fmt-clippy-test` job) builds
the module standalone and runs its unit tests: Xfce's defaults (the case
above), a manager with no scale keys, window scale 2 with the scaled
`Xft/DPI` beside it, fractional text DPI, both byte orders, out-of-range
values, and every truncation of a valid property.

## Verified

On the Xfce machine above (Ubuntu 24.04, X11, Rust 1.100.0), with `Xft.dpi`
absent from `xrdb -query` and `Gdk/WindowScalingFactor` 1 in the Xfce
settings:

- `bash scripts/test-x11-scale.sh`: 10 tests pass.
- `cargo check --locked -p gpui-pre-linux` and
  `cargo build --locked --features ui` succeed.
- The debug build, started without `GPUI_X11_SCALE_FACTOR`, opens its
  default 1200 by 760 window (`src/main.rs`) at 1200 by 760 pixels: scale
  1.0. Started with `GPUI_X11_SCALE_FACTOR=randr` (the old fallback) the
  same window is 2100 by 1330 pixels and is clamped to the 1920 by 1080
  screen: scale 1.75.
- The released 2026.1112.6544 build on the same machine: a maximized
  window 1097 logical pixels wide by default, 1920 with
  `GPUI_X11_SCALE_FACTOR=1`.

## Not verified

Other desktops. GNOME and KDE on X11 should be unchanged because `Xft.dpi`
still answers first there; MATE, LXQt, Cinnamon and a 2x Xfce setting are
covered by the unit tests' property layouts only, not by a launch. The
screenshot demos were not run.
