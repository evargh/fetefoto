use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use std::collections::HashSet;
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
    pub async fn new(filepath: String) -> Result<ImageDB, DatabaseError> {
        let pool = SqlitePool::connect(&filepath).await.map_err(|e| DatabaseError::SQLXError(e))?;
        Ok(ImageDB { filepath, pool })
    }

    pub fn get_filepath(&self) -> &str {
        &self.filepath
    }

    pub fn set_filepath(&mut self, fp: String) {
        self.filepath = fp;
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

    pub async fn create_table(&self) -> Result<(), DatabaseError> {
        let mut db = self.pool.acquire().await.map_err(|e| DatabaseError::SQLXError(e))?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS images (
                id   INTEGER PRIMARY KEY NOT NULL, 
                hash TEXT NOT NULL,
                tags TEXT NOT NULL
            )").execute(&mut *db).await.map_err(|e| DatabaseError::SQLXError(e))?;
        Ok(())
    }

    pub async fn add_images_to_db(&self, ims: HashSet<Image>) -> Result<(), DatabaseError> {
        let mut db = self.pool.acquire().await.map_err(|e| DatabaseError::SQLXError(e))?;

        for i in ims {
            sqlx::query("INSERT INTO images (hash, tags) VALUES (?1, ?2)")
                .bind(i.get_hash())
                .bind(i.tags_to_string())
                .execute(&mut *db)
                .await
                .map_err(|e| DatabaseError::SQLXError(e))?;
        }
        Ok(())
    }

    pub async fn get_images_from_db(&self, hs: HashSet<String>) -> Result<(), DatabaseError> {
        let mut db = self.pool.acquire().await.map_err(|e| DatabaseError::SQLXError(e))?;

        for i in hs {
            sqlx::query("SELECT id, hash, tags FROM images WHERE hash=?1")
                .bind(i)
                .execute(&mut *db)
                .await
                .map_err(|e| DatabaseError::SQLXError(e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_new_database() {

    }

    #[test]
    fn create_table_in_database() {
    }

    #[test]
    fn add_image_to_database() {
    }

    #[test]
    fn remove_image_from_database() {

    }
 
    #[test]
    fn select_images_from_database() {

    }
}
