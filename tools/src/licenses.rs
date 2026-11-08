//! Retain license texts for every package actually included by the browser bundler.
use super::Result;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

pub(super) fn generate(output: &Path, metafile: &Path) -> Result {
    let metadata: Value = serde_json::from_slice(&fs::read(metafile)?)?;
    let mut packages = BTreeMap::new();
    for input in metadata["inputs"]
        .as_object()
        .ok_or("missing bundle inputs")?
        .keys()
    {
        if !input.replace('\\', "/").contains("node_modules/") {
            continue;
        }
        let mut directory = Path::new(input).parent().ok_or("invalid bundled input")?;
        let package: Value = loop {
            let manifest = directory.join("package.json");
            if manifest.is_file() {
                let package: Value = serde_json::from_slice(&fs::read(manifest)?)?;
                if package["name"].is_string() && package["version"].is_string() {
                    break package;
                }
            }
            directory = directory
                .parent()
                .ok_or("missing bundled package identity")?;
        };
        let name = package["name"]
            .as_str()
            .ok_or("missing bundled package name")?;
        let version = package["version"]
            .as_str()
            .ok_or("missing bundled package version")?;
        if packages.contains_key(name) {
            continue;
        }
        let license = package["license"]
            .as_str()
            .ok_or("missing bundled package license")?;
        if !matches!(license, "MIT" | "BSD-3-Clause" | "Apache-2.0" | "0BSD") {
            return Err("unreviewed bundled package license".into());
        }
        if !name
            .bytes()
            .chain(version.bytes())
            .all(|b| b.is_ascii_alphanumeric() || b"@/._-".contains(&b))
        {
            return Err("invalid package identity".into());
        }
        let mut texts = String::new();
        let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for file in entries {
            let name = file.file_name().to_string_lossy().to_ascii_lowercase();
            if file.file_type()?.is_file()
                && (name == "license"
                    || name.starts_with("license.")
                    || name == "licence"
                    || name == "notice"
                    || name.starts_with("notice."))
            {
                if !texts.is_empty() && !texts.ends_with('\n') {
                    texts.push('\n');
                }
                texts.push_str(&fs::read_to_string(file.path())?);
            }
        }
        if texts.is_empty() {
            return Err("a bundled dependency has no distributed license text".into());
        }
        packages.insert(
            name.to_owned(),
            (version.to_owned(), license.to_owned(), texts),
        );
    }
    let directory = output.join("licenses/npm");
    if directory.exists() {
        fs::remove_dir_all(&directory)?;
    }
    fs::create_dir_all(&directory)?;
    let mut rows = Vec::new();
    for (name, (version, license, text)) in packages {
        let filename = format!("{}-{version}.txt", name.replace('@', "").replace('/', "--"));
        fs::write(
            directory.join(&filename),
            format!("{name} {version}\nSPDX: {license}\n\n{text}"),
        )?;
        rows.push(serde_json::json!({ "name": name, "version": version, "license": license, "file": filename }));
    }
    let mut bytes = serde_json::to_vec_pretty(&rows)?;
    bytes.push(b'\n');
    fs::write(directory.join("manifest.json"), bytes)?;
    Ok(())
}
