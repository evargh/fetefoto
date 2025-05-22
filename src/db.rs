use crate::ast::AST;
use crate::image::Image;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror;

#[derive(Debug, Default, sqlx::FromRow)]
#[sqlx(default)]
pub struct ImageRow {
    pub hash: Option<String>,
    pub fp: Option<String>,
    pub name: Option<String>,
}

impl ImageRow {
    pub fn same_image(&self, other: &ImageRow) -> bool {
        self.hash == other.hash
    }
}

pub struct ImageDB {
    filepath: PathBuf,
    pool: SqlitePool,
}

pub enum FilterType {
    AND,
    OR,
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Database Already Exists")]
    DatabaseExists,
    #[error("JSON Parse Error")]
    JSONError,
    #[error("Inherited SQLX Error")]
    SQLXError(sqlx::Error),
}

impl ImageDB {
    pub fn get_filepath(&self) -> &str {
        &self.filepath.as_os_str().to_str().unwrap()[..]
    }

    pub async fn create_db(filepath: PathBuf) -> Result<ImageDB, DatabaseError> {
        let path_string = format!("sqlite://{}", filepath.as_os_str().to_str().unwrap());
        println!("reported by creation: {}", path_string);
        if !Sqlite::database_exists(&path_string).await.unwrap_or(false) {
            Sqlite::create_database(&path_string)
                .await
                .map_err(DatabaseError::SQLXError)?;
            let pool = SqlitePool::connect(&path_string)
                .await
                .map_err(DatabaseError::SQLXError)?;
            Ok(ImageDB { filepath, pool })
        } else {
            Err(DatabaseError::DatabaseExists)
        }
    }

