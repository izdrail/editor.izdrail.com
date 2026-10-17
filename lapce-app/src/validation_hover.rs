//! Hover help for Laravel validation rule strings: the cursor inside
//! `'required|max:255'` explains the rule under it. A quoted string only
//! counts when every `|`-separated rule name is known, and (for a lone word)
//! it is an array value rather than a key, so ordinary text is left alone.
//! Descriptions follow the Laravel 12.x validation docs. Textual only;
//! nothing is executed.

const MAX_LINE: usize = 4096;

/// Rule name -> what it checks.
const RULES: &[(&str, &str)] = &[
    ("accepted", "must be \"yes\", \"on\", 1, \"1\", true or \"true\" (terms acceptance)"),
    ("accepted_if", "must be accepted when another field equals a given value"),
    ("active_url", "hostname must have a valid A or AAAA record (dns_get_record)"),
    ("after", "must be after the given date (dates go through strtotime)"),
    ("after_or_equal", "must be at or after the given date"),
    ("alpha", "only Unicode letters (\\p{L}, \\p{M})"),
    ("alpha_dash", "only Unicode alphanumerics plus - and _"),
    ("alpha_num", "only Unicode alphanumerics (\\p{L}, \\p{M}, \\p{N})"),
    ("array", "must be a PHP array"),
    ("ascii", "only 7-bit ASCII characters"),
    ("bail", "stop validating this field after the first failure"),
    ("before", "must be before the given date (strtotime)"),
    ("before_or_equal", "must be at or before the given date"),
    ("between", "size between min and max inclusive; measured like `size`"),
    ("boolean", "must cast to boolean: true, false, 1, 0, \"1\", \"0\""),
    ("confirmed", "needs a matching `{field}_confirmation` field"),
    ("contains", "array must contain all of the given values"),
    ("current_password", "must match the authenticated user's password; optional guard parameter"),
    ("date", "must be a valid, non-relative date (strtotime)"),
    ("date_equals", "must equal the given date"),
    ("date_format", "must match one of the given date formats; use `date` or `date_format`, not both"),
    ("decimal", "must be numeric with the given number of decimal places"),
    ("declined", "must be \"no\", \"off\", 0, \"0\", false or \"false\""),
    ("declined_if", "must be declined when another field equals a given value"),
    ("different", "must differ from the given field"),
    ("digits", "integer with exactly this many digits"),
    ("digits_between", "integer with a digit count between min and max"),
    ("dimensions", "image must meet the given width/height/ratio constraints"),
    ("distinct", "array must not contain duplicate values"),
    ("doesnt_contain", "array must contain none of the given values"),
    ("doesnt_end_with", "must not end with any of the given values"),
    ("doesnt_start_with", "must not start with any of the given values"),
    ("email", "must be an email address (egulias/email-validator, RFC style by default)"),
    ("ends_with", "must end with one of the given values"),
    ("enum", "must be a valid value of the given enum (class-based rule)"),
    ("exists", "must exist in the given database table/column"),
    ("extensions", "uploaded file must have one of the listed extensions"),
    ("file", "must be a successfully uploaded file"),
    ("filled", "must not be empty when it is present"),
    ("gt", "must be greater than the given field/value, same type, measured like `size`"),
    ("gte", "must be greater than or equal to the given field/value, measured like `size`"),
    ("image", "must be an image (jpg, jpeg, png, bmp, gif, webp)"),
    ("in", "must be one of the listed values"),
    ("integer", "must be an integer"),
    ("ip", "must be an IP address"),
    ("ipv4", "must be an IPv4 address"),
    ("ipv6", "must be an IPv6 address"),
    ("json", "must be a valid JSON string"),
    ("list", "must be an array with consecutive keys 0..count-1"),
    ("lowercase", "must be lowercase"),
    ("lt", "must be less than the given field/value, measured like `size`"),
    ("lte", "must be less than or equal to the given field/value, measured like `size`"),
    ("max", "at most the given value: string length, number, array count or file kilobytes (same as `size`)"),
    ("mimes", "uploaded file's MIME type must match one of the listed extensions"),
    ("mimetypes", "uploaded file must match one of the given MIME types"),
    ("min", "minimum value: string length, number, array count or file kilobytes (same as `size`)"),
    ("missing", "must not be present in the input"),
    ("not_in", "must not be one of the listed values"),
    ("not_regex", "must not match the regular expression"),
    ("nullable", "may be null"),
    ("numeric", "must be numeric (is_numeric)"),
    ("present", "must be present in the input (may be empty)"),
    ("prohibited", "must be missing or empty"),
    ("regex", "must match the regular expression (for a `|` in the pattern use Rule::regex)"),
    ("required", "must be present and not empty"),
    ("required_if", "required when the other field equals one of the given values"),
    ("required_unless", "required unless the other field equals one of the given values"),
    ("required_with", "required when any of the listed fields is present and not empty"),
    ("required_without", "required when any of the listed fields is absent or empty"),
    ("same", "must match the given field"),
    ("size", "exact size: string characters, integer value (with the numeric/integer rule), array count, file kilobytes"),
    ("sometimes", "only validated when the field is present in the input"),
    ("starts_with", "must start with one of the given values"),
    ("string", "must be a string"),
    ("timezone", "must be a valid DateTimeZone identifier"),
    ("unique", "must not already exist in the given database table/column"),
    ("uppercase", "must be uppercase"),
    ("url", "must be a valid URL"),
    ("uuid", "must be a valid RFC 9562 UUID"),
];

