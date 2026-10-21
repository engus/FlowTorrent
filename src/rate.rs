use crate::{
    domain::MIN_LIMIT_BPS,
    error::{AppError, AppResult},
};
use std::num::NonZeroU32;

pub fn parse_rate_mbps(text: &str, unlimited: bool) -> AppResult<Option<NonZeroU32>> {
    if unlimited {
        return Ok(None);
    }
    let text = text.trim();
    let normalized = text.replace(',', ".");
    let mut parts = normalized.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next().unwrap_or_default().trim_end_matches('0');
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(AppError::InvalidRate);
    }
    let whole: u32 = whole.parse().map_err(|_| AppError::InvalidRate)?;
    let fraction_value: u32 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().map_err(|_| AppError::InvalidRate)?
    };
    let multiplier = 10u32.pow(6 - fraction.len() as u32);
    let bps = whole
        .checked_mul(1_000_000)
        .and_then(|value| value.checked_add(fraction_value * multiplier))
        .filter(|value| *value >= MIN_LIMIT_BPS)
        .ok_or(AppError::InvalidRate)?;
    Ok(NonZeroU32::new(bps))
}
