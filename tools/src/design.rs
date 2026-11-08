//! Consume a qualified public DS release without editing its generated package.
use super::{Result, run};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const WEB: &str = "packages/desktop/extension/vendor/design-system";
const MANIFEST: &str = "packages/desktop/extension/vendor/design-fonts.json";
const FONTS: &str = "packages/desktop/extension/styles/fonts.css";
const BADGE: &str = "packages/desktop/extension/design-values.json";

fn badge() -> Result<Vec<u8>> {
    let tokens: Value = serde_json::from_slice(&fs::read(Path::new(WEB).join("tokens.json"))?)?;
    let color = |css| -> Result<String> {
        let token = tokens["tokens"]
            .as_array()
            .ok_or("missing DS tokens")?
            .iter()
            .find(|token| token["css"].as_str() == Some(css))
            .ok_or("missing DS color")?;
        let value = field(token, "value")?;
        if value.len() != 7
            || !value.starts_with('#')
            || !value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid DS color".into());
        }
        Ok(value.into())
    };
    let mut output = serde_json::to_vec_pretty(&nddev_browseruse_contract::BadgeColors {
        neutral: color("--text-secondary")?,
        warning: color("--status-warning-text")?,
        error: color("--status-danger-text")?,
        accent: color("--accent-fill")?,
    })?;
    output.push(b'\n');
    Ok(output)
}

pub(super) fn generate() -> Result {
    fs::write(FONTS, fonts()?)?;
    fs::write(BADGE, badge()?)?;
    Ok(())
}

fn hash(path: &Path) -> Result<String> {
    Ok(Sha256::digest(fs::read(path)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn files(root: &Path) -> Result<Vec<PathBuf>> {
    fn walk(root: &Path, paths: &mut Vec<PathBuf>) -> Result {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                walk(&entry.path(), paths)?;
            } else if kind.is_file() {
                paths.push(entry.path());
            } else {
                return Err("DS artifacts must contain regular files only".into());
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    walk(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn tree_hash(root: &Path) -> Result<String> {
    let mut rows = String::new();
    for path in files(root)? {
        let name = path
            .strip_prefix(root)?
            .to_str()
            .ok_or("non-UTF8 DS path")?
            .replace('\\', "/");
        if name.contains(['\r', '\n']) {
            return Err("invalid DS path".into());
        }
        rows.push_str(&format!("{}  {name}\n", hash(&path)?));
    }
    Ok(Sha256::digest(rows.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value[name]
        .as_str()
        .ok_or_else(|| format!("missing DS field {name}").into())
}

fn fonts() -> Result<String> {
    let manifest: Value = serde_json::from_slice(&fs::read(MANIFEST)?)?;
    let mut css = String::from(
        "/* Generated from the pinned OpenNetwork DS font manifest. Do not edit. */\n",
    );
    for family in manifest["families"]
        .as_array()
        .ok_or("missing font families")?
    {
        let Some(web) = family.get("web") else {
            continue;
        };
        let name = serde_json::to_string(field(family, "family")?)?;
        for file in web["files"].as_array().ok_or("missing web font files")? {
            let filename = field(file, "file")?;
            if Path::new(filename).components().count() != 1 {
                return Err("invalid font asset path".into());
            }
            let path = Path::new(WEB).join("fonts").join(filename);
            if hash(&path)? != field(file, "sha256")? {
                return Err("DS font checksum mismatch".into());
            }
            let weight = file["weight"].as_u64().ok_or("missing font weight")?;
            let range = field(file, "unicodeRange")?;
            css.push_str(&format!("@font-face {{\n  font-family: {name};\n  font-style: normal;\n  font-weight: {weight};\n  font-display: swap;\n  src: url(\"../vendor/design-system/fonts/{filename}\") format(\"woff2\");\n  unicode-range: {range};\n}}\n"));
        }
    }
    Ok(css)
}

pub(super) fn check() -> Result {
    let lock: Value = serde_json::from_slice(&fs::read("design-system.lock.json")?)?;
    if tree_hash(Path::new(WEB))? != field(&lock, "webSha256")?
        || hash(Path::new(MANIFEST))? != field(&lock, "fontManifestSha256")?
    {
        return Err("vendored DS drift; use the pinned release producer".into());
    }
    if fs::read_to_string(FONTS)? != fonts()? {
        return Err("generated product font CSS drift".into());
    }
    if fs::read(BADGE)? != badge()? {
        return Err("generated DS badge values drift".into());
    }
    Ok(())
}

pub(super) fn vendor() -> Result {
    let lock: Value = serde_json::from_slice(&fs::read("design-system.lock.json")?)?;
    let archive = Path::new(".tools/design-release/archive.tar.gz");
    fs::create_dir_all(archive.parent().ok_or("invalid DS cache")?)?;
    if !archive.is_file() {
        let url = field(&lock, "archive")?;
        if !url.starts_with("https://github.com/NDDev-Technocracy-OpenNetwork/nddev-opennetwork-design-system/releases/download/") { return Err("unexpected DS archive origin".into()); }
        run(
            "curl",
            &[
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--max-time",
                "300",
                "--output",
                archive.to_str().ok_or("invalid archive path")?,
                url,
            ],
        )?;
    }
    if fs::metadata(archive)?.len()
        != lock["archiveBytes"]
            .as_u64()
            .ok_or("missing DS archive size")?
        || hash(archive)? != field(&lock, "archiveSha256")?
    {
        return Err("DS release archive checksum mismatch".into());
    }
    let extracted = Path::new(".tools/design-release/extracted");
    if extracted.exists() {
        fs::remove_dir_all(extracted)?;
    }
    fs::create_dir(extracted)?;
    run(
        "tar",
        &[
            "-xzf",
            archive.to_str().ok_or("invalid DS archive path")?,
            "-C",
            extracted.to_str().ok_or("invalid DS extraction path")?,
        ],
    )?;
    let source = extracted.join(field(&lock, "directory")?);
    if tree_hash(&source.join("packages/web"))? != field(&lock, "webSha256")? {
        return Err("DS package digest mismatch".into());
    }
    let output = Path::new(WEB);
    if output.exists() {
        fs::remove_dir_all(output)?;
    }
    fs::create_dir_all(output)?;
    for path in files(&source.join("packages/web"))? {
        let target = output.join(path.strip_prefix(source.join("packages/web"))?);
        fs::create_dir_all(target.parent().ok_or("invalid DS asset path")?)?;
        fs::copy(path, target)?;
    }
    fs::copy(source.join("packages/fonts/manifest.json"), MANIFEST)?;
    fs::create_dir_all(
        Path::new(FONTS)
            .parent()
            .ok_or("invalid product font path")?,
    )?;
    generate()?;
    check()
}
