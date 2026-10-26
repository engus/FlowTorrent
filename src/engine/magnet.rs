use crate::error::{AppError, AppResult};

pub fn normalize_magnet(link: &str) -> AppResult<String> {
    let invalid = || AppError::Magnet("Invalid magnet link".into());
    if link.len() > 16 * 1024 {
        return Err(invalid());
    }
    let source = url::Url::parse(link.trim()).map_err(|_| invalid())?;
    if source.scheme() != "magnet" {
        return Err(invalid());
    }
    let mut normalized = url::Url::parse("magnet:").unwrap();
    {
        let mut query = normalized.query_pairs_mut();
        // In particular, never pass `so` to librqbit's range-expanding parser.
        // File selection belongs to the confirmation preview, not the link.
        for (key, value) in source.query_pairs() {
            if matches!(key.as_ref(), "xt" | "dn" | "tr") {
                query.append_pair(&key, &value);
            }
        }
    }
    let parsed = librqbit::Magnet::parse(normalized.as_str()).map_err(|_| invalid())?;
    if parsed.as_id20().is_none() {
        return Err(AppError::UnsupportedFormat);
    }
    Ok(normalized.into())
}
