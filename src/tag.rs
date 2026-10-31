#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagPath(Vec<String>);

#[derive(Debug, thiserror::Error, PartialEq, Hash)]
pub enum TagError {
    #[error("invalid tag path: {0:?}")]
    Invalid(String),
    #[error("tag {tag:?} already has a different parent")]
    ParentConflict { tag: String },
}

impl TagPath {
    pub fn parse(s: &str) -> Result<Self, TagError> {
        let segments: Vec<String> = s.split('/').map(|p| p.trim().to_owned()).collect();
        // verify that all segments are composed of valid characters
        let valid = segments
            .iter()
            .all(|p| !p.is_empty() && !p.contains('"') && !p.chars().any(char::is_control));
        if valid {
            Ok(TagPath(segments))
        } else {
            Err(TagError::Invalid(s.to_owned()))
        }
    }

    pub fn segments(&self) -> &[String] {
        &self.0
    }

    pub fn leaf(&self) -> &str {
        self.0
            .last()
            .expect("parse guarantees at least one segment")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_singleton_tag() -> Result<(), Box<dyn std::error::Error>> {
        let input = TagPath::parse("root")?;
        assert_eq!(input.segments(), ["root"]);
        assert_eq!(input.leaf(), "root");
        Ok(())
    }

    #[test]
    fn parse_empty_tag() -> Result<(), Box<dyn std::error::Error>> {
        let input = TagPath::parse("");
        assert_eq!(input, Err(TagError::Invalid(String::new())));
        assert_eq!(input.unwrap_err().to_string(), "invalid tag path: \"\"");
        Ok(())
    }

    #[test]
    fn parse_unicode_tag() -> Result<(), Box<dyn std::error::Error>> {
        let input = TagPath::parse("你好")?;
        assert_eq!(input.segments(), ["你好"]);

        Ok(())
    }

    #[test]
    fn parse_multilevel_unicode_tag() -> Result<(), Box<dyn std::error::Error>> {
        let input = TagPath::parse("世界/café/😀")?;
        assert_eq!(input.segments(), ["世界", "café", "😀"]);
        assert_eq!(input.leaf(), "😀");
        Ok(())
    }

    #[test]
    fn parse_multilevel_tag() -> Result<(), Box<dyn std::error::Error>> {
        let input = TagPath::parse("root/mid/leaf")?;
        assert_eq!(input.segments(), ["root", "mid", "leaf"]);
        assert_eq!(input.leaf(), "leaf");
        Ok(())
    }

    #[test]
    fn parse_trims_whitespace() -> Result<(), Box<dyn std::error::Error>> {
        let input = TagPath::parse("  root / mid tier  ")?;
        assert_eq!(input.segments(), ["root", "mid tier"]);
        Ok(())
    }

    #[test]
    fn parse_gapped_tag() -> Result<(), Box<dyn std::error::Error>> {
        for s in ["root//leaf", "/root", "root/", "/", "root/   /leaf"] {
            assert_eq!(
                TagPath::parse(s),
                Err(TagError::Invalid(s.to_owned())),
                "input {s:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn parse_illegal_tag() -> Result<(), Box<dyn std::error::Error>> {
        // verify quotes, quotes in leaves, and escape characters are all scrubbed
        for s in [
            "say \"hi\"",
            "root/\"leaf\"",
            "new\nline",
            "tab\there",
            "nul\0byte",
        ] {
            assert_eq!(
                TagPath::parse(s),
                Err(TagError::Invalid(s.to_owned())),
                "input {s:?}"
            );
        }

        // verify that any control-related filenames should be valid
        for s in ["Bob's", "a (b)", "AND", "'; DROP TABLE tags; --"] {
            assert!(TagPath::parse(s).is_ok(), "input {s:?} should be valid");
        }
        Ok(())
    }
}
