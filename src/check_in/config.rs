use std::collections::BTreeMap;
use crate::app::utils::get_env_var;
use crate::check_in::models::Product;

#[derive(Debug, Clone)]
pub struct CheckInConfig {
    pub secret_key: String,
    pub products: BTreeMap<String, Product>,
}

impl CheckInConfig {
    pub fn init() -> Self {
        let secret_key = get_env_var("STRIPE_SECRET_KEY");

        let yaml_content = std::fs::read_to_string("products.yaml")
            .expect("Failed to read products.yaml");

        let products: Vec<Product> = serde_yaml::from_str(&yaml_content)
            .expect("Failed to parse products.yaml");

        let products: BTreeMap<String, Product> = products
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();

        Self {
            secret_key,
            products
        }
    }
}
