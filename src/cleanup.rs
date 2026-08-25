use regex::Regex;

// ─── Regex cleanup layer ───

pub(crate) struct RegexFixes {
    double_space: Regex,
    broken_newlines: Regex,
}

impl RegexFixes {
    pub(crate) fn new() -> Self {
        Self {
            double_space: Regex::new(r"[ \t]{2,}").unwrap(),
            // ponytail: join only within one OCR box; cross-box reading order belongs to layout code.
            broken_newlines: Regex::new(r"(\p{L}|\p{N})\s*\n\s*(\p{L}|\p{N})").unwrap(),
        }
    }

    pub(crate) fn apply(&self, text: &str) -> String {
        let mut s = text.to_string();
        s = self.double_space.replace_all(&s, " ").to_string();
        s = self.broken_newlines.replace_all(&s, "$1 $2").to_string();
        s.trim().to_string()
    }
}

pub(crate) fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::RegexFixes;

    #[test]
    fn cleanup_is_language_and_symbol_preserving() {
        let fixes = RegexFixes::new();
        assert_eq!(fixes.apply("  Hello   world  "), "Hello world");
        assert_eq!(fixes.apply("line\n\nbreak"), "line break");
        assert_eq!(
            fixes.apply("0 halaman | x ~ y - z"),
            "0 halaman | x ~ y - z"
        );
        assert_eq!(
            fixes.apply("日本語   текст  العربية"),
            "日本語 текст العربية"
        );
    }
}
