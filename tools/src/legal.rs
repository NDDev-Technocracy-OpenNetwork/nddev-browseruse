//! Local, localized legal screen. Original notices remain readable and packaged unchanged.
use super::{Result, product_version};
use serde_json::Value;
use std::{fs, path::Path};

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn pages() -> Result<Vec<(String, String)>> {
    let root = Path::new("packages/desktop/extension");
    let mut notices = vec![
        ("LICENSE".to_owned(), fs::read_to_string("LICENSE")?),
        ("NOTICE".to_owned(), fs::read_to_string("NOTICE")?),
    ];
    for directory in ["licenses/npm", "vendor/design-system/licenses"] {
        for entry in fs::read_dir(root.join(directory))? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "txt")
            {
                let metadata = fs::symlink_metadata(entry.path())?;
                if !metadata.is_file() || metadata.len() > 1_048_576 || notices.len() >= 64 {
                    return Err("legal notice input exceeds its bounds".into());
                }
                notices.push((
                    format!(
                        "{directory}/{}",
                        entry.file_name().to_str().ok_or("invalid notice name")?
                    ),
                    fs::read_to_string(entry.path())?,
                ));
            }
        }
    }
    notices.sort_by(|a, b| a.0.cmp(&b.0));
    if notices.iter().map(|(_, text)| text.len()).sum::<usize>() > 4_194_304 {
        return Err("legal screen exceeds its text bound".into());
    }
    let details: String = notices
        .iter()
        .map(|(name, text)| {
            format!(
                "<details><summary>{}</summary><pre>{}</pre></details>\n",
                escape(name),
                escape(&text.replace("\r\n", "\n"))
            )
        })
        .collect();
    let cargo: toml::Value = toml::from_str(&fs::read_to_string("Cargo.toml")?)?;
    let repository = cargo["package"]["repository"]
        .as_str()
        .ok_or("missing source repository")?;
    ["en","ru","kk","ar","zh-CN"].iter().map(|language| {
        let catalog: Value = serde_json::from_slice(&fs::read(root.join(format!("_locales/{}/messages.json",language.replace('-',"_"))))?)?;
        let message = |key: &str| -> Result<String> { Ok(escape(catalog[key]["message"].as_str().ok_or("missing legal-screen translation")?)) };
        let html=format!("<!doctype html>\n<html lang=\"{language}\" dir=\"{}\" data-platform=\"desktop\"><head><meta charset=\"utf-8\"><meta name=\"color-scheme\" content=\"light dark\"><title>{}</title><link rel=\"stylesheet\" href=\"../styles/fonts.css\"><link rel=\"stylesheet\" href=\"../vendor/design-system/tokens.css\"><link rel=\"stylesheet\" href=\"../styles/legal.css\"></head><body><main><header><img src=\"../icons/icon-48.png\" alt=\"\"><span>nddev-browseruse · v{}</span></header><h1>{}</h1><p>{}</p><p><a href=\"{}\" rel=\"noreferrer\">{}</a></p>{details}</main><script type=\"module\" src=\"../legal.js\"></script></body></html>\n",if *language=="ar" {"rtl"} else {"ltr"},message("licensesPageTitle")?,product_version()?,message("licensesPageTitle")?,message("licensesPageDescription")?,escape(repository),message("sourceLink")?);
        Ok((format!("{language}.html"),html))
    }).collect()
}

pub(super) fn generate() -> Result {
    let directory = Path::new("packages/desktop/extension/licenses");
    fs::create_dir_all(directory)?;
    for (name, html) in pages()? {
        fs::write(directory.join(name), html)?;
    }
    Ok(())
}

pub(super) fn check() -> Result {
    for (name, html) in pages()? {
        if fs::read_to_string(Path::new("packages/desktop/extension/licenses").join(name))? != html
        {
            return Err("generated legal screen drift; run just generate on a build host".into());
        }
    }
    Ok(())
}
