//! "Open in Browser" for static web files.
use std::path::Path;

use lsp_types::Url;

const EXTENSIONS: &[&str] = &["html", "htm", "xhtml", "svg"];

/// The `file://` URL to hand to the system browser for `path`, or `None` when
/// the file is not a static web page (PHP, Twig and templates need a server).
pub fn browser_url(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !EXTENSIONS.contains(&ext.as_str()) || !path.is_absolute() {
        return None;
    }
    Url::from_file_path(path).ok().map(|u| u.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_files_get_file_urls() {
        let url = browser_url(Path::new("/tmp/my site/index.html")).unwrap();
        assert_eq!(url, "file:///tmp/my%20site/index.html");
        assert!(browser_url(Path::new("/x/a.HTM")).is_some());
    }

    #[test]
    fn server_side_files_are_refused() {
        assert!(browser_url(Path::new("/x/index.php")).is_none());
        assert!(browser_url(Path::new("/x/page.twig")).is_none());
        assert!(browser_url(Path::new("/x/readme")).is_none());
        assert!(browser_url(Path::new("relative.html")).is_none());
    }
}
