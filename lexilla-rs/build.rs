use std::path::Path;
fn main() {
    let mut b = cc::Build::new();
    b.cpp(true).std("c++17").opt_level(3).debug(false).warnings(false)
        .define("LEXILLA_NO_EXPORT", None).define("NDEBUG", None)
        .include("vendor/scintilla/include").include("vendor/lexilla/include")
        .include("vendor/lexilla/lexlib").file("shim.cpp").file("vendor/lexilla/src/Lexilla.cxx");
    for dir in ["vendor/lexilla/lexlib", "vendor/lexilla/lexers"] {
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "cxx")).collect();
        paths.sort();
        for p in paths { b.file(p); }
    }
    b.compile("lexilla_static");
    println!("cargo:rerun-if-changed=shim.cpp");
    println!("cargo:rerun-if-changed={}", Path::new("vendor").display());
}
