use super::views::ModifyUserQuery;
use crate::auth::{
    errors::AuthError,
    models::{Roles, User},
    utils::{get_user_by_id, grant_roles, revoke_roles},
};
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use tracing::instrument;

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
