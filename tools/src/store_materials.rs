//! Store copy and code-native artwork around authentic isolated-browser captures.
use super::{Result, complete, copy_tree, product_version, source_commit, test_browser, tool};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("missing store field {key}").into())
}

fn hash(path: &Path) -> Result<String> {
    Ok(Sha256::digest(fs::read(path)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

pub(super) fn prepare(captures: &Path, output: &Path) -> Result {
    if !captures.is_absolute()
        || !output.is_absolute()
        || output.starts_with(std::env::current_dir()?)
    {
        return Err("choose absolute captures and a new packet outside the source tree".into());
    }
    let commit = source_commit()?;
    if commit.len() != 40 {
        return Err("store material requires a clean source commit".into());
    }
    let copy: Value = serde_json::from_slice(&fs::read("distribution/listings.json")?)?;
    let artwork: Value = serde_json::from_slice(&fs::read("distribution/artwork.json")?)?;
    let locales = ["en", "ru", "kk", "ar", "zh-CN"];
    for locale in locales {
        let listing = &copy["locales"][locale];
        if text(listing, "name")? != "nddev-browseruse"
            || text(listing, "shortDescription")?.chars().count() > 132
            || text(listing, "description")?.chars().count() > 16_000
        {
            return Err(format!("invalid listing fields for {locale}").into());
        }
        for kind in ["popup", "consent"] {
            let path = captures.join(format!("{kind}-{locale}.png"));
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.is_file() || metadata.len() > 4_194_304 {
                return Err("a bounded regular PNG capture is required".into());
            }
            let bytes = fs::read(&path)?;
            if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.len() > 4_194_304 {
                return Err("a bounded authentic PNG capture is required".into());
            }
        }
    }
    fs::create_dir(output)?;
    fs::write(
        output.join("INCOMPLETE.txt"),
        "Store material generation incomplete.\n",
    )?;
    for name in [
        "PERMISSIONS.en.txt",
        "PRIVACY-REVIEW.en.txt",
        "REVIEWER-STEPS.en.txt",
        "SOURCES.txt",
    ] {
        fs::copy(Path::new("distribution").join(name), output.join(name))?;
    }
    let base = "https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse";
    fs::write(
        output.join("URLS.json"),
        serde_json::to_vec_pretty(&json!({
            "source":format!("{base}/tree/{commit}"),
            "privacy":format!("{base}/blob/{commit}/docs/privacy.md"),
            "installation":format!("{base}/blob/{commit}/docs/desktop.md"),
            "support":format!("{base}/issues"),
            "security":format!("{base}/blob/{commit}/SECURITY.md"),
            "website":base
        }))?,
    )?;
    let source = output.join("source");
    fs::create_dir(&source)?;
    copy_tree(
        Path::new("packages/desktop/extension/vendor/design-system"),
        &source.join("design-system"),
    )?;
    let mark = fs::read_to_string("distribution/branding/nddev-mark.svg")?;
    if hash(Path::new("distribution/branding/nddev-mark.svg"))?
        != "3997b08aa3c47d0d5827484db657c8e18a53dc0a00e1a875cc57eb931c6f4877"
    {
        return Err("canonical brand mark digest changed".into());
    }
    fs::write(
        source.join("mark.svg"),
        mark.replace("currentColor", "#FFD83F"),
    )?;
    for name in ["LICENSE", "NOTICE"] {
        fs::copy(name, source.join(name))?;
    }
    let css = "@import url('design-system/tokens.css');@import url('design-system/reference-fonts.css');*{box-sizing:border-box}html,body{margin:0;width:100%;height:100%;overflow:hidden}body{background:var(--surface-base);color:var(--text-primary);font-family:var(--font-sans)}html[lang=ar] body{font-family:var(--font-sans-ar)}html[lang=zh-CN] body{font-family:var(--font-sans-zh)}main{height:100%;padding:64px;display:flex;align-items:center;gap:64px}.copy{flex:1;min-width:0}.brand{display:flex;align-items:center;gap:16px;font-size:27px;font-weight:500;direction:ltr}.mark{background:#000;border-radius:22%;padding:12px;width:64px;height:64px}h1{font-size:56px;line-height:1.08;font-weight:500;letter-spacing:-1.2px;margin:48px 0 24px}p{font-size:24px;line-height:1.4;color:var(--text-secondary);margin:0 0 30px}.host{display:inline-block;border-block-start:5px solid var(--accent-brand);padding-block-start:15px;font-size:19px;color:var(--text-primary)}.capture{width:448px;height:auto;max-height:640px;object-fit:contain;flex:none;border:1px solid var(--border-subtle);border-radius:16px;background:var(--surface-raised)}.promo main{padding:22px;gap:16px}.promo .brand{font-size:21px}.promo .mark{width:40px;height:40px;padding:8px}.promo h1{font-size:31px;margin:24px 0 14px}.promo p{font-size:16px}.promo .host{font-size:14px}.marquee h1{font-size:58px;margin:32px 0}.logo main{padding:15%;display:block;background:#000}.logo img{width:100%;height:100%;object-fit:contain}";
    fs::write(source.join("artwork.css"), css)?;
    let mut frames = Vec::new();
    let mut input_hashes = serde_json::Map::new();
    for locale in locales {
        let dir = output.join(locale);
        fs::create_dir(&dir)?;
        let listing = &copy["locales"][locale];
        fs::write(
            dir.join("listing.txt"),
            format!(
                "{}\n\n{}\n\n{}\n",
                text(listing, "name")?,
                text(listing, "shortDescription")?,
                text(listing, "description")?
            ),
        )?;
        let words = &artwork[locale];
        for (kind, title, detail) in [
            ("popup", "control", "detail"),
            ("consent", "observation", "consent"),
        ] {
            let file = format!("{kind}-{locale}.png");
            fs::copy(captures.join(&file), source.join(&file))?;
            input_hashes.insert(file.clone(), hash(&captures.join(&file))?.into());
            let html = format!(
                "<!doctype html><html lang=\"{locale}\" dir=\"{}\" data-theme=\"light\"><meta charset=\"utf-8\"><link rel=\"stylesheet\" href=\"artwork.css\"><main><section class=\"copy\"><div class=\"brand\"><img class=\"mark\" src=\"mark.svg\">nddev-browseruse</div><h1>{}</h1><p>{}</p><div class=\"host\">{}</div></section><img class=\"capture\" src=\"{file}\"></main></html>",
                if locale == "ar" { "rtl" } else { "ltr" },
                escape(text(words, title)?),
                escape(text(words, detail)?),
                escape(text(words, "host")?)
            );
            let html_path = source.join(format!("{kind}-{locale}.html"));
            fs::write(&html_path, html)?;
            frames.push(json!({"html":html_path,"png":dir.join(format!("{kind}-1280x800.png")),"width":1280,"height":800}));
        }
        for (kind, width, height) in [("small-promo", 440, 280), ("marquee", 1400, 560)] {
            let class = if kind == "small-promo" {
                "promo"
            } else {
                "marquee"
            };
            let html = format!(
                "<!doctype html><html lang=\"{locale}\" dir=\"{}\" data-theme=\"light\"><meta charset=\"utf-8\"><link rel=\"stylesheet\" href=\"artwork.css\"><body class=\"{class}\"><main><section class=\"copy\"><div class=\"brand\"><img class=\"mark\" src=\"mark.svg\">nddev-browseruse</div><h1>{}</h1><div class=\"host\">{}</div></section></main></body></html>",
                if locale == "ar" { "rtl" } else { "ltr" },
                escape(text(words, "control")?),
                escape(text(words, "host")?)
            );
            let html_path = source.join(format!("{kind}-{locale}.html"));
            fs::write(&html_path, html)?;
            frames.push(json!({"html":html_path,"png":dir.join(format!("{kind}-{width}x{height}.png")),"width":width,"height":height}));
        }
    }
    let logos = output.join("logos");
    fs::create_dir(&logos)?;
    for size in [128, 300] {
        let html = source.join(format!("logo-{size}.html"));
        fs::write(
            &html,
            "<!doctype html><meta charset=\"utf-8\"><link rel=\"stylesheet\" href=\"artwork.css\"><body class=\"logo\"><main><img src=\"mark.svg\"></main></body>",
        )?;
        frames.push(json!({"html":html,"png":logos.join(format!("logo-{size}.png")),"width":size,"height":size}));
    }
    let plan = output.join("render-plan.json");
    fs::write(
        &plan,
        serde_json::to_vec_pretty(&json!({"chrome":test_browser()?,"frames":frames}))?,
    )?;
    let mut render = Command::new(tool("bun"));
    render.arg("e2e/render-store.ts").arg(&plan);
    complete(render)?;
    let mut sums = String::new();
    for frame in &frames {
        let path = Path::new(frame["png"].as_str().ok_or("missing rendered path")?);
        sums.push_str(&format!(
            "{}  {}\n",
            hash(path)?,
            path.strip_prefix(output)?.display()
        ));
    }
    fs::write(output.join("SHA256SUMS"), sums)?;
    fs::write(
        output.join("LISTINGS.json"),
        serde_json::to_vec_pretty(&copy)?,
    )?;
    fs::write(
        output.join("PROVENANCE.json"),
        serde_json::to_vec_pretty(
            &json!({"source":commit,"version":product_version()?,"inputCaptures":input_hashes,"designSystem":serde_json::from_slice::<Value>(&fs::read("design-system.lock.json")?)?,"render":"isolated sandboxed Chromium; actual popup captures, no owner data","review":"recorded in the release receipt","uploaded":false}),
        )?,
    )?;
    fs::remove_file(output.join("INCOMPLETE.txt"))?;
    println!("{}", output.display());
    Ok(())
}
