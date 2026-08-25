//! How branch names carry the resume hierarchy.
//!
//! An archetype is a master resume for a kind of role; an application is one tailored from it.
//! The two live under separate prefixes because a git ref is a file: `archetype/systems` and
//! `archetype/systems/acme` cannot both exist, so a child never nests under its parent's name.

use serde::{Deserialize, Serialize};

/// Prefix for a master resume covering one kind of role.
pub const ARCHETYPE: &str = "archetype";

/// Prefix for a resume tailored to one application.
pub const APPLICATION: &str = "apply";

/// What a branch is, read from its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Lineage {
    /// The trunk every archetype starts from.
    Trunk,
    /// A master resume for `slug`.
    Archetype { slug: String },
    /// A resume tailored from the archetype `slug` for `target`.
    Application { slug: String, target: String },
    /// A branch that follows no convention.
    Loose,
}

/// Reads what a branch is from its name.
pub fn read(branch: &str) -> Lineage {
    if branch == "main" || branch == "master" {
        return Lineage::Trunk;
    }
    if let Some(slug) = branch.strip_prefix(&format!("{ARCHETYPE}/"))
        && !slug.is_empty()
        && !slug.contains('/')
    {
        return Lineage::Archetype {
            slug: slug.to_string(),
        };
    }
    if let Some(rest) = branch.strip_prefix(&format!("{APPLICATION}/"))
        && let Some((slug, target)) = rest.split_once('/')
        && !slug.is_empty()
        && !target.is_empty()
        && !target.contains('/')
    {
        return Lineage::Application {
            slug: slug.to_string(),
            target: target.to_string(),
        };
    }
    Lineage::Loose
}

/// The branch a given branch inherits from.
pub fn parent(branch: &str) -> Option<String> {
    match read(branch) {
        Lineage::Archetype { .. } => Some("main".to_string()),
        Lineage::Application { slug, .. } => Some(archetype_branch(&slug)),
        Lineage::Trunk | Lineage::Loose => None,
    }
}

/// The branch name for an archetype.
pub fn archetype_branch(slug: &str) -> String {
    format!("{ARCHETYPE}/{}", slugify(slug))
}

/// The branch name for a resume tailored from `slug` to `target`.
pub fn application_branch(slug: &str, target: &str) -> String {
    format!("{APPLICATION}/{}/{}", slugify(slug), slugify(target))
}

/// Reduces a label to the characters a branch name may hold.
pub fn slugify(label: &str) -> String {
    let mut slug = String::with_capacity(label.len());
    let mut dashed = false;
    for character in label.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            dashed = false;
        } else if !dashed && !slug.is_empty() {
            slug.push('-');
            dashed = true;
        }
    }
    slug.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trunk_is_read_by_either_name() {
        assert_eq!(read("main"), Lineage::Trunk);
        assert_eq!(read("master"), Lineage::Trunk);
    }

    #[test]
    fn prefixes_name_the_two_kinds() {
        assert_eq!(
            read("archetype/systems"),
            Lineage::Archetype {
                slug: "systems".into()
            }
        );
        assert_eq!(
            read("apply/systems/acme"),
            Lineage::Application {
                slug: "systems".into(),
                target: "acme".into(),
            }
        );
    }

    #[test]
    fn anything_else_is_loose() {
        assert_eq!(read("scratch"), Lineage::Loose);
        assert_eq!(read("archetype/systems/extra"), Lineage::Loose);
    }

    #[test]
    fn an_application_inherits_from_its_archetype() {
        assert_eq!(
            parent("apply/systems/acme"),
            Some("archetype/systems".into())
        );
        assert_eq!(parent("archetype/systems"), Some("main".into()));
        assert_eq!(parent("main"), None);
    }

    #[test]
    fn names_are_built_from_labels() {
        assert_eq!(
            archetype_branch("Systems Programmer"),
            "archetype/systems-programmer"
        );
        assert_eq!(
            application_branch("Systems Programmer", "Acme Corp."),
            "apply/systems-programmer/acme-corp"
        );
    }

    #[test]
    fn the_two_prefixes_never_collide_as_refs() {
        // A git ref is a file, so a child must not nest under its parent's name.
        let archetype = archetype_branch("systems");
        let application = application_branch("systems", "acme");
        assert!(!application.starts_with(&format!("{archetype}/")));
    }
}
