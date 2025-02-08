use crate::db::ImageDB;
use std::{
    env, error, fs,
    path::{Path, PathBuf},
};
use thiserror;

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("No Dir Given")]
    NoDir,
    #[error("Invalid Path")]
    BadPath,
}

pub struct Init {
    dir: PathBuf,
}

impl Init {
    pub fn create_scan(dir: Option<String>) -> Result<Init, Box<dyn error::Error>> {
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
                            Err(Box::new(InitError::BadPath))
                        }
                    }
                    Err(_) => Err(Box::new(InitError::BadPath)),
                }
            }
            None => {
                let home_dir = env::var("HOME");
                match home_dir {
                    Ok(path) => {
                        let mut fp = Path::new(&path).to_path_buf();
                        fp.push(".fetefoto");
                        Ok(Init { dir: fp })
                    }
                    Err(_) => Err(Box::new(InitError::BadPath)),
                }
            }
        }
    }

    pub async fn execute(mut self) -> Result<(), Box<dyn error::Error>> {
        fs::create_dir_all(&self.dir)?;
        self.dir.push("fetefoto.db");
        println!("{}", self.dir.display());
        let filename = format!("sqlite://{}", self.dir.display());
        ImageDB::create_db(&filename).await.map_err(Box::new)?;

        let mut idb = ImageDB::new(&filename).await?;
        idb.create_table().await?;
        Ok(())
    }
}
