use crate::ast::AST;
use crate::image::Image;
use crate::tag::TagPath;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Encode, QueryBuilder, Sqlite, SqliteConnection, SqlitePool, Type};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// conservative estimate on the maximum number of binds for a sqlx query
const MAX_BINDS: usize = 500;

/// representation of an entry joining the sql Images table to the Images-To-Tags table and to the
/// Tags table, an intermediary for constructing Image objects
#[derive(Debug, sqlx::FromRow)]
struct ImageRow {
    // primary key in the Images table
    id: i64,
    // image hash
    hash: String,
    // file location
    fp: String,
    // the full path from the Images-To-Tags and Tags join
    tag: Option<String>,
}

/// representation of the different ways images can be queried. see `ImageRow` for the meaning of the fields
enum ImageKey {
    Id,
    Hash,
    Fp,
}

impl ImageKey {
    fn column(&self) -> &'static str {
        match self {
            ImageKey::Id => "images.id",
            ImageKey::Hash => "images.hash",
            ImageKey::Fp => "images.fp",
        }
    }
}

pub struct ImageDB {
    filepath: PathBuf,
    pool: SqlitePool,
}

/// query template for querybuilder. the formulation of the query is meant to support ImageRow
const IMAGE_SELECT: &str = "SELECT images.id, images.fp, images.hash, tags.path AS tag
     FROM images
     LEFT JOIN images_to_tags it ON it.image_id = images.id
     LEFT JOIN tags ON tags.id = it.tag_id
     WHERE ";

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database already exists")]
    DatabaseExists,
    #[error(transparent)]
    SQLXError(#[from] sqlx::Error),
}

impl ImageDB {
    pub fn get_filepath(&self) -> &Path {
        &self.filepath
    }

    pub async fn create_db(filepath: PathBuf) -> Result<Self, DatabaseError> {
        if filepath.try_exists().unwrap_or(true) {
            return Err(DatabaseError::DatabaseExists);
        }
        let opts = SqliteConnectOptions::new()
            .filename(&filepath)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await?;

        let db = ImageDB { filepath, pool };
        db.create_table().await?;

        Ok(db)
    }

    pub async fn get_connection(filepath: PathBuf) -> Result<Self, DatabaseError> {
        let opts = SqliteConnectOptions::new().filename(&filepath);
        let pool = SqlitePool::connect_with(opts).await?;
        Ok(ImageDB { filepath, pool })
    }

    pub async fn create_table(&self) -> Result<(), DatabaseError> {
        let mut conn = self.pool.begin().await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS images (
                id   INTEGER PRIMARY KEY NOT NULL,
                fp   TEXT NOT NULL UNIQUE,
                hash TEXT NOT NULL UNIQUE
            );",
        )
        .execute(&mut *conn)
        .await?;

