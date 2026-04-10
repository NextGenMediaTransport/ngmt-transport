use std::collections::HashSet;
use std::env;
use std::path::PathBuf;

fn dedupe_include_lines(header: &str) -> String {
    let mut seen = HashSet::<String>::new();
    let mut out = String::with_capacity(header.len());
    for line in header.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#include") {
            if seen.insert(trimmed.to_string()) {
                out.push_str(line);
                out.push('\n');
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let package_name = env::var("CARGO_PKG_NAME").unwrap();
    let header_name = format!("{}.h", package_name.replace('-', "_"));

    let include_dir = PathBuf::from(&crate_dir).join("include");
    std::fs::create_dir_all(&include_dir).expect("Unable to create include directory");

    let output_path = include_dir.join(header_name);

    let config =
        cbindgen::Config::from_file(format!("{crate_dir}/cbindgen.toml")).unwrap_or_default();

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(cbindgen::Language::C)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(&output_path);

    let raw = std::fs::read_to_string(&output_path).expect("read generated header");
    let cleaned = dedupe_include_lines(&raw);
    std::fs::write(output_path, cleaned).expect("write deduped header");
}
