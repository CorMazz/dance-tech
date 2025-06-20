use crate::app::utils::get_env_var;
use crate::check_in::models::Product;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone)]
pub struct CheckInConfig {
    /// The Stripe API secret key
    pub secret_key: String,
    /// A message channel to request the list of available products from the `ProductManager` actor
    pub product_request_tx: mpsc::Sender<oneshot::Sender<Vec<Product>>>,
    /// A message channel to trigger the `ProductManager` actor to query Stripe for updates to the
    /// list of products
    pub trigger_update_tx: mpsc::Sender<()>,
}

impl CheckInConfig {
    pub fn init(
        product_request_tx: mpsc::Sender<oneshot::Sender<Vec<Product>>>,
        trigger_update_tx: mpsc::Sender<()>,
    ) -> Self {
        let secret_key = get_env_var("STRIPE_SECRET_KEY");

        Self {
            secret_key,
            product_request_tx,
            trigger_update_tx,
        }
    }
}
