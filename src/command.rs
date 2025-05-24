pub mod init;
pub mod mount;
pub mod scan;
use clap::Subcommand;
use directories::ProjectDirs;
use pest::Parser;
use pest_derive::Parser;
use std::{
    error, fmt, fs,
    io::prelude::*,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[grammar = "config.pest"]
struct ConfigParser;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Command Not Found")]
    NoSuchCommand,
    #[error("Bad Config Data")]
    BadConfig,
}

impl fmt::Display for CommandManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Subcommand, Debug)]
pub enum CommandManager {
    Init { dir: Option<String> },
    Scan { dir: Option<String> },
    Pull,
    AddTags,
    RmTags,
    RmImage,
    Mount { dir: Option<String> },
}

pub struct ConfigData {
    db_location: Option<PathBuf>,
}

impl CommandManager {
    pub async fn parse_command(inp: Option<CommandManager>) -> Result<(), Box<dyn error::Error>> {
        match inp {
            Some(CommandManager::Scan { dir }) => {
                let comm = scan::Scan::create(dir)?;
                comm.execute().await.map(Box::new)?;
                Ok(())
            }
            Some(CommandManager::Init { dir }) => {
                let comm = init::Init::create(dir)?;
                comm.execute().await.map(Box::new)?;
                Ok(())
            }
            Some(CommandManager::Mount { dir }) => {
                let comm = mount::Mount::create(dir)?;
                comm.execute()?;
                Ok(())
            }

            None => Err(Box::new(CommandError::NoSuchCommand)),
            _ => Err(Box::new(CommandError::NoSuchCommand)),
        }
    }

    pub fn get_config() -> Result<ConfigData, Box<dyn error::Error>> {
        let mut temp_data = ConfigData { db_location: None };
        // parse the config file based on simple string matching
        if let Some(proj_dirs) = ProjectDirs::from("", "FeteFoto", "FeteFoto") {
            let config_dir = proj_dirs.config_dir();
            let mut fp = Path::new(config_dir).to_path_buf();
            fp.push("config");
            let file = fs::read_to_string(fp)?;
            let parsed_config = ConfigParser::parse(Rule::file, &file[..])
                .expect("Parse error")
                .next()
                .unwrap()
                .into_inner();

            println!("config: {:?}", parsed_config);

            for line in parsed_config {
                match line.as_rule() {
                    Rule::db_location_pair => {
                        let mut inner = line.into_inner();
                        let value = inner.next().unwrap().as_str().trim();
                        temp_data.db_location = Some(PathBuf::from(value));
                    }
                    Rule::dummy_pair => {}
                    _ => {}
                }
            }
            return Ok(temp_data);
        } else {
            Err(Box::new(CommandError::BadConfig))
        }
    }

    pub fn set_config(d: ConfigData) -> Result<(), Box<dyn error::Error>> {
        // create a file just called config which stores key-value pairs
        if let Some(proj_dirs) = ProjectDirs::from("", "FeteFoto", "FeteFoto") {
            let config_dir = proj_dirs.config_dir();
            let mut fp = Path::new(config_dir).to_path_buf();
            fp.push("config");
            let mut file = fs::File::create(fp)?;
            file.write_all(
                format!(
                    "db_location: {}",
                    d.db_location.unwrap().as_os_str().to_str().unwrap()
                )
                .as_bytes(),
            )?;
            Ok(())
        } else {
            Err(Box::new(CommandError::BadConfig))
        }
    }
}

#[cfg(test)]
mod tests {
    // TODO: figuring out testing
    // test reading the config file properly (pass a string and see)
    // test writing to config properly (pass non-unicode characters and see)
    // test writing to a place with bad permissions
}
