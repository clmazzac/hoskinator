//! Job applications, and the resume branch each was sent with.
//!
//! The tracker a user would otherwise keep in a spreadsheet. An application names the branch its
//! resume came from, which is the only link between a posting and a version of the resume — the
//! repository holds no reference back (ADR-0001).

use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::store::schema::application as application_table;
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
    pub notes: Option<String>,
    /// The pasted posting this application answers.
    pub jd_text: Option<String>,
    pub created_at: String,
}

/// The fields an application is created or updated with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Insertable, AsChangeset)]
#[diesel(table_name = application_table)]
pub struct NewApplication {
    pub company: String,
    pub position: String,
    pub status: String,
    pub date_applied: Option<String>,
    pub listing_url: Option<String>,
    pub resume_branch: Option<String>,
    pub notes: Option<String>,
    pub jd_text: Option<String>,
}

impl Store {
    /// Saves an application.
    pub async fn create_application(
        &self,
        application: &NewApplication,
    ) -> Result<Application, StoreError> {
        let row = application.clone();
        self.with_connection(move |connection| {
            diesel::insert_into(application_table::table)
                .values(row)
                .returning(Application::as_returning())
                .get_result(connection)
                .map_err(StoreError::WriteApplication)
        })
        .await
    }

    /// Every application, newest first.
    pub async fn applications(&self) -> Result<Vec<Application>, StoreError> {
        self.with_connection(move |connection| {
            application_table::table
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
            diesel::update(application_table::table.find(id))
                .set(row)
                .returning(Application::as_returning())
                .get_result(connection)
                .map_err(StoreError::WriteApplication)
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
