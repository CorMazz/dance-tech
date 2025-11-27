use crate::auth::models::Roles;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// The Product item to be used for this application
///
/// The Stripe API has its own definition for the Product item that has information that we don't
/// necessarily need.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    /// The name to be displayed to the user. Make it pretty
    pub name: String,
    /// An internal id, used in creating the url.
    pub id: String,
    /// A description to be displayed to the user. Make it pretty.
    pub description: String,
    /// Price in dollars
    pub dollar_price: f64,
    /// Price ID from Stripe
    pub price_id: String,
    /// The roles a user must have to view this item on the `check-in` page.
    pub requires_roles: HashSet<Roles>,
    /// Show a greyed out preview of the product to people who don't yet have
    /// the requisite roles. The default is `false`.
    pub show_preview: bool,
    /// A higher number puts this product further down the list on the check-in page. 
    /// If a product doesn't have it specified, the default is 0. All products on the same level
    /// are sorted lexicographically.
    pub sort_level: i32
}

/// Store a hashmap of `product_id`: (product, quantity) pairs.
#[derive(Debug, Serialize, Deserialize)]
pub struct ShoppingCart {
    pub items: std::collections::HashMap<String, (Product, u64)>,
}

impl ShoppingCart {
    pub fn new() -> Self {
        Self { items: std::collections::HashMap::new() }
    }

    /// Add a new item to the shopping cart or increment quantity if it already exists
    pub fn add_item(&mut self, product_id: &str, product: Product, quantity: u64) {
        if quantity == 0 {
            self.items.remove(product_id);
        } else {
            self.items
                .entry(product_id.to_string())
                .and_modify(|(_, q)| *q += quantity)
                .or_insert((product, quantity));
        }
    }
    
    /// Update the quantity of a specific item
    pub fn update_item(&mut self, product_id: &str, quantity: u64) {
        if quantity == 0 {
            self.items.remove(product_id);
        } else if let Some((_product, qty)) = self.items.get_mut(product_id) {
            *qty = quantity;
        }
    }

    /// Total number of items in cart
    pub fn total_items(&self) -> usize {
        self.items.len()
    }

    /// Total quantity of all items in cart
    pub fn total_quantity(&self) -> u64 {
        self.items.values().map(|(_, qty)| *qty).sum()
    }

    /// Price times quantity for all items, in dollars.
    pub fn subtotal(&self) -> f64 {
        self.items.values()
            .map(|(product, quantity)| product.dollar_price * *quantity as f64).sum()
    }
}


