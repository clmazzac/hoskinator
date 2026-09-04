//! Job applications, and the resume branch each was sent with.
//!
//! The tracker a user would otherwise keep in a spreadsheet. An application names the branch its
//! resume came from, which is the only link between a posting and a version of the resume — the
//! repository holds no reference back (ADR-0001).
//!
//! Tracked in the store, not in the repository (docs/decisions/workspace.md). Each is scoped to
//! the resume repository it was tracked against.

use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::store::schema::application as application_table;
use crate::store::schema::job_description as job_description_table;
use crate::store::{Store, StoreError};

/// Where an application has got to.
pub const STATUSES: &[&str] = &["draft", "applied", "interview", "offer", "rejected"];

/// Statuses that mean the resume behind them is settled and no longer worth editing.
pub const SETTLED: &[&str] = &["offer", "rejected"];

/// One application, as the store holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name = application_table)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Application {
    pub id: i64,
    pub company: String,
    pub position: String,
    pub status: String,
    pub date_applied: Option<String>,
    pub listing_url: Option<String>,
    /// The branch whose resume was sent, if one was.
    pub resume_branch: Option<String>,
    /// The Drive link for the resume that was actually sent, filled in by hand.
    pub resume_drive_link: Option<String>,
    pub notes: Option<String>,
    /// The pasted posting this application answers.
    pub jd_text: Option<String>,
    pub created_at: String,
}

/// The fields an application is created or updated with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Insertable, AsChangeset)]
#[diesel(table_name = application_table)]
#[diesel(treat_none_as_null = true)]
pub struct NewApplication {
    pub company: String,
    pub position: String,
    pub status: String,
    pub date_applied: Option<String>,
    pub listing_url: Option<String>,
    pub resume_branch: Option<String>,
    pub resume_drive_link: Option<String>,
    pub notes: Option<String>,
    pub jd_text: Option<String>,
}

impl Store {
    /// Saves an application, scoped to the resume repository currently in use.
    pub async fn create_application(
        &self,
        application: &NewApplication,
        repository: &str,
    ) -> Result<Application, StoreError> {
        let row = application.clone();
        let repository = repository.to_string();
        self.with_connection(move |connection| {
            let created = diesel::insert_into(application_table::table)
                .values((application_table::repository.eq(repository), row))
                .returning(Application::as_returning())
                .get_result::<Application>(connection)
                .map_err(StoreError::WriteApplication)?;

            sync_job_description(connection, &created).map_err(StoreError::WriteApplication)?;

            Ok(created)
        })
        .await
    }

    /// Every application tracked against `repository`, newest first.
    pub async fn applications(&self, repository: &str) -> Result<Vec<Application>, StoreError> {
        let repository = repository.to_string();
        self.with_connection(move |connection| {
            application_table::table
                .filter(application_table::repository.eq(repository))
                .order(application_table::id.desc())
                .select(Application::as_select())
                .load(connection)
                .map_err(StoreError::ReadApplications)
        })
        .await
    }

    /// Replaces an application's fields.
    pub async fn update_application(
        &self,
        id: i64,
        application: &NewApplication,
    ) -> Result<Application, StoreError> {
        let row = application.clone();
        self.with_connection(move |connection| {
            let updated = diesel::update(application_table::table.find(id))
                .set(row)
                .returning(Application::as_returning())
                .get_result::<Application>(connection)
                .map_err(StoreError::WriteApplication)?;

            sync_job_description(connection, &updated).map_err(StoreError::WriteApplication)?;

            Ok(updated)
        })
        .await
    }

    /// Removes an application.
    pub async fn delete_application(&self, id: i64) -> Result<(), StoreError> {
        self.with_connection(move |connection| {
            diesel::delete(application_table::table.find(id))
                .execute(connection)
                .map(|_| ())
                .map_err(StoreError::WriteApplication)
        })
        .await
    }
}

