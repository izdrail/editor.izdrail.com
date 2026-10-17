//! Hover help for Laravel's `Str::` and `Arr::` helpers: the cursor on the
//! method name in `Str::slug('...')` or `Arr::get(...)` shows one line on the
//! method from a built-in table, grounded in the Laravel 12.x strings and
//! helpers docs. Static text scan; nothing is executed. Import (`use`)
//! lines are skipped.

const MAX_LINE: usize = 4096;

const STR_METHODS: &[(&str, &str)] = &[
    ("after", "returns everything after the given value in a string. The entire string will be returned if the value does not exist within the string."),
    ("afterLast", "returns everything after the last occurrence of the given value in a string. The entire string will be returned if the value does not exist within the string."),
    ("apa", "converts the given string to title case following the APA guidelines."),
    ("ascii", "will attempt to transliterate the string into an ASCII value."),
    ("before", "returns everything before the given value in a string."),
    ("beforeLast", "returns everything before the last occurrence of the given value in a string."),
    ("between", "returns the portion of a string between two values."),
    ("betweenFirst", "returns the smallest possible portion of a string between two values."),
    ("camel", "converts the given string to `camelCase`."),
    ("charAt", "returns the character at the specified index. If the index is out of bounds, `false` is returned."),
    ("chopStart", "removes the first occurrence of the given value only if the value appears at the start of the string."),
    ("chopEnd", "removes the last occurrence of the given value only if the value appears at the end of the string."),
    ("contains", "determines if the given string contains the given value. By default, this method is case sensitive."),
    ("containsAll", "determines if the given string contains all of the values in a given array."),
    ("doesntContain", "determines if the given string doesn't contain the given value. By default, this method is case sensitive."),
    ("deduplicate", "replaces consecutive instances of a character with a single instance of that character in the given string. By default, the method deduplicates spaces."),
    ("doesntEndWith", "determines if the given string doesn't end with the given value."),
    ("doesntStartWith", "determines if the given string doesn't begin with the given value."),
    ("endsWith", "determines if the given string ends with the given value."),
    ("excerpt", "extracts an excerpt from a given string that matches the first instance of a phrase within that string."),
    ("finish", "adds a single instance of the given value to a string if it does not already end with that value."),
    ("fromBase64", "decodes the given Base64 string."),
    ("headline", "will convert strings delimited by casing, hyphens, or underscores into a space delimited string with each word's first letter capitalized."),
    ("initials", "will return the initials of a given string, optionally capitalizing them."),
    ("inlineMarkdown", "converts GitHub flavored Markdown into inline HTML using CommonMark. However, unlike the `markdown` method, it does not wrap all generated HTML in a block-level element."),
    ("is", "determines if a given string matches a given pattern. Asterisks may be used as wildcard values."),
    ("isAscii", "determines if a given string is 7 bit ASCII."),
    ("isJson", "determines if the given string is valid JSON."),
    ("isUrl", "determines if the given string is a valid URL."),
    ("isUlid", "determines if the given string is a valid ULID."),
    ("isUuid", "determines if the given string is a valid UUID."),
    ("kebab", "converts the given string to `kebab-case`."),
    ("lcfirst", "returns the given string with the first character lowercased."),
    ("length", "returns the length of the given string."),
    ("limit", "truncates the given string to the specified length."),
    ("lower", "converts the given string to lowercase."),
    ("markdown", "converts GitHub flavored Markdown into HTML using CommonMark."),
    ("mask", "masks a portion of a string with a repeated character, and may be used to obfuscate segments of strings such as email addresses and phone numbers."),
    ("match", "will return the portion of a string that matches a given regular expression pattern."),
    ("matchAll", "will return a collection containing the portions of a string that match a given regular expression pattern."),
    ("isMatch", "will return `true` if the string matches a given regular expression."),
    ("orderedUuid", "generates a \"timestamp first\" UUID that may be efficiently stored in an indexed database column. Each UUID that is generated using this method will be sorted after UUIDs previously generated using ..."),
    ("padBoth", "wraps PHP's `str_pad` function, padding both sides of a string with another string until the final string reaches a desired length."),
    ("padLeft", "wraps PHP's `str_pad` function, padding the left side of a string with another string until the final string reaches a desired length."),
    ("padRight", "wraps PHP's `str_pad` function, padding the right side of a string with another string until the final string reaches a desired length."),
    ("password", "may be used to generate a secure, random password of a given length. The password will consist of a combination of letters, numbers, symbols, and spaces. By default, passwords are 32 characters long."),
    ("plural", "converts a singular word string to its plural form. This function supports any of the languages supported by Laravel's pluralizer."),
    ("pluralStudly", "converts a singular word string formatted in studly caps case to its plural form. This function supports any of the languages supported by Laravel's pluralizer."),
    ("position", "returns the position of the first occurrence of a substring in a string. If the substring does not exist in the given string, `false` is returned."),
    ("random", "generates a random string of the specified length. This function uses PHP's `random_bytes` function."),
    ("remove", "removes the given value or array of values from the string."),
    ("repeat", "repeats the given string."),
    ("replace", "replaces a given string within the string."),
    ("replaceArray", "replaces a given value in the string sequentially using an array."),
    ("replaceFirst", "replaces the first occurrence of a given value in a string."),
    ("replaceLast", "replaces the last occurrence of a given value in a string."),
    ("replaceMatches", "replaces all portions of a string matching a pattern with the given replacement string."),
    ("replaceStart", "replaces the first occurrence of the given value only if the value appears at the start of the string."),
    ("replaceEnd", "replaces the last occurrence of the given value only if the value appears at the end of the string."),
    ("reverse", "reverses the given string."),
    ("singular", "converts a string to its singular form. This function supports any of the languages supported by Laravel's pluralizer."),
    ("slug", "generates a URL friendly \"slug\" from the given string."),
    ("snake", "converts the given string to `snake_case`."),
    ("squish", "removes all extraneous white space from a string, including extraneous white space between words."),
    ("start", "adds a single instance of the given value to a string if it does not already start with that value."),
    ("startsWith", "determines if the given string begins with the given value."),
    ("studly", "converts the given string to `StudlyCase`."),
    ("substr", "returns the portion of string specified by the start and length parameters."),
    ("substrCount", "returns the number of occurrences of a given value in the given string."),
    ("substrReplace", "replaces text within a portion of a string, starting at the position specified by the third argument and replacing the number of characters specified by the fourth argument. Passing `0` to the ..."),
    ("swap", "replaces multiple values in the given string using PHP's `strtr` function."),
    ("take", "returns a specified number of characters from the beginning of a string."),
    ("title", "converts the given string to `Title Case`."),
    ("toBase64", "converts the given string to Base64."),
    ("transliterate", "will attempt to convert a given string into its closest ASCII representation."),
    ("trim", "strips whitespace (or other characters) from the beginning and end of the given string. Unlike PHP's native `trim` function, the `Str::trim` method also removes unicode whitespace characters."),
    ("ltrim", "strips whitespace (or other characters) from the beginning of the given string. Unlike PHP's native `ltrim` function, the `Str::ltrim` method also removes unicode whitespace characters."),
    ("rtrim", "strips whitespace (or other characters) from the end of the given string. Unlike PHP's native `rtrim` function, the `Str::rtrim` method also removes unicode whitespace characters."),
    ("ucfirst", "returns the given string with the first character capitalized."),
    ("ucsplit", "splits the given string into an array by uppercase characters."),
    ("ucwords", "converts the first character of each word in the given string to uppercase."),
    ("upper", "converts the given string to uppercase."),
    ("ulid", "generates a ULID, which is a compact, time-ordered unique identifier."),
    ("unwrap", "removes the specified strings from the beginning and end of a given string."),
    ("uuid", "generates a UUID (version 4)."),
    ("uuid7", "generates a UUID (version 7)."),
    ("wordCount", "returns the number of words that a string contains."),
    ("wordWrap", "wraps a string to a given number of characters."),
    ("words", "limits the number of words in a string. An additional string may be passed to this method via its third argument to specify which string should be appended to the end of the truncated string."),
    ("wrap", "wraps the given string with an additional string or pair of strings."),
];