/// The response from the Stripe Checkout Session API.
///
/// Used to let us direct users to Stripe to enact payment.
///
/// The full response looks like this:
/// ```
/// {
///   "id": "cs_test_a1xDgt75iRy0gfNypidMbbgeDm2C5FBZUmKNHMsMRgPU2ntor9i7bsKmJA",
///   "object": "checkout.session",
///   "adaptive_pricing": {
///     "enabled": true
///   },
///   "after_expiration": null,
///   "allow_promotion_codes": null,
///   "amount_subtotal": 500,
///   "amount_total": 500,
///   "automatic_tax": {
///     "enabled": false,
///     "liability": null,
///     "provider": null,
///     "status": null
///   },
///   "billing_address_collection": null,
///   "cancel_url": "http://localhost:8000/stripe/cancel",
///   "client_reference_id": null,
///   "client_secret": null,
///   "collected_information": null,
///   "consent": null,
///   "consent_collection": null,
///   "created": 1749590442,
///   "currency": "usd",
///   "currency_conversion": null,
///   "custom_fields": [],
///   "custom_text": {
///     "after_submit": null,
///     "shipping_address": null,
///     "submit": null,
///     "terms_of_service_acceptance": null
///   },
///   "customer": null,
///   "customer_creation": "if_required",
///   "customer_details": null,
///   "customer_email": null,
///   "discounts": [],
///   "expires_at": 1749676842,
///   "invoice": null,
///   "invoice_creation": {
///     "enabled": false,
///     "invoice_data": {
///       "account_tax_ids": null,
///       "custom_fields": null,
///       "description": null,
///       "footer": null,
///       "issuer": null,
///       "metadata": {},
///       "rendering_options": null
///     }
///   },
///   "livemode": false,
///   "locale": null,
///   "metadata": {},
///   "mode": "payment",
///   "payment_intent": null,
///   "payment_link": null,
///   "payment_method_collection": "if_required",
///   "payment_method_configuration_details": {
///     "id": "pmc_1RWqgNQ2wtFMkx0YuoIH56VJ",
///     "parent": null
///   },
///   "payment_method_options": {
///     "card": {
///       "request_three_d_secure": "automatic"
///     }
///   },
///   "payment_method_types": [
///     "card",
///     "klarna",
///     "link",
///     "cashapp",
///     "amazon_pay"
///   ],
///   "payment_status": "unpaid",
///   "permissions": null,
///   "phone_number_collection": {
///     "enabled": false
///   },
///   "recovered_from": null,
///   "saved_payment_method_options": null,
///   "setup_intent": null,
///   "shipping_address_collection": null,
///   "shipping_cost": null,
///   "shipping_options": [],
///   "status": "open",
///   "submit_type": null,
///   "subscription": null,
///   "success_url": "http://localhost:8000/stripe/success?session_id={CHECKOUT_SESSION_ID}",
///   "total_details": {
///     "amount_discount": 0,
///     "amount_shipping": 0,
///     "amount_tax": 0
///   },
///   "ui_mode": "hosted",
///   "url": "https://checkout.stripe.com/c/pay/cs_test_a1xDgt75iRy0gfNypidMbbgeDm2C5FBZUmKNHMsMRgPU2ntor9i7bsKmJA#fidkdWxOYHwnP
/// yd1blpxYHZxWjA0V1J0Y3RUN3JxQ0hufTVcQ2ZyU2pAPX18UnFkS0JCYzNwTXVsREBCbk51XWB2Q1JnT2B2UkB8PXB8Z3xPd11JblNxQ0FNVVJGZ2pkdzNjSn1QZ
/// mc3UmtqNTVLXUJoVzB%2FTScpJ2N3amhWYHdzYHcnP3F3cGApJ2lkfGpwcVF8dWAnPyd2bGtiaWBabHFgaCcpJ2BrZGdpYFVpZGZgbWppYWB3dic%2FcXdwYHgl"
/// ,
///   "wallet_options": null
/// }
/// ```
#[derive(Deserialize, Debug)]
pub struct CheckoutSessionResponse {
    /// The Stripe url we need to redirect the user to.
    pub url: String,
}

