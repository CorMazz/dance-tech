/// Stripe product metadata keys used by this app.
///
/// Same idea as [`crate::app::router::ROUTES`]: one struct is the source of truth so parsers,
/// Stripe search queries, and the admin docs cannot drift.
pub struct StripeMetadataKeys {
    /// Product appears in the DanceTech catalog when this is `"true"`.
    pub show_on_dancetech: &'static str,
    /// Comma-separated roles required to buy the product.
    pub requires_roles: &'static str,
    /// `"true"` shows a greyed-out preview to users missing `requires_roles`.
    pub show_role_preview: &'static str,
    /// `"true"` shows a greyed-out preview before a purchase window (not after it ends).
    /// Weekly windows preview whenever the product is not currently for sale.
    pub show_time_preview: &'static str,
    /// Integer order inside a category, or among ungrouped products.
    pub sort_level: &'static str,
    /// Optional Check In group name. Same value = same collapsible section.
    pub category: &'static str,
    /// Integer order of that category section on Check In.
    pub category_sort_level: &'static str,
    /// IANA timezone for show windows. Defaults to America/New_York when omitted.
    pub show_timezone: &'static str,
    /// One-shot visibility windows.
    pub show_interval: &'static str,
    /// Recurring weekly visibility windows.
    pub show_weekly: &'static str,
}

impl StripeMetadataKeys {
    /// Stripe product search: active products tagged for this app.
    pub fn catalog_search_query(&self) -> String {
        format!(
            "active:'true' AND metadata['{}']:'true'",
            self.show_on_dancetech
        )
    }
}

/// Absolute truth for Stripe metadata keys.
pub const STRIPE_KEYS: StripeMetadataKeys = StripeMetadataKeys {
    show_on_dancetech: "show-on-dancetech",
    requires_roles: "requires-roles",
    show_role_preview: "show-role-preview",
    show_time_preview: "show-time-preview",
    sort_level: "sort-level",
    category: "category",
    category_sort_level: "category-sort-level",
    show_timezone: "show-timezone",
    show_interval: "show-interval",
    show_weekly: "show-weekly",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_search_uses_show_on_dancetech_key() {
        let query = STRIPE_KEYS.catalog_search_query();
        assert!(query.contains(STRIPE_KEYS.show_on_dancetech));
        assert!(query.contains("active:'true'"));
    }
}
