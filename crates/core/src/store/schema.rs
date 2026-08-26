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
    entry (id) {
        id -> BigInt,
        entry_type -> Text,
        fields -> Text,
        date -> Nullable<Text>,
        start_date -> Nullable<Text>,
        end_date -> Nullable<Text>,
        created_at -> Text,
        search_text -> Text,
    }
}

diesel::table! {
    bullet (id) {
        id -> BigInt,
        entry_id -> BigInt,
        position -> Integer,
    }
}

diesel::table! {
    variant (id) {
        id -> BigInt,
        bullet_id -> BigInt,
        text -> Text,
        note -> Nullable<Text>,
        is_default -> Bool,
    }
}

diesel::table! {
    section (name) {
        name -> Text,
        entry_type -> Text,
    }
}

diesel::table! {
    application (id) {
        id -> BigInt,
        company -> Text,
        position -> Text,
        status -> Text,
        date_applied -> Nullable<Text>,
        listing_url -> Nullable<Text>,
        resume_branch -> Nullable<Text>,
        notes -> Nullable<Text>,
        jd_text -> Nullable<Text>,
        created_at -> Text,
    }
}
