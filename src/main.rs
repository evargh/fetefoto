use crate::command::Command;
use std::{error, io, io::Write};

pub mod command;
pub mod db;
pub mod image;

// try out some test-driven development:
// first, get the Image object off the ground
//
//      then, make a shell that handles all of these commands (instead of requiring a rerun)

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let mut child = Command::parse_command(input)?;

        print!("{}", child);
    }
}
