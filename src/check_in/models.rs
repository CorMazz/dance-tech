use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    /// The name to be displayed to the user. Make it pretty
    pub name: String,
    /// An internal id, used in creating the url. Usually kebab-case.
    pub id: String,
    /// A description to be displayed to the user. Make it pretty.
    pub description: String,
    /// Price, in cents
    pub price: u64     
}
