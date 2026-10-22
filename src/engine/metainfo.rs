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

/// Read only the format markers. Building a generic bencode tree here amplifies
/// large extension lists even though the engine would otherwise ignore them.
pub(super) fn check_supported_format(bytes: &[u8]) -> AppResult<()> {
    use serde::Deserialize;
    check_nesting(bytes)?;
    #[derive(Deserialize)]
    struct Header {
        info: Format,
    }
    #[derive(Deserialize)]
    struct Format {
        #[serde(default, rename = "meta version", deserialize_with = "is_two")]
        v2: bool,
        pieces: Option<serde::de::IgnoredAny>,
    }
    let header: Header = librqbit_bencode::from_bytes(bytes)
        .map_err(|error| AppError::InvalidMetadata(error.to_string()))?;
    if header.info.v2 && header.info.pieces.is_none() {
        return Err(AppError::UnsupportedFormat);
    }
    Ok(())
}

fn is_two<'de, D: serde::Deserializer<'de>>(de: D) -> Result<bool, D::Error> {
    use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
    struct Marker;
    impl<'de> Visitor<'de> for Marker {
        type Value = bool;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a bencode format marker")
        }
        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<bool, E> {
            Ok(value == 2)
        }
        fn visit_bytes<E: serde::de::Error>(self, _: &[u8]) -> Result<bool, E> {
            Ok(false)
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<bool, A::Error> {
            while sequence.next_element::<IgnoredAny>()?.is_some() {}
            Ok(false)
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<bool, A::Error> {
            while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            Ok(false)
        }
    }
    de.deserialize_any(Marker)
}

#[cfg(test)]
mod format_tests {
    use super::*;

    #[test]
    fn pure_v2_is_unsupported_but_hybrid_keeps_the_v1_path() {
        assert!(matches!(
            check_supported_format(b"d4:infod12:meta versioni2eee"),
            Err(AppError::UnsupportedFormat)
        ));
        assert!(check_supported_format(b"d4:infod12:meta versioni2e6:pieces0:ee").is_ok());
    }

    #[test]
    fn non_integer_markers_remain_ignored_extensions() {
        for bytes in [
            b"d4:infod12:meta version4:v2??ee".as_slice(),
            b"d4:infod12:meta versionli2eeee".as_slice(),
            b"d4:infod12:meta versiond1:ai2eeee".as_slice(),
        ] {
            assert!(check_supported_format(bytes).is_ok());
        }
    }
}
