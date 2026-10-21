use crate::error::{AppError, AppResult};
/// Bound recursive work before passing untrusted bytes to the engine's serde
/// decoder. String payloads (including binary piece hashes) remain opaque.
pub(super) fn check_nesting(bytes: &[u8]) -> AppResult<()> {
    let invalid =
        || AppError::InvalidMetadata("Malformed or excessively nested torrent metadata".into());
    let mut index = 0usize;
    let mut depth = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'd' | b'l' => {
                depth += 1;
                if depth > 128 {
                    return Err(invalid());
                }
                index += 1;
            }
            b'e' => {
                depth = depth.checked_sub(1).ok_or_else(invalid)?;
                index += 1;
            }
            b'i' => {
                index += 1;
                let end = bytes[index..]
                    .iter()
                    .position(|byte| *byte == b'e')
                    .ok_or_else(invalid)?;
                index += end + 1;
            }
            b'0'..=b'9' => {
                let mut length = 0usize;
                while let Some(digit @ b'0'..=b'9') = bytes.get(index) {
                    length = length
                        .checked_mul(10)
                        .and_then(|n| n.checked_add(usize::from(*digit - b'0')))
                        .ok_or_else(invalid)?;
                    index += 1;
                }
                if bytes.get(index) != Some(&b':') {
                    return Err(invalid());
                }
                index = index
                    .checked_add(1)
                    .and_then(|n| n.checked_add(length))
                    .filter(|n| *n <= bytes.len())
                    .ok_or_else(invalid)?;
            }
            _ => return Err(invalid()),
        }
    }
    if depth != 0 {
        return Err(invalid());
    }
    Ok(())
}
