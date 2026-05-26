fn main() {
    expose_dune_server_service_version();
    tauri_build::build();
}

fn expose_dune_server_service_version() {
    let cargo_toml = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/dune-server-service/Cargo.toml");
    println!("cargo:rerun-if-changed={}", cargo_toml.display());
    let contents = std::fs::read_to_string(&cargo_toml)
        .unwrap_or_else(|err| panic!("reading {}: {err}", cargo_toml.display()));
    let version = parse_package_version(&contents).unwrap_or_else(|| {
        panic!(
            "could not find [package].version in {}",
            cargo_toml.display()
        )
    });
    println!("cargo:rustc-env=DUNE_SERVER_SERVICE_VERSION={version}");
}

fn parse_package_version(toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("version") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('=')?.trim_start();
            let rest = rest.trim_start_matches('"');
            let end = rest.find('"')?;
            return Some(rest[..end].to_string());
        }
    }
    None
}
