use std::collections::HashSet;
use serde_json::json;
use std::fs;
use sha2::{Sha256, Digest};
use base64ct::{Base64, Encoding};

#[derive(Debug)]
pub struct Image {
    hash: String,
    tags: HashSet<String>,
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        (&self.hash == &other.hash) && (&self.tags == &other.tags)
    }
}

impl Image {
    pub fn new(hash:String) -> Image {
        Image { hash, tags: HashSet::default() }
    }

    pub fn new_with_tags(hash: String, tags:HashSet<String>) -> Image {
        Image { hash, tags }
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

    pub fn tags_to_string(&self) -> String {
        json!(self.tags).to_string()
    }

    pub fn string_to_tags(ts: String) -> Result<HashSet<String>, serde_json::Error> {
        let ret: HashSet<String> = serde_json::from_str(&ts)?;
        Ok(ret)
    }

    pub fn hash_image(fp: String) -> std::io::Result<String> {
        let data: Vec<u8> = fs::read(fp)?;
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
    fn add_tag_to_image() {
        let mut input: Image = Image::new(String::from("abc"));
        let output: Image = Image::new_with_tags(String::from("abc"), HashSet::from([String::from("hi")]));

        input.add_tag(String::from("hi"));

        assert_eq!(input, output);

    }

    #[test]
    fn remove_tag_from_image() {
        let mut input: Image = Image::new_with_tags(String::from("abc"), HashSet::from([String::from("hi")]));
        let output: Image = Image::new(String::from("abc"));

        input.remove_tag("hi");

        assert_eq!(input, output);
    }

    #[test]
    fn compare_tags_to_string() {
        let input: Image = Image::new_with_tags(String::from("abc"), HashSet::from([String::from("a"), String::from("b"), String::from("c")]));
        let data = "[\"b\",\"c\",\"a\"]";

        assert_eq!(input.tags_to_string(), data);
    }
}
