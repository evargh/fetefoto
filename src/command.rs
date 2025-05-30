pub mod init;
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
    #[error("Attempting to Push Bad Config Data")]
    BadConfigPush,
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

            None => Err(Box::new(CommandError::NoSuchCommand)),
            _ => Err(Box::new(CommandError::NoSuchCommand)),
        }
    }

    pub fn get_config(dir_override: Option<&PathBuf>) -> ConfigData {
        let mut temp_data = ConfigData { db_location: None };
        // parse the config file based on simple string matching
        let file;
        if let Some(override_dir) = dir_override {
            let mut fp = override_dir.clone();
            fp.push("config");
            file = fs::read_to_string(fp).expect("Failed to open config file.");
        } else {
            if let Some(proj_dirs) = ProjectDirs::from("", "FeteFoto", "FeteFoto") {
                let config_dir = proj_dirs.config_dir();
                let mut fp = Path::new(config_dir).to_path_buf();
                fp.push("config");
                file = fs::read_to_string(fp).expect("Failed to open config file.");
            } else {
                panic!("Invalid Config.");
            }
        }
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
                _ => {}
            }
        }
        return temp_data;
    }

    pub fn set_config(
        d: ConfigData,
        dir_override: Option<&PathBuf>,
    ) -> Result<(), Box<dyn error::Error>> {
        // create a file just called config which stores key-value pairs
        let mut file;
        if let Some(override_dir) = dir_override {
            let mut fp = override_dir.clone();
            fp.push("config");
            file = fs::File::create(fp)?;
        } else {
            if let Some(proj_dirs) = ProjectDirs::from("", "FeteFoto", "FeteFoto") {
                let config_dir = proj_dirs.config_dir();
                let mut fp = Path::new(config_dir).to_path_buf();
                fp.push("config");
                file = fs::File::create(fp)?;
            } else {
                return Err(Box::new(CommandError::BadConfigPush));
            }
        }

        file.write_all(
            format!(
                "db_location: {}",
                d.db_location.unwrap().as_os_str().to_str().unwrap()
            )
            .as_bytes(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // test reading the config file properly
    #[test]
    fn read_correct_config() -> Result<(), Box<dyn error::Error>> {
        let path_buf = PathBuf::from("persistent_test_data/config_test/correct");
        let config_data = CommandManager::get_config(Some(&path_buf));
        if let Some(location) = config_data.db_location {
            let true_path = PathBuf::from("/some/path");
            assert!(location == true_path);
        } else {
            panic!("db_config was None");
        }
        Ok(())
    }

    // should return a None if the pair doesn't exist
    #[test]
    fn read_empty_config() -> Result<(), Box<dyn error::Error>> {
        let path_buf = PathBuf::from("persistent_test_data/config_test/empty_config");
        let config_data = CommandManager::get_config(Some(&path_buf));
        if let Some(_location) = config_data.db_location {
            panic!()
        } else {
            return Ok(());
        }
    }

    // should return a None if the pair doesn't exist
    #[test]
    #[should_panic]
    fn read_config_with_extra_entry() {
        let path_buf = PathBuf::from("persistent_test_data/config_test/extra_config");
        CommandManager::get_config(Some(&path_buf));
    }

    #[test]
    #[should_panic]
    fn read_nonexistent_config() {
        let path_buf = PathBuf::from("persistent_test_data/config_test/dir_no_config");
        CommandManager::get_config(Some(&path_buf));
    }
    // test writing to config properly (pass non-unicode characters and see)
    // test writing to a place with bad permissions
}
