use super::{Result, run, tool};
use nddev_browseruse_contract::Settings;
use std::{fs, path::Path};

const SCHEMA: &str = "contracts/openapi.json";
const EXECUTOR: &str = "packages/executor/settings.json";
const PUBLIC: &str = "packages/desktop/extension/public-settings.json";

fn settings(profile: &str) -> Result<Settings> {
    Ok(Settings::parse(
        &fs::read_to_string("config/common.toml")?,
        &fs::read_to_string(format!("config/{profile}.toml"))?,
    )?)
}

pub(super) fn check() -> Result {
    settings("dev")?;
    let settings = settings("production")?;
    if fs::read(SCHEMA)? != schema()?
        || fs::read(PUBLIC)? != public(&settings)?
        || fs::read(EXECUTOR)? != executor(&settings)?
    {
        return Err(
            "generated public configuration or schema drift; run just generate on a build host"
                .into(),
        );
    }
    Ok(())
}

fn schema() -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&nddev_browseruse_contract::schema())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn public(settings: &Settings) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&settings.public)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn executor(settings: &Settings) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&settings.executor)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn generate() -> Result {
    settings("dev")?;
    fs::create_dir_all("contracts")?;
    fs::write(SCHEMA, schema()?)?;
    fs::write(PUBLIC, public(&settings("production")?)?)?;
    fs::write(EXECUTOR, executor(&settings("production")?)?)?;
    types("packages/desktop/extension/src/generated")?;
    types("packages/executor/generated")
}

fn types(output: &str) -> Result {
    run(tool("bun"), &["tools/codegen/generate.ts", output])
}

pub(super) fn check_generated() -> Result {
    check()?;
    let path = ".tools/generated-contracts";
    if Path::new(path).exists() {
        fs::remove_dir_all(path)?;
    }
    types(path)?;
    for file in ["types.gen.ts", "index.ts"] {
        if fs::read(Path::new(path).join(file))?
            != fs::read(Path::new("packages/desktop/extension/src/generated").join(file))?
            || fs::read(Path::new(path).join(file))?
                != fs::read(Path::new("packages/executor/generated").join(file))?
        {
            return Err("generated TypeScript contract drift".into());
        }
    }
    Ok(())
}
