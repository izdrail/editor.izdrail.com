//! Agent `web_fetch` tool: bounded, read-only web access for the agent
//! loop (parity item 5 - the composer/agent had no web access at all).
//! Fetched content is untrusted data, never instructions. Redirects are
//! refused (the model must name the final URL, never follow a redirect
//! target it did not ask for), and obviously local/private hosts are
//! refused so the tool cannot be pointed at the machine's own services.
//! A public hostname that RESOLVES to a private address is not detected
//! in v1 (no DNS re-check); that is a documented limitation, not a
//! guarantee.

const MAX_WIRE_BYTES: usize = 1024 * 1024;
const MAX_RETURN_BYTES: usize = 64 * 1024;
const MAX_URL_BYTES: usize = 2048;
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const TOTAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Fetch one http(s) URL and return its text content (HTML stripped to
/// plain text), capped for the agent loop's message budget.
pub fn fetch(url: &str) -> Result<String, String> {
    if url.len() > MAX_URL_BYTES {
        return Err("URL too long".into());
    }
    let url = checked_url(url)?;
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TOTAL_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(url).send().map_err(|e| e.to_string())?;
    let status = response.status();
    if status.is_redirection() {
        let target = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_string();
        let hint = if target.is_empty() {
            String::new()
        } else {
            format!("; fetch the target URL directly: {target}")
        };
        return Err(format!("Redirect refused ({status}){hint}"));
    }
    if !status.is_success() {
        return Err(format!("Fetch failed with status {status}"));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_html = content_type.contains("text/html");
    if !content_type.is_empty()
        && !(content_type.starts_with("text/")
            || content_type.contains("json")
            || content_type.contains("xml")
            || content_type.contains("javascript"))
    {
        return Err(format!("Unsupported content type: {content_type}"));
    }
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(response, MAX_WIRE_BYTES as u64 + 1),
        &mut bytes,
    )
    .map_err(|e| e.to_string())?;
    let wire_truncated = bytes.len() > MAX_WIRE_BYTES;
    bytes.truncate(MAX_WIRE_BYTES);
    let text = String::from_utf8_lossy(&bytes);
    let text = if is_html {
        html_to_text(&text)
    } else {
        text.into_owned()
    };
    let mut out = text.trim().to_string();
    if out.is_empty() {
        return Err("Empty response body".into());
    }
    if out.len() > MAX_RETURN_BYTES {
        let mut end = MAX_RETURN_BYTES;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push_str("\n...[truncated]");
    }
    if wire_truncated {
        out.push_str("\n...[response exceeded 1 MiB, truncated]");
    }
    Ok(out)
}

/// Validate the URL and refuse non-http(s) schemes, missing hosts and
/// local/private hosts.
fn checked_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed =
        reqwest::Url::parse(url.trim()).map_err(|_| "Invalid URL".to_string())?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("Only http(s) URLs can be fetched".into()),
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?
        .to_ascii_lowercase();
    if host_is_local(&host) {
        return Err("Refusing to fetch a local or private host".into());
    }
    Ok(parsed)
}

/// True for localhost names and literal IPs in loopback, private,
/// link-local, CGNAT, multicast, documentation or unspecified ranges.
/// Hostnames that resolve to private addresses are NOT detected (v1).
fn host_is_local(host: &str) -> bool {
    let host = host.trim_matches(|c| c == '[' || c == ']');
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return ip.is_loopback()
            || ip.is_private()
            || ip.is_link_local()
            || ip.is_multicast()
            || ip.is_broadcast()
            || ip.is_documentation()
            || ip.is_unspecified()
            || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 0x40);
    }
    if let Ok(ip) = host.parse::<std::net::Ipv6Addr>() {
        return ip.is_loopback()
            || ip.is_unspecified()
            || ip.is_multicast()
            || (ip.segments()[0] & 0xffc0) == 0xfe80
            || (ip.segments()[0] & 0xfe00) == 0xfc00;
    }
    false
}

