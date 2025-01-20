use base64ct::{Base64, Encoding};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs};

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("Failed to Read Image")]
    ReadFail,
}

#[derive(Debug)]
pub struct Image {
    hash: String,
    filepath: String,
    tags: HashSet<String>,
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        (&self.hash == &other.hash) && (&self.tags == &other.tags)
    }
}

impl Image {
    pub fn new(filepath: String) -> Result<Image, ImageError> {
        if let Ok(hs) = Image::hash_image(&filepath) {
            Ok(Image {
                filepath,
                hash: hs,
                tags: HashSet::default(),
            })
        } else {
            Err(ImageError::ReadFail)
        }
    }

    pub fn new_with_tags(filepath: String, tags: HashSet<String>) -> Result<Image, ImageError> {
        if let Ok(hs) = Image::hash_image(&filepath) {
            Ok(Image {
                filepath,
                hash: hs,
                tags,
            })
        } else {
            Err(ImageError::ReadFail)
        }
    }

    pub fn get_fp(&self) -> &str {
        &self.filepath[..]
    }

    pub fn get_hash(&self) -> &str {
        &self.hash[..]
    }

    pub fn add_tag(&mut self, t: String) {
        self.tags.insert(t);
    }

    pub fn remove_tag(&mut self, t: &str) {
        self.tags.remove(t);
    }

    pub fn get_tags(&self) -> &HashSet<String> {
        &self.tags
    }

    pub fn hash_image(fp: &str) -> Result<String, ImageError> {
        let data: Vec<u8> = fs::read(fp).map_err(|_| ImageError::ReadFail)?;
        let mut hasher = Sha256::new();

        hasher.update(data);
        let hash = hasher.finalize();
        Ok(Base64::encode_string(&hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_tag_to_image() -> Result<(), Box<dyn std::error::Error>> {
        let mut input: Image = Image::new(String::from("test/abc.gif"))?;
        let output: Image = Image::new_with_tags(
            String::from("test/abc.gif"),
            HashSet::from([String::from("hi")]),
        )?;

        input.add_tag(String::from("hi"));

        assert_eq!(input, output);
        Ok(())
    }

    #[test]
    fn add_unicode_tag_to_image() -> Result<(), Box<dyn std::error::Error>> {
        let mut input: Image = Image::new(String::from("test/abc.gif"))?;
        let output: Image = Image::new_with_tags(
            String::from("test/abc.gif"),
            HashSet::from([String::from("你好")]),
        )?;

        input.add_tag(String::from("你好"));

        assert_eq!(input, output);
        Ok(())
    }

    #[test]
    fn remove_tag_from_image() -> Result<(), Box<dyn std::error::Error>> {
        let mut input: Image = Image::new_with_tags(
            String::from("test/abc.gif"),
            HashSet::from([String::from("hi")]),
        )?;
        let output: Image = Image::new(String::from("test/abc.gif"))?;

        input.remove_tag("hi");

        assert_eq!(input, output);
        Ok(())
    }
}
