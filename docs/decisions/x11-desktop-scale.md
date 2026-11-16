# X11: follow the desktop's scale, not the monitor's pixel density

Contributed by `@pgiacome` (#1471, branch `fix/x11-desktop-scale`); the
review fixes were made on top by the maintainers. No parity item.

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
it wanted no scaling (`Gdk/WindowScalingFactor` 1, no `Xft/DPI`), and
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
  `_XSETTINGS_SETTINGS` property. When the manager answers, its scale is
  used and RandR is not consulted.
- `src/linux/x11/xsettings_scale.rs` (new, std-only) parses the property.
  The scale is `Gdk/WindowScalingFactor` (1 when unset) times a chosen text
  DPI over 96 (96 when unset), where the text DPI is `Gdk/UnscaledDPI`,
  else `Xft/DPI`. This is the rule Chromium and Qt use, and the `Xft.dpi`
  path above also takes DPI over 96. GTK 3 does not do this: it uses the
  window scale for windows and the DPI only for fonts. `Gdk/UnscaledDPI`
  comes first because gsd-xsettings publishes `Xft/DPI` multiplied by the
  window scale, and using that would apply the window scale twice.
- A value that is not a usable choice counts as unset, whichever key it
  is in: a window scale outside 1 to 20, a DPI outside 48 to 1920 (0 and
  -1 are how managers write "not set"), and an `Xft/DPI` that xfsettingsd
  measured (below). The product must lie between 0.5 and 20.
- A manager that sets neither a window scale nor a chosen DPI gives no
  answer, and the RandR guess runs as before. Sessions that run
  `xsettingsd` or lxsession only for the GTK theme (i3, bspwm, LXDE)
  publish no scale key, and a 4K monitor there should keep its 1.75 rather
  than drop to 1.0. Xfce always publishes `Gdk/WindowScalingFactor`, so it
  does not need a "no keys means 1.0" rule.

### What xfsettingsd actually publishes

Checked in xfce4-settings' `xfsettingsd/xsettings.c` (4.16 to master) and
by capturing the property from xfsettingsd 4.18.4 on Xvfb
(`xsettings_fixtures/capture.sh`):

- Shipped defaults (`/Xft/DPI` empty in `xsettings.xml`): no `Xft/DPI` in
  the property at all, `Gdk/WindowScalingFactor` 1. Scale 1.0.
- Custom DPI off in the Appearance dialog (`/Xft/DPI` -1): xfsettingsd
  overwrites the slot with the DPI it measures from the core screen's
  millimetre size (`xfce_xsettings_helper_screen_dpi`: each axis
  truncated, the smaller one, 96 unless both lie strictly between 50 and
  500). It never publishes -1 and never `Gdk/UnscaledDPI`, and does not
  multiply by the window scale. Xorg usually reports a 96 DPI core screen
  (1920x1080 over 508x286 mm, which xfsettingsd measures as 95), but a
  server that reports the panel's real size (the NVIDIA driver takes it
  from EDID) gives the panel density, 165 on the laptop above. Trusting it
  would bring back the 1.75, and at 2x on a 15.6 inch 4K panel about 5.9.
  So an `Xft/DPI` equal to that measurement of the core screen counts as
  unset (except 96, which asks for 1.0 either way and which an
  `xsettingsd` user may have chosen). Measured on 294x165 mm: scale 1.0;
  at 2x on 173x97 mm (281 DPI): scale 2.0.
- Custom DPI on: xfsettingsd also writes `Xft.dpi` to the resource
  database, so step 2 answers before XSETTINGS is read.

An `Xft/DPI` chosen in `xsettingsd`'s config (144, say) does not match the
core screen's measurement and gives 1.5.

What stays the same:

- `GPUI_X11_SCALE_FACTOR` and `Xft.dpi` keep their priority, so GNOME, KDE
  and anyone who set either see no change.
- With no XSETTINGS manager (a bare window manager) the RandR guess is
  still used. Nothing there says what the user wants, and on a high-density
  monitor a guess is better than a tiny window.
- Wayland is untouched; the compositor gives the scale.
- The scale is read once at startup, like `Xft.dpi`. Changing the desktop's
  scale takes effect the next time Quill starts. Quill can start at login
  (`src/autostart.rs`), possibly before xfsettingsd owns `_XSETTINGS_S0`;
  then no manager answers and that session keeps the RandR guess until
  Quill restarts. xfce4-session's start order was not checked. Watching
  the selection owner and `PropertyNotify`, as GTK does, would fix it.

Nothing is stored. The scale is detected at every start, so a new install
needs no first-run step, and moving to another monitor or changing the
desktop's scale is picked up without resetting a saved value. The
Interface scale setting still applies on top, for people who want Quill
larger or smaller than the desktop.

## Never crash

The property is written by another process. The parser reads through a
bounds-checked cursor (no indexing, no `unwrap`), refuses an unknown byte
order or setting type and a truncated property, and the caller then falls
through to the RandR guess as before. Every X request error on the way (no
selection owner, the owner going away) also falls through. The atoms are
looked up with `only_if_exists`, so a server where no manager ever ran
answers `NONE` and is not left with new atoms.

## Tests

`scripts/test-x11-scale.sh` (in CI's `linux-fmt-clippy-test` job) builds
the module standalone and runs its unit tests. Five properties captured
from real managers (`xsettings_fixtures/`, made by `capture.sh`) anchor the
wire format: Xfce's defaults, Custom DPI off at 1x and 2x on core screens
that measure 165 and 281 DPI, and `xsettingsd` with only a theme and with
a chosen DPI of 144. Encoded properties then cover gsd-xsettings at 2x
(`Gdk/UnscaledDPI` beside the scaled `Xft/DPI`), a window scale times a
chosen DPI, both byte orders, out-of-range values in each key, a product
out of range, unknown types, a string length past the end, and every
truncation of a captured property.

## Verified

On the Xfce machine above (Ubuntu 24.04, X11, Rust 1.100.0), with `Xft.dpi`
absent from `xrdb -query` and `Gdk/WindowScalingFactor` 1 in the Xfce
settings:

- `bash scripts/test-x11-scale.sh`: the first version's 10 tests pass.
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

After the review fixes, on a Linux cloud machine (Rust 1.100, Xvfb,
xfsettingsd 4.18.4, xsettingsd 1.0.2): the 14 unit tests pass, UI clippy is
clean, and the debug build's default window, with a temporary print of the
chosen source, came up as follows.

| Core screen DPI | Manager and settings | Source | Window |
| --- | --- | --- | --- |
| 166 | xfsettingsd, shipped defaults | XSETTINGS 1 | 1200x740 |
| 166 | xfsettingsd, Custom DPI off (publishes 165) | XSETTINGS 1 | 1200x740 |
| 282 | xfsettingsd 2x, Custom DPI off (publishes 281) | XSETTINGS 2 | 2400x1480 |
| 166 | xsettingsd, theme only | RandR | 1200x740 |
| 96 | xsettingsd, `Xft/DPI` 144 | XSETTINGS 1.5 | 1800x1110 |
| 166 | none | RandR | 1200x740 |

Xvfb's RandR output reports no physical size, so RandR answers 1 there.

## Not verified

Other desktops. GNOME and KDE on X11 should be unchanged because `Xft.dpi`
still answers first there. MATE, LXQt and Cinnamon were not captured or
launched. A real NVIDIA machine was not tried; its core screen was
reproduced with Xvfb's `-dpi`. The
screenshot demos were not run.
