use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use thiserror;
use crate::image::Image;

pub struct ImageDB {
    filepath: String,
    pool: SqlitePool
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Database Already Exists")]
    DatabaseExists,
    #[error("JSON Parse Error")]
    JSONError,
    #[error("Inherited SQLX Error")]
    SQLXError(sqlx::Error)
}

impl ImageDB {
    pub async fn new(filepath: &str) -> Result<ImageDB, DatabaseError> {
        let pool = SqlitePool::connect(&filepath).await.map_err(|e| DatabaseError::SQLXError(e))?;
        Ok(ImageDB { filepath: String::from(filepath), pool })
    }

    pub fn get_filepath(&self) -> &str {
        &self.filepath
    }

    pub async fn create_db(filepath: &str) -> Result<(), DatabaseError> {

        if !Sqlite::database_exists(filepath)
            .await
            .unwrap_or(false) {
                Sqlite::create_database(filepath).await.map_err(|e| DatabaseError::SQLXError(e))?;
                Ok(())
        }
        else {
            Err(DatabaseError::DatabaseExists)
        }
    }

    pub async fn create_table(&mut self) -> Result<(), DatabaseError> {
        let mut db = self.pool.acquire().await.map_err(|e| DatabaseError::SQLXError(e))?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS images (
                id   INTEGER PRIMARY KEY NOT NULL, 
                hash TEXT NOT NULL,
                tags TEXT NOT NULL
            )").execute(&mut *db).await.map_err(|e| DatabaseError::SQLXError(e))?;
        Ok(())
    }

    pub async fn add_images_to_db<'a>(&mut self, ims: impl IntoIterator<Item=&'a Image>) -> Result<(), DatabaseError> {
        let mut db = self.pool.acquire().await.map_err(|e| DatabaseError::SQLXError(e))?;

        let pairs = &ims.into_iter().map(|x| format!("(\'{}\', \'{}\')", x.get_hash(), x.tags_to_string())).collect::<Vec<String>>().join(" ")[..];
        println!("{}", pairs);
        sqlx::query(&format!("INSERT INTO images (hash, tags) VALUES {};", pairs)[..])
            .execute(&mut *db)
            .await
            .map_err(|e| DatabaseError::SQLXError(e))?;

        Ok(())
    }

    pub async fn get_images_from_db<'a>(&self, hs: impl IntoIterator<Item=&'a str>) -> Result<(), DatabaseError> {
        let mut db = self.pool.acquire().await.map_err(|e| DatabaseError::SQLXError(e))?;

        let pairs = &hs.into_iter().collect::<Vec<&str>>().join(" OR ")[..];
        println!("{}", pairs);
        // TODO: FIX THIS
        sqlx::query(&format!("SELECT id, name FROM users WHERE {} ", pairs)[..])
            .execute(&mut *db)
            .await
            .map_err(|e| DatabaseError::SQLXError(e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{error, collections::HashSet};

    // I can't directly test some of the database logic, so some of the tests will have
    // dependencies
    // I'm going to just make one integration test

    #[tokio::test]
    async fn create_new_database() -> Result<(), Box<dyn error::Error>>{
        let filename = "sqlite://fetefoto.db";
        println!("creating file");
        ImageDB::create_db(filename).await?;

        println!("creating object");
        let mut db = ImageDB::new(filename).await?;

        println!("creating table");
        db.create_table().await?;
 
        println!("adding image");
        let output: Image = Image::new_with_tags(String::from("abc"), HashSet::from([String::from("hi")]));
        db.add_images_to_db(std::iter::once(&output)).await?;

        //fs::remove_file("fetefoto.db")?;
        Ok(())
    }

    #[test]
    fn remove_images_from_database() {

    }

    #[test]
    fn update_images_in_database() {

    }
 
    #[test]
    fn select_images_from_database() {

    }
}
