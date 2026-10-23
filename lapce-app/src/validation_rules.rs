//! Laravel validation rule names (item 378): offered at rule-name
//! positions in the rules string of `->validate([...])`,
//! `Validator::make(..., [...])`, `$validator->sometimes(..., '...')`
//! and the Livewire `#[Validate('...')]` attribute. The list follows
//! the Laravel 12.x validation documentation's rule sections, plus
//! `sometimes` (documented inline) and `after_or_equal` /
//! `before_or_equal` (category index; implemented as
//! `validateAfterOrEqual` / `validateBeforeOrEqual` in
//! `ValidatesAttributes`). String names only - fluent `Rule::`
//! builders are PHP code, not strings.

use lsp_types::{CompletionItem, CompletionItemKind};

/// The rule names, sorted.
pub(crate) const RULES: &[&str] = &[
    "accepted",
    "accepted_if",
    "active_url",
    "after",
    "after_or_equal",
    "alpha",
    "alpha_dash",
    "alpha_num",
    "array",
    "ascii",
    "bail",
    "before",
    "before_or_equal",
    "between",
    "boolean",
    "confirmed",
    "contains",
    "current_password",
    "date",
    "date_equals",
    "date_format",
    "decimal",
    "declined",
    "declined_if",
    "different",
    "digits",
    "digits_between",
    "dimensions",
    "distinct",
    "doesnt_contain",
    "doesnt_end_with",
    "doesnt_start_with",
    "email",
    "encoding",
    "ends_with",
    "enum",
    "exclude",
    "exclude_if",
    "exclude_unless",
    "exclude_with",
    "exclude_without",
    "exists",
    "extensions",
    "file",
    "filled",
    "gt",
    "gte",
    "hex_color",
    "image",
    "in",
    "in_array",
    "in_array_keys",
    "integer",
    "ip",
    "ipv4",
    "ipv6",
    "json",
    "list",
    "lowercase",
    "lt",
    "lte",
    "mac_address",
    "max",
    "max_digits",
    "mimes",
    "mimetypes",
    "min",
    "min_digits",
    "missing",
    "missing_if",
    "missing_unless",
    "missing_with",
    "missing_with_all",
    "multiple_of",
    "not_in",
    "not_regex",
    "nullable",
    "numeric",
    "present",
    "present_if",
    "present_unless",
    "present_with",
    "present_with_all",
    "prohibited",
    "prohibited_if",
    "prohibited_if_accepted",
    "prohibited_if_declined",
    "prohibited_unless",
    "prohibits",
    "regex",
    "required",
    "required_array_keys",
    "required_if",
    "required_if_accepted",
    "required_if_declined",
    "required_unless",
    "required_with",
    "required_with_all",
    "required_without",
    "required_without_all",
    "same",
    "size",
    "sometimes",
    "starts_with",
    "string",
    "timezone",
    "ulid",
    "unique",
    "uppercase",
    "url",
    "uuid",
];

/// Every rule as a completion item; the editor filters by the typed
/// partial (the word after the last `|`).
pub(crate) fn items() -> Vec<CompletionItem> {
    RULES.iter()
        .map(|n| CompletionItem {
            label: (*n).to_string(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some("validation rule".to_string()),
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_are_sorted_unique_and_bare_names() {
        assert!(RULES.windows(2).all(|w| w[0] < w[1]), "not sorted");
        assert!(
            RULES.iter().all(|r| !r.is_empty() && !r.contains(':')),
            "rule names carry no parameters"
        );
    }
}