        // The database preserves both path and parent_id.
        // parent_id is useful for taking care of cascading deletions.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tags (
                id        INTEGER PRIMARY KEY NOT NULL,
                path      TEXT NOT NULL UNIQUE,
                parent_id INTEGER REFERENCES tags (id) ON DELETE CASCADE
        );",
        )
        .execute(&mut *conn)
        .await?;

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
        .execute(&mut *conn)
        .await?;

        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS images_to_tags_unique
                ON images_to_tags (image_id, tag_id);",
        )
        .execute(&mut *conn)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS images_to_tags_tag 
                ON images_to_tags (tag_id);",
        )
        .execute(&mut *conn)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS tags_parent 
                ON tags (parent_id);",
        )
        .execute(&mut *conn)
        .await?;

        conn.commit().await?;
        Ok(())
    }

    // *********************************************************************
    // ADD IMAGES
    // *********************************************************************

    /// add_images
    ///
    /// adds multiple images to the database contained in its wrapper
    ///
    /// params:
    /// * `ims`: slice of images to be upserted into the database
    ///
    /// returns:
    /// A vector of primary keys for the successfully added images, or an error
    pub async fn add_images(&self, ims: &[Image]) -> Result<Vec<i64>, DatabaseError> {
        let mut conn = self.pool.begin().await?;
        let mut ids = Vec::with_capacity(ims.len());

        let mut tag_ids: HashMap<&TagPath, i64> = HashMap::new();

        for im in ims {
            // NOTE: two files with identical contents share one row, and the later file's path
            // silently replaces the earlier one
            // TODO: address duplicate files
            let image_id: i64 = sqlx::query_scalar(
                "INSERT INTO images (fp, hash) VALUES (?, ?)
                ON CONFLICT(hash) DO UPDATE SET fp = excluded.fp
                ON CONFLICT(fp) DO UPDATE SET hash = excluded.hash
                RETURNING id",
            )
            .bind(im.get_fp())
            .bind(im.get_hash())
            .fetch_one(&mut *conn)
            .await?;

            for tag in im.get_tags() {
                // lookup and add new tags as necessary
                let tag_id = match tag_ids.get(tag) {
                    Some(&id) => id,
                    None => {
                        let id = Self::upsert_tag_path(&mut conn, tag).await?;
                        tag_ids.insert(tag, id);
                        id
                    }
                };

                sqlx::query(
                    "INSERT OR IGNORE INTO images_to_tags (image_id, tag_id) VALUES (?, ?)",
                )
                .bind(image_id)
                .bind(tag_id)
                .execute(&mut *conn)
                .await?;
            }

            ids.push(image_id);
        }
        conn.commit().await?;
        Ok(ids)
    }

    // *********************************************************************
    // GET IMAGES
    // *********************************************************************

    /// get_images_by_key
    ///
    /// gets images through a query keyed on one of the values of the `ImageKey` enum. exists to
    /// deduplicate code, and wrapped by public functions that can correctly relate the key to the
    /// values
    ///
    /// params:
    /// * `key`: the kind of key to be queried against
    /// * `values`: a slice of sqlite-queriable values that can be used with the provided `key`.
    ///
    /// returns:
    /// A vector of images, or an error
    async fn get_images_by_key<'a, T>(
        &self,
        key: ImageKey,
        values: &'a [T],
    ) -> Result<Vec<Image>, DatabaseError>
    where
        T: Encode<'a, Sqlite> + Type<Sqlite>,
    {
        let mut conn = self.pool.begin().await?;
        let mut out = Vec::new();

        for chunk in values.chunks(MAX_BINDS) {
            let mut qb = QueryBuilder::<Sqlite>::new(IMAGE_SELECT);
            qb.push(key.column()).push(" IN (");
            let mut list = qb.separated(", ");
            for v in chunk {
                list.push_bind(v);
            }
            list.push_unseparated(") ORDER BY images.id");

            let rows: Vec<ImageRow> = qb.build_query_as().fetch_all(&mut *conn).await?;
            out.extend(Self::convert_imagerow_to_images(&rows));
        }

        conn.commit().await?;
        Ok(out)
    }

    /// get_images_by_hashes
    ///
    /// gets images through a query keyed by Image hashes
    ///
    /// params:
    /// * `hs`: a slice of stringified Image hashes
    ///
    /// returns:
    /// A vector of images, or an error
    // TODO: these should be tested for being valid hashes
    pub async fn get_images_by_hashes(&self, hs: &[String]) -> Result<Vec<Image>, DatabaseError> {
        self.get_images_by_key(ImageKey::Hash, hs).await
    }

    /// get_images_by_fps
    ///
    /// gets images through a query keyed by Image file paths
    ///
    /// params:
    /// * `fps`: a slice of stringified Image file paths
    ///
    /// returns:
    /// A vector of images, or an error
    // TODO: take paths (impl AsRef<Path>), and return an error for non-UTF-8 paths
    pub async fn get_images_by_fps(&self, fps: &[String]) -> Result<Vec<Image>, DatabaseError> {
        self.get_images_by_key(ImageKey::Fp, fps).await
    }

    /// get_images_by_ids
    ///
    /// gets images through a query keyed by Image ids
    ///
    /// params:
    /// * `ids`: a slice of integer image ids
    ///
    /// returns:
    /// A vector of images, or an error
    pub async fn get_images_by_ids(&self, ids: &[i64]) -> Result<Vec<Image>, DatabaseError> {
        self.get_images_by_key(ImageKey::Id, ids).await
    }

    /// get_images_by_tags
    ///
    /// gets images by their relationships as specified by Images-To-Tags
    ///
    /// params:
    /// * `q`: a tag query to be evaluated
    ///
    /// returns:
    /// A vector of images, or an error
    pub async fn get_images_by_tags(&self, q: &AST) -> Result<Vec<Image>, DatabaseError> {
        let mut qb = QueryBuilder::<Sqlite>::new(IMAGE_SELECT);
        Self::push_tag_query(&mut qb, q);
        qb.push(" ORDER BY images.id");

        let rows: Vec<ImageRow> = qb.build_query_as().fetch_all(&self.pool).await?;
        Ok(Self::convert_imagerow_to_images(&rows))
    }

    // *********************************************************************
    // DELETE IMAGES
    // *********************************************************************

    /// delete_images_by_key
    ///
    /// deletes images through a query keyed on one of the values of the `ImageKey` enum. exists to
    /// deduplicate code, and wrapped by public functions that can correctly relate the key to the
    /// values
    ///
    /// params:
    /// * `key`: the kind of key to be queried against
    /// * `values`: a slice of sqlite-queriable values that can be used with the provided `key`.
    ///
    /// returns:
    /// A vector of Image ids of the images that were deleted, or an error
    async fn delete_images_by_key<'a, T>(
        &self,
        key: ImageKey,
        values: &'a [T],
    ) -> Result<Vec<i64>, DatabaseError>
    where
        T: Encode<'a, Sqlite> + Type<Sqlite>,
    {
        let mut conn = self.pool.begin().await?;
        let mut deleted = Vec::new();

        for chunk in values.chunks(MAX_BINDS) {
            let mut qb = QueryBuilder::<Sqlite>::new("DELETE FROM images WHERE ");
            qb.push(key.column()).push(" IN (");
            let mut list = qb.separated(", ");
            for v in chunk {
                list.push_bind(v);
            }
            list.push_unseparated(") RETURNING id");

            let ids: Vec<i64> = qb.build_query_scalar().fetch_all(&mut *conn).await?;
            deleted.extend(ids);
        }

        conn.commit().await?;
        Ok(deleted)
    }

    pub async fn delete_images_by_hashes(&self, hs: &[String]) -> Result<Vec<i64>, DatabaseError> {
        self.delete_images_by_key(ImageKey::Hash, hs).await
    }

    pub async fn delete_images_by_fps(&self, fps: &[String]) -> Result<Vec<i64>, DatabaseError> {
        self.delete_images_by_key(ImageKey::Fp, fps).await
    }

    pub async fn delete_images_by_ids(&self, ids: &[i64]) -> Result<Vec<i64>, DatabaseError> {
        self.delete_images_by_key(ImageKey::Id, ids).await
    }

    pub async fn delete_images_by_tags(&self, q: &AST) -> Result<Vec<i64>, DatabaseError> {
        let mut qb = QueryBuilder::<Sqlite>::new("DELETE FROM images WHERE ");
        Self::push_tag_query(&mut qb, q);
        qb.push(" RETURNING id");

        Ok(qb.build_query_scalar().fetch_all(&self.pool).await?)
    }

    /// clean_up_tags
    ///
    /// public wrapper around a function that deletes unused tags i.e. tags that are not related to
    /// Images according to Images-To-Tags
    ///
    /// returns:
    /// A vector of Tag ids of the tags that were deleted, or an error
    pub async fn clean_up_tags(&self) -> Result<Vec<i64>, DatabaseError> {
        let mut conn = self.pool.begin().await?;

        let results = Self::tag_cleanup_helper(&mut conn).await?;

        conn.commit().await?;
        Ok(results)
    }

    // *********************************************************************
    // ADD TAGS
    // *********************************************************************

    /// add_tags
    ///
    /// public wrapper around tag upsertion
    ///
    /// returns:
    /// A vector of Tag ids for the tags that were added, or an error
    pub async fn add_tags(&self, tags: &[TagPath]) -> Result<Vec<i64>, DatabaseError> {
        let mut conn = self.pool.begin().await?;
        let mut ids = Vec::with_capacity(tags.len());

        for tag in tags {
            ids.push(Self::upsert_tag_path(&mut conn, tag).await?);
        }

        conn.commit().await?;
        Ok(ids)
    }

    // *********************************************************************
    // ASSIGN TAG PARENT/CHILD
    // *********************************************************************

    // TODO: write this

    // *********************************************************************
    // RENAME TAG PARENT/CHILD
    // *********************************************************************

    // TODO: write this

    // *********************************************************************
    // DELETE TAGS
    // *********************************************************************

    /// delete_tags_by_ids
    ///
    /// deletes tags
    ///
    /// params:
    /// - `ids`: A slice of Tag ids to be deleted
    ///
    /// returns:
    /// A vector of Tag ids for the tags that were deleted, or an error
    pub async fn delete_tags_by_ids(&self, ids: &[i64]) -> Result<Vec<i64>, DatabaseError> {
        let mut conn = self.pool.begin().await?;
        let mut deleted = Vec::new();

        for chunk in ids.chunks(MAX_BINDS) {
            let mut qb = QueryBuilder::<Sqlite>::new("DELETE FROM tags WHERE id IN (");
            let mut list = qb.separated(", ");
            for id in chunk {
                list.push_bind(*id);
            }
            list.push_unseparated(") RETURNING id");

            let removed: Vec<i64> = qb.build_query_scalar().fetch_all(&mut *conn).await?;
            deleted.extend(removed);
        }

        conn.commit().await?;
        Ok(deleted)
    }

    // *********************************************************************
    // HELPERS
    // *********************************************************************

    /// upsert_tag_path
    ///
    /// inserts a tag, and all its parents if they do not exist
    ///
    /// params:
    /// - `conn`: a mutable connection to the database
    /// - `path`: a TagPath object, representing a leaf containing its parents, that can be added
    ///
    /// returns:
    /// Tag id for the leaf that was added, or an error
    async fn upsert_tag_path(
        conn: &mut SqliteConnection,
        path: &TagPath,
    ) -> Result<i64, DatabaseError> {
        let segs = path.segments();
        let mut parent: Option<i64> = None;

        for depth in 1..=segs.len() {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO tags (path, parent_id) VALUES (?, ?)
                 ON CONFLICT(path) DO UPDATE SET path = excluded.path
                 RETURNING id",
            )
            .bind(segs[..depth].join("/"))
            .bind(parent)
            .fetch_one(&mut *conn)
            .await?;
            parent = Some(id);
        }

        Ok(parent.expect("TagPath always has at least one segment"))
    }

    fn push_tag_query<'a>(qb: &mut QueryBuilder<'a, Sqlite>, q: &'a AST) {
        // leverages the grammar defined in AST
        match q {
            AST::AND(l, r) | AST::OR(l, r) => {
                qb.push("(");
                Self::push_tag_query(qb, l);
                qb.push(if matches!(q, AST::AND(..)) {
                    " AND "
                } else {
                    " OR "
                });
                Self::push_tag_query(qb, r);
                qb.push(")");
            }
            AST::NOT(e) => {
                qb.push("(NOT ");
                Self::push_tag_query(qb, e);
                qb.push(")");
            }
            AST::VAL(path) => {
                let p = path.segments().join("/");
                // generates a subquery that can evaluate the AST logic, as well as subpaths, by
                // evaluating whether, if the tag is sorted, the range matches ["p/", "p0"]
                qb.push(
                    "EXISTS (SELECT 1 FROM images_to_tags it2 \
                     JOIN tags t2 ON t2.id = it2.tag_id \
                     WHERE it2.image_id = images.id AND (t2.path = ",
                )
                .push_bind(p.clone())
                .push(" OR (t2.path >= ")
                .push_bind(format!("{p}/"))
                .push(" AND t2.path < ")
                .push_bind(format!("{p}0"))
                .push(")))");
            }
        }
    }

    fn convert_imagerow_to_images(irs: &[ImageRow]) -> Vec<Image> {
        let mut index: HashMap<i64, usize> = HashMap::new();
        let mut images: Vec<Image> = Vec::new();
        for ir in irs {
            // get the id of the Image referenced by the ImageRow, inserting as necessary
            let i = *index.entry(ir.id).or_insert_with(|| {
                images.push(Image::from_parts(ir.fp.clone(), ir.hash.clone()));
                images.len() - 1
            });
            if let Some(tag) = &ir.tag {
                images[i]
                    .add_tag(tag)
                    .expect("Tags from the DB should be valid");
            }
        }
        images
    }

    pub async fn drop_connections(self) {
        self.pool.close().await;
    }

    async fn tag_cleanup_helper(conn: &mut SqliteConnection) -> Result<Vec<i64>, DatabaseError> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "DELETE FROM tags WHERE NOT EXISTS (
                SELECT 1 FROM images_to_tags it
                JOIN tags u ON u.id = it.tag_id
                WHERE u.path = tags.path
                   OR (u.path >= tags.path || '/' AND u.path < tags.path || '0')
            )
            RETURNING id",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(ids)
    }
}