    pub async fn create_table(&mut self) -> Result<(), DatabaseError> {
        let mut db = self
            .pool
            .acquire()
            .await
            .map_err(DatabaseError::SQLXError)?;

        // TODO: make it so that this pragma is always applied, since it's only connection-specific
        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&mut *db)
            .await
            .map_err(DatabaseError::SQLXError)?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS images (
                id   INTEGER PRIMARY KEY NOT NULL,
                fp   TEXT NOT NULL UNIQUE,
                hash TEXT NOT NULL UNIQUE
            );",
        )
        .execute(&mut *db)
        .await
        .map_err(DatabaseError::SQLXError)?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tags (
                id   INTEGER PRIMARY KEY NOT NULL, 
                name TEXT NOT NULL UNIQUE,
                parent_id INTEGER,
                FOREIGN KEY (parent_id)
                    REFERENCES tags (id)
            );",
        )
        .execute(&mut *db)
        .await
        .map_err(DatabaseError::SQLXError)?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS images_to_tags (
                id       INTEGER PRIMARY KEY NOT NULL, 
                image_id INTEGER NOT NULL,
                tag_id   INTEGER NOT NULL,
                FOREIGN KEY (image_id) REFERENCES images (id)
                    ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags (id)
                    ON DELETE CASCADE
            );",
        )
        .execute(&mut *db)
        .await
        .map_err(DatabaseError::SQLXError)?;

        Ok(())
    }

    // This function is bizarre, because it needs to first add tags and hashes before resolving the
    // many-to-many relationship
    pub async fn add_images_to_db<'a>(
        &mut self,
        ims: impl IntoIterator<Item = &'a Image>,
    ) -> Result<(), DatabaseError> {
        let mut db = self
            .pool
            .acquire()
            .await
            .map_err(DatabaseError::SQLXError)?;

        let ((fps, hashes), tags): ((Vec<&str>, Vec<&str>), Vec<&HashSet<String>>) = ims
            .into_iter()
            .map(|x| ((x.get_fp(), x.get_hash()), x.get_tags()))
            .unzip();

        let tags_str: Vec<String> = tags
            .clone()
            .into_iter()
            .fold(HashSet::<String>::new(), |acc, x| &acc | x)
            .into_iter()
            .map(|x| format!("(\'{}\')", x))
            .collect::<Vec<String>>();

        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&mut *db)
            .await
            .map_err(DatabaseError::SQLXError)?;

        let fps_hashes_zipped = std::iter::zip(&fps, &hashes)
            .map(|x| format!("(\'{}\', \'{}\')", x.0, x.1))
            .collect::<Vec<String>>();

        sqlx::query(
            &format!(
                "INSERT OR IGNORE INTO images (fp, hash) VALUES {};",
                fps_hashes_zipped.join(", ")
            )[..],
        )
        .execute(&mut *db)
        .await
        .map_err(DatabaseError::SQLXError)?;

        sqlx::query(
            &format!(
                "INSERT OR IGNORE INTO tags (name) VALUES {};",
                tags_str.join(", ")
            )[..],
        )
        .execute(&mut *db)
        .await
        .map_err(DatabaseError::SQLXError)?;

        for im in std::iter::zip(hashes, tags).collect::<Vec<(&str, &HashSet<String>)>>() {
            let personal_tags_str: String =
                im.1.iter()
                    .map(|x| format!("name = \'{}\'", x))
                    .collect::<Vec<String>>()
                    .join(" OR ");

            let tag_results: Vec<u32> = sqlx::query_scalar(
                &format!("SELECT (id) FROM tags WHERE {};", personal_tags_str)[..],
            )
            .fetch_all(&mut *db)
            .await
            .map_err(DatabaseError::SQLXError)?;

            let image_result: u32 = sqlx::query_scalar(
                &format!("SELECT (id) FROM images WHERE hash = \'{}\';", im.0)[..],
            )
            .fetch_one(&mut *db)
            .await
            .map_err(DatabaseError::SQLXError)?;

            let image_tag_tuples = tag_results
                .into_iter()
                .map(|x| format!("({}, {})", image_result, x))
                .collect::<Vec<String>>();

            sqlx::query(
                &format!(
                    "INSERT INTO images_to_tags (image_id, tag_id) VALUES {};",
                    image_tag_tuples.join(", ")
                )[..],
            )
            .execute(&mut *db)
            .await
            .map_err(DatabaseError::SQLXError)?;
        }

        Ok(())
    }

    pub async fn get_images_from_db_by_fp(
        &self,
        fp: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Vec<Image>, DatabaseError> {
        let hashes = fp
            .into_iter()
            .map(|x| Image::hash_image(x.as_ref()))
            .take_while(|x| x.is_ok())
            .map(|x| x.unwrap())
            .collect::<Vec<String>>();
        self.get_images_from_db_by_hashes(hashes).await
    }

    pub async fn get_images_from_db_by_tag_query(
        &self,
        q: AST,
    ) -> Result<Vec<Image>, DatabaseError> {
        self.req_imagerow_from_db(format!("{}", q)).await
    }

    // TODO: delete by submitting image, not hash
    pub async fn delete_images_from_db(
        &self,
        hs: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Vec<String>, DatabaseError> {
        let mut db = self
            .pool
            .acquire()
            .await
            .map_err(DatabaseError::SQLXError)?;

        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&mut *db)
            .await
            .map_err(DatabaseError::SQLXError)?;

        let pairs = &hs
            .into_iter()
            .map(|x| format!("hash = \'{}\'", x.as_ref()))
            .collect::<Vec<String>>()
            .join(" OR ")[..];
        let image_hashes: Vec<String> = sqlx::query_scalar(
            &format!(
                "DELETE FROM images 
                WHERE {};",
                pairs
            )[..],
        )
        .fetch_all(&mut *db)
        .await
        .map_err(DatabaseError::SQLXError)?;

        Ok(image_hashes)
    }

    async fn get_images_from_db_by_hashes(
        &self,
        hs: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Vec<Image>, DatabaseError> {
        let pairs = hs
            .into_iter()
            .map(|x| format!("name = \'{}\'", x.as_ref()))
            .collect::<Vec<String>>()
            .join(" OR ");

        self.req_imagerow_from_db(pairs).await
    }

    // TODO: lots of unwraps
    fn convert_imagerow_to_images(irs: impl IntoIterator<Item = ImageRow>) -> Vec<Image> {
        let mut image_map: HashMap<String, Image> = HashMap::new();

        let irs = irs.into_iter().collect::<Vec<ImageRow>>();
        for ir in irs {
            let fp = ir.fp.unwrap();
            let tag = ir.name.unwrap();

            image_map
                .entry(ir.hash.unwrap())
                .and_modify(|im| im.add_tag(tag.to_owned()))
                .or_insert_with(|| Image::new_with_tags(fp, vec![tag.to_owned()]).unwrap());
        }
        image_map.into_values().collect::<Vec<Image>>()
    }

    async fn req_imagerow_from_db(
        &self,
        pairs: impl AsRef<str>,
    ) -> Result<Vec<Image>, DatabaseError> {
        println!("{}", pairs.as_ref());
        let mut db = self
            .pool
            .acquire()
            .await
            .map_err(DatabaseError::SQLXError)?;

        // select all hashes such that there is a tag match
        let hashes: Vec<String> = sqlx::query_scalar(
            &format!(
                "SELECT hash FROM images 
                INNER JOIN images_to_tags ON images.id = images_to_tags.image_id 
                INNER JOIN tags ON images_to_tags.tag_id = tags.id 
                WHERE {};",
                pairs.as_ref()
            )[..],
        )
        .fetch_all(&mut *db)
        .await
        .map_err(DatabaseError::SQLXError)?;

        // query for all those hashes
        let hash_pairs = hashes
            .into_iter()
            .map(|x| format!("hash = \'{}\'", x))
            .collect::<Vec<String>>()
            .join(" OR ");

        let image_data: Vec<ImageRow> = sqlx::query_as(
            &format!(
                "SELECT fp, hash, name FROM images 
                INNER JOIN images_to_tags ON images.id = images_to_tags.image_id 
                INNER JOIN tags ON images_to_tags.tag_id = tags.id 
                WHERE {};",
                hash_pairs
            )[..],
        )
        .fetch_all(&mut *db)
        .await
        .map_err(DatabaseError::SQLXError)?;

        Ok(ImageDB::convert_imagerow_to_images(image_data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashSet, error};

    // I can't directly test some of the database logic, so some of the tests will have
    // dependencies
    // I'm going to just make one integration test

    #[tokio::test]
    async fn create_new_database() -> Result<(), Box<dyn error::Error>> {
        let filename = PathBuf::from("fetefoto.db");
        println!("creating file");
        let mut db = ImageDB::create_db(filename).await?;

        println!("creating table");
        db.create_table().await?;

        println!("adding image");
        let output: Image = Image::new_with_tags(
            String::from("test/abc.gif"),
            HashSet::from([String::from("hi"), String::from("bye")]),
        )?;
        db.add_images_to_db(std::iter::once(&output)).await?;

        println!("adding image with unicode tags");
        let output: Image = Image::new_with_tags(
            String::from("test/def.gif"),
            HashSet::from([String::from("你好")]),
        )?;
        db.add_images_to_db(std::iter::once(&output)).await?;

        println!("adding image with redundant tags");
        let output: Image = Image::new_with_tags(
            String::from("test/test2/ghi.jpg"),
            HashSet::from([String::from("hi"), String::from("hi")]),
        )?;
        db.add_images_to_db(std::iter::once(&output)).await?;

        println!("retrieving image by tag");

        let query = AST::parse_query("(hi AND bye) OR NOT 你好");
        let output = db.get_images_from_db_by_tag_query(query.unwrap()).await?;
        let im1: Image = Image::new_with_tags(
            String::from("test/abc.gif"),
            HashSet::from([String::from("hi"), String::from("bye")]),
        )?;
        let im2: Image = Image::new_with_tags(
            String::from("test/test2/ghi.jpg"),
            HashSet::from([String::from("hi"), String::from("hi")]),
        )?;
        let v = vec![im1, im2];
        for i in &output {
            let mut notinotherset = false;
            for j in &v {
                if i == j {
                    notinotherset = true;
                }
            }
            assert!(notinotherset);
        }

        println!("removing image");
        db.delete_images_from_db(std::iter::once("test/test2/ghi.jpg"))
            .await?;

        Ok(())
    }

    #[test]
    fn update_images_in_database() {}
}
