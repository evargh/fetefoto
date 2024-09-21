use sqlx::{migrate::MigrateDatabase, Sqlite};
use std::error;

pub mod image;
use crate::image::Image;
pub mod db;

// try out some test-driven development:
// first, get the Image object off the ground
//      - guideline shell commands:
//          - add <file>
//              - adds file to the database by hash
//          - pull <file>
//              - in a session, pulls a database entry from a file
//          - addtags <tags>
//              - adds tags to the currently pulled file
//          - rmtags <tags>
//              - removes tags from the currently pulled file
//          - rmimage
//              - removes the current image from the database
//          - push
//              - push image changes to the database
//
//      then, make a shell that handles all of these commands (instead of requiring a rerun)

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {

    Ok(())
}

//      - an image should be added to the database, and then removed. the database should not change
//      (i can write an in-memory database, populate it, and then print out the select. then the
//      image is added and removed)
//      - an image should be added with no tags, and then tags should be added
//      - an image should be added with no tags, and then tags should be added, then removed
//      - an image that already exists should be attempted to be added, and fail
//      - an image that does not exist should be attempted to be removed, and fail
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_image_to_database() {

    }

    #[test]
    fn remove_image_from_database() {

    }

    #[test]
    fn add_then_remove_image() {

    }

    #[test]
    fn add_existing_image() {

    }

    #[test]
    fn remove_nonexistant_image() {

    }
}