// TODO: NEED THE FOLLOWING TESTS:
// lookup/delete via hash
#[cfg(test)]
mod tests {
    use super::*;
    use std::{error, fs};

    fn create_temp_dir() -> Result<(), Box<dyn error::Error>> {
        fs::create_dir("temp")?;
        Ok(())
    }

    async fn delete_database(db: ImageDB, dbname: &str) -> Result<(), Box<dyn error::Error>> {
        db.drop_connections().await;
        let mut filename = PathBuf::from("temp");
        filename.push(dbname);
        println!("{}", filename.as_os_str().to_str().unwrap());
        if filename.exists() {
            fs::remove_file(filename)?;
        }
        Ok(())
    }

    async fn create_new_database(dbname: &str) -> Result<ImageDB, Box<dyn error::Error>> {
        // create_temp_dir's check can pass but the attempt to create a directory can fail
        //
        // ultimately, it's an error we can ignore for the sake of testing the database
        let _ = create_temp_dir();
        let mut filename = PathBuf::from("temp");
        filename.push(dbname);
        println!("{}", filename.as_os_str().to_str().unwrap());
        let db = ImageDB::create_db(filename).await?;
        return Ok(db);
    }

    fn fp(name: &str) -> String {
        format!("persistent_test_data/image_db_test/{name}")
    }

