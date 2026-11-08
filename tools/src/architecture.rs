//! The independent client and contracts must never acquire a native-host dependency.
use super::Result;
use std::{fs, path::Path};

const COMPONENTS: &[(&str, &[&str])] = &[
    (
        ".",
        &["crates/contract", "crates/client", "crates/local-transport"],
    ),
    ("tools", &["crates/contract"]),
    ("crates/contract", &[]),
    ("crates/local-transport", &[]),
    (
        "crates/client",
        &["crates/contract", "crates/local-transport"],
    ),
];

pub(super) fn check() -> Result {
    // Windows canonical paths carry the extended-length prefix; compare canonical roots too.
    let root = std::env::current_dir()?.canonicalize()?;
    let workspace: toml::Value = toml::from_str(&fs::read_to_string("Cargo.toml")?)?;
    let members = workspace["workspace"]["members"]
        .as_array()
        .ok_or("missing workspace")?;
    if members.len() != COMPONENTS.len() - 1
        || members.iter().any(|member| {
            !COMPONENTS
                .iter()
                .any(|(directory, _)| Some(*directory) == member.as_str())
        })
    {
        return Err("workspace members differ from the reviewed component graph".into());
    }
    for (directory, allowed) in COMPONENTS {
        let path = root.join(directory);
        let manifest: toml::Value = toml::from_str(&fs::read_to_string(path.join("Cargo.toml"))?)?;
        if manifest
            .get("lints")
            .and_then(|lints| lints.get("workspace"))
            .and_then(toml::Value::as_bool)
            != Some(true)
        {
            return Err(
                format!("{directory} must inherit workspace visibility and safety lints").into(),
            );
        }
        dependencies(&manifest, &path, &root, allowed)?;
    }
    Ok(())
}

fn dependencies(value: &toml::Value, path: &Path, root: &Path, allowed: &[&str]) -> Result {
    let Some(table) = value.as_table() else {
        return Ok(());
    };
    for (key, value) in table {
        if matches!(
            key.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) {
            for (name, dependency) in value.as_table().ok_or("invalid dependency table")? {
                if let Some(relative) = dependency.get("path").and_then(toml::Value::as_str) {
                    let target = path.join(relative).canonicalize()?;
                    if !allowed
                        .iter()
                        .any(|directory| root.join(directory) == target)
                    {
                        return Err(format!(
                            "forbidden component dependency: {} -> {name}",
                            path.display()
                        )
                        .into());
                    }
                }
            }
        } else {
            dependencies(value, path, root, allowed)?;
        }
    }
    Ok(())
}
