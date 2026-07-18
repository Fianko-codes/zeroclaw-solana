//! Exact decimal-string amount conversion. No floating-point values are used.

use std::fmt;

/// A `u64` can represent powers of ten only through 10^19.
pub const MAX_SUPPORTED_DECIMALS: u8 = 19;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmountError {
    Empty,
    InvalidSyntax,
    TooManyFractionalDigits { provided: usize, allowed: u8 },
    UnsupportedDecimals(u8),
    Overflow,
}

impl fmt::Display for AmountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("amount is empty"),
            Self::InvalidSyntax => formatter.write_str(
                "amount must be unsigned decimal digits with an optional fractional part",
            ),
            Self::TooManyFractionalDigits { provided, allowed } => write!(
                formatter,
                "amount has {provided} fractional digits; mint allows {allowed}"
            ),
            Self::UnsupportedDecimals(decimals) => write!(
                formatter,
                "mint decimals {decimals} exceed the supported u64 scale limit of {MAX_SUPPORTED_DECIMALS}"
            ),
            Self::Overflow => formatter.write_str("amount exceeds u64::MAX base units"),
        }
    }
}

impl std::error::Error for AmountError {}

/// Convert an exact UI amount such as `"25.01"` into raw base units.
///
/// Leading zeroes are accepted, but signs, exponents, whitespace, `.5`, and
/// `1.` are rejected. Precision beyond the mint's decimals is rejected even
/// when the excess digits are zero.
pub fn parse_ui_amount(input: &str, decimals: u8) -> Result<u64, AmountError> {
    if input.is_empty() {
        return Err(AmountError::Empty);
    }
    let scale = decimal_scale(decimals)?;

    let mut pieces = input.split('.');
    let whole = pieces.next().ok_or(AmountError::InvalidSyntax)?;
    let fraction = pieces.next();
    if pieces.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AmountError::InvalidSyntax);
    }

    let whole_value = parse_digits(whole.as_bytes())?;
    let mut raw = whole_value
        .checked_mul(scale)
        .ok_or(AmountError::Overflow)?;

    if let Some(fraction) = fraction {
        if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(AmountError::InvalidSyntax);
        }
        if fraction.len() > usize::from(decimals) {
            return Err(AmountError::TooManyFractionalDigits {
                provided: fraction.len(),
                allowed: decimals,
            });
        }

        let mut fractional_value = parse_digits(fraction.as_bytes())?;
        for _ in fraction.len()..usize::from(decimals) {
            fractional_value = fractional_value
                .checked_mul(10)
                .ok_or(AmountError::Overflow)?;
        }
        raw = raw
            .checked_add(fractional_value)
            .ok_or(AmountError::Overflow)?;
    }

    Ok(raw)
}

/// Format raw base units as a canonical UI amount without redundant zeroes.
pub fn format_ui_amount(raw: u64, decimals: u8) -> Result<String, AmountError> {
    let scale = decimal_scale(decimals)?;
    if decimals == 0 {
        return Ok(raw.to_string());
    }

    let whole = raw / scale;
    let remainder = raw % scale;
    if remainder == 0 {
        return Ok(whole.to_string());
    }

    let mut fraction = format!("{remainder:0width$}", width = usize::from(decimals));
    while fraction.ends_with('0') {
        fraction.pop();
    }
    Ok(format!("{whole}.{fraction}"))
}

fn decimal_scale(decimals: u8) -> Result<u64, AmountError> {
    if decimals > MAX_SUPPORTED_DECIMALS {
        return Err(AmountError::UnsupportedDecimals(decimals));
    }
    let mut scale = 1_u64;
    for _ in 0..decimals {
        scale = scale.checked_mul(10).ok_or(AmountError::Overflow)?;
    }
    Ok(scale)
}

fn parse_digits(digits: &[u8]) -> Result<u64, AmountError> {
    let mut value = 0_u64;
    for digit in digits {
        value = value
            .checked_mul(10)
            .and_then(|current| current.checked_add(u64::from(digit - b'0')))
            .ok_or(AmountError::Overflow)?;
    }
    Ok(value)
}
