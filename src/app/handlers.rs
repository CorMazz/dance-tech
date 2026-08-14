use super::views::{BulkGrantForm, ModifyUserQuery};
use crate::auth::{
    errors::AuthError,
    models::{Roles, User},
    utils::{get_user_by_email, get_user_by_id, grant_roles, revoke_roles},
};
use crate::exam::models::Test;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use tracing::instrument;

/// Roles admins can pick from a list, plus any custom role they type (for example `frozen`).
pub fn known_grantable_roles(tests: &[Test]) -> Vec<String> {
    let mut roles = HashSet::from(["admin".to_string(), "proctor".to_string()]);
    for test in tests {
        for role in &test.metadata.config.grants_roles {
            let role = role.trim().to_ascii_lowercase();
            if !role.is_empty() {
                roles.insert(role);
            }
        }
    }
    let mut roles: Vec<String> = roles.into_iter().collect();
    roles.sort();
    roles
}

/// Split pasted text on commas, semicolons, and whitespace. Lowercased, unique, order preserved.
pub fn parse_pasted_emails(raw: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut emails = Vec::new();
    for token in raw.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let email = token.trim().to_ascii_lowercase();
        if email.is_empty() || !seen.insert(email.clone()) {
            continue;
        }
        emails.push(email);
    }
    emails
}

/// Result of granting one role to a pasted email list.
pub struct BulkGrantOutcome {
    pub summary: String,
    pub not_found: Vec<String>,
    pub invalid: Vec<String>,
}

/// Delete a role from a user via the admin dashboard
#[instrument(skip(db))]
pub async fn delete_user_roles_widget_handler(
    query: ModifyUserQuery,
    db: &Pool<Postgres>,
) -> Result<(), AuthError> {
    let mut user = get_user_by_id(&query.user_id, db)
        .await?
        .ok_or(AuthError::InvalidUser)?;
    let roles_to_revoke = HashSet::from([Roles::new(&query.role)]);
    revoke_roles(&mut user, roles_to_revoke, db).await?;
    Ok(())
}

/// Add a role to a user via the admin dashboard
#[instrument(skip(db))]
pub async fn post_user_roles_widget_handler(
    query: ModifyUserQuery,
    db: &Pool<Postgres>,
) -> Result<Vec<User>, AuthError> {
    let mut user = get_user_by_id(&query.user_id, db)
        .await?
        .ok_or(AuthError::InvalidUser)?;
    let roles_to_add = HashSet::from([Roles::new(&query.role)]);
    grant_roles(&mut user, roles_to_add, db).await?;
    Ok(vec![user])
}

/// Grant one role to every matching account in a pasted email list.
#[instrument(skip(db, form))]
pub async fn post_user_roles_bulk_handler(
    form: BulkGrantForm,
    db: &Pool<Postgres>,
) -> Result<BulkGrantOutcome, AuthError> {
    let role_label = form.role.trim();
    if role_label.is_empty() {
        return Ok(BulkGrantOutcome {
            summary: "Enter a role.".to_string(),
            not_found: Vec::new(),
            invalid: Vec::new(),
        });
    }

    let role = Roles::new(role_label);
    let role_name = role.to_string();
    let pasted = parse_pasted_emails(&form.emails);
    if pasted.is_empty() {
        return Ok(BulkGrantOutcome {
            summary: "Paste at least one email.".to_string(),
            not_found: Vec::new(),
            invalid: Vec::new(),
        });
    }

    let mut granted = 0;
    let mut already_had = 0;
    let mut not_found = Vec::new();
    let mut invalid = Vec::new();

    for email in pasted {
        if !email.contains('@') {
            invalid.push(email);
            continue;
        }
        let Some(mut user) = get_user_by_email(&email, db).await? else {
            not_found.push(email);
            continue;
        };
        if user.has_role(role.clone()) {
            already_had += 1;
            continue;
        }
        grant_roles(&mut user, HashSet::from([role.clone()]), db).await?;
        granted += 1;
    }

    let summary = bulk_grant_summary(&role_name, granted, already_had);
    Ok(BulkGrantOutcome {
        summary,
        not_found,
        invalid,
    })
}

fn bulk_grant_summary(role: &str, granted: usize, already_had: usize) -> String {
    match (granted, already_had) {
        (0, 0) => format!("No matching accounts for {role}."),
        (0, already) => format!("No new grants of {role}. {already} already had it."),
        (granted, 0) => format!(
            "Granted {role} to {granted} account{}.",
            if granted == 1 { "" } else { "s" }
        ),
        (granted, already) => format!(
            "Granted {role} to {granted} account{} ({already} already had it).",
            if granted == 1 { "" } else { "s" }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pasted_emails_splits_and_dedupes() {
        let emails = parse_pasted_emails(
            "Abby@Example.com, pat@example.com\nbob@example.com; ABBY@example.com",
        );
        assert_eq!(
            emails,
            vec!["abby@example.com", "pat@example.com", "bob@example.com"]
        );
    }

    #[test]
    fn parse_pasted_emails_ignores_empty_tokens() {
        assert!(parse_pasted_emails("  \n, ; ").is_empty());
    }

    #[test]
    fn bulk_grant_summary_counts() {
        assert_eq!(
            bulk_grant_summary("frozen", 2, 1),
            "Granted frozen to 2 accounts (1 already had it)."
        );
        assert_eq!(
            bulk_grant_summary("frozen", 1, 0),
            "Granted frozen to 1 account."
        );
        assert_eq!(
            bulk_grant_summary("frozen", 0, 3),
            "No new grants of frozen. 3 already had it."
        );
    }
}
