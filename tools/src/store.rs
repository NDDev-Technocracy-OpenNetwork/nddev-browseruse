//! Canonical Linux ZIP packaging. Store upload and native-host registration are owner actions.
use super::{
    Result, capture, check_generated, check_version, copy_tree, product_version, source_commit,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, FileTimes},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, UNIX_EPOCH},
};

const CHROME_ITEM: &str = "lpnfnhkdjamhnoknoohhmjfhhjkeofio";
const SOURCE: &str = "packages/desktop/extension";

pub(super) fn package(output: &Path) -> Result {
    if !cfg!(target_os = "linux") {
        return Err("store ZIPs are packaged on the canonical Linux build host".into());
    }
    if !output.is_absolute() || output.starts_with(std::env::current_dir()?) {
        return Err("choose an absolute new packet directory outside the source tree".into());
    }
    check_version()?;
    check_generated()?;
    let commit = source_commit()?;
    if commit.len() != 40 || !commit.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(
            "store packaging requires a clean Git checkout with a full source commit".into(),
        );
    }
    let design: Value = serde_json::from_slice(&fs::read("design-system.lock.json")?)?;
    assemble(
        Path::new("."),
        output,
        &product_version()?,
        &commit,
        &design,
    )?;
    println!("{}", output.display());
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}

fn unpacked_id(manifest: &Value) -> Result<String> {
    let key = STANDARD.decode(
        manifest["key"]
            .as_str()
            .ok_or("unpacked extension key is missing")?,
    )?;
    let hash = Sha256::digest(key);
    Ok(hash[..16]
        .iter()
        .flat_map(|byte| {
            [
                char::from(b'a' + (byte >> 4)),
                char::from(b'a' + (byte & 15)),
            ]
        })
        .collect())
}

fn assemble(root: &Path, output: &Path, version: &str, commit: &str, design: &Value) -> Result {
    // Never erase or reuse the caller's existing folder.
    fs::create_dir(output)?;
    fs::write(
        output.join("INCOMPLETE.txt"),
        "Packaging is incomplete. Do not upload these files.\n",
    )?;
    let unpacked = output.join("unpacked");
    copy_tree(&root.join(SOURCE), &unpacked)?;
    for file in ["LICENSE", "NOTICE"] {
        fs::copy(root.join(file), unpacked.join(file))?;
    }
    let mut manifest: Value = serde_json::from_slice(&fs::read(unpacked.join("manifest.json"))?)?;
    if manifest["manifest_version"] != 3 || manifest["version"] != version {
        return Err("extension and Cargo release versions must agree".into());
    }
    let unpacked_id = unpacked_id(&manifest)?;
    let store = output.join(".store-input");
    copy_tree(&unpacked, &store)?;
    manifest
        .as_object_mut()
        .ok_or("manifest must be an object")?
        .remove("key");
    write_json(&store.join("manifest.json"), &manifest)?;
    let build = json!({ "product": "nddev-browseruse", "version": version, "source": commit,
        "designSystem": design });
    write_json(&unpacked.join("BUILD.json"), &build)?;
    write_json(&store.join("BUILD.json"), &build)?;
    let chrome = format!("nddev-browseruse-{version}-chrome.zip");
    let edge = format!("nddev-browseruse-{version}-edge.zip");
    zip(&store, &output.join(&chrome))?;
    // The two stores consume the same engine and permission surface.
    fs::copy(output.join(&chrome), output.join(&edge))?;
    let bytes = fs::read(output.join(&chrome))?;
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    fs::write(
        output.join("SHA256SUMS"),
        format!("{digest}  {chrome}\n{digest}  {edge}\n"),
    )?;
    write_json(
        &output.join("STORE-TARGETS.json"),
        &json!({
            "format": 1, "product": "nddev-browseruse", "version": version, "source": commit,
            "qualification": "recorded in the release receipt", "uploaded": false, "nativeHostRegistered": false,
            "unpacked": { "directory": "unpacked", "extensionId": unpacked_id, "keyPreserved": true },
            "chrome": { "operation": "update-existing", "itemId": CHROME_ITEM, "zip": chrome, "sha256": digest },
            "edge": { "operation": "create-future-listing", "itemId": null, "zip": edge, "sha256": digest,
                "assignedExtensionIdAdmissionPending": true },
            "zipTool": capture("zip", &["-v"])?.lines().take(2).collect::<Vec<_>>()
        }),
    )?;
    fs::write(
        output.join("READ-ME.ru.txt"),
        "Пакеты подготовлены из указанного коммита. STORE-TARGETS.json фиксирует исходник и назначения; приёмка записана отдельно в release receipt.\nChrome: обновить существующую карточку; ZIP не содержит ключ unpacked-установки.\nEdge: пакет для будущей новой карточки; после назначения runtime extension ID потребуется допуск в native host и проверка Edge.\nUnpacked сохраняет прежний публичный ключ и ID. Обновление host до совместимой версии обязательно.\nНичего не зарегистрировано в браузере и не опубликовано. Отдельно приложите проверенный native-пакет, листинги, изображения и ответы проверки приватности.\n",
    )?;
    fs::remove_dir_all(store)?;
    fs::remove_file(output.join("INCOMPLETE.txt"))?;
    Ok(())
}

