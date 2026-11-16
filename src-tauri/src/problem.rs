//! A user-facing failure: a short title and the one thing to do about it.
//!
//! Each error source builds its own `Problem`, so the overlay and Settings show
//! it as written. `detail` keeps the raw cause for logs and history.

use serde::Serialize;
use std::fmt;
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct Problem {
    pub title: String,
    pub fix: String,
    pub detail: Option<String>,
}

impl Problem {
    pub fn new(title: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            fix: fix.into(),
            detail: None,
        }
    }

    pub fn because(mut self, detail: impl fmt::Display) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    /// Fold a second problem into this one, keeping both fixes visible.
    pub fn and(mut self, other: Problem) -> Self {
        self.fix = format!(
            "{} Also, {}: {}",
            self.fix,
            other.title.to_lowercase(),
            other.fix
        );
        self.detail = match (self.detail, other.detail) {
            (Some(a), Some(b)) => Some(format!("{a}; {b}")),
            (a, b) => a.or(b),
        };
        self
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}. {}", self.title, self.fix)?;
        if let Some(detail) = &self.detail {
            write!(f, " ({detail})")?;
        }
        Ok(())
    }
}

impl From<Problem> for String {
    fn from(problem: Problem) -> Self {
        problem.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_keeps_title_fix_and_cause_apart() {
        let problem = Problem::new("Speech model missing", "Download ggml-base.en.bin.")
            .because("no file at models/ggml-base.en.bin");
        assert_eq!(
            problem.to_string(),
            "Speech model missing. Download ggml-base.en.bin. (no file at models/ggml-base.en.bin)"
        );
    }

    #[test]
    fn typescript_contract_matches_rust_problem() {
        let checked_in = include_str!("../../src/problem.generated.ts");
        let compact = |text: &str| {
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        };
        assert_eq!(
            compact(checked_in),
            compact(&format!("export {}\n", Problem::decl())).replace(',', ";")
        );
    }
}
