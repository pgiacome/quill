// Added by the Quill project (2026) to gpui-pre-linux 0.3.12 (Apache-2.0).
// See QUILL-CHANGES.md next to this crate's Cargo.toml.
// scripts/test-x11-scale.sh runs its tests.

//! The X11 scale factor the desktop asks for, read from XSETTINGS.
//!
//! Upstream reads `Xft.dpi` from the X resource database and, when it is not
//! there, computes a scale from the monitor's physical size (RandR). A
//! desktop that never writes `Xft.dpi` (Xfce, MATE, LXQt and others at their
//! default settings) then gets a window scaled by the panel's pixel density,
//! for example 1.75 on a 13 inch 1920x1080 laptop, while every other
//! application on that desktop draws at 1.0.
//!
//! Such desktops still publish their scale through the XSETTINGS manager
//! (`_XSETTINGS_SETTINGS` on the owner of the `_XSETTINGS_S<n>` selection).
//! [`xsettings_scale_factor`] reads two things from it:
//!
//! - `Gdk/WindowScalingFactor`: the integer window scale.
//! - A text DPI the user chose: `Gdk/UnscaledDPI`, else `Xft/DPI`, both
//!   stored times 1024. `Gdk/UnscaledDPI` comes first because a manager
//!   that publishes it (gsd-xsettings) multiplies `Xft/DPI` by the window
//!   scale.
//!
//! The scale is the window scale (1 when unset) times the text DPI over 96
//! (96 when unset). That is how Chromium and Qt combine the two, and it
//! matches the `Xft.dpi` path, which also takes DPI over 96. GTK 3 itself
//! uses only the window scale for windows and the DPI for fonts.
//!
//! A value that is not a usable choice counts as unset, so a bad value is
//! never half-trusted:
//!
//! - an out-of-range window scale or DPI;
//! - an `Xft/DPI` equal to the core screen's measured DPI. With Custom DPI
//!   off, xfsettingsd overwrites `Xft/DPI` with the DPI it measures from
//!   the core screen's millimetre size ([`CoreScreen::xfsettingsd_dpi`]).
//!   That is the panel's density on servers that report the real size (the
//!   NVIDIA driver takes it from EDID), exactly what this module avoids.
//!   When Custom DPI is on, xfsettingsd also writes `Xft.dpi`, which the
//!   caller reads first.
//!
//! When neither a window scale nor a chosen DPI is set, the manager says
//! nothing about scale (an `xsettingsd` run only for the GTK theme), so the
//! answer is `None` and the caller keeps guessing from the monitor.
//!
//! The parser takes bytes another process wrote, so it never indexes or
//! trusts a length: a truncated or malformed property gives `None` and the
//! caller falls back to what it did before.

/// The text DPI every scale is relative to.
const BASE_DPI: f64 = 96.0;
/// XSETTINGS stores DPI values multiplied by this.
const DPI_FIXED_POINT: i32 = 1024;
/// Scales above this are treated as a broken setting (same bound as the
/// RandR path's `MAX_SCALE`).
const MAX_SCALE: f64 = 20.0;
/// The lowest scale accepted from the settings (a 48 DPI text setting).
const MIN_SCALE: f64 = 0.5;

const WINDOW_SCALING_FACTOR: &[u8] = b"Gdk/WindowScalingFactor";
const UNSCALED_DPI: &[u8] = b"Gdk/UnscaledDPI";
const XFT_DPI: &[u8] = b"Xft/DPI";

const TYPE_INTEGER: u8 = 0;
const TYPE_STRING: u8 = 1;
const TYPE_COLOR: u8 = 2;

/// The core X screen's size in pixels and millimetres (the connection
/// setup's screen), which xfsettingsd measures its DPI from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CoreScreen {
    pub(crate) width_px: u16,
    pub(crate) height_px: u16,
    pub(crate) width_mm: u16,
    pub(crate) height_mm: u16,
}

impl CoreScreen {
    /// The DPI xfsettingsd publishes as `Xft/DPI` when Custom DPI is off:
    /// `xfce_xsettings_helper_screen_dpi` in xfce4-settings'
    /// `xfsettingsd/xsettings.c` (4.16 to master). Each axis is truncated,
    /// both must lie strictly between 50 and 500, and the smaller wins;
    /// otherwise 96.
    fn xfsettingsd_dpi(self) -> i32 {
        const FALLBACK: i32 = 96;
        let axis = |px: u16, mm: u16| {
            (mm > 0)
                .then(|| (25.4 * f64::from(px) / f64::from(mm)) as i32)
                .filter(|dpi| (51..500).contains(dpi))
        };
        match (
            axis(self.width_px, self.width_mm),
            axis(self.height_px, self.height_mm),
        ) {
            (Some(width), Some(height)) => width.min(height),
            _ => FALLBACK,
        }
    }
}

