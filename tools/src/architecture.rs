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
    let shared = workspace["workspace"]["dependencies"]
        .as_table()
        .ok_or("missing shared dependency versions")?;
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
        dependencies(&manifest, &path, &root, allowed, shared)?;
    }
    Ok(())
}

fn dependencies(
    value: &toml::Value,
    path: &Path,
    root: &Path,
    allowed: &[&str],
    shared: &toml::Table,
) -> Result {
    let Some(table) = value.as_table() else {
        return Ok(());
    };
    for (key, value) in table {
        if matches!(
            key.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) {
            for (name, dependency) in value.as_table().ok_or("invalid dependency table")? {
                if dependency.get("workspace").and_then(toml::Value::as_bool) != Some(true)
                    || dependency.get("version").is_some()
                {
                    return Err(format!(
                        "{} dependency {name} must inherit its workspace version",
                        path.display()
                    )
                    .into());
                }
                let dependency = shared.get(name).ok_or("missing shared dependency")?;
                if let Some(relative) = dependency.get("path").and_then(toml::Value::as_str) {
                    let target = root.join(relative).canonicalize()?;
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
        } else if key != "workspace" {
            dependencies(value, path, root, allowed, shared)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_dependencies_still_enforce_component_boundaries() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap();
        let shared: toml::Table = toml::from_str("engine = { path = '.' }").unwrap();
        let client: toml::Value =
            toml::from_str("[dependencies]\nengine.workspace = true").unwrap();
        let error = dependencies(&client, &root, &root, &[], &shared).unwrap_err();
        assert!(error.to_string().contains("forbidden component dependency"));
    }

    #[test]
    fn members_cannot_select_their_own_dependency_versions() {
        let member: toml::Value = toml::from_str("[dependencies]\nserde = '=1.0.229'").unwrap();
        let error = dependencies(
            &member,
            Path::new("client"),
            Path::new("."),
            &[],
            &toml::Table::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("must inherit"));
    }
}
