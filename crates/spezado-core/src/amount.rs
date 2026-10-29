// SPDX-License-Identifier: AGPL-3.0-or-later
//! Amounts as people write them – `12.40`, `-3`, `1234.5` – and as the protocol counts them, in base units.

use librotenado::{Currency, CurrencyTable, Date, Decimal, Money};

/// Why an amount could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AmountError {
    /// The currency is not in the currency table, or does not divide into decimal places.
    #[error("{0} is not a currency with decimal places")]
    Currency(Currency),
    /// Not a number: an optional `-`, digits, and optionally `.` and more digits.
    #[error("{0:?} is not an amount")]
    Syntax(String),
    /// More places than the currency has.
    #[error("{amount:?} has more places than {currency} ({places})")]
    Places {
        /// The amount as written.
        amount: String,
        /// The currency.
        currency: Currency,
        /// Its places.
        places: u32,
    },
    /// Too large to be counted.
    #[error("{0:?} is too large")]
    Overflow(String),
}

/// The places of a currency after the decimal point, from the currency table.
fn places(currency: &Currency) -> Result<u32, AmountError> {
    CurrencyTable::v0_1()
        .get(currency)
        .and_then(|definition| definition.decimals())
        .ok_or_else(|| AmountError::Currency(currency.clone()))
}

/// Reads an amount written with a decimal point (`12.40`, `-3`) in `currency`.
pub fn parse_amount(text: &str, currency: &Currency) -> Result<Money, AmountError> {
    let places = places(currency)?;
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let is_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() || !is_digits(whole) || !is_digits(fraction) || (digits.contains('.') && fraction.is_empty()) {
        return Err(AmountError::Syntax(text.to_owned()));
    }
    if fraction.len() > places as usize {
        return Err(AmountError::Places { amount: text.to_owned(), currency: currency.clone(), places });
    }
    let padded = format!("{whole}{fraction:0<width$}", width = places as usize);
    let units: i128 = padded.parse().map_err(|_| AmountError::Overflow(text.to_owned()))?;
    Money::new(if negative { -units } else { units }, currency.clone())
        .ok_or_else(|| AmountError::Overflow(text.into()))
}

/// Splits a gross amount at a VAT rate in percent into the net amount and the VAT: the net amount is the gross divided
/// by 1 + rate, rounded half away from zero to base units; the VAT is the rest, so that both add up to the gross.
pub fn split_gross(gross: &Money, rate: Decimal) -> Option<(Money, Money)> {
    let scale = 10i128.checked_pow(u32::from(rate.scale()))?;
    // gross · 100 / (100 + rate), with the rate as units · 10^-scale.
    let numerator = gross.base_units().checked_mul(100)?.checked_mul(scale)?;
    let denominator = scale.checked_mul(100)?.checked_add(rate.units())?;
    if denominator <= 0 {
        return None;
    }
    let (quotient, remainder) = (numerator / denominator, numerator % denominator);
    let net = if remainder.abs() * 2 >= denominator { quotient + numerator.signum() } else { quotient };
    let currency = gross.currency().clone();
    Some((Money::new(net, currency.clone())?, Money::new(gross.base_units() - net, currency)?))
}

/// Writes an amount with a decimal point, as many places as its currency has (`12.40`).
pub fn format_amount(money: &Money) -> String {
    let units = money.base_units();
    let Ok(places) = places(money.currency()) else { return units.to_string() };
    let digits = format!("{:0>width$}", units.unsigned_abs(), width = places as usize + 1);
    let (whole, fraction) = digits.split_at(digits.len() - places as usize);
    let sign = if units < 0 { "-" } else { "" };
    if fraction.is_empty() { format!("{sign}{whole}") } else { format!("{sign}{whole}.{fraction}") }
}

/// Writes an amount as people of a language write it: `1.234,56` in German, `1,234.56` in any other.
pub fn format_amount_in(money: &Money, language: &str) -> String {
    let plain = format_amount(money);
    let (sign, digits) = plain.strip_prefix('-').map_or(("", plain.as_str()), |rest| ("-", rest));
    let (whole, fraction) = digits.split_once('.').map_or((digits, None), |(w, f)| (w, Some(f)));
    let (group, point) = if language.starts_with("de") { ('.', ',') } else { (',', '.') };
    let mut grouped = String::new();
    for (i, digit) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(group);
        }
        grouped.push(digit);
    }
    match fraction {
        Some(fraction) => format!("{sign}{grouped}{point}{fraction}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Writes a date as people of a language write it: `29.10.2026` in German, `2026-10-29` in any other.
pub fn format_date_in(date: Date, language: &str) -> String {
    if language.starts_with("de") {
        format!("{:02}.{:02}.{}", date.day(), date.month(), date.year())
    } else {
        date.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eur() -> Currency {
        "EUR".parse().unwrap()
    }

    #[test]
    fn amounts_are_read_in_base_units() {
        let units = |text: &str| parse_amount(text, &eur()).map(|m| m.base_units());
        assert_eq!(units("12.40"), Ok(1240));
        assert_eq!(units("12.4"), Ok(1240));
        assert_eq!(units("-3"), Ok(-300));
        assert_eq!(units("0.05"), Ok(5));
        assert!(matches!(units("1.234"), Err(AmountError::Places { .. })));
        for wrong in ["", "-", "1.", ".5", "1,50", "1e3", "--1", "+1"] {
            assert!(matches!(units(wrong), Err(AmountError::Syntax(_))), "{wrong}");
        }
        let jpy: Currency = "JPY".parse().unwrap();
        assert_eq!(parse_amount("1200", &jpy).map(|m| m.base_units()), Ok(1200));
        assert!(parse_amount("12.5", &jpy).is_err());
    }

    #[test]
    fn gross_amounts_split_into_net_and_vat() {
        let split = |gross: i128, rate: &str| {
            let (net, vat) = split_gross(&Money::new(gross, eur()).unwrap(), rate.parse().unwrap()).unwrap();
            (net.base_units(), vat.base_units())
        };
        assert_eq!(split(11_900, "19"), (10_000, 1_900));
        assert_eq!(split(10_700, "7"), (10_000, 700));
        assert_eq!(split(100, "19"), (84, 16), "0.84 + 0.16");
        assert_eq!(split(-11_900, "19"), (-10_000, -1_900));
        assert_eq!(split(1_055, "5.5"), (1_000, 55));
        assert_eq!(split(500, "0"), (500, 0));
    }

    #[test]
    fn amounts_are_written_with_the_places_of_their_currency() {
        let money = |units: i128| Money::new(units, eur()).unwrap();
        assert_eq!(format_amount(&money(1240)), "12.40");
        assert_eq!(format_amount(&money(-5)), "-0.05");
        assert_eq!(format_amount(&money(0)), "0.00");
        assert_eq!(format_amount(&Money::new(1200, "JPY".parse().unwrap()).unwrap()), "1200");
        for text in ["12.40", "-0.05", "1000000.00"] {
            assert_eq!(format_amount(&parse_amount(text, &eur()).unwrap()), text);
        }
    }
}
