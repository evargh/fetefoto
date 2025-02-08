use std::error;
use std::path::{Path, PathBuf};
use thiserror;

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("No Dir Given")]
    NoDir,
    #[error("Invalid Path")]
    BadPath,
}

pub struct Scan {
    dir: PathBuf,
}
impl Scan {
    pub fn create_scan(dir: Option<String>) -> Result<Scan, Box<dyn error::Error>> {
        match dir {
            Some(str) => {
                let p: &Path = Path::new(&str);
                match p.try_exists() {
                    Ok(val) => {
                        if val {
                            Ok(Scan {
                                dir: p.to_path_buf(),
                            })
                        } else {
                            Err(Box::new(ScanError::BadPath))
                        }
                    }
                    Err(_) => Err(Box::new(ScanError::BadPath)),
                }
            }
            None => Err(Box::new(ScanError::NoDir)),
        }
    }

    pub fn execute(self) {
        println!("Hello {}!", self.dir.display());
    }
}
