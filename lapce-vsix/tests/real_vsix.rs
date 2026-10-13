//! Runs against real marketplace packages when `VSIX_FIXTURES` points at a
//! directory of `.vsix` files downloaded from Open VSX. Skipped otherwise.
use std::path::PathBuf;

use lapce_vsix::{import_vsix, load_registry};

fn fixtures() -> Option<PathBuf> {
    std::env::var_os("VSIX_FIXTURES")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

fn tmpdir(name: &str) -> PathBuf {
    let d =
        std::env::temp_dir().join(format!("vsx-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn import_and_render_real_packages() {
    let Some(dir) = fixtures() else { return };
    let root = tmpdir("all");
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().and_then(|e| e.to_str()) != Some("vsix") {
            continue;
        }
        let rep = import_vsix(&std::fs::read(&p).unwrap(), &root).unwrap();
        println!(
            "IMPORT {} v{} themes={:?} icon_themes={:?} icon_mappings={} grammars={} snippets={} langs={} has_code={} license={:?}\n  skipped={:?}",
            rep.id,
            rep.version,
            rep.color_themes,
            rep.icon_themes,
            rep.icon_mappings,
            rep.grammars.len(),
            rep.snippet_count,
            rep.languages.len(),
            rep.has_code,
            rep.license,
            rep.skipped
        );
        // converted themes must be valid TOML with the sections Lapce reads
        for t in std::fs::read_dir(rep.dir.join("themes"))
            .into_iter()
            .flatten()
            .flatten()
        {
            let v: toml::Value =
                toml::from_str(&std::fs::read_to_string(t.path()).unwrap()).unwrap();
            assert!(v["color-theme"]["name"].is_str());
            assert!(v["color-theme"]["syntax"]["comment"].is_str());
        }
    }
    let loaded = load_registry(&root);
    let rust = loaded.scope_for_path(std::path::Path::new("src/main.rs"));
    println!("scope for main.rs: {rust:?}");
    if let Some(scope) = rust {
        let text =
            "// hi\nfn main() {\n    let s = \"x\\n\"; println!(\"{}\", 42);\n}\n";
        let snap = loaded.registry.highlight(scope, text).unwrap();
        let lines: Vec<&str> = text.split('\n').collect();
        for (i, l) in snap.lines.iter().enumerate().take(3) {
            for (s, e, k) in l {
                println!(
                    "RS {i} {:?} => {}",
                    &lines[i][*s as usize..*e as usize],
                    k
                );
            }
        }
        assert!(snap.lines[0].iter().any(|t| t.2.contains("comment")));
        assert!(
            snap.lines[1]
                .iter()
                .any(|t| t.2.contains("keyword") || t.2.contains("storage"))
        );
    }
    let docker = loaded.scope_for_path(std::path::Path::new("Dockerfile"));
    println!("scope for Dockerfile: {docker:?}");
    let ini = loaded.scope_for_path(std::path::Path::new("a.ini"));
    println!("scope for a.ini: {ini:?}");
    if let Some(scope) = ini {
        let text = "; c\n[sec]\nkey=value\n";
        let snap = loaded.registry.highlight(scope, text).unwrap();
        let lines: Vec<&str> = text.split('\n').collect();
        for (i, l) in snap.lines.iter().enumerate() {
            for (s, e, k) in l {
                println!(
                    "INI {i} {:?} => {}",
                    &lines[i][*s as usize..*e as usize],
                    k
                );
            }
        }
    }
    let php = loaded.snippets_for("php");
    println!("php snippets: {}", php.len());
    println!(
        "rust langconfig line comment: {:?}",
        loaded
            .language_configs
            .get("rust")
            .and_then(|c| c.line_comment.clone())
    );
}
