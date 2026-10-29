// SPDX-License-Identifier: AGPL-3.0-or-later
//! Public holidays in Germany: those of the whole country, and those a state's law adds for all of the state.
//!
//! Sources: the Einigungsvertrag for the 3rd of October, the holiday laws of the states, as collected in
//! <https://de.wikipedia.org/wiki/Gesetzliche_Feiertage_in_Deutschland>; the Weltkindertag in Thuringia since 2019
//! (Drittes Gesetz zur Änderung des Thüringer Feier- und Gedenktagsgesetzes, Drucksache 6/6163); the 15th of August in
//! all of the Saarland (Gesetz Nr. 1040 über die Sonn- und Feiertage). OPEN: a review of each state's law in its
//! current wording.
//!
//! Holidays in only some municipalities – Fronleichnam in parts of Saxony and Thuringia, Mariä Himmelfahrt in parts
//! of Bavaria, the Augsburger Friedensfest – are left out: a deadline then falls on the day before, never after.

use librotenado::Date;

/// Days since 1970-01-01 of a civil date (H. Hinnant, chrono-compatible algorithms).
pub(crate) fn days(date: Date) -> i64 {
    let (y, m, d) = (i64::from(date.year()), i64::from(date.month()), i64::from(date.day()));
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a number of days since 1970-01-01.
fn civil(days: i64) -> Date {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Date::new(y as u16, m as u8, d as u8).expect("a civil date")
}

/// Easter Sunday of a year in the Gregorian calendar (the anonymous Gregorian algorithm).
fn easter(year: u16) -> Date {
    let y = i64::from(year);
    let (a, b, c) = (y % 19, y / 100, y % 100);
    let (d, e) = (b / 4, b % 4);
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let (i, k) = (c / 4, c % 4);
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    Date::new(year, month as u8, day as u8).expect("Easter is in March or April")
}

/// Whether a day is a public holiday in all of a state (`DE-BY`), or – without a state – in all of Germany.
pub fn is_holiday(date: Date, state: Option<&str>) -> bool {
    let year = date.year();
    let in_state = |states: &[&str]| state.is_some_and(|s| states.iter().any(|x| s == format!("DE-{x}")));
    let after_easter = days(date) - days(easter(year));
    let national = match (date.month(), date.day()) {
        (1, 1) | (5, 1) | (10, 3) | (12, 25) | (12, 26) => true,
        // The 500th anniversary of the Reformation, once in all states.
        (10, 31) if year == 2017 => true,
        _ => matches!(after_easter, -2 | 1 | 39 | 50),
    };
    if national {
        return true;
    }
    match (date.month(), date.day()) {
        (1, 6) => in_state(&["BW", "BY", "ST"]),
        (3, 8) => (year >= 2019 && in_state(&["BE"])) || (year >= 2023 && in_state(&["MV"])),
        (5, 8) => matches!(year, 2020 | 2025) && in_state(&["BE"]),
        (8, 15) => in_state(&["SL"]),
        (9, 20) => year >= 2019 && in_state(&["TH"]),
        (10, 31) => in_state(&["BB", "MV", "SN", "ST", "TH"]) || (year >= 2018 && in_state(&["HB", "HH", "NI", "SH"])),
        (11, 1) => in_state(&["BW", "BY", "NW", "RP", "SL"]),
        // Buß- und Bettag: the Wednesday before the 23rd of November.
        (11, 16..=22) if (days(date) + 3).rem_euclid(7) == 2 => in_state(&["SN"]),
        // Fronleichnam, sixty days after Easter.
        _ => after_easter == 60 && in_state(&["BW", "BY", "HE", "NW", "RP", "SL"]),
    }
}

/// The day after.
pub(crate) fn next_day(date: Date) -> Date {
    civil(days(date) + 1)
}

/// The day of the week, Monday 0 to Sunday 6 (1970-01-01 was a Thursday).
pub(crate) fn weekday(date: Date) -> u8 {
    (days(date) + 3).rem_euclid(7) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn easter_and_the_holidays_after_it() {
        assert_eq!(easter(2025), date("2025-04-20"));
        assert_eq!(easter(2026), date("2026-04-05"));
        assert_eq!(easter(2027), date("2027-03-28"));
        for day in ["2026-04-03", "2026-04-06", "2026-05-14", "2026-05-25"] {
            assert!(is_holiday(date(day), None), "{day}");
        }
        assert!(is_holiday(date("2026-06-04"), Some("DE-NW")) && !is_holiday(date("2026-06-04"), Some("DE-SN")));
    }

    #[test]
    fn holidays_of_some_states() {
        assert!(is_holiday(date("2026-01-06"), Some("DE-BY")) && !is_holiday(date("2026-01-06"), Some("DE-BE")));
        assert!(is_holiday(date("2026-03-08"), Some("DE-BE")) && !is_holiday(date("2018-03-08"), Some("DE-BE")));
        assert!(is_holiday(date("2025-05-08"), Some("DE-BE")) && !is_holiday(date("2026-05-08"), Some("DE-BE")));
        assert!(is_holiday(date("2026-08-15"), Some("DE-SL")) && !is_holiday(date("2026-08-15"), Some("DE-BY")));
        assert!(is_holiday(date("2026-10-31"), Some("DE-NI")) && !is_holiday(date("2026-10-31"), Some("DE-NW")));
        assert!(is_holiday(date("2026-11-18"), Some("DE-SN")) && !is_holiday(date("2026-11-18"), Some("DE-BY")));
        assert!(is_holiday(date("2027-11-17"), Some("DE-SN")));
        assert!(!is_holiday(date("2026-11-01"), None) && is_holiday(date("2026-11-01"), Some("DE-RP")));
    }

    #[test]
    fn days_and_dates_agree() {
        assert_eq!(days(date("1970-01-01")), 0);
        assert_eq!(next_day(date("2026-12-31")), date("2027-01-01"));
        assert_eq!(next_day(date("2028-02-28")), date("2028-02-29"));
        assert_eq!(weekday(date("2026-10-29")), 3);
    }
}
