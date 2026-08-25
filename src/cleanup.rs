use regex::Regex;

// ─── Regex cleanup layer ───

pub(crate) struct RegexFixes {
    double_space: Regex,
    broken_emdash: Regex,
    stray_pipes: Regex,
    stray_tildes: Regex,
    o_before_halaman: Regex,
    broken_newlines: Regex,
}

impl RegexFixes {
    pub(crate) fn new() -> Self {
        Self {
            double_space: Regex::new(r"  +").unwrap(),
            broken_emdash: Regex::new(r"(\w)\s-\s(\w)").unwrap(),
            stray_pipes: Regex::new(r"\s*[|]+\s*").unwrap(),
            stray_tildes: Regex::new(r"\s*[~]+\s*").unwrap(),
            // ponytail: regex crate has no lookahead — match full pattern, replace with capture
            o_before_halaman: Regex::new(r"(?i)\b0(\s*(?:halaman|page))").unwrap(),
            // ponytail: join broken lines is handled per-box, not cross-box
            broken_newlines: Regex::new(r"(\w)\s\n\s*(\w)").unwrap(),
        }
    }

    pub(crate) fn apply(&self, text: &str) -> String {
        let mut s = text.to_string();
        s = self.double_space.replace_all(&s, " ").to_string();
        s = self.broken_emdash.replace_all(&s, "$1 — $2").to_string();
        s = self.stray_pipes.replace_all(&s, " ").to_string();
        s = self.stray_tildes.replace_all(&s, " ").to_string();
        s = self.o_before_halaman.replace_all(&s, "O$1").to_string();
        s = self.broken_newlines.replace_all(&s, "$1 $2").to_string();
        s.trim().to_string()
    }
}

pub(crate) fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
