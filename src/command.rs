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

use enum_iterator;
use std::{ffi::OsString, fmt};

// all commands need to implement a few traits:
// CanSpawn: dictates a way, with args, that the command can be run
// ToString: allows for easy matching of commands with string inputs

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Command Not Found")]
    NoSuchCommand,
}

impl fmt::Display for CommandType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(enum_iterator::Sequence, Debug)]
pub enum CommandType {
    Scan,
    Pull,
    AddTags,
    RmTags,
    RmImage,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {:?}", self.comm, self.args)
    }
}

pub struct Command {
    comm: CommandType,
    args: Vec<String>,
}

impl Command {
    pub fn parse_command(inp: impl AsRef<str>) -> Result<Command, CommandError> {
        let mut parts = inp.as_ref().trim().split_whitespace();
        let command = parts.next().unwrap();
        let args = parts;

        let command = enum_iterator::all::<CommandType>()
            .find(|x| x.to_string().to_lowercase().split("::").last().unwrap() == command);

        match command {
            Some(real) => Ok(Command {
                comm: real,
                args: args.map(|x| x.to_string()).collect::<Vec<String>>(),
            }),
            None => Err(CommandError::NoSuchCommand),
        }
    }

    pub fn spawn(self) -> Result<(), CommandError> {
        // make it so that there is some trait that needs to be implemented for spawning
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generate_command() -> Result<(), Box<dyn std::error::Error>> {
        let co = "scan /";
        let comm = Command::parse_command(co)?;
        print!("{}", comm.to_string());
        assert!(&comm.to_string()[..] == "Scan: [/]");
        Ok(())
    }
}