    fn test_image(name: &str, tags: &[&str]) -> Result<Image, Box<dyn error::Error>> {
        Ok(Image::new_with_tags(fp(name), tags.iter().copied())?)
    }

    async fn tag_paths(db: &ImageDB) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT path FROM tags ORDER BY path")
            .fetch_all(&db.pool)
            .await
    }

    fn paths(p: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = p.iter().map(|s| s.to_string()).collect();
        v.sort();
        v
    }

    async fn fps_matching(db: &ImageDB, query: &str) -> Result<Vec<String>, Box<dyn error::Error>> {
        let ast = AST::parse_query(query)?;
        let mut fps: Vec<String> = db
            .get_images_by_tags(&ast)
            .await?
            .iter()
            .map(|im| im.get_fp().to_owned())
            .collect();
        fps.sort();
        Ok(fps)
    }

    async fn add_images_to_db_test_scaffolding(db: &ImageDB) -> Result<(), Box<dyn error::Error>> {
        println!("adding single default image");
        let output1 = test_image("a", &["hi", "bye"])?;
        db.add_images(&[output1]).await?;

        println!("adding image with unicode tags");
        let output2 = test_image("b", &["你好"])?;
        db.add_images(&[output2]).await?;

        println!("adding image with redundant tags");
        let output3 = test_image("你好", &["hi", "hi"])?;
        db.add_images(&[output3]).await?;

        println!("adding multiple images");
        let output4 = test_image("test2/c", &["hi"])?;
        let output5 = test_image("test2/d", &["你好"])?;
        db.add_images(&[output4, output5]).await?;

        println!("adding untagged image");
        let output6 = test_image("e", &[])?;
        db.add_images(&[output6]).await?;

        Ok(())
    }

    #[tokio::test]
    async fn create_new_database_test() -> Result<(), Box<dyn error::Error>> {
        println!("creating database");
        let db = create_new_database("test1.db").await?;
        println!("deleting database");
        delete_database(db, "test1.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn create_new_table_test() -> Result<(), Box<dyn error::Error>> {
        println!("creating database");
        let db = create_new_database("test2.db").await?;
        println!("deleting database");
        delete_database(db, "test2.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn add_images_to_db_test() -> Result<(), Box<dyn error::Error>> {
        println!("creating database");
        let db = create_new_database("test3.db").await?;
        println!("adding images to database");
        add_images_to_db_test_scaffolding(&db).await?;
        println!("deleting database");
        delete_database(db, "test3.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn retrieve_image_by_tag_test() -> Result<(), Box<dyn error::Error>> {
        println!("creating database");
        let db = create_new_database("test4.db").await?;
        println!("adding images to database");
        add_images_to_db_test_scaffolding(&db).await?;
        let query = AST::parse_query("(hi AND bye) OR NOT 你好");
        let output = db.get_images_by_tags(&query.unwrap()).await?;
        let im1 = test_image("a", &["hi", "bye"])?;
        let im2 = test_image("你好", &["hi"])?;
        let im3 = test_image("test2/c", &["hi"])?;
        let im4 = test_image("e", &[])?;

        let v = vec![im1, im2, im3, im4];
        println!("{:?}", output);
        assert_eq!(output.len(), v.len(), "unexpected result set: {:?}", output);
        for expected in &v {
            assert!(output.contains(expected), "missing {:?}", expected);
        }

        println!("deleting database");
        delete_database(db, "test4.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn remove_image_by_name_test() -> Result<(), Box<dyn error::Error>> {
        println!("creating database");
        let db = create_new_database("test5.db").await?;
        println!("adding images to database");
        add_images_to_db_test_scaffolding(&db).await?;

        let fps = vec![fp("test2/d")];
        let deleted = db.delete_images_by_fps(&fps).await?;
        assert_eq!(deleted.len(), 1);
        assert!(db.get_images_by_fps(&fps).await?.is_empty());

        println!("deleting database");
        delete_database(db, "test5.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn add_image_with_parent_tags() -> Result<(), Box<dyn error::Error>> {
        println!("creating database");
        let db = create_new_database("test6.db").await?;
        let image = test_image("a", &["root/mid/leaf"])?;
        db.add_images(&[image.clone()]).await?;

        assert_eq!(
            tag_paths(&db).await?,
            paths(&["root", "root/mid", "root/mid/leaf"])
        );
        assert_eq!(db.get_images_by_fps(&[fp("a")]).await?, vec![image]);
        for q in ["root", "root/mid", "root/mid/leaf"] {
            assert_eq!(fps_matching(&db, q).await?, [fp("a")], "query {q:?}");
        }
        for q in ["mid", "leaf", "mid/leaf"] {
            assert!(fps_matching(&db, q).await?.is_empty(), "query {q:?}");
        }

        delete_database(db, "test6.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn directly_add_tag_test() -> Result<(), Box<dyn error::Error>> {
        println!("creating database");
        let db = create_new_database("test7.db").await?;
        println!("adding tags to database");
        let desired_tags = ["hi", "bye", "root", "你好", "mid"];
        let desired_tagpaths: Vec<TagPath> = desired_tags
            .iter()
            .map(|x| TagPath::parse(x))
            .collect::<Result<_, _>>()?;

        let ids = db.add_tags(&desired_tagpaths).await?;
        assert_eq!(db.add_tags(&desired_tagpaths).await?, ids);

        assert_eq!(tag_paths(&db).await?, paths(&desired_tags));

        delete_database(db, "test7.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn add_image_with_existing_tags() -> Result<(), Box<dyn error::Error>> {
        let db = create_new_database("test8.db").await?;
        db.add_images(&[test_image("a", &["hi"])?]).await?;
        db.add_images(&[test_image("b", &["hi"])?]).await?;

        assert_eq!(tag_paths(&db).await?, paths(&["hi"]));
        assert_eq!(fps_matching(&db, "hi").await?, [fp("a"), fp("b")]);

        delete_database(db, "test8.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn add_image_with_existing_parent_tags() -> Result<(), Box<dyn error::Error>> {
        let db = create_new_database("test9.db").await?;
        db.add_images(&[test_image("a", &["root/mid"])?]).await?;
        db.add_images(&[test_image("b", &["root/mid2", "root2/mid"])?])
            .await?;

        assert_eq!(
            tag_paths(&db).await?,
            paths(&["root", "root/mid", "root/mid2", "root2", "root2/mid"])
        );
        assert_eq!(fps_matching(&db, "root/mid").await?, [fp("a")]);
        assert_eq!(fps_matching(&db, "root2/mid").await?, [fp("b")]);
        assert!(fps_matching(&db, "mid").await?.is_empty());
        assert_eq!(fps_matching(&db, "root").await?, [fp("a"), fp("b")]);
        assert_eq!(fps_matching(&db, "root2").await?, [fp("b")]);

        delete_database(db, "test9.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn add_image_with_similar_tags() -> Result<(), Box<dyn error::Error>> {
        let db = create_new_database("test10.db").await?;
        db.add_images(&[test_image("a", &["root2/mid/leaf"])?])
            .await?;
        db.add_images(&[test_image("b", &["root/mid/leaf"])?])
            .await?;

        assert_eq!(
            tag_paths(&db).await?,
            paths(&[
                "root",
                "root/mid",
                "root/mid/leaf",
                "root2",
                "root2/mid",
                "root2/mid/leaf",
            ])
        );
        assert_eq!(fps_matching(&db, "root/mid").await?, [fp("b")]);
        assert_eq!(fps_matching(&db, "root2/mid").await?, [fp("a")]);
        assert_eq!(fps_matching(&db, "root/mid/leaf").await?, [fp("b")]);

        delete_database(db, "test10.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn delete_tag() -> Result<(), Box<dyn error::Error>> {
        let db = create_new_database("test11.db").await?;
        db.add_images(&[
            test_image("a", &["root/mid/leaf"])?,
            test_image("b", &["root/mid2"])?,
        ])
        .await?;

        let mid: i64 = sqlx::query_scalar("SELECT id FROM tags WHERE path = 'root/mid'")
            .fetch_one(&db.pool)
            .await?;
        assert_eq!(db.delete_tags_by_ids(&[mid]).await?, [mid]);
        assert_eq!(tag_paths(&db).await?, paths(&["root", "root/mid2"]));

        assert!(db.get_images_by_fps(&[fp("a")]).await?[0]
            .get_tags()
            .is_empty());
        assert_eq!(fps_matching(&db, "root").await?, [fp("b")]);

        delete_database(db, "test11.db").await?;
        Ok(())
    }

    #[tokio::test]
    async fn delete_orphaned_tags() -> Result<(), Box<dyn error::Error>> {
        let db = create_new_database("test12.db").await?;
        db.add_images(&[
            test_image("a", &["root/mid/leaf"])?,
            test_image("b", &["root/mid2"])?,
        ])
        .await?;

        db.delete_images_by_fps(&[fp("b")]).await?;
        assert_eq!(tag_paths(&db).await?.len(), 4);

        assert_eq!(db.clean_up_tags().await?.len(), 1);
        assert_eq!(
            tag_paths(&db).await?,
            paths(&["root", "root/mid", "root/mid/leaf"])
        );

        // Tags outlive their images until cleanup runs.
        db.delete_images_by_fps(&[fp("a")]).await?;
        assert_eq!(tag_paths(&db).await?.len(), 3);
        db.clean_up_tags().await?;
        assert!(tag_paths(&db).await?.is_empty());

        delete_database(db, "test12.db").await?;
        Ok(())
    }
}
