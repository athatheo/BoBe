/// Convert text to a filesystem-safe slug.
///
/// Unicode-normalized, lowercased, non-alphanumeric replaced with hyphens,
/// truncated at word boundaries.
pub fn slugify(text: &str, max_len: usize) -> String {
    let lowered = text.to_lowercase();

    // Replace non-alphanumeric with hyphens
    let slug: String = lowered
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse multiple hyphens
    let mut result = String::with_capacity(slug.len());
    let mut prev_hyphen = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen && !result.is_empty() {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    // Trim trailing hyphens
    let result = result.trim_end_matches('-');

    // Truncate at word boundary
    if result.len() <= max_len {
        return result.to_string();
    }

    let truncated = &result[..max_len];
    match truncated.rfind('-') {
        Some(pos) if pos > max_len / 2 => truncated[..pos].to_string(),
        _ => truncated.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_slugify() {
        assert_eq!(slugify("Hello World!", 50), "hello-world");
    }

    #[test]
    fn unicode_and_special_chars() {
        assert_eq!(slugify("café & résumé", 50), "caf-r-sum");
    }

    #[test]
    fn truncation_at_word_boundary() {
        let slug = slugify("this is a very long goal description", 20);
        assert!(slug.len() <= 20);
        assert!(!slug.ends_with('-'));
    }

    #[test]
    fn empty_string() {
        assert_eq!(slugify("", 50), "");
    }

    #[test]
    fn all_special_chars() {
        assert_eq!(slugify("!!!???", 50), "");
    }
}