/// The scale-related integers of one `_XSETTINGS_SETTINGS` property.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ScaleSettings {
    window_scaling_factor: Option<i32>,
    unscaled_dpi: Option<i32>,
    xft_dpi: Option<i32>,
}

/// The scale factor a `_XSETTINGS_SETTINGS` property value asks for, in
/// `MIN_SCALE..=MAX_SCALE`.
///
/// `None` when the bytes are not a complete XSETTINGS property, when the
/// manager sets neither a window scale nor a chosen DPI, or when the result
/// is out of range; the caller then uses its other sources.
pub(crate) fn xsettings_scale_factor(data: &[u8], core_screen: CoreScreen) -> Option<f32> {
    let settings = parse(data)?;

    let window_scale = settings
        .window_scaling_factor
        .map(f64::from)
        .filter(|factor| (1.0..=MAX_SCALE).contains(factor));
    // xfsettingsd's measured DPI is below 500, so the product cannot
    // overflow. 96 is never treated as measured: it asks for 1.0 either way,
    // and a manager without a window scale (xsettingsd) may have chosen it.
    let measured_dpi = core_screen.xfsettingsd_dpi();
    let is_measured = |stored: i32| measured_dpi != 96 && stored == measured_dpi * DPI_FIXED_POINT;
    let chosen_dpi = settings.unscaled_dpi.and_then(text_dpi).or_else(|| {
        settings
            .xft_dpi
            .filter(|stored| !is_measured(*stored))
            .and_then(text_dpi)
    });

    let scale = match (window_scale, chosen_dpi) {
        (None, None) => return None,
        (window_scale, dpi) => window_scale.unwrap_or(1.0) * dpi.unwrap_or(BASE_DPI) / BASE_DPI,
    };
    (MIN_SCALE..=MAX_SCALE)
        .contains(&scale)
        .then_some(scale as f32)
}

/// A stored DPI as dots per inch, or `None` when it is outside the DPIs
/// that give a scale from `MIN_SCALE` to `MAX_SCALE` (managers store 0 or
/// -1 for "not set").
fn text_dpi(stored: i32) -> Option<f64> {
    Some(f64::from(stored) / f64::from(DPI_FIXED_POINT))
        .filter(|dpi| (MIN_SCALE * BASE_DPI..=MAX_SCALE * BASE_DPI).contains(dpi))
}

/// A cursor over the property bytes. Every read is bounds-checked.
struct Reader<'a> {
    data: &'a [u8],
    big_endian: bool,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let (head, tail) = self.data.split_at_checked(len)?;
        self.data = tail;
        Some(head)
    }

    /// Takes `len` bytes and the padding that rounds `len` up to four.
    fn take_padded(&mut self, len: usize) -> Option<&'a [u8]> {
        let bytes = self.take(len)?;
        self.take(len.wrapping_neg() & 3)?;
        Some(bytes)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1)?.first().copied()
    }

    fn u16(&mut self) -> Option<u16> {
        let bytes: [u8; 2] = self.take(2)?.try_into().ok()?;
        Some(if self.big_endian {
            u16::from_be_bytes(bytes)
        } else {
            u16::from_le_bytes(bytes)
        })
    }

    fn u32(&mut self) -> Option<u32> {
        let bytes: [u8; 4] = self.take(4)?.try_into().ok()?;
        Some(if self.big_endian {
            u32::from_be_bytes(bytes)
        } else {
            u32::from_le_bytes(bytes)
        })
    }

    fn i32(&mut self) -> Option<i32> {
        self.u32().map(|value| value as i32)
    }
}

