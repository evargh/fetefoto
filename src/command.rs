pub mod init;
pub mod mount;
pub mod scan;
use clap::Subcommand;
use std::error;
use std::fmt;

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

#[derive(Subcommand, Debug)]
pub enum CommandType {
    Init { dir: Option<String> },
    Scan { dir: Option<String> },
    Pull,
    AddTags,
    RmTags,
    RmImage,
    Mount { dir: Option<String> },
}

impl CommandType {
    pub async fn parse_command(inp: Option<CommandType>) -> Result<(), Box<dyn error::Error>> {
        match inp {
            Some(CommandType::Scan { dir }) => {
                let comm = scan::Scan::create(dir)?;
                comm.execute().await.map(Box::new)?;
                Ok(())
            }
            Some(CommandType::Init { dir }) => {
                let comm = init::Init::create(dir)?;
                comm.execute().await.map(Box::new)?;
                Ok(())
            }
            Some(CommandType::Mount { dir }) => {
                let comm = mount::Mount::create(dir)?;
                comm.execute()?;
                Ok(())
            }

            None => Err(Box::new(CommandError::NoSuchCommand)),
            _ => Err(Box::new(CommandError::NoSuchCommand)),
        }
    }
}

#[cfg(test)]
mod tests {
    // TODO: figuring out testing
}