/// Keeps the Job Description an application's `jd_text` implies in step with it: created when
/// pasted, updated as it or the company/position change, removed once cleared. `ON DELETE
/// CASCADE` (migration 0011) handles the application itself being deleted.
fn sync_job_description(
    connection: &mut SqliteConnection,
    application: &Application,
) -> QueryResult<()> {
    let linked: Option<i64> = job_description_table::table
        .filter(job_description_table::application_id.eq(application.id))
        .select(job_description_table::id)
        .first(connection)
        .optional()?;

    let text = application
        .jd_text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());

    match (linked, text) {
        (Some(existing), None) => {
            diesel::delete(job_description_table::table.find(existing)).execute(connection)?;
        }
        (Some(existing), Some(text)) => {
            diesel::update(job_description_table::table.find(existing))
                .set((
                    job_description_table::title.eq(Some(format!(
                        "{} — {}",
                        application.company, application.position
                    ))),
                    job_description_table::text.eq(text),
                ))
                .execute(connection)?;
        }
        (None, Some(text)) => {
            diesel::insert_into(job_description_table::table)
                .values((
                    job_description_table::application_id.eq(Some(application.id)),
                    job_description_table::title.eq(Some(format!(
                        "{} — {}",
                        application.company, application.position
                    ))),
                    job_description_table::text.eq(text),
                ))
                .execute(connection)?;
        }
        (None, None) => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    async fn open_temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");
        let store = Store::open(&path).await.expect("opening the store");
        (dir, store)
    }

    fn sample(company: &str) -> NewApplication {
        NewApplication {
            company: company.to_string(),
            position: "Engineer".to_string(),
            status: "draft".to_string(),
            date_applied: None,
            listing_url: None,
            resume_branch: None,
            resume_drive_link: None,
            notes: None,
            jd_text: None,
        }
    }

    #[tokio::test]
    async fn a_created_application_carries_the_fields_it_was_given() {
        let (_dir, store) = open_temp_store().await;

        let created = store
            .create_application(&sample("Acme"), "owner/one")
            .await
            .unwrap();

        assert_eq!(created.company, "Acme");
        assert_eq!(created.position, "Engineer");
        assert_eq!(created.status, "draft");
    }

    #[tokio::test]
    async fn updating_with_none_actually_clears_an_optional_field() {
        let (_dir, store) = open_temp_store().await;

        let created = store
            .create_application(&sample("Acme"), "owner/one")
            .await
            .unwrap();
        store
            .update_application(
                created.id,
                &NewApplication {
                    notes: Some("call back Tuesday".to_string()),
                    ..sample("Acme")
                },
            )
            .await
            .unwrap();

        let cleared = store
            .update_application(created.id, &sample("Acme"))
            .await
            .unwrap();

        assert_eq!(cleared.notes, None);
    }

    #[tokio::test]
    async fn an_application_is_scoped_to_the_repository_it_was_created_against() {
        let (_dir, store) = open_temp_store().await;

        store
            .create_application(&sample("Acme"), "owner/one")
            .await
            .unwrap();

        let one = store.applications("owner/one").await.unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].company, "Acme");

        let other = store.applications("owner/two").await.unwrap();
        assert!(other.is_empty(), "got {other:?}");
    }

    #[tokio::test]
    async fn switching_repositories_does_not_carry_applications_along() {
        let (_dir, store) = open_temp_store().await;

        store
            .create_application(&sample("Old Co"), "owner/old-repo")
            .await
            .unwrap();
        store
            .create_application(&sample("New Co"), "owner/new-repo")
            .await
            .unwrap();

        let old_repo = store.applications("owner/old-repo").await.unwrap();
        let new_repo = store.applications("owner/new-repo").await.unwrap();

        assert_eq!(
            old_repo.iter().map(|a| &a.company).collect::<Vec<_>>(),
            vec!["Old Co"]
        );
        assert_eq!(
            new_repo.iter().map(|a| &a.company).collect::<Vec<_>>(),
            vec!["New Co"]
        );
    }

    fn with_jd(company: &str, jd_text: &str) -> NewApplication {
        NewApplication {
            jd_text: Some(jd_text.to_string()),
            ..sample(company)
        }
    }

    #[tokio::test]
    async fn a_posting_creates_a_linked_job_description() {
        let (_dir, store) = open_temp_store().await;

        let created = store
            .create_application(&with_jd("Acme", "Build things."), "owner/one")
            .await
            .unwrap();

        let jds = store.job_descriptions(None).await.unwrap();
        assert_eq!(jds.len(), 1);
        assert_eq!(jds[0].application_id, Some(created.id));
        assert_eq!(jds[0].text, "Build things.");
        assert_eq!(jds[0].title.as_deref(), Some("Acme — Engineer"));
    }

    #[tokio::test]
    async fn no_posting_creates_no_job_description() {
        let (_dir, store) = open_temp_store().await;

        store
            .create_application(&sample("Acme"), "owner/one")
            .await
            .unwrap();

        assert!(store.job_descriptions(None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn editing_the_posting_updates_the_same_job_description() {
        let (_dir, store) = open_temp_store().await;

        let created = store
            .create_application(&with_jd("Acme", "Build things."), "owner/one")
            .await
            .unwrap();

        store
            .update_application(created.id, &with_jd("Acme Corp", "Build bigger things."))
            .await
            .unwrap();

        let jds = store.job_descriptions(None).await.unwrap();
        assert_eq!(
            jds.len(),
            1,
            "editing should not add a second job description"
        );
        assert_eq!(jds[0].text, "Build bigger things.");
        assert_eq!(jds[0].title.as_deref(), Some("Acme Corp — Engineer"));
    }

    #[tokio::test]
    async fn clearing_the_posting_removes_the_linked_job_description() {
        let (_dir, store) = open_temp_store().await;

        let created = store
            .create_application(&with_jd("Acme", "Build things."), "owner/one")
            .await
            .unwrap();

        store
            .update_application(created.id, &sample("Acme"))
            .await
            .unwrap();

        assert!(store.job_descriptions(None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn deleting_the_application_removes_its_linked_job_description() {
        let (_dir, store) = open_temp_store().await;

        let created = store
            .create_application(&with_jd("Acme", "Build things."), "owner/one")
            .await
            .unwrap();

        store.delete_application(created.id).await.unwrap();

        assert!(store.job_descriptions(None).await.unwrap().is_empty());
    }
}