fn files(root: &Path, below: &Path, result: &mut Vec<PathBuf>) -> Result {
    for entry in fs::read_dir(root.join(below))? {
        let entry = entry?;
        let relative = below.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            files(root, &relative, result)?;
        } else if kind.is_file() {
            if result.len() >= 10_000 {
                return Err("ZIP file count exceeds its bound".into());
            }
            result.push(relative);
        } else {
            return Err("ZIP inputs must be regular files without symbolic links".into());
        }
    }
    Ok(())
}

fn zip(input: &Path, output: &Path) -> Result {
    let mut paths = Vec::new();
    files(input, Path::new(""), &mut paths)?;
    paths.sort();
    if paths.is_empty() || paths.len() > 10_000 {
        return Err("invalid ZIP file count".into());
    }
    let modified = UNIX_EPOCH + Duration::from_secs(315_532_800); // 1980-01-01 UTC, DOS epoch.
    let mut list = Vec::new();
    let mut total = 0_u64;
    for path in paths {
        let text = path.to_str().ok_or("ZIP names must be UTF-8")?;
        if text.contains(['\n', '\r', '\\']) {
            return Err("invalid ZIP file name".into());
        }
        total = total.saturating_add(fs::metadata(input.join(&path))?.len());
        if total > 64 * 1024 * 1024 {
            return Err("store ZIP input exceeds 64 MiB".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(input.join(&path), fs::Permissions::from_mode(0o644))?;
        }
        File::open(input.join(&path))?.set_times(FileTimes::new().set_modified(modified))?;
        list.extend_from_slice(text.as_bytes());
        list.push(b'\n');
    }
    let mut child = Command::new("zip")
        .args(["-q", "-X", "-0"])
        .arg(output)
        .arg("-@")
        .current_dir(input)
        .env("TZ", "UTC")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::piped())
        .spawn()?;
    let written = child
        .stdin
        .take()
        .ok_or("ZIP input pipe is missing")?
        .write_all(&list);
    if let Err(error) = written {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }
    if !child.wait()?.success() {
        return Err("ZIP packaging failed".into());
    }
    let mut check = Command::new("unzip");
    check.arg("-tqq").arg(output);
    if !check.status()?.success() {
        return Err("ZIP integrity check failed".into());
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            for _ in 0..100 {
                let path = std::env::temp_dir().join(format!(
                    "nddev-browseruse-store-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("store fixture directory failed: {error}"),
                }
            }
            panic!("store fixture directory capacity exhausted");
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn store_archives_are_repeatable_keep_notices_and_omit_unpacked_key_and_sources() {
        let fixture = Fixture::new();
        let root = fixture.0.join("source");
        let extension = root.join(SOURCE);
        fs::create_dir_all(extension.join("src")).unwrap();
        fs::create_dir_all(extension.join("test")).unwrap();
        fs::create_dir_all(extension.join("node_modules")).unwrap();
        fs::write(
            extension.join("node_modules/cache.js"),
            "dependency-cache fixture",
        )
        .unwrap();
        fs::write(extension.join("src/private.ts"), "source-only fixture").unwrap();
        fs::write(extension.join("test/fixture.ts"), "test-only fixture").unwrap();
        fs::write(extension.join("tsconfig.json"), "{}").unwrap();
        fs::write(extension.join("background.js"), "// public fixture\n").unwrap();
        fs::write(root.join("LICENSE"), "Apache license fixture\n").unwrap();
        fs::write(root.join("NOTICE"), "MIT attribution fixture\n").unwrap();
        let manifest = json!({ "manifest_version": 3, "version": "0.2.5", "key": "AA==" });
        write_json(&extension.join("manifest.json"), &manifest).unwrap();
        let first = fixture.0.join("first");
        let second = fixture.0.join("second");
        for output in [&first, &second] {
            assemble(
                &root,
                output,
                "0.2.5",
                &"a".repeat(40),
                &json!({ "fixture": true }),
            )
            .unwrap();
        }
        let chrome = "nddev-browseruse-0.2.5-chrome.zip";
        assert_eq!(
            fs::read(first.join(chrome)).unwrap(),
            fs::read(second.join(chrome)).unwrap()
        );
        assert_eq!(
            fs::read(first.join(chrome)).unwrap(),
            fs::read(first.join("nddev-browseruse-0.2.5-edge.zip")).unwrap()
        );
        let zip = first.join(chrome);
        let listing = capture("unzip", &["-Z1", zip.to_str().unwrap()]).unwrap();
        let names: Vec<_> = listing.lines().collect();
        assert_eq!(
            names,
            [
                "BUILD.json",
                "LICENSE",
                "NOTICE",
                "background.js",
                "manifest.json"
            ]
        );
        let packed: Value = serde_json::from_str(
            &capture("unzip", &["-p", zip.to_str().unwrap(), "manifest.json"]).unwrap(),
        )
        .unwrap();
        assert!(packed.get("key").is_none());
        let retained: Value =
            serde_json::from_slice(&fs::read(first.join("unpacked/manifest.json")).unwrap())
                .unwrap();
        assert_eq!(retained["key"], "AA==");
        assert_eq!(
            fs::read_to_string(first.join("unpacked/NOTICE")).unwrap(),
            "MIT attribution fixture\n"
        );
        assert!(!first.join("INCOMPLETE.txt").exists());
        let previous = fs::read(first.join("STORE-TARGETS.json")).unwrap();
        assert!(assemble(&root, &first, "0.2.5", &"b".repeat(40), &json!({})).is_err());
        assert_eq!(
            fs::read(first.join("STORE-TARGETS.json")).unwrap(),
            previous
        );
    }
}