const ARR_METHODS: &[(&str, &str)] = &[
    ("accessible", "determines if the given value is array accessible."),
    ("add", "adds a given key / value pair to an array if the given key doesn't already exist in the array or is set to `null`."),
    ("array", "retrieves a value from a deeply nested array using \"dot\" notation (just as Arr::get() does), but throws an `InvalidArgumentException` if the requested value is not an `array`."),
    ("boolean", "retrieves a value from a deeply nested array using \"dot\" notation (just as Arr::get() does), but throws an `InvalidArgumentException` if the requested value is not a `boolean`."),
    ("collapse", "collapses an array of arrays or collections into a single array."),
    ("crossJoin", "cross joins the given arrays, returning a Cartesian product with all possible permutations."),
    ("divide", "returns two arrays: one containing the keys and the other containing the values of the given array."),
    ("dot", "flattens a multi-dimensional array into a single level array that uses \"dot\" notation to indicate depth."),
    ("every", "ensures that all values in the array pass a given truth test."),
    ("except", "removes the given key / value pairs from an array."),
    ("exceptValues", "removes the specified values from an array."),
    ("exists", "checks that the given key exists in the provided array."),
    ("first", "returns the first element of an array passing a given truth test."),
    ("flatten", "flattens a multi-dimensional array into a single level array."),
    ("float", "retrieves a value from a deeply nested array using \"dot\" notation (just as Arr::get() does), but throws an `InvalidArgumentException` if the requested value is not a `float`."),
    ("forget", "removes a given key / value pairs from a deeply nested array using \"dot\" notation."),
    ("from", "converts various input types into a plain PHP array. It supports a range of input types, including arrays, objects, and several common Laravel interfaces, such as `Arrayable`, `Enumerable`, ..."),
    ("get", "retrieves a value from a deeply nested array using \"dot\" notation."),
    ("has", "checks whether a given item or items exists in an array using \"dot\" notation."),
    ("hasAll", "determines if all of the specified keys exist in the given array using \"dot\" notation."),
    ("hasAny", "checks whether any item in a given set exists in an array using \"dot\" notation."),
    ("integer", "retrieves a value from a deeply nested array using \"dot\" notation (just as Arr::get() does), but throws an `InvalidArgumentException` if the requested value is not an `int`."),
    ("isAssoc", "returns `true` if the given array is an associative array. An array is considered \"associative\" if it doesn't have sequential numerical keys beginning with zero."),
    ("isList", "returns `true` if the given array's keys are sequential integers beginning from zero."),
    ("join", "joins array elements with a string. Using this method's third argument, you may also specify the joining string for the final element of the array."),
    ("keyBy", "keys the array by the given key. If multiple items have the same key, only the last one will appear in the new array."),
    ("last", "returns the last element of an array passing a given truth test."),
    ("map", "iterates through the array and passes each value and key to the given callback. The array value is replaced by the value returned by the callback."),
    ("mapSpread", "iterates over the array, passing each nested item value into the given closure. The closure is free to modify the item and return it, thus forming a new array of modified items."),
    ("mapWithKeys", "iterates through the array and passes each value to the given callback. The callback should return an associative array containing a single key / value pair."),
    ("only", "returns only the specified key / value pairs from the given array."),
    ("onlyValues", "returns only the specified values from an array."),
    ("partition", "may be combined with PHP array destructuring to separate elements that pass a given truth test from those that do not."),
    ("pluck", "retrieves all of the values for a given key from an array."),
    ("prepend", "will push an item onto the beginning of an array."),
    ("prependKeysWith", "The `Arr::prependKeysWith` prepends all key names of an associative array with the given prefix."),
    ("pull", "returns and removes a key / value pair from an array."),
    ("push", "pushes an item into an array using \"dot\" notation. If an array does not exist at the given key, it will be created."),
    ("query", "converts the array into a query string."),
    ("random", "returns a random value from an array."),
    ("reject", "removes items from an array using the given closure."),
    ("select", "selects an array of values from an array."),
    ("set", "sets a value within a deeply nested array using \"dot\" notation."),
    ("shuffle", "randomly shuffles the items in the array."),
    ("sole", "retrieves a single value from an array using the given closure. If more than one value within the array matches the given truth test, an `Illuminate\\Support\\MultipleItemsFoundException` exception ..."),
    ("some", "ensures that at least one of the values in the array passes a given truth test."),
    ("sort", "sorts an array by its values."),
    ("sortDesc", "sorts an array in descending order by its values."),
    ("sortRecursive", "recursively sorts an array using the `sort` function for numerically indexed sub-arrays and the `ksort` function for associative sub-arrays."),
    ("string", "retrieves a value from a deeply nested array using \"dot\" notation (just as Arr::get() does), but throws an `InvalidArgumentException` if the requested value is not a `string`."),
    ("take", "returns a new array with the specified number of items."),
    ("toCssClasses", "conditionally compiles a CSS class string. The method accepts an array of classes where the array key contains the class or classes you wish to add, while the value is a boolean expression. If the ..."),
    ("toCssStyles", "conditionally compiles a CSS style string. The method accepts an array of CSS declarations where the array key contains the CSS declaration you wish to add, while the value is a boolean expression. ..."),
    ("undot", "expands a single-dimensional array that uses \"dot\" notation into a multi-dimensional array."),
    ("where", "filters an array using the given closure."),
    ("whereNotNull", "removes all `null` values from the given array."),
    ("wrap", "wraps the given value in an array. If the given value is already an array it will be returned without modification."),
];


fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Markdown hover for the `Str::method` / `Arr::method` at byte `col`.
pub fn help(line: &str, col: usize) -> Option<String> {
    if line.len() > MAX_LINE || line.trim_start().starts_with("use ") {
        return None;
    }
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    for (ns, table) in [("Str::", STR_METHODS), ("Arr::", ARR_METHODS)] {
        for (i, _) in line.match_indices(ns) {
            // Whole word: `MyStr::` or `$str::` do not count.
            if line[..i].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '>') {
                continue;
            }
            let start = i + ns.len();
            let name: String = line[start..]
                .chars()
                .take_while(|c| is_ident(*c))
                .collect();
            if name.is_empty() {
                continue;
            }
            // The cursor must sit on the method name (or its opening paren).
            let end = start + name.len();
            if col < start || col > end {
                continue;
            }
            let entry = table.iter().find(|(n, _)| *n == name)?;
            let text = entry.1;
            return Some(format!(
                "**`Laravel {ns}{name}()`** - {text}\n\n_Laravel 12.x docs._"
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(line: &str, needle: &str) -> Option<String> {
        help(line, line.rfind(needle).unwrap() + 1)
    }

    #[test]
    fn helpers_are_explained() {
        let l = "$slug = Str::slug($title);";
        let md = h(l, "slug").unwrap();
        assert!(md.contains("Str::slug()") && md.contains("URL friendly"), "{md}");
        let l = "$v = Arr::get($array, 'products.desk.price', 100);";
        assert!(h(l, "get").unwrap().contains("Arr::get()"));
        // Namespace prefix required: bare functions and lookalikes stay quiet.
        assert_eq!(h("$s = slug($title);", "slug"), None);
        assert_eq!(h("$s = MyStr::slug($title);", "slug"), None);
        // A unicode class name ending in Str does not count either.
        assert_eq!(h("$s = 日本Str::get($t);", "get"), None);
        assert_eq!(h("use Illuminate\\Support\\Str;", "Str"), None);
        // Unknown method, and cursor on the receiver rather than the name.
        assert_eq!(h("$x = Str::nope($y);", "nope"), None);
        assert_eq!(h("$x = Str::slug($y);", "$y"), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        for l in ["Str::", "Arr::", "Str::é()", "日本Str::get()", "::Str::", "Str::get(", ""] {
            for c in 0..=l.len() + 2 {
                let _ = help(l, c);
            }
        }
    }
}
