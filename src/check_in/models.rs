use serde::{Deserialize, Serialize};

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
    /// Price to be displayed, ie "$5"
    pub price: String,
    /// Price ID from Stripe
    pub price_id: String,
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
/// A successful response looks like this
/// ```
/// {
/// "id": "cs_test_a1xDgt75iRy0gfNypidMbbgeDm2C5FBZUmKNHMsMRgPU2ntor9i7bsKmJA",
/// "object": "checkout.session",
/// "adaptive_pricing": {
/// "enabled": true
/// },
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
///   "customer_details": {
///     "address": {
///       "city": null,
///       "country": "US",
///       "line1": null,
///       "line2": null,
///       "postal_code": "11111",
///       "state": null
///     },
///     "email": "john@doe.com",
///     "name": "John Doe",
///     "phone": null,
///     "tax_exempt": "none",
///     "tax_ids": []
///   },
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
///   "payment_intent": "pi_3RYZbmQ2wtFMkx0Y1u0tmTHP",
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
///   "success_url": "http://localhost:8000/stripe/success?session_id={CHECKOUT_SESSION_ID}",
///   "total_details": {
///     "amount_discount": 0,
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
}

/// A Product as returned by Stripe's search product API.
#[derive(Debug, Deserialize)]
pub struct StripeProduct {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_price: Option<String>,
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
