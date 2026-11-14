//! Wording for the gift code and language pack boxes. Pure, so it is tested
//! without a window. Gift wording follows tdesktop `boxes/gift_premium_box.cpp`
//! (`lng_gift_link_*`).

use crate::lang::{keys, tr, tr_with, trn};

/// "3 months", "1 year", "10 days": the gift's length. tdesktop shows days
/// below 30, whole months below a year and years after that.
pub fn gift_duration(month_count: i64, day_count: i64) -> String {
    let days = if day_count > 0 {
        day_count
    } else {
        month_count * 30
    };
    if days < 30 {
        trn(keys::DAYS, days.max(1))
    } else if days < 30 * 12 {
        trn(keys::MONTHS, days / 30)
    } else {
        trn(keys::YEARS, days / (30 * 12))
    }
}

/// The "Reason" row: `qll_rest_gift_reason_*`.
pub fn gift_reason(is_from_giveaway: bool, used: bool) -> String {
    match (is_from_giveaway, used) {
        (true, true) => tr(keys::Q_REST_GIFT_REASON_GIVEAWAY).to_string(),
        (true, false) => tr(keys::Q_REST_GIFT_REASON_INCOMPLETE_GIVEAWAY).to_string(),
        (false, _) => tr(keys::Q_REST_GIFT_REASON_SELECTED).to_string(),
    }
}

/// How much of a language pack is translated, e.g. "90% translated".
pub fn language_progress(total: i64, translated: i64) -> Option<String> {
    (total > 0).then(|| {
        let percent = (translated.clamp(0, total) * 100) / total;
        tr_with(
            keys::Q_LANGUAGE_TRANSLATED_PERCENT,
            &[("percent", &percent.to_string())],
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{gift_duration, gift_reason, language_progress};
    use crate::lang::{keys, tr, tr_with, trn};

    #[test]
    fn duration_uses_days_months_and_years() {
        assert_eq!(gift_duration(0, 7), trn(keys::DAYS, 7));
        assert_eq!(gift_duration(1, 30), trn(keys::MONTHS, 1));
        assert_eq!(gift_duration(3, 90), trn(keys::MONTHS, 3));
        assert_eq!(gift_duration(12, 360), trn(keys::YEARS, 1));
        assert_eq!(gift_duration(24, 720), trn(keys::YEARS, 2));
        assert_eq!(gift_duration(3, 0), trn(keys::MONTHS, 3));
    }

    #[test]
    fn reason_names_giveaways() {
        assert_eq!(
            gift_reason(true, true),
            tr(keys::Q_REST_GIFT_REASON_GIVEAWAY)
        );
        assert_eq!(
            gift_reason(true, false),
            tr(keys::Q_REST_GIFT_REASON_INCOMPLETE_GIVEAWAY)
        );
        assert_eq!(
            gift_reason(false, false),
            tr(keys::Q_REST_GIFT_REASON_SELECTED)
        );
    }

    #[test]
    fn progress_is_a_clamped_percentage() {
        assert_eq!(language_progress(0, 0), None);
        let quarter = tr_with(keys::Q_LANGUAGE_TRANSLATED_PERCENT, &[("percent", "25")]);
        assert_eq!(
            language_progress(200, 50).as_deref(),
            Some(quarter.as_str())
        );
        let full = tr_with(keys::Q_LANGUAGE_TRANSLATED_PERCENT, &[("percent", "100")]);
        assert_eq!(language_progress(10, 99).as_deref(), Some(full.as_str()));
    }
}
