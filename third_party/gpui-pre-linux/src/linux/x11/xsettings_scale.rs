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
//! [`xsettings_scale_factor`] reads it the way GTK 3 does:
//!
//! - `Gdk/WindowScalingFactor`: the integer window scale (1 when absent).
//! - `Gdk/UnscaledDPI`, else `Xft/DPI`: the text DPI times 1024 (96 when
//!   both are absent or not positive, which is how the managers spell
//!   "not set").
//!
//! The scale is `window scale * text DPI / 96`. A manager that publishes
//! neither key still answers: the desktop is running and asks for no
//! scaling, so the scale is 1.0 and the monitor's size is not consulted.
//!
//! The parser takes bytes another process wrote, so it never indexes or
//! trusts a length: a truncated or malformed property gives `None` and the
//! caller falls back to what it did before.

/// The text DPI every scale is relative to.
const BASE_DPI: f64 = 96.0;
/// XSETTINGS stores DPI values multiplied by this.
const DPI_FIXED_POINT: f64 = 1024.0;
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

/// The scale-related integers of one `_XSETTINGS_SETTINGS` property.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ScaleSettings {
    window_scaling_factor: Option<i32>,
    unscaled_dpi: Option<i32>,
    xft_dpi: Option<i32>,
}

/// The scale factor a `_XSETTINGS_SETTINGS` property value asks for.
///
/// `None` when the bytes are not a complete XSETTINGS property or the
/// result is out of range; the caller then uses its other sources.
pub(crate) fn xsettings_scale_factor(data: &[u8]) -> Option<f32> {
    let settings = parse(data)?;

    let window_scale = match settings.window_scaling_factor {
        Some(factor) if factor >= 1 && f64::from(factor) <= MAX_SCALE => f64::from(factor),
        _ => 1.0,
    };
    let text_dpi = [settings.unscaled_dpi, settings.xft_dpi]
        .into_iter()
        .flatten()
        .find(|dpi| *dpi > 0)
        .map_or(BASE_DPI, |dpi| f64::from(dpi) / DPI_FIXED_POINT);

    let scale = window_scale * text_dpi / BASE_DPI;
    if !(MIN_SCALE..=MAX_SCALE).contains(&scale) {
        return None;
    }
    Some(scale as f32)
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

    enum Value<'a> {
        Int(i32),
        Str(&'a str),
        Color,
    }

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
                Value::Str(_) => TYPE_STRING,
                Value::Color => TYPE_COLOR,
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
                Value::Color => out.extend([0; 8]),
            }
        }
        out
    }

    /// Xfce at its defaults: window scale 1, DPI "not set" (-1), plus the
    /// string settings that sit around them. This is the 13 inch 1920x1080
    /// laptop that RandR scaled to 1.75.
    #[test]
    fn xfce_defaults_ask_for_no_scaling() {
        let data = property(
            false,
            &[
                ("Gdk/WindowScalingFactor", Value::Int(1)),
                ("Net/ThemeName", Value::Str("Greybird")),
                ("Xft/DPI", Value::Int(-1)),
                ("Xft/HintStyle", Value::Str("hintnone")),
            ],
        );
        assert_eq!(xsettings_scale_factor(&data), Some(1.0));
    }

    #[test]
    fn a_manager_with_no_scale_keys_asks_for_no_scaling() {
        let data = property(false, &[("Net/ThemeName", Value::Str("Adwaita"))]);
        assert_eq!(xsettings_scale_factor(&data), Some(1.0));
        assert_eq!(xsettings_scale_factor(&property(false, &[])), Some(1.0));
    }

    /// GNOME and Xfce at 2x publish the window scale, the unscaled text DPI
    /// and `Xft/DPI` already multiplied by the window scale. The unscaled
    /// value wins, so the window scale is not applied twice.
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
        assert_eq!(xsettings_scale_factor(&data), Some(2.0));
    }

    #[test]
    fn text_dpi_alone_scales_fractionally() {
        let data = property(false, &[("Xft/DPI", Value::Int(120 * 1024))]);
        assert_eq!(xsettings_scale_factor(&data), Some(1.25));

        let data = property(
            false,
            &[
                ("Gdk/WindowScalingFactor", Value::Int(1)),
                ("Xft/DPI", Value::Int(144 * 1024)),
            ],
        );
        assert_eq!(xsettings_scale_factor(&data), Some(1.5));
    }

    #[test]
    fn window_scale_multiplies_the_text_dpi() {
        let data = property(
            false,
            &[
                ("Gdk/UnscaledDPI", Value::Int(120 * 1024)),
                ("Gdk/WindowScalingFactor", Value::Int(2)),
            ],
        );
        assert_eq!(xsettings_scale_factor(&data), Some(2.5));
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
        assert_eq!(xsettings_scale_factor(&data), Some(1.25));
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
        assert_eq!(xsettings_scale_factor(&data), Some(2.0));
    }

    #[test]
    fn a_window_scale_out_of_range_counts_as_one() {
        for factor in [0, -3, 1000] {
            let data = property(false, &[("Gdk/WindowScalingFactor", Value::Int(factor))]);
            assert_eq!(xsettings_scale_factor(&data), Some(1.0), "factor {factor}");
        }
    }

    #[test]
    fn a_scale_out_of_range_is_refused() {
        let tiny = property(false, &[("Xft/DPI", Value::Int(1024))]);
        assert_eq!(xsettings_scale_factor(&tiny), None);
        let huge = property(false, &[("Xft/DPI", Value::Int(i32::MAX))]);
        assert_eq!(xsettings_scale_factor(&huge), None);
    }

    #[test]
    fn malformed_properties_are_refused() {
        assert_eq!(xsettings_scale_factor(&[]), None);
        assert_eq!(xsettings_scale_factor(&[0, 0, 0]), None);
        // A byte order that is neither LSB (0) nor MSB (1).
        assert_eq!(
            xsettings_scale_factor(&[9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            None
        );

        let data = property(
            false,
            &[
                ("Gdk/WindowScalingFactor", Value::Int(2)),
                ("Net/ThemeName", Value::Str("Greybird")),
            ],
        );
        // Every proper prefix is short of what the header promises.
        for len in 0..data.len() {
            assert_eq!(
                data.get(..len).and_then(xsettings_scale_factor),
                None,
                "prefix of {len} bytes"
            );
        }

        // An unknown setting type cannot be skipped, since its size is unknown.
        let mut unknown = property(false, &[("Gdk/WindowScalingFactor", Value::Int(2))]);
        if let Some(kind) = unknown.get_mut(12) {
            *kind = 3;
        }
        assert_eq!(xsettings_scale_factor(&unknown), None);

        // A string length that runs past the end.
        let mut long = property(false, &[("Net/ThemeName", Value::Str("Greybird"))]);
        let len_at = long.len() - 12;
        if let Some(len) = long.get_mut(len_at..len_at + 4) {
            len.copy_from_slice(&u32::MAX.to_le_bytes());
        }
        assert_eq!(xsettings_scale_factor(&long), None);
    }
}
