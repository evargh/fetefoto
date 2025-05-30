use crate::command::{CommandManager, ConfigData};
use crate::db::ImageDB;
use directories::ProjectDirs;
use std::{
    error, fs,
    path::{Path, PathBuf},
};
use thiserror;

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("No Dir Given")]
    NoDir,
    #[error("Invalid Config Path")]
    BadConfigPath,
    #[error("Invalid Data Path")]
    BadDataPath,
}

pub struct Init {
    dir: PathBuf,
}

impl Init {
    pub fn create(dir: Option<String>) -> Result<Init, Box<dyn error::Error>> {
        match dir {
            Some(str) => {
                let p: &Path = Path::new(&str);
                match p.try_exists() {
                    Ok(val) => {
                        if val {
                            Ok(Init {
                                dir: p.to_path_buf(),
                            })
                        } else {
                            Err(Box::new(InitError::BadDataPath))
                        }
                    }
                    Err(_) => Err(Box::new(InitError::BadDataPath)),
                }
            }
            None => {
                if let Some(proj_dirs) = ProjectDirs::from("", "FeteFoto", "FeteFoto") {
                    let data_dir = proj_dirs.data_dir();
                    let fp = Path::new(data_dir).to_path_buf();
                    fs::create_dir_all(&fp)?;
                    Ok(Init { dir: fp })
                } else {
                    Err(Box::new(InitError::BadDataPath))
                }
            }
        }
    }

    // TODO: ideally, should be:
    //      if config doesn't exist, make it
    //      if config does exist, read it
    pub async fn execute(mut self) -> Result<(), Box<dyn error::Error>> {
        self.dir.push("fetefoto.db");
        let db = ImageDB::create_db(self.dir).await.map_err(Box::new)?;
        db.create_table().await?;

        let cd = ConfigData {
            db_location: Some(db.get_filepath().to_owned()),
        };
        db.drop_connections().await;
        CommandManager::set_config(cd, None)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_command_from_correct_passed_dir() {
        let input_path_string = "persistent_test_data/config_test/correct".to_owned();
        let input_path_buf = PathBuf::from(&input_path_string);
        let dir_data = Init::create(Some(input_path_string)).unwrap();
        let pb = dir_data.dir;

        assert!(pb == input_path_buf);
    }

    #[test]
    fn read_nonexistent_passed_dir() {
        let input_path_string = "test/config_test/dir_does_not_exist".to_owned();
        let dir_err = Init::create(Some(input_path_string));

        match dir_err {
            Err(_) => (),
            _ => panic!(),
        }
    }

    // would need to run an integration test with the database for the execute command
}
