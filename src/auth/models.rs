use std::fmt;
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
    Dynamic(String),
}

impl Roles {
    pub fn new(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "admin" => Self::Admin,
            "proctor" => Self::Proctor,
            other => Self::Dynamic(other.to_string()),
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

/// Possibly refactor this into a `UserInfo` and `UserPrivate` set of structs to 
/// avoid passing the user's password around everytime I need the other info
/// Can use `#[sqlx(flatten)]` for this.
#[allow(non_snake_case)]
#[derive(Deserialize, sqlx::FromRow, Serialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password: String,
    /// Currently is Json because that's the only way I could get the damn thing to compile
    /// as opposed to just an Array in Postgres
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

    /// Check if the user is an Admin
    pub fn is_admin(&self) -> bool {
        self.has_role(Roles::Admin)
    }

    /// Check if the user is a Proctor
    pub fn is_proctor(&self) -> bool {
        self.has_role(Roles::Proctor)
    }
   
    /// Check if the user is an Admin or a Proctor
    pub fn is_superuser(&self) -> bool {
        self.is_admin() || self.is_proctor()
    }
}

impl fmt::Debug for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("first_name", &self.first_name)
            .field("last_name", &self.last_name)
            .field("email", &self.email)
            .field("roles", &self.roles)
            .finish_non_exhaustive()
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
