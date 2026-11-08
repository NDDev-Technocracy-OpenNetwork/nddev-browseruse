//! Retain original Rust dependency license files; never export Cargo's private cache paths.
use super::{Result, capture};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};

fn legal_directory(path: &Path, depth: usize, candidates: &mut Vec<std::path::PathBuf>) -> Result {
    if depth > 4 || candidates.len() > 64 {
        return Err("Rust legal directory exceeds its bounds".into());
    }
    for entry in fs::read_dir(path)? {
        if candidates.len() >= 64 {
            return Err("Rust legal directory exceeds its file bound".into());
        }
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            legal_directory(&entry.path(), depth + 1, candidates)?;
        } else if entry.file_type()?.is_file() {
            candidates.push(entry.path());
        } else {
            return Err("Rust legal directory contains a non-regular entry".into());
        }
    }
    Ok(())
}

pub(super) fn generate(output: &Path) -> Result {
    let compiler = capture("rustc", &["-vV"])?;
    let target = compiler
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("missing Rust build-host target")?;
    if target.len() > 128
        || !target
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err("invalid Rust build-host target".into());
    }
    let sysroot = capture("rustc", &["--print", "sysroot"])?;
    let notice = Path::new(sysroot.trim()).join("share/doc/rust/COPYRIGHT-library.html");
    if !notice.is_file() || fs::metadata(&notice)?.len() > 8 * 1024 * 1024 {
        return Err(
            "the pinned Rust toolchain's library copyright notice is missing or oversized".into(),
        );
    }
    let library = output.join("licenses/rust-toolchain");
    fs::create_dir_all(&library)?;
    let bytes = fs::read(notice)?;
    let sha256: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    fs::write(library.join("COPYRIGHT-library.html"), bytes)?;
    fs::write(
        library.join("toolchain.json"),
        serde_json::to_vec_pretty(&json!({
            "compiler": compiler.trim(), "target": target, "file": "COPYRIGHT-library.html", "sha256": sha256
        }))?,
    )?;
    let metadata: Value = serde_json::from_str(&capture(
        "cargo",
        &[
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--filter-platform",
            target,
        ],
    )?)?;
    let packages: BTreeMap<_, _> = metadata["packages"]
        .as_array()
        .ok_or("missing Cargo packages")?
        .iter()
        .filter_map(|package| Some((package["id"].as_str()?, package)))
        .collect();
    let nodes: BTreeMap<_, _> = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("missing Cargo dependency graph")?
        .iter()
        .filter_map(|node| Some((node["id"].as_str()?, node)))
        .collect();
    let own: HashSet<_> = metadata["workspace_members"]
        .as_array()
        .ok_or("missing Cargo workspace")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let root = packages
        .iter()
        .find(|(_, package)| package["name"] == "nddev-browseruse")
        .map(|(id, _)| *id)
        .ok_or("missing native product package")?;
    let mut queue = vec![root];
    let mut visited = HashSet::new();
    while let Some(id) = queue.pop() {
        if !visited.insert(id) {
            continue;
        }
        let node = nodes.get(id).ok_or("incomplete Cargo dependency graph")?;
        for dependency in node["deps"]
            .as_array()
            .ok_or("missing Cargo dependency edges")?
        {
            let retained = dependency["dep_kinds"].as_array().is_some_and(|kinds| {
                kinds
                    .iter()
                    .any(|kind| kind["kind"].is_null() || kind["kind"] == "build")
            });
            if retained {
                queue.push(
                    dependency["pkg"]
                        .as_str()
                        .ok_or("missing dependency identity")?,
                );
            }
        }
    }
    let directory = output.join("licenses/rust");
    fs::create_dir_all(&directory)?;
    let mut rows = Vec::new();
    for (id, package) in packages {
        if !visited.contains(id) || own.contains(id) {
            continue;
        }
        let name = package["name"]
            .as_str()
            .ok_or("missing Rust dependency name")?;
        let version = package["version"]
            .as_str()
            .ok_or("missing Rust dependency version")?;
        if !name
            .bytes()
            .chain(version.bytes())
            .all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
        {
            return Err("invalid Rust package identity".into());
        }
        let root = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("missing dependency manifest")?,
        )
        .parent()
        .ok_or("invalid dependency manifest")?
        .canonicalize()?;
        let mut candidates = Vec::new();
        if let Some(license) = package["license_file"].as_str() {
            candidates.push(root.join(license));
        }
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if entry.file_type()?.is_dir()
                && matches!(
                    filename.as_str(),
                    "licenses" | "licences" | "license" | "licence" | "legal"
                )
            {
                legal_directory(&entry.path(), 0, &mut candidates)?;
            } else if entry.file_type()?.is_file()
                && ["license", "licence", "notice", "copyright", "copying"]
                    .iter()
                    .any(|prefix| {
                        filename == *prefix
                            || filename.starts_with(&format!("{prefix}-"))
                            || filename.starts_with(&format!("{prefix}."))
                    })
            {
                candidates.push(entry.path());
            }
        }
        if !candidates.is_empty() && root.join("AUTHORS").is_file() {
            candidates.push(root.join("AUTHORS"));
        }
        if candidates.len() > 64 {
            return Err("Rust package has too many legal files".into());
        }
        let mut candidates = candidates
            .into_iter()
            .map(|path| path.canonicalize())
            .collect::<std::io::Result<Vec<_>>>()?;
        candidates.sort();
        candidates.dedup();
        if candidates.is_empty() {
            return Err(format!(
                "Rust dependency {name}-{version} has no distributed license text"
            )
            .into());
        }
        let target = directory.join(format!("{name}-{version}"));
        fs::create_dir(&target)?;
        let mut files = Vec::new();
        for license in candidates {
            let canonical = license.canonicalize()?;
            if !canonical.starts_with(&root)
                || !canonical.is_file()
                || fs::metadata(&canonical)?.len() > 1_048_576
            {
                return Err("Rust license file escapes its package or exceeds its bound".into());
            }
            let relative = canonical
                .strip_prefix(&root)
                .map_err(|_| "Rust license escapes its package")?;
            let filename = relative
                .to_str()
                .ok_or("invalid license filename")?
                .replace('\\', "/");
            let destination = target.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let bytes = fs::read(&canonical)?;
            let sha256: String = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            fs::write(destination, bytes)?;
            files.push(json!({ "file": filename, "sha256": sha256 }));
        }
        rows.push(json!({ "name": name, "version": version, "license": package["license"], "files": files }));
    }
    rows.sort_by(|a, b| {
        a["name"]
            .as_str()
            .cmp(&b["name"].as_str())
            .then(a["version"].as_str().cmp(&b["version"].as_str()))
    });
    let mut bytes = serde_json::to_vec_pretty(
        &json!({ "coverage": "resolved normal and build dependencies of the native product on the build host; conservative", "target": target, "packages": rows }),
    )?;
    bytes.push(b'\n');
    fs::write(directory.join("packages.json"), bytes)?;
    Ok(())
}
