use std::{env, fs, path::Path};

use anyhow::Result;

#[derive(Debug)]
struct ReleaseInfo {
    version: String,
    branch: String,
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    track_repo_refs();
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DISTRIBUTION");
    println!("cargo:rerun-if-env-changed=RELEASE_TAG_NAME");

    let release_info = get_info()?;

    // Print info to terminal during compilation
    println!("cargo::warning=Compiling meta: {release_info:?}");

    let meta_file = Path::new(&env::var("OUT_DIR")?).join("meta.rs");

    let ReleaseInfo { version, branch } = release_info;

    #[rustfmt::skip]
    let meta = format!(r#"
        pub const NAME: &str = "Lapce-{branch}";
        pub const VERSION: &str = "{version}";
        pub const RELEASE: ReleaseType = ReleaseType::{branch};
    "#);

    fs::write(meta_file, meta)?;

    Ok(())
}

/// Declares the git paths whose change must re-run this script so the
/// baked-in commit hash cannot go stale (item 389):
///
/// - `<gitdir>/HEAD` - moves between branches or on detach. `.git` is
///   resolved through the `gitdir:` pointer file in a linked worktree.
/// - `<commondir>/refs/heads` - a DIRECTORY, scanned recursively: a
///   new commit rewrites the branch's loose ref file, and the
///   packed-to-loose transition CREATES that file; both are caught by
///   the directory scan, where declaring only the paths that exist at
///   run time would miss the creation.
/// - `<commondir>/packed-refs`, when it exists - covers commits while
///   the branch ref is packed.
///
/// Branch refs live in the common dir shared by all worktrees, so the
/// `commondir` file is resolved too. Only paths that exist at run time
/// are declared: a declaration for a missing path is inert - in
/// current cargo it neither errors nor reliably re-triggers when the
/// path appears later (host lab, item 389/391).
fn track_repo_refs() {
    let dotgit = Path::new("../.git");
    let git_dir = if dotgit.is_dir() {
        dotgit.to_path_buf()
    } else if let Ok(ptr) = fs::read_to_string(dotgit) {
        let Some(dir) = ptr.trim().strip_prefix("gitdir: ") else { return };
        let dir = Path::new(dir);
        if dir.is_absolute() {
            dir.to_path_buf()
        } else {
            // Relative gitdir: relative to the directory holding the
            // `.git` file, i.e. the repo root (`..` from this crate).
            Path::new("..").join(dir)
        }
    } else {
        return;
    };
    let common_dir = fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .map(|p| git_dir.join(p.trim()))
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| git_dir.clone());
    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    let heads = common_dir.join("refs/heads");
    if heads.is_dir() {
        println!("cargo:rerun-if-changed={}", heads.display());
    }
    let packed = common_dir.join("packed-refs");
    if packed.exists() {
        println!("cargo:rerun-if-changed={}", packed.display());
    }
}

fn get_info() -> Result<ReleaseInfo> {
    // CARGO_PKG_* are always available, even in build scripts
    let cargo_tag = env!("CARGO_PKG_VERSION");

    // For any downstream that complains about us doing magic
    if env::var("CARGO_FEATURE_DISTRIBUTION").is_ok() {
        return Ok(ReleaseInfo {
            version: cargo_tag.to_string(),
            branch: String::from("Stable"),
        });
    }

    let release_info = {
        let release_tag = env::var("RELEASE_TAG_NAME").unwrap_or_default();

        if release_tag.starts_with('v') {
            ReleaseInfo {
                version: cargo_tag.to_string(),
                branch: "Stable".to_string(),
            }
        } else {
            #[cfg(not(debug_assertions))]
            let release = "Nightly";
            #[cfg(debug_assertions)]
            let release = "Debug";

            let tag = format!(
                "{cargo_tag}+{release}.{}",
                get_head().unwrap_or("unknown".to_string())
            );
            ReleaseInfo {
                version: tag,
                branch: release.to_string(),
            }
        }
    };

    Ok(release_info)
}

/// `<short-hash>` of HEAD, with a `-dirty` suffix when tracked files
/// had uncommitted changes when this script last ran (item 389).
/// Best effort, not a guarantee: edits made after that run change no
/// declared input, so a clean-looking stamp can lag a freshly dirtied
/// tree until the next re-run.
#[cfg(not(target_os = "linux"))]
fn get_head() -> Option<String> {
    let repo = match git2::Repository::discover(format!(
        "{}/..",
        env::var("CARGO_MANIFEST_DIR").ok()?
    )) {
        Ok(v) => v,
        Err(err) => {
            println!("cargo::warning=Failed to obtain git repo: {err}");
            return None;
        }
    };
    let reference = match repo.head() {
        Ok(v) => v,
        Err(err) => {
            println!("cargo::warning=Failed to obtain head: {err}");
            return None;
        }
    };
    let commit = reference.target();
    println!("cargo::warning=Commit found: {commit:?}");
    let commit = commit.map(|s| s.to_string().split_at(7).0.to_owned())?;
    let dirty = repo.statuses(None).map(|s| !s.is_empty()).unwrap_or(false);
    if dirty {
        Some(format!("{commit}-dirty"))
    } else {
        Some(commit)
    }
}

/// `<short-hash>` of HEAD, with a `-dirty` suffix when tracked files
/// had uncommitted changes when this script last ran (item 389).
/// Best effort, not a guarantee: edits made after that run change no
/// declared input, so a clean-looking stamp can lag a freshly dirtied
/// tree until the next re-run.
#[cfg(target_os = "linux")]
fn get_head() -> Option<String> {
    // Discover this repo explicitly: never whatever repo the caller's
    // cwd happens to be inside.
    let dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let cmd = std::process::Command::new("git")
        .args(["show", "--pretty=format:%h", "--no-patch"])
        .current_dir(&dir)
        .output()
        .ok()?;

    let commit = String::from_utf8_lossy(&cmd.stdout).trim().to_string();
    if commit.is_empty() {
        return None;
    }
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(&dir)
        .output()
        .ok()
        .is_some_and(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty());
    if dirty {
        Some(format!("{commit}-dirty"))
    } else {
        Some(commit)
    }
}
