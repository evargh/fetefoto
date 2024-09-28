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
