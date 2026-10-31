use crate::tag::{TagError, TagPath};
use base64ct::{Base64, Encoding};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io};

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("failed to read image {path:?}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Tag(#[from] TagError),
}

#[derive(Debug, Clone, Default)]
pub struct Image {
    hash: String,
    filepath: String,
    tags: HashSet<TagPath>,
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        (self.hash == other.hash) && (self.tags == other.tags)
    }
}

impl Image {
    pub fn new(filepath: String) -> Result<Self, ImageError> {
        Self::new_with_tags(filepath, std::iter::empty::<&str>())
    }

    pub fn new_with_tags<T, S>(filepath: String, tags: T) -> Result<Self, ImageError>
    where
        T: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let hash = Self::hash_image(&filepath)?;
        Ok(Self::from_parts(filepath, hash).with_tags(tags)?)
    }

    pub fn from_parts(filepath: String, hash: String) -> Self {
        Image {
            filepath,
            hash,
            tags: HashSet::new(),
        }
    }

    pub fn with_tags<T, S>(mut self, tags: T) -> Result<Self, TagError>
    where
        T: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for tag in tags {
            self.add_tag(tag.as_ref())?;
        }
        Ok(self)
    }

    pub fn get_fp(&self) -> &str {
        &self.filepath[..]
    }

    pub fn get_hash(&self) -> &str {
        &self.hash[..]
    }

    pub fn add_tag(&mut self, t: impl AsRef<str>) -> Result<(), TagError> {
        let tag = TagPath::parse(t.as_ref())?;
        self.tags.insert(tag);
        Ok(())
    }

    pub fn remove_tag(&mut self, t: impl AsRef<str>) -> Result<(), TagError> {
        let tag = TagPath::parse(t.as_ref())?;
        self.tags.remove(&tag);
        Ok(())
    }

    pub fn get_tags(&self) -> &HashSet<TagPath> {
        &self.tags
    }

    pub fn hash_image(fp: &str) -> Result<String, ImageError> {
        let data: Vec<u8> = fs::read(fp).map_err(|source| ImageError::Read {
            path: fp.to_owned(),
            source,
        })?;

        let mut hasher = Sha256::new();

        hasher.update(data);
        let hash = hasher.finalize();
        Ok(Base64::encode_string(&hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error;

    fn fp(name: &str) -> String {
        format!("persistent_test_data/image_db_test/{name}")
    }

    fn test_image(name: &str, tags: &[&str]) -> Result<Image, Box<dyn error::Error>> {
        Ok(Image::new_with_tags(fp(name), tags.iter().copied())?)
    }

    #[test]
    fn add_tag_to_image() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = test_image("a", &[])?;
        let output = test_image("a", &["root"])?;

        input.add_tag("root")?;

        assert_eq!(input, output);
        Ok(())
    }

    #[test]
    fn add_unicode_tag_to_image() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = test_image("a", &[])?;
        let output = test_image("a", &["你好"])?;

        input.add_tag("你好")?;

        assert_eq!(input, output);
        Ok(())
    }

    #[test]
    fn remove_tag_from_image() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = test_image("a", &["root"])?;
        let output = test_image("a", &[])?;

        input.remove_tag("root")?;

        assert_eq!(input, output);
        Ok(())
    }

    #[test]
    fn missing_file_reports_path_and_cause() {
        let path = "persistent_test_data/image_db_test/does_not_exist";
        let boxed = test_image("does_not_exist", &[]).unwrap_err();
        let err = boxed
            .downcast_ref::<ImageError>()
            .expect("expected an ImageError");

        match err {
            ImageError::Read { path: p, source } => {
                assert_eq!(p, path);
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("expected ImageError::Read, got {other:?}"),
        }
        assert_eq!(err.to_string(), format!("failed to read image {path:?}"));
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn invalid_tag_is_rejected() {
        let boxed = test_image("a", &["ok", "root//leaf"]).unwrap_err();
        let err = boxed
            .downcast_ref::<ImageError>()
            .expect("expected an ImageError");

        assert!(
            matches!(err, ImageError::Tag(TagError::Invalid(s)) if s == "root//leaf"),
            "got {err:?}"
        );
        assert_eq!(err.to_string(), "invalid tag path: \"root//leaf\"");
    }

    #[test]
    fn from_parts_then_with_tags() -> Result<(), Box<dyn std::error::Error>> {
        let image = Image::from_parts(String::from("x"), String::from("h")).with_tags(["a/b"])?;
        assert_eq!(image.get_fp(), "x");
        assert_eq!(image.get_hash(), "h");
        assert_eq!(image.get_tags(), &HashSet::from([TagPath::parse("a/b")?]));
        Ok(())
    }
}
