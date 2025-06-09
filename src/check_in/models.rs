use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    /// The name to be displayed to the user. Make it pretty
    pub name: String,
    /// An internal id, used in creating the url. Usually kebab-case.
    pub id: String,
    /// A description to be displayed to the user. Make it pretty.
    pub description: String,
    /// Price to be displayed, ie "$5"
    pub price: String,
    /// Price ID from Stripe
    pub price_id: String
}

#[derive(Deserialize, Debug)]
/// The response from the Stripe Checkout Session API.
///
/// Used to let us direct users to Stripe to enact payment.
pub struct CheckoutSessionResponse {
    /// The Stripe url we need to redirect the user to.
    pub url: String
}