/// The response from the Stripe API to verify a Checkout Session
///
/// Used to let us ensure users have paid.
///
/// The response to the request here looks like this:
/// ```
/// {
///   "id": "cs_test_b1vh2rtcq0dqyOLKqQDIhEEh9Hb43ankyYnnt13zSdPZb7pzlSzz6BvNuu",
///   "object": "checkout.session",
///   "adaptive_pricing": {
///     "enabled": true
///   },
///   "after_expiration": null,
///   "allow_promotion_codes": true,
///   "amount_subtotal": 1000,
///   "amount_total": 0,
///   "automatic_tax": {
///     "enabled": false,
///     "liability": null,
///     "provider": null,
///     "status": null
///   },
///   "billing_address_collection": null,
///   "branding_settings": {
///     "background_color": "#ffffff",
///     "border_style": "rounded",
///     "button_color": "#0074d4",
///     "display_name": "Greenville Westies sandbox",
///     "font_family": "default",
///     "icon": null,
///     "logo": null
///   },
///   "cancel_url": "http://localhost/check-in",
///   "client_reference_id": null,
///   "client_secret": null,
///   "collected_information": {
///     "business_name": null,
///     "individual_name": "Corrado Mazzarelli",
///     "shipping_details": null
///   },
///   "consent": {
///     "promotions": null,
///     "terms_of_service": "accepted"
///   },
///   "consent_collection": {
///     "payment_method_reuse_agreement": null,
///     "promotions": "none",
///     "terms_of_service": "required"
///   },
///   "created": 1764213792,
///   "currency": "usd",
///   "currency_conversion": null,
///   "custom_fields": [],
///   "custom_text": {
///     "after_submit": null,
///     "shipping_address": null,
///     "submit": null,
///     "terms_of_service_acceptance": {
///       "message": "I agree to the terms of the [Liability Waiver](https://google.com/)"
///     }
///   },
///   "customer": null,
///   "customer_creation": "if_required",
///   "customer_details": {
///     "address": null,
///     "business_name": null,
///     "email": "corrado@mazzarelli.biz",
///     "individual_name": "Corrado Mazzarelli",
///     "name": "Corrado Mazzarelli",
///     "phone": null,
///     "tax_exempt": "none",
///     "tax_ids": []
///   },
///   "customer_email": null,
///   "discounts": [
///     {
///       "coupon": null,
///       "promotion_code": "promo_1SXPsPQ2wtFMkx0Y2fmEZ9MO"
///     }
///   ],
///   "expires_at": 1764300192,
///   "invoice": null,
///   "invoice_creation": {
///     "enabled": false,
///     "invoice_data": {
///       "account_tax_ids": null,
///       "custom_fields": null,
///       "description": null,
///       "footer": null,
///       "issuer": null,
///       "metadata": {},
///       "rendering_options": null
///     }
///   },
///   "line_items": {
///     "object": "list",
///     "data": [
///       {
///         "id": "li_1SXvnYQ2wtFMkx0YI8Hyb20W",
///         "object": "item",
///         "amount_discount": 500,
///         "amount_subtotal": 500,
///         "amount_tax": 0,
///         "amount_total": 0,
///         "currency": "usd",
///         "description": "Social Dance",
///         "price": {
///           "id": "price_1RWqlNQ2wtFMkx0YHyrIPuFM",
///           "object": "price",
///           "active": true,
///           "billing_scheme": "per_unit",
///           "created": 1749179773,
///           "currency": "usd",
///           "custom_unit_amount": null,
///           "livemode": false,
///           "lookup_key": null,
///           "metadata": {},
///           "nickname": "Standard price for the social dance.",
///           "product": "prod_SRkFO2xHNexIoT",
///           "recurring": null,
///           "tax_behavior": "unspecified",
///           "tiers_mode": null,
///           "transform_quantity": null,
///           "type": "one_time",
///           "unit_amount": 500,
///           "unit_amount_decimal": "500"
///         },
///         "quantity": 1
///       },
///       {
///         "id": "li_1SXvnYQ2wtFMkx0Y6t7YjtNA",
///         "object": "item",
///         "amount_discount": 500,
///         "amount_subtotal": 500,
///         "amount_tax": 0,
///         "amount_total": 0,
///         "currency": "usd",
///         "description": "Beginner+ Lesson & Social",
///         "price": {
///           "id": "price_1RYugeQ2wtFMkx0YZv6Jz06E",
///           "object": "price",
///           "active": true,
///           "billing_scheme": "per_unit",
///           "created": 1749671512,
///           "currency": "usd",
///           "custom_unit_amount": null,
///           "livemode": false,
///           "lookup_key": null,
///           "metadata": {},
///           "nickname": null,
///           "product": "prod_STsRL6dA7jkiP0",
///           "recurring": null,
///           "tax_behavior": "unspecified",
///           "tiers_mode": null,
///           "transform_quantity": null,
///           "type": "one_time",
///           "unit_amount": 500,
///           "unit_amount_decimal": "500"
///         },
///         "quantity": 1
///       }
///     ],
///     "has_more": false,
///     "url": "/v1/checkout/sessions/cs_test_b1vh2rtcq0dqyOLKqQDIhEEh9Hb43ankyYnnt13zSdPZb7pzlSzz6BvNuu/line_items"
///   },
///   "livemode": false,
///   "locale": null,
///   "metadata": {},
///   "mode": "payment",
///   "name_collection": {
///     "individual": {
///       "enabled": true,
///       "optional": false
///     }
///   },
///   "origin_context": null,
///   "payment_intent": null,
///   "payment_link": null,
///   "payment_method_collection": "if_required",
///   "payment_method_configuration_details": {
///     "id": "pmc_1RWqgNQ2wtFMkx0YuoIH56VJ",
///     "parent": null
///   },
///   "payment_method_options": {
///     "card": {
///       "request_three_d_secure": "automatic"
///     }
///   },
///   "payment_method_types": [
///     "card",
///     "klarna",
///     "link",
///     "cashapp",
///     "amazon_pay"
///   ],
///   "payment_status": "paid",
///   "permissions": null,
///   "phone_number_collection": {
///     "enabled": false
///   },
///   "recovered_from": null,
///   "saved_payment_method_options": null,
///   "setup_intent": null,
///   "shipping_address_collection": null,
///   "shipping_cost": null,
///   "shipping_options": [],
///   "status": "complete",
///   "submit_type": null,
///   "subscription": null,
///   "success_url": "http://localhost/stripe/success?session_id={CHECKOUT_SESSION_ID}",
///   "total_details": {
///     "amount_discount": 1000,
///     "amount_shipping": 0,
///     "amount_tax": 0
///   },
///   "ui_mode": "hosted",
///   "url": null,
///   "wallet_options": null
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct StripeCheckoutSession {
    pub payment_status: String,
    pub line_items: LineItems,
    pub customer_details: CustomerDetails,
    /// Theoretically, Checkout Sessions expire after 1 day. So if we are past the expiry date,
    /// this should no longer be valid to show the success page.
    /// So that we can ensure users can't use old checkout success pages to trick the people at the
    /// front desk into believing they have already paid.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub expires_at: DateTime<Utc>,
}

