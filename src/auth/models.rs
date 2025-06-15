use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, Clone, Hash, Eq, PartialEq)]
pub enum Roles {
    /// Confers administrator privileges for the site
    Admin,
    /// Allows users to administer tests
    Proctor,
    /// Users can have roles that do not exist at compile time assigned to them.
    /// This is to enable flexiblity in restricting check-in options based on what tests users have
    /// passed. For instance, if you pass the standard leader test, in the leader test definition
    /// it may specify that passing confers the role of "Advanced Leader". Then, on the Stripe API
    /// you can specify that in order to see the "Advanced Class (Leader)" lesson, users must have
    /// the `Roles::Dynamic("Advanced Leader")` role. 
    Dynamic(String)
}

impl Roles {
    pub fn new(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "admin" => Self::Admin,
            "proctor" => Self::Proctor,
            other => Self::Dynamic(other.to_string())
        }
    }
}

impl From<&str> for Roles {
    /// Added so that we can make the `User.has_role()` method accept either a `Roles` instance or
    /// an `&str`.
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password: String,
    /// Is Json so that it can be stored in the DB
    pub roles: Json<Vec<Roles>>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<DateTime<Utc>>,
}

impl User {
    /// Check if a user has a given Role instance or `&str` that can be converted into a
    /// `Roles::Dynamic()`
    pub fn has_role<T: Into<Roles>>(&self, role: T) -> bool {
        self.roles.contains(&role.into())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenDetails {
    pub token: Option<String>,
    pub token_uuid: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub expires_in: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub token_uuid: String,
    pub exp: i64,
    pub iat: i64,
    pub nbf: i64,
}
