//! The tables `migrations/*.sql` creates, as Diesel sees them.
//!
//! Every column named here must exist in a migrated database, which
//! `every_declared_column_exists_after_migrating` checks.

diesel::table! {
    profile (id) {
        id -> Integer,
        name -> Nullable<Text>,
        headline -> Nullable<Text>,
        location -> Nullable<Text>,
        photo -> Nullable<Text>,
        email -> Nullable<Text>,
        phone -> Nullable<Text>,
        website -> Nullable<Text>,
        social_networks -> Nullable<Text>,
        custom_connections -> Nullable<Text>,
    }
}

diesel::table! {
    job_description (id) {
        id -> BigInt,
        title -> Nullable<Text>,
        text -> Text,
        created_at -> Text,
    }
}

diesel::table! {
    section (name) {
        name -> Text,
        entry_type -> Text,
    }
}
