use super::super::fuse::FetefotoFS;
use fuser::MountOption;
use std::{
    env, error, fs,
    path::{Path, PathBuf},
};
use thiserror;

#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("No Dir Given")]
    NoDir,
    #[error("Invalid Path")]
    BadPath,
}

pub struct Mount {
    dir: PathBuf,
}

impl Mount {
    pub fn create(dir: Option<String>) -> Result<Mount, Box<dyn error::Error>> {
        match dir {
            Some(str) => {
                let p: &Path = Path::new(&str);
                match p.try_exists() {
                    Ok(val) => {
                        if val {
                            Ok(Mount {
                                dir: p.to_path_buf(),
                            })
                        } else {
                            Err(Box::new(MountError::BadPath))
                        }
                    }
                    Err(_) => Err(Box::new(MountError::BadPath)),
                }
            }
            None => Err(Box::new(MountError::NoDir)),
        }
    }

    pub fn execute(self) -> Result<(), Box<dyn error::Error>> {
        let options = vec![MountOption::RO, MountOption::FSName("Fetefoto".to_string())];
        fuser::mount2(FetefotoFS, &self.dir, &options).unwrap();
        Ok(())
    }
}
