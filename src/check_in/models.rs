use crate::auth::models::Roles;
use crate::check_in::metadata::STRIPE_KEYS;
use crate::check_in::visibility::ShowSchedule;
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
    /// are sorted lexicographically. When `category` is set, this order applies inside that group.
    pub sort_level: i32,
    /// Stripe category tag. Empty means ungrouped on Check In.
    #[serde(default)]
    pub category: String,
    /// Stripe category-sort-level. Smaller numbers appear first. Default 0.
    #[serde(default)]
    pub category_sort_level: i32,
    /// Optional Stripe show windows. Empty means always visible.
    #[serde(default)]
    pub show_schedule: ShowSchedule,
}

impl Product {
    /// Whether this product can be purchased at `now`.
    pub fn is_live_at(&self, now: DateTime<Utc>) -> bool {
        self.show_schedule.is_visible(now)
    }

    /// Whether this product can be purchased right now.
    pub fn is_live(&self) -> bool {
        self.is_live_at(Utc::now())
    }

    /// Admin-facing Live / Hidden until … / parse error label.
    pub fn visibility_status(&self) -> String {
        self.show_schedule.status_label(Utc::now())
    }

    /// Timezone used to interpret window tags.
    pub fn admin_timezone(&self) -> String {
        self.show_schedule.timezone_display()
    }

    /// Parsed one-shot windows for the admin dashboard.
    pub fn admin_intervals(&self) -> Vec<String> {
        self.show_schedule.interval_summaries()
    }

    /// Parsed weekly windows for the admin dashboard.
    pub fn admin_weeklies(&self) -> Vec<String> {
        self.show_schedule.weekly_summaries()
    }

    /// Raw Stripe timezone tag, if present.
    pub fn admin_raw_timezone(&self) -> &str {
        self.show_schedule.raw_timezone()
    }

    /// Raw Stripe interval tag, if present.
    pub fn admin_raw_interval(&self) -> &str {
        self.show_schedule.raw_interval()
    }

    /// Raw Stripe weekly tag, if present.
    pub fn admin_raw_weekly(&self) -> &str {
        self.show_schedule.raw_weekly()
    }

    /// Whether Check In should list this product for the given viewer.
    pub fn visible_to(&self, is_admin: bool, roles: &HashSet<Roles>) -> bool {
        is_admin || (self.is_live() && (self.requires_roles.is_subset(roles) || self.show_preview))
    }
}

/// One Check In section: uncategorized products (`name` empty) or a named collapsible group.
#[derive(Debug, Clone)]
pub struct ProductGroup {
    pub name: String,
    pub sort_level: i32,
    pub products: Vec<Product>,
}

impl ProductGroup {
    pub fn is_named(&self) -> bool {
        !self.name.is_empty()
    }
}

/// Read Stripe category tags. Blank or missing category means ungrouped.
pub fn parse_category(metadata: &HashMap<String, String>) -> (String, i32) {
    let category = metadata
        .get(STRIPE_KEYS.category)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
        .to_string();
    let category_sort_level = metadata
        .get(STRIPE_KEYS.category_sort_level)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    (category, category_sort_level)
}

/// Bucket products by `category`. Ungrouped first, then named groups by sort level and name.
/// Products inside a group are ordered by `sort_level` then name.
pub fn group_products(products: Vec<Product>) -> Vec<ProductGroup> {
    let mut by_name: HashMap<String, Vec<Product>> = HashMap::new();
    let mut cat_sort: HashMap<String, i32> = HashMap::new();

    for product in products {
        let key = product.category.clone();
        cat_sort
            .entry(key.clone())
            .and_modify(|level| *level = (*level).max(product.category_sort_level))
            .or_insert(product.category_sort_level);
        by_name.entry(key).or_default().push(product);
    }

    let mut groups: Vec<ProductGroup> = by_name
        .into_iter()
        .map(|(name, mut products)| {
            products.sort_by_key(|p| (p.sort_level, p.name.to_lowercase()));
            ProductGroup {
                sort_level: cat_sort.get(&name).copied().unwrap_or(0),
                name,
                products,
            }
        })
        .collect();

    groups.sort_by(|a, b| match (a.name.is_empty(), b.name.is_empty()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => (a.sort_level, a.name.to_lowercase()).cmp(&(b.sort_level, b.name.to_lowercase())),
    });

    groups
}

