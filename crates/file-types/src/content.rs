//! `FileContent`: text, binary, or absent.
//!
//! A repository holds pictures as readily as source, so raw bytes are not
//! something a caller can use. Classifying them here means every caller does
//! not have to — and every caller would otherwise write the same check.

/// What one version of a file holds, once we know whether it is text.
///
/// `Text` here means *not binary*, which is the only question this type
/// answers. It is unrelated to how a file is shown — that is a `ui::Buffer`,
/// and the two used to share the word.
#[derive(Debug)]
pub enum FileContent {
    Text(String),
    /// Not diffable. Carried rather than discarded so the size can be shown.
    Binary {
        bytes: usize,
    },
    /// The file does not exist on this side — added, or deleted.
    Absent,
}

impl FileContent {
    /// Classifies raw bytes.
    pub fn from_bytes(bytes: Option<Vec<u8>>) -> Self {
        let Some(bytes) = bytes else {
            return FileContent::Absent;
        };
        if is_binary(&bytes) {
            return FileContent::Binary { bytes: bytes.len() };
        }
        match String::from_utf8(bytes) {
            Ok(text) => FileContent::Text(text),
            // Valid UTF-8 is what `&str` requires and what the engine measures
            // columns in. Bytes that are neither NUL-containing nor decodable
            // are rare, and treating them as binary is honest.
            Err(e) => FileContent::Binary {
                bytes: e.into_bytes().len(),
            },
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            FileContent::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The text, or an empty string for a side that has none.
    ///
    /// An added file has no before side, and the diff of "nothing" against
    /// "something" is what makes every line show as added.
    pub fn text_or_empty(&self) -> &str {
        self.text().unwrap_or("")
    }

    pub fn is_binary(&self) -> bool {
        matches!(self, FileContent::Binary { .. })
    }

    pub fn describe(&self) -> String {
        match self {
            FileContent::Text(text) => format!("{} bytes of text", text.len()),
            FileContent::Binary { bytes } => format!("{bytes} bytes, binary"),
            FileContent::Absent => "absent".to_owned(),
        }
    }
}

/// How git decides, and for the same reason: a zero byte cannot appear in text,
/// and scanning the whole of a large file to learn what its first kilobyte
/// already says would be waste.
fn is_binary(bytes: &[u8]) -> bool {
    const SNIFF: usize = 8000;
    bytes.iter().take(SNIFF).any(|b| *b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_byte_means_binary() {
        assert!(FileContent::from_bytes(Some(vec![0x89, b'P', b'N', b'G', 0])).is_binary());
        assert!(!FileContent::from_bytes(Some(b"fn main() {}".to_vec())).is_binary());
    }

    #[test]
    fn a_zero_byte_far_into_a_large_file_is_not_looked_for() {
        // Deliberate: git stops at 8000 bytes too. A file that is text for its
        // first kilobyte is text for our purposes.
        let mut bytes = vec![b'a'; 9000];
        bytes.push(0);
        assert!(!FileContent::from_bytes(Some(bytes)).is_binary());
    }

    #[test]
    fn bytes_that_are_not_utf8_are_treated_as_binary() {
        assert!(FileContent::from_bytes(Some(vec![0xff, 0xfe, 0xfd])).is_binary());
    }

    #[test]
    fn a_missing_side_is_absent_rather_than_empty() {
        // Absent and empty are different: one file was added, the other was
        // always blank.
        assert!(matches!(FileContent::from_bytes(None), FileContent::Absent));
        assert_eq!(FileContent::from_bytes(None).text_or_empty(), "");
    }
}
