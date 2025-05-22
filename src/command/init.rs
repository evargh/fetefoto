use crate::db::ImageDB;
use crate::image::Image;
use directories::ProjectDirs;
use std::{
    env, error, fs,
    io::prelude::*,
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

    pub fn init_config(db_name: &str) -> Result<(), Box<dyn error::Error>> {
        // create a file just called config which stores key-value pairs
        if let Some(proj_dirs) = ProjectDirs::from("", "FeteFoto", "FeteFoto") {
            let config_dir = proj_dirs.config_dir();
            let mut fp = Path::new(config_dir).to_path_buf();
            fp.push("config");
            let mut file = fs::File::create(fp)?;
            file.write_all(format!("config: \"{}\"", db_name).as_bytes())?;
            Ok(())
        } else {
            Err(Box::new(InitError::BadConfigPath))
        }
    }

    // TODO: ideally, should be:
    //      if config doesn't exist, make it
    //      if config does exist, read it
    pub async fn execute(mut self) -> Result<(), Box<dyn error::Error>> {
        self.dir.push("fetefoto.db");
        let mut idb = ImageDB::create_db(self.dir).await.map_err(Box::new)?;
        idb.create_table().await?;

        println!("{}", idb.get_filepath());
        Init::init_config(idb.get_filepath())?;
        Ok(())
    }
}