/// Reads the XSETTINGS wire format (the freedesktop XSETTINGS
/// specification): a byte-order byte, three unused bytes, a serial, the
/// number of settings, then each setting as type, unused byte, name length,
/// name (padded to four bytes), last-change serial and a typed value.
fn parse(data: &[u8]) -> Option<ScaleSettings> {
    let mut reader = Reader {
        data,
        big_endian: false,
    };
    reader.big_endian = match reader.u8()? {
        0 => false,
        1 => true,
        _ => return None,
    };
    reader.take(3)?;
    let _serial = reader.u32()?;
    let count = reader.u32()?;

    let mut settings = ScaleSettings::default();
    for _ in 0..count {
        let kind = reader.u8()?;
        reader.take(1)?;
        let name_len = usize::from(reader.u16()?);
        let name = reader.take_padded(name_len)?;
        let _last_change = reader.u32()?;
        match kind {
            TYPE_INTEGER => {
                let value = reader.i32()?;
                if name == WINDOW_SCALING_FACTOR {
                    settings.window_scaling_factor = Some(value);
                } else if name == UNSCALED_DPI {
                    settings.unscaled_dpi = Some(value);
                } else if name == XFT_DPI {
                    settings.xft_dpi = Some(value);
                }
            }
            TYPE_STRING => {
                let len = usize::try_from(reader.u32()?).ok()?;
                reader.take_padded(len)?;
            }
            TYPE_COLOR => {
                reader.take(8)?;
            }
            _ => return None,
        }
    }
    Some(settings)
}


#[cfg(test)]
mod tests {
    use super::*;

    /// A 1920x1080 core screen at the given millimetre size.
    fn core_1080p(width_mm: u16, height_mm: u16) -> CoreScreen {
        CoreScreen {
            width_px: 1920,
            height_px: 1080,
            width_mm,
            height_mm,
        }
    }

    /// Xorg's usual core screen: 96 DPI whatever the panel is.
    const XORG_96: CoreScreen = CoreScreen {
        width_px: 1920,
        height_px: 1080,
        width_mm: 508,
        height_mm: 286,
    };

    // Captured from real managers by xsettings_fixtures/capture.sh
    // (xfsettingsd 4.18.4, xsettingsd 1.0.2, Ubuntu 24.04).

    /// Xfce's shipped settings: window scale 1, no `Xft/DPI` at all.
    const XFCE_DEFAULTS: &[u8] = include_bytes!("xsettings_fixtures/xfce-defaults.bin");
    /// Custom DPI off (`Xft/DPI` -1 in xfconf) on a core screen of
    /// 1920x1080 over 294x165 mm: xfsettingsd publishes 165 DPI.
    const XFCE_MEASURED_165: &[u8] = include_bytes!("xsettings_fixtures/xfce-measured-165dpi.bin");
    /// Window scale 2, Custom DPI off, core screen 1920x1080 over 173x97
    /// mm: xfsettingsd publishes 281 DPI, not multiplied by the scale.
    const XFCE_2X_MEASURED_281: &[u8] =
        include_bytes!("xsettings_fixtures/xfce-2x-measured-281dpi.bin");
    /// xsettingsd run only for the GTK theme (i3, bspwm).
    const XSETTINGSD_THEME_ONLY: &[u8] =
        include_bytes!("xsettings_fixtures/xsettingsd-theme-only.bin");
    /// xsettingsd with the theme and a chosen `Xft/DPI` of 144.
    const XSETTINGSD_DPI_144: &[u8] = include_bytes!("xsettings_fixtures/xsettingsd-dpi-144.bin");

    #[test]
    fn captured_properties_parse_to_their_settings() {
        assert_eq!(
            parse(XFCE_DEFAULTS),
            Some(ScaleSettings {
                window_scaling_factor: Some(1),
                ..ScaleSettings::default()
            })
        );
        assert_eq!(
            parse(XFCE_MEASURED_165),
            Some(ScaleSettings {
                window_scaling_factor: Some(1),
                xft_dpi: Some(165 * 1024),
                ..ScaleSettings::default()
            })
        );
        assert_eq!(
            parse(XFCE_2X_MEASURED_281),
            Some(ScaleSettings {
                window_scaling_factor: Some(2),
                xft_dpi: Some(281 * 1024),
                ..ScaleSettings::default()
            })
        );
        assert_eq!(parse(XSETTINGSD_THEME_ONLY), Some(ScaleSettings::default()));
        assert_eq!(
            parse(XSETTINGSD_DPI_144),
            Some(ScaleSettings {
                xft_dpi: Some(144 * 1024),
                ..ScaleSettings::default()
            })
        );
    }

    /// The laptop RandR scaled to 1.75, under Xfce's shipped settings.
    #[test]
    fn xfce_defaults_ask_for_no_scaling() {
        assert_eq!(xsettings_scale_factor(XFCE_DEFAULTS, XORG_96), Some(1.0));
        assert_eq!(
            xsettings_scale_factor(XFCE_DEFAULTS, core_1080p(293, 165)),
            Some(1.0)
        );
    }

