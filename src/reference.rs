//! The string inserted on `Enter`: `@"<absolute path>"` (FR11).

use std::path::Path;

/// Build the insertion string: a literal `@`, then the absolute path wrapped in
/// double quotes. Always quoted, even when the path contains no spaces.
pub fn at_quoted(abs: &Path) -> String {
    format!("@\"{}\"", abs.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn formats_at_quoted_absolute_path() {
        // AC17: literal @, then the double-quoted absolute path.
        let p = PathBuf::from(r"D:\jfuchs\dev\second-brain\note.md");
        assert_eq!(at_quoted(&p), r#"@"D:\jfuchs\dev\second-brain\note.md""#);
    }

    #[test]
    fn always_quotes_even_without_spaces() {
        let p = PathBuf::from(r"C:\a\b.md");
        assert_eq!(at_quoted(&p), r#"@"C:\a\b.md""#);
    }
}
