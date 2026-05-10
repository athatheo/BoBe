pub(crate) fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::truncate_str;

    #[test]
    fn ascii_within_limit() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn ascii_truncated() {
        assert_eq!(truncate_str("hello world", 5), "hello");
    }

    #[test]
    fn empty_string() {
        assert_eq!(truncate_str("", 100), "");
    }

    #[test]
    fn multibyte_boundary() {
        let s = "a€b";
        assert_eq!(truncate_str(s, 2), "a");
        assert_eq!(truncate_str(s, 4), "a€");
    }

    #[test]
    fn emoji_boundary() {
        let s = "hi👋ok";
        assert_eq!(truncate_str(s, 3), "hi");
        assert_eq!(truncate_str(s, 6), "hi👋");
    }

    #[test]
    fn zero_limit() {
        assert_eq!(truncate_str("hello", 0), "");
    }
}
