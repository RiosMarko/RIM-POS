pub fn is_public_setting_key(key: &str) -> bool {
    matches!(
        key,
        "tax_enabled"
            | "tax_default_rate"
            | "tax_prices_include_tax"
            | "tax_show_breakdown"
            | "tax_auto_apply_new_products"
            | "ticket_store_name"
            | "ticket_header"
            | "ticket_footer"
            | "ticket_width"
            | "ticket_show_logo"
            | "ticket_show_date"
            | "ticket_show_cashier"
            | "ticket_show_barcode"
            | "ticket_show_item_count"
            | "ticket_start_lines"
            | "ticket_extra_lines"
            | "ticket_copies"
            | "scale_baud_rate"
    )
}

pub fn is_invoice_setting_key(key: &str) -> bool {
    matches!(
        key,
        "company_rfc"
            | "company_fiscal_regime"
            | "company_fiscal_postal_code"
            | "default_cfdi_use"
            | "invoice_series"
    )
}
