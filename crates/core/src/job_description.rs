//! Standalone Job Descriptions stored in the Master Store.

use serde::{Deserialize, Serialize};

use crate::store::{Store, StoreError};

/// A saved role posting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobDescription {
    pub id: i64,
    pub title: Option<String>,
    pub text: String,
    pub created_at: String,
}

/// The text and optional label for a new Job Description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewJobDescription {
    pub title: Option<String>,
    pub text: String,
}

impl Store {
    /// Saves a Job Description and returns its durable record.
    pub async fn create_job_description(
        &self,
        job_description: &NewJobDescription,
    ) -> Result<JobDescription, StoreError> {
        let mut rows = self
            .connection()
            .query(
                "INSERT INTO job_description (title, text) VALUES (?, ?) \
                 RETURNING id, title, text, created_at",
                vec![
                    text(job_description.title.as_deref()),
                    libsql::Value::Text(job_description.text.clone()),
                ],
            )
            .await
            .map_err(StoreError::CreateJobDescription)?;

        let row = rows
            .next()
            .await
            .map_err(StoreError::CreateJobDescription)?
            .ok_or(StoreError::CreateJobDescription(
                libsql::Error::QueryReturnedNoRows,
            ))?;

        decode(&row)
    }

    /// Reads the Job Description with `id`, if it exists.
    pub async fn job_description(&self, id: i64) -> Result<Option<JobDescription>, StoreError> {
        let mut rows = self
            .connection()
            .query(
                "SELECT id, title, text, created_at FROM job_description WHERE id = ?",
                vec![libsql::Value::Integer(id)],
            )
            .await
            .map_err(StoreError::ReadJobDescriptions)?;

        let Some(row) = rows.next().await.map_err(StoreError::ReadJobDescriptions)? else {
            return Ok(None);
        };

        decode(&row).map(Some)
    }

    /// Lists Job Descriptions, optionally restricted by a full-text query.
    pub async fn job_descriptions(
        &self,
        query: Option<&str>,
    ) -> Result<Vec<JobDescription>, StoreError> {
        let query = query.filter(|query| !query.trim().is_empty());
        let mut rows = match query {
            Some(query) => {
                self.connection()
                    .query(
                        "SELECT job_description.id, job_description.title, \
                         job_description.text, job_description.created_at \
                         FROM job_description \
                         JOIN job_description_fts \
                         ON job_description_fts.rowid = job_description.id \
                         WHERE job_description_fts MATCH ? \
                         ORDER BY rank, job_description.id DESC",
                        vec![libsql::Value::Text(query.to_owned())],
                    )
                    .await
            }
            None => {
                self.connection()
                    .query(
                        "SELECT id, title, text, created_at FROM job_description \
                         ORDER BY created_at DESC, id DESC",
                        (),
                    )
                    .await
            }
        }
        .map_err(StoreError::ReadJobDescriptions)?;

        let mut job_descriptions = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::ReadJobDescriptions)? {
            job_descriptions.push(decode(&row)?);
        }
        Ok(job_descriptions)
    }

    /// Deletes the Job Description with `id`, returning whether it existed.
    pub async fn delete_job_description(&self, id: i64) -> Result<bool, StoreError> {
        self.connection()
            .execute(
                "DELETE FROM job_description WHERE id = ?",
                vec![libsql::Value::Integer(id)],
            )
            .await
            .map(|changed| changed > 0)
            .map_err(StoreError::DeleteJobDescription)
    }
}

fn decode(row: &libsql::Row) -> Result<JobDescription, StoreError> {
    Ok(JobDescription {
        id: row.get(0).map_err(StoreError::ReadJobDescriptions)?,
        title: row.get(1).map_err(StoreError::ReadJobDescriptions)?,
        text: row.get(2).map_err(StoreError::ReadJobDescriptions)?,
        created_at: row.get(3).map_err(StoreError::ReadJobDescriptions)?,
    })
}

fn text(value: Option<&str>) -> libsql::Value {
    value.map_or(libsql::Value::Null, |value| {
        libsql::Value::Text(value.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    async fn open_temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store").join("hoskinator.db"))
            .await
            .expect("opening the store");
        (dir, store)
    }

    fn new(title: Option<&str>, text: &str) -> NewJobDescription {
        NewJobDescription {
            title: title.map(str::to_owned),
            text: text.to_owned(),
        }
    }

    #[tokio::test]
    async fn a_created_job_description_can_be_read_back() {
        let (_dir, store) = open_temp_store().await;
        let input = new(Some("Platform engineer"), "Build reliable Rust services.");

        let created = store.create_job_description(&input).await.unwrap();

        assert!(created.id > 0);
        assert_eq!(created.title, input.title);
        assert_eq!(created.text, input.text);
        assert!(!created.created_at.is_empty());
        assert_eq!(
            store.job_descriptions(None).await.unwrap(),
            vec![created.clone()]
        );
        assert_eq!(
            store.job_description(created.id).await.unwrap(),
            Some(created)
        );
    }

    #[tokio::test]
    async fn full_text_search_finds_a_saved_posting() {
        let (_dir, store) = open_temp_store().await;
        let rust = store
            .create_job_description(&new(
                Some("Systems engineer"),
                "Build low-latency Rust services.",
            ))
            .await
            .unwrap();
        store
            .create_job_description(&new(Some("Designer"), "Shape product interaction."))
            .await
            .unwrap();

        assert_eq!(
            store.job_descriptions(Some("Rust")).await.unwrap(),
            vec![rust]
        );
    }

    #[tokio::test]
    async fn deleting_a_job_description_removes_it_from_reads_and_search() {
        let (_dir, store) = open_temp_store().await;
        let created = store
            .create_job_description(&new(None, "Python automation role."))
            .await
            .unwrap();

        assert!(store.delete_job_description(created.id).await.unwrap());
        assert_eq!(store.job_description(created.id).await.unwrap(), None);
        assert!(
            store
                .job_descriptions(Some("Python"))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(!store.delete_job_description(created.id).await.unwrap());
    }

    #[tokio::test]
    async fn a_job_description_survives_reopening_the_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");
        let created = Store::open(&path)
            .await
            .unwrap()
            .create_job_description(&new(Some("Data engineer"), "Own warehouse pipelines."))
            .await
            .unwrap();

        let reopened = Store::open(&path).await.unwrap();

        assert_eq!(
            reopened.job_description(created.id).await.unwrap(),
            Some(created)
        );
    }
}
