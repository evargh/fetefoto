use crate::db::ImageDB;
use crate::image::Image;
use std::collections::VecDeque;
use std::error;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror;

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("No Dir Given")]
    NoDir,
    #[error("Invalid Path")]
    BadPath,
    #[error("File Error")]
    FileError,
}

pub struct Scan {
    dir: PathBuf,
}
impl Scan {
    pub fn create(dir: Option<String>) -> Result<Scan, Box<dyn error::Error>> {
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

    pub async fn execute(self) -> Result<(), ScanError> {
        // needs to read the path to the database
        let mut q: VecDeque<PathBuf> = VecDeque::new();
        if !self.dir.is_dir() {
            return Err(ScanError::BadPath);
        }
        q.push_back(self.dir);
        // This function should go through the directory and scan all files into the database
        while q.front().is_some() {
            let dir = q.pop_front().unwrap();
            for entry_result in fs::read_dir(dir).map_err(|_| ScanError::BadPath)? {
                let entry = entry_result.map_err(|_| ScanError::BadPath)?;
                let path = entry.path();

                if path.is_dir() {
                    q.push_back(path);
                } else if path.is_file() {
                    println!("File: {}", path.display());
                    let output: Image = Image::new(String::from(format!("{}", path.display())))
                        .map_err(|_| ScanError::FileError)?;
                    //db.add_images_to_db(std::iter::once(&output)).await?;
                }
            }
        }
        Ok(())
    }
}
