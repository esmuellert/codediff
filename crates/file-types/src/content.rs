//! File content classified as text, binary, or absent.

/// Content of one file version.
#[derive(Debug)]
pub enum FileContent {
    Text(String),
    /// Binary content and its byte length.
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
            // Non-UTF-8 content is treated as binary.
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

/// Returns true when the first 8000 bytes contain a NUL.
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
        // Absent and empty are distinct states.
        assert!(matches!(FileContent::from_bytes(None), FileContent::Absent));
        assert_eq!(FileContent::from_bytes(None).text_or_empty(), "");
    }
}