/// Grab this information so that we can show what product was purchased on the success page.
#[derive(Debug, Deserialize)]
pub struct CustomerDetails {
    pub name: String,
}

/// Grab this information so that we can show what product was purchased on the success page.
#[derive(Debug, Deserialize)]
pub struct LineItems {
    pub data: Vec<LineItem>,
}

/// Grab this information so that we can show what product was purchased on the success page.
#[derive(Debug, Deserialize)]
pub struct LineItem {
    pub description: String,
    pub amount_total: u64,  // This is in cents
    pub quantity: u64,
}

/// A Product as returned by Stripe's search product API.
#[derive(Debug, Deserialize)]
pub struct StripeProduct {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_price: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// A struct to deserialize Stripe's search API response when looking for what
/// products are available.
///
/// The full response looks like this:
/// ```
///{
///   "object": "search_result",
///   "data": [
///     {
///       "id": "prod_STsRL6dA7jkiP0",
///       "object": "product",
///       "active": true,
///       "attributes": [],
///       "created": 1749671512,
///       "default_price": "price_1RYugeQ2wtFMkx0YZv6Jz06E",
///       "description": "The beginner+ lesson is for dancers who have come to the beginner class for at least 4 weeks and have
/// grasped the concepts within.",
///       "images": [],
///       "livemode": false,
///       "marketing_features": [],
///       "metadata": {
///         "show-on-dancetech": "true"
///       },
///       "name": "Beginner+ Lesson & Social",
///       "package_dimensions": null,
///       "shippable": null,
///       "statement_descriptor": null,
///       "tax_code": null,
///       "type": "service",
///       "unit_label": null,
///       "updated": 1749671535,
///       "url": null
///     },
///     {
///       "id": "prod_SRkFO2xHNexIoT",
///       "object": "product",
///       "active": true,
///       "attributes": [],
///       "created": 1749179773,
///       "default_price": "price_1RWqlNQ2wtFMkx0YHyrIPuFM",
///       "description": "The beginner lesson is free, with the social dance. Or vice versa. Whatever floats your boat :)",
///       "images": [],
///       "livemode": false,
///       "marketing_features": [],
///       "metadata": {
///         "metadata_key": "metadata_value",
///         "metadata_key_2": "metadata_value_2",
///         "show-on-dancetech": "true"
///       },
///       "name": "Social Dance",
///       "package_dimensions": null,
///       "shippable": null,
///       "statement_descriptor": null,
///       "tax_code": null,
///       "type": "service",
///       "unit_label": null,
///       "updated": 1749670706,
///       "url": null
///     }
///   ],
///   "has_more": false,
///   "next_page": null,
///   "url": "/v1/products/search"
/// }
///
/// ```
#[derive(Debug, Deserialize)]
pub struct StripeProductSearchResponse {
    /// The actual products available on the Stripe API
    pub data: Vec<StripeProduct>,
    /// A string to indicate if there is another page of data to request
    pub has_more: bool,
    /// A tag to add to the query to indicate what page to request
    pub next_page: Option<String>,
}

/// A price object returned by the Stripe API
///
/// These objects look like this:
/// ```
///{
///   "id": "price_1RYugeQ2wtFMkx0YZv6Jz06E",
///   "object": "price",
///   "active": true,
///   "billing_scheme": "per_unit",
///   "created": 1749671512,
///   "currency": "usd",
///   "custom_unit_amount": null,
///   "livemode": false,
///   "lookup_key": null,
///   "metadata": {},
///   "nickname": null,
///   "product": "prod_STsRL6dA7jkiP0",
///   "recurring": null,
///   "tax_behavior": "unspecified",
///   "tiers_mode": null,
///   "transform_quantity": null,
///   "type": "one_time",
///   "unit_amount": 500,
///   "unit_amount_decimal": "500"
/// },
/// ```
#[derive(Debug, Deserialize)]
pub struct StripePrice {
    pub id: String,
    pub unit_amount: Option<i64>,
}

/// A list of prices returned by the Stripe Prices API
///
///
/// The response may be paginated by Stripe. See the `has_more` field.
/// The list looks like this:
/// ```
///{
///   "object": "list",
///   "data": [
///     {
///       "id": "price_1RYugeQ2wtFMkx0YZv6Jz06E",
///       "object": "price",
///       "active": true,
///       "billing_scheme": "per_unit",
///       "created": 1749671512,
///       "currency": "usd",
///       "custom_unit_amount": null,
///       "livemode": false,
///       "lookup_key": null,
///       "metadata": {},
///       "nickname": null,
///       "product": "prod_STsRL6dA7jkiP0",
///       "recurring": null,
///       "tax_behavior": "unspecified",
///       "tiers_mode": null,
///       "transform_quantity": null,
///       "type": "one_time",
///       "unit_amount": 500,
///       "unit_amount_decimal": "500"
///     },
///     {
///       "id": "price_1RYtlHQ2wtFMkx0YncfpRB4h",
///       "object": "price",
///       "active": true,
///       "billing_scheme": "per_unit",
///       "created": 1749667955,
///       "currency": "usd",
///       "custom_unit_amount": null,
///       "livemode": false,
///       "lookup_key": null,
///       "metadata": {},
///       "nickname": "Nice price",
///       "product": "prod_SRkFO2xHNexIoT",
///       "recurring": null,
///       "tax_behavior": "unspecified",
///       "tiers_mode": null,
///       "transform_quantity": null,
///       "type": "one_time",
///       "unit_amount": 6900,
///       "unit_amount_decimal": "6900"
///     },
///     {
///       "id": "price_1RWqnPQ2wtFMkx0YXDSw4GiG",
///       "object": "price",
///       "active": true,
///       "billing_scheme": "per_unit",
///       "created": 1749179899,
///       "currency": "usd",
///       "custom_unit_amount": null,
///       "livemode": false,
///       "lookup_key": null,
///       "metadata": {},
///       "nickname": "Standard fee for the pro lesson.",
///       "product": "prod_SRkHmJL8FGGjTF",
///       "recurring": null,
///       "tax_behavior": "unspecified",
///       "tiers_mode": null,
///       "transform_quantity": null,
///       "type": "one_time",
///       "unit_amount": 1000,
///       "unit_amount_decimal": "1000"
///     },
///     {
///       "id": "price_1RWqlNQ2wtFMkx0YHyrIPuFM",
///       "object": "price",
///       "active": true,
///       "billing_scheme": "per_unit",
///       "created": 1749179773,
///       "currency": "usd",
///       "custom_unit_amount": null,
///       "livemode": false,
///       "lookup_key": null,
///       "metadata": {},
///       "nickname": "Standard price for the social dance.",
///       "product": "prod_SRkFO2xHNexIoT",
///       "recurring": null,
///       "tax_behavior": "unspecified",
///       "tiers_mode": null,
///       "transform_quantity": null,
///       "type": "one_time",
///       "unit_amount": 500,
///       "unit_amount_decimal": "500"
///     }
///   ],
///   "has_more": false,
///   "url": "/v1/prices"
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct StripePriceList {
    pub data: Vec<StripePrice>,
    pub has_more: bool,
}