    /// With Custom DPI off on a server that reports the panel's real size,
    /// the published DPI is the panel density, which must not scale.
    #[test]
    fn xfce_measured_dpi_is_not_a_scale() {
        assert_eq!(
            xsettings_scale_factor(XFCE_MEASURED_165, core_1080p(294, 165)),
            Some(1.0)
        );
        assert_eq!(
            xsettings_scale_factor(XFCE_2X_MEASURED_281, core_1080p(173, 97)),
            Some(2.0)
        );
    }

    /// The same bytes on a core screen that does not measure 165 DPI were
    /// not measured by xfsettingsd on this server, so the DPI is a choice.
    #[test]
    fn a_dpi_that_is_not_the_measured_one_is_a_choice() {
        let scale = xsettings_scale_factor(XFCE_MEASURED_165, XORG_96);
        assert_eq!(scale, Some(165.0 / 96.0));
    }

    #[test]
    fn a_manager_with_no_scale_keys_leaves_the_scale_to_the_caller() {
        assert_eq!(xsettings_scale_factor(XSETTINGSD_THEME_ONLY, XORG_96), None);
        assert_eq!(xsettings_scale_factor(&property(false, &[]), XORG_96), None);
    }

    #[test]
    fn a_chosen_dpi_without_a_window_scale_scales_fractionally() {
        assert_eq!(
            xsettings_scale_factor(XSETTINGSD_DPI_144, XORG_96),
            Some(1.5)
        );
        // 96 is a choice even on a 96 DPI core screen: it asks for 1.0.
        let data = property(false, &[("Xft/DPI", Value::Int(96 * 1024))]);
        assert_eq!(xsettings_scale_factor(&data, XORG_96), Some(1.0));
    }

    /// gsd-xsettings at 2x publishes the window scale, the unscaled text
    /// DPI and `Xft/DPI` already multiplied by the window scale. The
    /// unscaled value wins, so the window scale is not applied twice.
    #[test]
    fn window_scale_two_is_not_doubled_by_the_scaled_xft_dpi() {
        let data = property(
            false,
            &[
                ("Gdk/UnscaledDPI", Value::Int(96 * 1024)),
                ("Gdk/WindowScalingFactor", Value::Int(2)),
                ("Xft/DPI", Value::Int(192 * 1024)),
            ],
        );
        assert_eq!(xsettings_scale_factor(&data, XORG_96), Some(2.0));
    }

    /// A window scale and a chosen DPI multiply, as in the `Xft.dpi` path.
    #[test]
    fn window_scale_multiplies_a_chosen_dpi() {
        let data = property(
            false,
            &[
                ("Gdk/UnscaledDPI", Value::Int(120 * 1024)),
                ("Gdk/WindowScalingFactor", Value::Int(2)),
            ],
        );
        assert_eq!(xsettings_scale_factor(&data, XORG_96), Some(2.5));

        let data = property(
            false,
            &[
                ("Gdk/WindowScalingFactor", Value::Int(2)),
                ("Xft/DPI", Value::Int(120 * 1024)),
            ],
        );
        assert_eq!(xsettings_scale_factor(&data, XORG_96), Some(2.5));
    }

    #[test]
    fn an_unset_unscaled_dpi_falls_back_to_xft_dpi() {
        let data = property(
            false,
            &[
                ("Gdk/UnscaledDPI", Value::Int(0)),
                ("Xft/DPI", Value::Int(120 * 1024)),
            ],
        );
        assert_eq!(xsettings_scale_factor(&data, XORG_96), Some(1.25));
    }

    #[test]
    fn big_endian_properties_are_read() {
        let data = property(
            true,
            &[
                ("Gdk/WindowScalingFactor", Value::Int(2)),
                ("Net/CursorBlink", Value::Int(1)),
                ("Some/Color", Value::Color),
            ],
        );
        assert_eq!(xsettings_scale_factor(&data, XORG_96), Some(2.0));
    }

    /// A bad value counts as unset, whichever key it is in.
    #[test]
    fn out_of_range_values_count_as_unset() {
        for factor in [0, -3, 1000] {
            let alone = property(false, &[("Gdk/WindowScalingFactor", Value::Int(factor))]);
            assert_eq!(xsettings_scale_factor(&alone, XORG_96), None, "{factor}");

            let with_dpi = property(
                false,
                &[
                    ("Gdk/WindowScalingFactor", Value::Int(factor)),
                    ("Xft/DPI", Value::Int(120 * 1024)),
                ],
            );
            assert_eq!(
                xsettings_scale_factor(&with_dpi, XORG_96),
                Some(1.25),
                "{factor}"
            );
        }
        for dpi in [-1, 0, 1024, i32::MAX] {
            let alone = property(false, &[("Xft/DPI", Value::Int(dpi))]);
            assert_eq!(xsettings_scale_factor(&alone, XORG_96), None, "{dpi}");

            let with_scale = property(
                false,
                &[
                    ("Gdk/WindowScalingFactor", Value::Int(2)),
                    ("Xft/DPI", Value::Int(dpi)),
                ],
            );
            assert_eq!(
                xsettings_scale_factor(&with_scale, XORG_96),
                Some(2.0),
                "{dpi}"
            );
        }
    }