/// Crude HTML-to-text: drop script/style/template/noscript blocks
/// wholesale, turn block-level tags into newlines, strip other tags,
/// decode the common entities, collapse whitespace. Not a browser
/// rendering - enough for an agent to read documentation pages.
fn html_to_text(html: &str) -> String {
    const BLOCK_TAGS: [&str; 16] = [
        "p", "div", "br", "li", "tr", "h1", "h2", "h3", "h4", "h5", "h6",
        "section", "article", "ul", "ol", "pre",
    ];
    const DROP_TAGS: [&str; 4] = ["script", "style", "template", "noscript"];
    let lower = html.to_ascii_lowercase();
    let mut stripped = String::with_capacity(html.len());
    let mut pos = 0usize;
    while pos < html.len() {
        let Some(rel) = lower[pos..].find('<') else {
            stripped.push_str(&html[pos..]);
            break;
        };
        let lt = pos + rel;
        stripped.push_str(&html[pos..lt]);
        let after = &lower[lt + 1..];
        let name_start = after.trim_start_matches('/');
        let name_len = name_start
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .map(char::len_utf8)
            .sum::<usize>();
        let name = &name_start[..name_len];
        if DROP_TAGS.contains(&name) {
            let close = format!("</{name}");
            pos = match lower[lt..].find(&close) {
                Some(off) => match lower[lt + off..].find('>') {
                    Some(gt) => lt + off + gt + 1,
                    None => html.len(),
                },
                // Unclosed drop-tag: nothing after it is trustworthy.
                None => html.len(),
            };
            continue;
        }
        if BLOCK_TAGS.contains(&name) {
            stripped.push('\n');
        }
        pos = match lower[lt..].find('>') {
            Some(gt) => lt + gt + 1,
            None => html.len(),
        };
    }
    decode_entities(&stripped)
}

/// Decode the handful of entities documentation pages actually use.
/// `&amp;` is decoded LAST so `&amp;lt;` becomes `&lt;`, not `<`.
fn decode_entities(text: &str) -> String {
    let text = text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    collapse_whitespace(&text)
}

/// Collapse runs of inline whitespace, trim lines, drop blank-line
/// runs down to a single separator newline.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::new();
    let mut pending_blank = false;
    for line in text.lines() {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            pending_blank = true;
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
            if pending_blank {
                out.push('\n');
            }
        }
        out.push_str(&collapsed);
        pending_blank = false;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_and_private_hosts_are_refused() {
        for host in [
            "localhost",
            "db.localhost",
            "printer.local",
            "vault.internal",
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.1.1",
            "100.64.1.1",
            "0.0.0.0",
            "255.255.255.255",
            "[::1]",
            "fe80::1",
            "fc00::1",
        ] {
            assert!(host_is_local(host), "{host} should be refused");
        }
        for host in [
            "example.com",
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "2606:4700:4700::1111",
        ] {
            assert!(!host_is_local(host), "{host} should be allowed");
        }
    }

    #[test]
    fn checked_url_accepts_public_http_and_refuses_the_rest() {
        assert!(checked_url("https://example.com/docs").is_ok());
        assert!(checked_url("http://example.com").is_ok());
        assert!(checked_url("ftp://example.com/f").is_err());
        assert!(checked_url("file:///etc/passwd").is_err());
        assert!(checked_url("https://127.0.0.1/admin").is_err());
        assert!(checked_url("https://localhost:11434/api").is_err());
        assert!(checked_url("not a url").is_err());
    }

    #[test]
    fn html_to_text_drops_scripts_and_strips_tags() {
        let html = "<html><head><style>body{color:red}</style></head>\
            <body><h1>Title</h1><p>Hello <b>world</b></p>\
            <script>alert('x')</script><p>Second &amp; third</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello world"));
        assert!(text.contains("Second & third"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("color:red"));
        assert!(!text.contains("<b>"));
        // Block tags became line breaks.
        assert!(text.lines().count() >= 3);
    }

    #[test]
    fn html_to_text_decodes_entities_in_the_safe_order() {
        assert_eq!(html_to_text("&amp;lt;"), "&lt;");
        assert_eq!(html_to_text("&lt;"), "<");
        assert_eq!(html_to_text("&quot;a&#39;b&quot;"), "\"a'b\"");
    }

    #[test]
    fn html_to_text_unclosed_drop_tag_discards_the_rest() {
        let text = html_to_text("<p>kept</p><script>never closed");
        assert_eq!(text, "kept");
    }

    #[test]
    fn collapse_whitespace_drops_blank_runs() {
        // A run of blank (or whitespace-only) lines collapses to at
        // most one blank separator line, never vanishes and never
        // multiplies.
        assert_eq!(
            collapse_whitespace("a  b\n\n\n\nc\n   \nd"),
            "a b\n\nc\n\nd"
        );
    }
}