/// Store a hashmap of `product_id`: (product, quantity) pairs.
#[derive(Debug, Serialize, Deserialize)]
pub struct ShoppingCart {
    pub items: std::collections::HashMap<String, (Product, u64)>,
}

impl ShoppingCart {
    pub fn new() -> Self {
        Self {
            items: std::collections::HashMap::new(),
        }
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
        self.items
            .values()
            .map(|(product, quantity)| product.dollar_price * *quantity as f64)
            .sum()
    }
}

/// The response from the Stripe Checkout Session API.
///
/// Used to let us direct users to Stripe to enact payment.
///
/// The full response looks like this (anonymized test-mode fixture):
/// ```
/// {
///   "id": "cs_test_a1ExampleCheckoutSessionId00000000000000000000000000",
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
///     "id": "pmc_1AAAAExamplePaymentMethodConfig",
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
///   "url": "https://checkout.stripe.com/c/pay/cs_test_a1ExampleCheckoutSessionId00000000000000000000000000",
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
/// The response to the request here looks like this (anonymized test-mode fixture):
/// ```
/// {
///   "id": "cs_test_b1ExamplePaidSessionId00000000000000000000000000000",
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
///     "individual_name": "Alex Rivera",
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
///     "email": "alex@example.com",
///     "individual_name": "Alex Rivera",
///     "name": "Alex Rivera",
///     "phone": null,
///     "tax_exempt": "none",
///     "tax_ids": []
///   },
///   "customer_email": null,
///   "discounts": [
///     {
///       "coupon": null,
///       "promotion_code": "promo_1AAAAExamplePromotionCode"
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
///         "id": "li_1AAAAExampleLineItem01",
///         "object": "item",
///         "amount_discount": 500,
///         "amount_subtotal": 500,
///         "amount_tax": 0,
///         "amount_total": 0,
///         "currency": "usd",
///         "description": "Social Dance",
///         "price": {
///           "id": "price_1AAAAExamplePriceLesson",
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
///           "product": "prod_ExampleLesson",
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
///         "id": "li_1AAAAExampleLineItem02",
///         "object": "item",
///         "amount_discount": 500,
///         "amount_subtotal": 500,
///         "amount_tax": 0,
///         "amount_total": 0,
///         "currency": "usd",
///         "description": "Beginner+ Lesson & Social",
///         "price": {
///           "id": "price_1AAAAExamplePriceSocial",
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
///           "product": "prod_ExampleSocial",
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
///     "url": "/v1/checkout/sessions/cs_test_b1ExamplePaidSessionId00000000000000000000000000000/line_items"
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
///     "id": "pmc_1AAAAExamplePaymentMethodConfig",
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
    pub amount_total: u64, // This is in cents
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
/// The full response looks like this (anonymized test-mode fixture):
/// ```
///{
///   "object": "search_result",
///   "data": [
///     {
///       "id": "prod_ExampleSocial",
///       "object": "product",
///       "active": true,
///       "attributes": [],
///       "created": 1749671512,
///       "default_price": "price_1AAAAExamplePriceSocial",
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
///       "id": "prod_ExampleLesson",
///       "object": "product",
///       "active": true,
///       "attributes": [],
///       "created": 1749179773,
///       "default_price": "price_1AAAAExamplePriceLesson",
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
///   "id": "price_1AAAAExamplePriceSocial",
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
///   "product": "prod_ExampleSocial",
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
///       "id": "price_1AAAAExamplePriceSocial",
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
///       "product": "prod_ExampleSocial",
///       "recurring": null,
///       "tax_behavior": "unspecified",
///       "tiers_mode": null,
///       "transform_quantity": null,
///       "type": "one_time",
///       "unit_amount": 500,
///       "unit_amount_decimal": "500"
///     },
///     {
///       "id": "price_1AAAAExamplePriceAlt",
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
///       "product": "prod_ExampleLesson",
///       "recurring": null,
///       "tax_behavior": "unspecified",
///       "tiers_mode": null,
///       "transform_quantity": null,
///       "type": "one_time",
///       "unit_amount": 6900,
///       "unit_amount_decimal": "6900"
///     },
///     {
///       "id": "price_1AAAAExamplePriceDropin",
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
///       "product": "prod_ExampleDropin",
///       "recurring": null,
///       "tax_behavior": "unspecified",
///       "tiers_mode": null,
///       "transform_quantity": null,
///       "type": "one_time",
///       "unit_amount": 1000,
///       "unit_amount_decimal": "1000"
///     },
///     {
///       "id": "price_1AAAAExamplePriceLesson",
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
///       "product": "prod_ExampleLesson",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::models::Roles;

    fn product(name: &str, sort_level: i32, category: &str, category_sort_level: i32) -> Product {
        Product {
            name: name.into(),
            id: name.into(),
            description: String::new(),
            dollar_price: 5.0,
            price_id: "price".into(),
            requires_roles: HashSet::new(),
            show_preview: false,
            sort_level,
            category: category.into(),
            category_sort_level,
            show_schedule: ShowSchedule::default(),
        }
    }

    #[test]
    fn parse_category_blank_when_missing() {
        assert_eq!(parse_category(&HashMap::new()), (String::new(), 0));
    }

    #[test]
    fn parse_category_trims_and_reads_sort() {
        let mut metadata = HashMap::new();
        metadata.insert(STRIPE_KEYS.category.into(), "  Lessons  ".into());
        metadata.insert(STRIPE_KEYS.category_sort_level.into(), "2".into());
        assert_eq!(parse_category(&metadata), ("Lessons".into(), 2));
    }

    #[test]
    fn parse_category_whitespace_is_ungrouped() {
        let mut metadata = HashMap::new();
        metadata.insert(STRIPE_KEYS.category.into(), "   ".into());
        metadata.insert(STRIPE_KEYS.category_sort_level.into(), "nope".into());
        assert_eq!(parse_category(&metadata), (String::new(), 0));
    }

    #[test]
    fn group_products_keeps_ungrouped_flat_and_sorted() {
        let groups = group_products(vec![
            product("Zebra", 0, "", 0),
            product("Alpha", 0, "", 0),
            product("Last", 5, "", 0),
        ]);
        assert_eq!(groups.len(), 1);
        assert!(!groups[0].is_named());
        let names: Vec<_> = groups[0].products.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "Zebra", "Last"]);
    }

    #[test]
    fn group_products_orders_named_groups_and_items() {
        let groups = group_products(vec![
            product("Social", 0, "Nights", 2),
            product("Beginner", 1, "Lessons", 1),
            product("Advanced", 0, "Lessons", 1),
            product("Drop-in", 0, "", 99),
        ]);
        assert_eq!(groups.len(), 3);
        assert!(!groups[0].is_named());
        assert_eq!(groups[0].products[0].name, "Drop-in");
        assert_eq!(groups[1].name, "Lessons");
        let lesson_names: Vec<_> = groups[1].products.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(lesson_names, ["Advanced", "Beginner"]);
        assert_eq!(groups[2].name, "Nights");
    }

    #[test]
    fn group_products_uses_highest_category_sort_level() {
        let groups = group_products(vec![
            product("A", 0, "Events", 5),
            product("B", 0, "Events", 0),
            product("C", 0, "Club", 3),
        ]);
        assert_eq!(groups[0].name, "Club");
        assert_eq!(groups[0].sort_level, 3);
        assert_eq!(groups[1].name, "Events");
        assert_eq!(groups[1].sort_level, 5);
    }

    #[test]
    fn old_product_json_defaults_category_fields() {
        let json = r#"{
            "name":"A",
            "id":"1",
            "description":"",
            "dollar_price":1.0,
            "price_id":"p",
            "requires_roles":[],
            "show_preview":false,
            "sort_level":0
        }"#;
        let product: Product = serde_json::from_str(json).unwrap();
        assert!(product.category.is_empty());
        assert_eq!(product.category_sort_level, 0);
    }

    #[test]
    fn visible_to_admin_sees_gated_products() {
        let mut gated = product("Gated", 0, "", 0);
        gated.requires_roles.insert(Roles::new("advanced"));
        assert!(gated.visible_to(true, &HashSet::new()));
        assert!(!gated.visible_to(false, &HashSet::new()));
        gated.show_preview = true;
        assert!(gated.visible_to(false, &HashSet::new()));
    }
}