    #[test]
    fn a_product_out_of_range_is_refused() {
        let data = property(
            false,
            &[
                ("Gdk/WindowScalingFactor", Value::Int(20)),
                ("Xft/DPI", Value::Int(192 * 1024)),
            ],
        );
        assert_eq!(xsettings_scale_factor(&data, XORG_96), None);
    }

    #[test]
    fn xfsettingsd_dpi_matches_xfsettingsd() {
        // 1080 px over 286 mm is 95.9, truncated.
        assert_eq!(XORG_96.xfsettingsd_dpi(), 95);
        assert_eq!(core_1080p(294, 165).xfsettingsd_dpi(), 165);
        assert_eq!(core_1080p(173, 97).xfsettingsd_dpi(), 281);
        // A missing or unreasonable size falls back to 96.
        assert_eq!(core_1080p(0, 165).xfsettingsd_dpi(), 96);
        assert_eq!(core_1080p(10, 5).xfsettingsd_dpi(), 96);
        assert_eq!(core_1080p(1000, 600).xfsettingsd_dpi(), 96);
    }

    #[test]
    fn malformed_properties_are_refused() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[0, 0, 0]), None);
        // A byte order that is neither LSB (0) nor MSB (1).
        assert_eq!(parse(&[9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), None);

        // Every proper prefix is short of what the header promises.
        for data in [XFCE_DEFAULTS, XSETTINGSD_DPI_144] {
            for len in 0..data.len() {
                assert_eq!(data.get(..len).and_then(parse), None, "prefix of {len}");
            }
        }

        // An unknown setting type cannot be skipped, since its size is unknown.
        let unknown = property(
            false,
            &[
                ("Gdk/WindowScalingFactor", Value::Int(2)),
                ("Some/Future", Value::Raw { kind: 3 }),
            ],
        );
        assert_eq!(parse(&unknown), None);

        // A string length that runs past the end.
        let long = property(
            false,
            &[
                ("Gdk/WindowScalingFactor", Value::Int(2)),
                ("Net/ThemeName", Value::StrWithLen(u32::MAX, "Greybird")),
            ],
        );
        assert_eq!(parse(&long), None);
    }

    enum Value<'a> {
        Int(i32),
        Str(&'a str),
        /// A string whose length field says `len` instead of its real length.
        StrWithLen(u32, &'a str),
        Color,
        /// A setting of type `kind` with a 4-byte body.
        Raw { kind: u8 },
    }

    /// Encodes a property the way the XSETTINGS specification lays it out.
    /// The captured fixtures above check this reading of the spec.
    fn property(big_endian: bool, settings: &[(&str, Value<'_>)]) -> Vec<u8> {
        let u16_bytes = |value: u16| {
            if big_endian {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        let u32_bytes = |value: u32| {
            if big_endian {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        let pad = |out: &mut Vec<u8>| {
            while out.len() % 4 != 0 {
                out.push(0);
            }
        };

        let mut out = vec![u8::from(big_endian), 0, 0, 0];
        out.extend(u32_bytes(7));
        out.extend(u32_bytes(settings.len() as u32));
        for (name, value) in settings {
            out.push(match value {
                Value::Int(_) => TYPE_INTEGER,
                Value::Str(_) | Value::StrWithLen(..) => TYPE_STRING,
                Value::Color => TYPE_COLOR,
                Value::Raw { kind } => *kind,
            });
            out.push(0);
            out.extend(u16_bytes(name.len() as u16));
            out.extend(name.as_bytes());
            pad(&mut out);
            out.extend(u32_bytes(1));
            match value {
                Value::Int(value) => out.extend(u32_bytes(*value as u32)),
                Value::Str(text) => {
                    out.extend(u32_bytes(text.len() as u32));
                    out.extend(text.as_bytes());
                    pad(&mut out);
                }
                Value::StrWithLen(len, text) => {
                    out.extend(u32_bytes(*len));
                    out.extend(text.as_bytes());
                    pad(&mut out);
                }
                Value::Color => out.extend([0; 8]),
                Value::Raw { .. } => out.extend([0; 4]),
            }
        }
        out
    }
}