/// `(string content start, content)` for the quoted span covering `col`.
fn quoted_at(line: &str, col: usize) -> Option<(usize, &str)> {
    for q in ['\'', '"'] {
        for (i, _) in line.match_indices(q) {
            let start = i + 1;
            let Some(len) = line[start..].find(q) else { break };
            let end = start + len;
            if col >= start && col <= end {
                return Some((start, &line[start..end]));
            }
        }
    }
    None
}

/// Markdown hover for the validation rule under byte `col` of `line`.
pub fn help(line: &str, col: usize) -> Option<String> {
    if line.len() > MAX_LINE {
        return None;
    }
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let (start, content) = quoted_at(line, col)?;
    if content.len() > 200 {
        return None;
    }
    // Every `|`-separated rule must be known; a lone word only counts as an
    // array value (`'title' => 'required'`), not as a key.
    let tokens: Vec<&str> = content.split('|').collect();
    let all_known = !tokens.is_empty()
        && tokens.iter().all(|t| {
            let name = t.split(':').next().unwrap_or("");
            RULES.iter().any(|(n, _)| *n == name)
        });
    if !all_known {
        return None;
    }
    if tokens.len() == 1 {
        let after = line[start + content.len()..].trim_start();
        if after.starts_with("=>") || !line[..start].contains("=>") {
            return None;
        }
    }
    // The token under the cursor (byte offsets are ASCII-delimited).
    let rel = col - start;
    let mut pos = 0;
    for tok in tokens {
        let end = pos + tok.len();
        if rel <= end {
            let (name, param) = match tok.split_once(':') {
                Some((n, p)) => (n, Some(p)),
                None => (tok, None),
            };
            let entry = RULES.iter().find(|(n, _)| *n == name)?;
            let text = entry.1;
            let mut md = format!("**Validation rule** `{tok}`\n\n{text}.\n");
            if let Some(p) = param {
                md.push_str(&format!("\nParameter: `{p}`\n"));
            }
            md.push_str("\n_Laravel 12.x validation rules._");
            return Some(md);
        }
        pos = end + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(line: &str, needle: &str) -> Option<String> {
        help(line, line.find(needle).unwrap() + 1)
    }

    #[test]
    fn rules_are_explained() {
        let l = "'title' => 'required|string|max:255',";
        assert!(h(l, "required").unwrap().contains("must be present and not empty"));
        assert!(h(l, "string").unwrap().contains("must be a string"));
        let m = h(l, "max").unwrap();
        assert!(m.contains("at most the given value") && m.contains("Parameter: `255`"), "{m}");
        // Cursor on the field name (a key, not a value): no hover.
        assert_eq!(h(l, "title"), None);
        // A lone rule word as an array value still counts.
        assert!(h("'title' => 'required',", "required").is_some());
        assert!(h("'title' => \"required\",", "required").is_some());
        let e = h("'email' => 'sometimes|required|email:rfc,dns',", "email:rfc").unwrap();
        assert!(e.contains("egulias") && e.contains("Parameter: `rfc,dns`"), "{e}");
    }

    #[test]
    fn ordinary_strings_are_ignored() {
        for (l, n) in [
            ("'status' => 'in-progress',", "in-progress"),  // unknown rule name
            ("'status' => 'unknown|required',", "unknown"), // one unknown is enough
            ("$x = 'list';", "list"),                       // lone word, not an array value
            ("'list' => [];", "list"),                      // a key
            ("'sometimes'", "sometimes"),                   // bare lone word
            ("'title' => $value,", "value"),                // not a quoted rule
            ("$request->input('title');", "title"),         // field name
        ] {
            assert_eq!(h(l, n), None, "{l}");
        }
        assert_eq!(help(&format!("'{}'", "a".repeat(5000)), 10), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        for l in ["'", "\"", "'|'", "'||'", "'é|required'", "'日本'", "'max:'", "':'", "'required' =>", ""] {
            for c in 0..=l.len() + 2 {
                let _ = help(l, c);
            }
        }
    }
}
