//! Qualify staged bytes once; publication consumes those bytes without rebuilding the application.
use super::{
    Result, browser_environment, complete, executable, product_version, source_commit, tool,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, FileTimes},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, UNIX_EPOCH},
};

pub(super) fn check() -> Result {
    super::run(
        tool("bun"),
        &[
            "-e",
            r#"
const fs = require('node:fs');
const AsyncFunction = Object.getPrototypeOf(async function() {}).constructor;
for (const file of fs.readdirSync('.github/workflows').filter(name => /\.ya?ml$/.test(name))) {
  const workflow = Bun.YAML.parse(fs.readFileSync(`.github/workflows/${file}`, 'utf8'));
  for (const job of Object.values(workflow.jobs ?? {})) {
    for (const step of job.steps ?? []) {
      if (step.uses?.startsWith('actions/github-script@')) {
        if (typeof step.with?.script !== 'string') throw new Error('Missing GitHub script');
        new AsyncFunction('github', 'context', 'core', 'require', 'fetch', step.with.script);
      }
    }
  }
}
"#,
        ],
    )
}

fn read(path: &Path) -> Result<Value> {
    if fs::metadata(path)?.len() > 16 * 1024 * 1024 {
        return Err("release JSON exceeds its bound".into());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn write(path: &Path, value: &Value) -> Result {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn digest(path: &Path) -> Result<String> {
    if fs::metadata(path)?.len() > 256 * 1024 * 1024 {
        return Err("hashed release file exceeds its bound".into());
    }
    Ok(Sha256::digest(fs::read(path)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn browser_summary(report: &Value) -> Result<Value> {
    let mut passed = 0_u64;
    let mut skipped = Vec::new();
    if report["success"] != true || report["numFailedTests"] != 0 {
        return Err("browser acceptance did not succeed".into());
    }
    let suites = report["testResults"]
        .as_array()
        .ok_or("missing browser suites")?;
    for suite in suites {
        for case in suite["assertionResults"]
            .as_array()
            .ok_or("missing browser cases")?
        {
            match case["status"].as_str() {
                Some("passed") => passed += 1,
                Some("pending" | "skipped" | "todo") => {
                    let name = case["fullName"]
                        .as_str()
                        .ok_or("missing skipped case name")?;
                    if name.len() > 1024 || skipped.len() >= 1024 {
                        return Err("browser skip report exceeds its bound".into());
                    }
                    skipped.push(name.to_owned());
                }
                _ => return Err("browser case has a non-successful outcome".into()),
            }
        }
    }
    if passed == 0 || report["numTotalTests"].as_u64() != Some(passed + skipped.len() as u64) {
        return Err("browser report is empty or incomplete".into());
    }
    Ok(json!({"passed":passed,"skipped":skipped,"failed":0}))
}

pub(super) fn accept(output: &Path, captures: Option<&Path>) -> Result<Value> {
    let report = output.join("browser-results.json");
    let mut browser = Command::new(tool("bun"));
    browser.args([
        "run",
        "e2e",
        "--reporter=default",
        "--reporter=json",
        "--outputFile",
    ]);
    browser.arg(&report);
    browser.env(
        "NDS_TEST_HOST",
        output.join("bin").join(executable("nddev-browseruse")),
    );
    browser.env("NDS_TEST_EXTENSION", output.join("extension"));
    browser.env("NDS_TEST_EXECUTOR", output.join("executor/executor.ts"));
    if let Some(captures) = captures {
        browser.env("NDS_E2E_SHOTS", captures);
    }
    browser_environment(&mut browser)?;
    complete(browser)?;
    let summary = browser_summary(&read(&report)?)?;
    write(
        &output.join("BROWSER-ACCEPTANCE.json"),
        &json!({
            "source":source_commit()?,"version":product_version()?,
            "platform":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH},
            "browser":summary,
            "scope":"isolated real Chromium and ADB fixtures; physical Android and actual Edge remain separate"
        }),
    )?;
    // Keep the measured summary, without absolute workspace paths from the runner's raw report.
    fs::remove_file(report)?;
    Ok(summary)
}

fn tree(root: &Path, directory: &Path, files: &mut Vec<PathBuf>, bytes: &mut u64) -> Result {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.strip_prefix(root)?.components().count() > 64 {
            return Err("release tree is too deep".into());
        }
        if !path.canonicalize()?.starts_with(root) {
            return Err("release tree contains an external link".into());
        }
        let metadata = fs::metadata(&path)?;
        if metadata.is_dir() {
            if fs::symlink_metadata(&path)?.is_symlink() {
                return Err("release tree contains a directory link".into());
            }
            tree(root, &path, files, bytes)?;
        } else if metadata.is_file() {
            files.push(path);
            *bytes = bytes.saturating_add(metadata.len());
            if files.len() > 20_000 || *bytes > 256 * 1024 * 1024 {
                return Err("release tree exceeds its bound".into());
            }
        } else {
            return Err("release tree contains a special file".into());
        }
    }
    Ok(())
}

fn archive(input: &Path, output: &Path) -> Result {
    let root = input.canonicalize()?;
    let mut files = Vec::new();
    tree(&root, &root, &mut files, &mut 0)?;
    if output.exists() || output.starts_with(input) {
        return Err("release archive must be a new file outside its input".into());
    }
    files.sort();
    let mut list = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&root)?
            .to_str()
            .ok_or("non-UTF-8 archive path")?;
        if relative.contains(['\n', '\r', '\\']) {
            return Err("invalid archive path".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if fs::metadata(&file)?.permissions().mode() & 0o111 == 0 {
                0o644
            } else {
                0o755
            };
            fs::set_permissions(&file, fs::Permissions::from_mode(mode))?;
        }
        File::open(&file)?.set_times(
            FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(315_532_800)),
        )?;
        list.extend_from_slice(relative.as_bytes());
        list.push(b'\n');
    }
    let mut zip = Command::new("zip");
    zip.args(["-q", "-X", "-9"])
        .arg(output)
        .arg("-@")
        .env("TZ", "UTC")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::piped())
        .current_dir(input);
    let mut child = zip.spawn()?;
    let written = child
        .stdin
        .take()
        .ok_or("missing archive input pipe")?
        .write_all(&list);
    if let Err(error) = written {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }
    if !child.wait()?.success() {
        return Err("release archive failed".into());
    }
    let mut check = Command::new("unzip");
    check.arg("-tqq").arg(output);
    complete(check)
}

fn zip_info(input: &Path, option: &str, limit: u64) -> Result<String> {
    let mut child = Command::new("unzip")
        .arg(option)
        .arg(input)
        .env("LC_ALL", "C")
        .stdout(Stdio::piped())
        .spawn()?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or("missing ZIP listing pipe")?
        .take(limit + 1)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() as u64 > limit {
        let _ = child.kill();
        let _ = child.wait();
        return Err("ZIP listing exceeds its bound or could not be read".into());
    }
    if !child.wait()?.success() {
        return Err("invalid ZIP directory".into());
    }
    Ok(String::from_utf8(bytes)?)
}

fn unpack(input: &Path, output: &Path) -> Result {
    if fs::metadata(input)?.len() > 256 * 1024 * 1024 {
        return Err("downloaded artifact exceeds its bound".into());
    }
    let totals = zip_info(input, "-Zt", 4096)?;
    let mut fields = totals.split_whitespace();
    let entries = fields.next().ok_or("missing ZIP count")?.parse::<usize>()?;
    let bytes = fields.nth(1).ok_or("missing ZIP size")?.parse::<u64>()?;
    if entries == 0 || entries > 20_000 || bytes > 256 * 1024 * 1024 {
        return Err("ZIP contents exceed their bound".into());
    }
    let listing = zip_info(input, "-Z1", 4 * 1024 * 1024)?;
    for name in listing.lines() {
        if name.len() > 4096
            || Path::new(name).components().count() > 64
            || name.contains('\\')
            || Path::new(name).is_absolute()
            || Path::new(name)
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err("artifact path escapes its directory".into());
        }
    }
    fs::create_dir(output)?;
    let mut unzip = Command::new("unzip");
    unzip.arg("-q").arg(input).arg("-d").arg(output);
    complete(unzip)?;
    let root = output.canonicalize()?;
    tree(&root, &root, &mut Vec::new(), &mut 0)
}

pub(super) fn prepare() -> Result {
    let root = std::env::current_dir()?;
    let workspace = tempfile::Builder::new()
        .prefix("qualified-")
        .tempdir_in(root.join(".tools"))?;
    let packet = workspace.path().join("packet");
    fs::create_dir(&packet)?;
    let native = packet.join("Native");
    super::stage(&native, "release")?;
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        accept(&native, None)?;
        return Ok(());
    }
    let captures = packet.join("QA-captures");
    let summary = accept(&native, Some(&captures))?;
    if summary["skipped"]
        .as_array()
        .is_none_or(|skipped| !skipped.is_empty())
    {
        return Err("Linux release qualification must execute every browser case".into());
    }
    super::store::package(&packet.join("Extension"))?;
    super::store_materials::prepare(&captures, &packet.join("Store-materials"))?;
    let archive_path = workspace.path().join("qualified-linux.zip");
    archive(&packet, &archive_path)?;
    fs::create_dir_all(root.join("dist"))?;
    fs::rename(archive_path, root.join("dist/qualified-linux.zip"))?;
    Ok(())
}

fn requirements(source: &str, version: &str) -> Value {
    json!({"source":source,"version":version,"workflows":[
        {"key":"check","file":"ci.yml","jobs":["check / full check"],"artifacts":[{"key":"linux","name":"qualified-linux"}]},
        {"key":"native","file":"native.yml","jobs":["native (macos-15)","native (windows-2025)"],"artifacts":[{"key":"macos","name":"native-macos-15"},{"key":"windows","name":"native-windows-2025"}]},
        {"key":"security","file":"security.yml","jobs":["scan / daily security"],"artifacts":[]}
    ]})
}

pub(super) fn write_requirements() -> Result {
    let source = source_commit()?;
    if source.len() != 40 {
        return Err("release requirements need a clean source commit".into());
    }
    fs::create_dir_all(".tools")?;
    write(
        Path::new(".tools/release-requirements.json"),
        &requirements(&source, &product_version()?),
    )
}

fn workflow_evidence(evidence: &Value, source: &str, version: &str) -> Result {
    if evidence["source"] != source {
        return Err("workflow evidence belongs to another source".into());
    }
    let requirements = requirements(source, version);
    for needed in requirements["workflows"]
        .as_array()
        .ok_or("invalid release requirements")?
    {
        let key = needed["key"].as_str().ok_or("missing workflow key")?;
        let run = &evidence["runs"][key];
        if run["head_sha"] != source
            || run["head_branch"] != "dev"
            || run["event"] != "push"
            || run["conclusion"] != "success"
            || run["id"].as_u64().is_none_or(|id| id == 0)
        {
            return Err("release needs successful dev push runs of this exact commit".into());
        }
        let jobs = run["jobs"].as_array().ok_or("missing workflow jobs")?;
        for name in needed["jobs"].as_array().ok_or("missing expected jobs")? {
            if !jobs
                .iter()
                .any(|job| job["name"] == *name && job["conclusion"] == "success")
            {
                return Err("an expected release check did not succeed".into());
            }
        }
        for artifact in needed["artifacts"]
            .as_array()
            .ok_or("missing expected artifacts")?
        {
            let key = artifact["key"].as_str().ok_or("invalid artifact key")?;
            let actual = &evidence["artifacts"][key];
            if actual["workflow_run_id"] != run["id"]
                || actual["name"] != artifact["name"]
                || actual["expired"] != false
                || actual["id"].as_u64().is_none_or(|id| id == 0)
            {
                return Err(
                    "artifact is not the expected live output of the verified dev run".into(),
                );
            }
        }
    }
    Ok(())
}

fn native(packet: &Path, source: &str, version: &str, os: &str, arch: &str) -> Result<Value> {
    let build = read(&packet.join("CANDIDATE.json"))?;
    let acceptance = read(&packet.join("BROWSER-ACCEPTANCE.json"))?;
    for value in [&build, &acceptance] {
        if value["source"] != source
            || value["version"] != version
            || value["platform"]["os"] != os
            || value["platform"]["arch"] != arch
        {
            return Err("native qualification source, version or target mismatch".into());
        }
    }
    if build["profile"] != "release"
        || acceptance["browser"]["failed"] != 0
        || acceptance["browser"]["passed"]
            .as_u64()
            .is_none_or(|count| count == 0)
        || acceptance["browser"]["skipped"].as_array().is_none()
    {
        return Err("native packet has no successful release qualification".into());
    }
    let binary = packet.join("bin").join(if os == "windows" {
        "nddev-browseruse.exe"
    } else {
        "nddev-browseruse"
    });
    #[cfg(unix)]
    if os != "windows" {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
    }
    Ok(json!({"browser":acceptance["browser"],"nativeExecutableSha256":digest(&binary)?}))
}

pub(super) fn assemble(input: &Path, output: &Path) -> Result {
    if !input.is_absolute() || !output.is_absolute() {
        return Err("release assembly requires absolute input/output directories".into());
    }
    let source = source_commit()?;
    if source.len() != 40 {
        return Err("release assembly requires a clean source commit".into());
    }
    let version = product_version()?;
    let evidence = read(&input.join("evidence.json"))?;
    workflow_evidence(&evidence, &source, &version)?;
    for key in ["linux", "macos", "windows"] {
        let expected = evidence["artifacts"][key]["digest"]
            .as_str()
            .ok_or("missing artifact digest")?;
        if expected != format!("sha256:{}", digest(&input.join(format!("{key}.zip")))?) {
            return Err("downloaded Actions artifact digest mismatch".into());
        }
    }
    let linux_outer = input.join("linux-outer");
    unpack(&input.join("linux.zip"), &linux_outer)?;
    let linux = input.join("linux");
    unpack(&linux_outer.join("qualified-linux.zip"), &linux)?;
    let macos = input.join("macos");
    unpack(&input.join("macos.zip"), &macos)?;
    let windows = input.join("windows");
    unpack(&input.join("windows.zip"), &windows)?;
    fs::create_dir(output)?;
    let mut platforms = serde_json::Map::new();
    for (target, path, os, arch) in [
        ("linux-x64", linux.join("Native"), "linux", "x86_64"),
        ("macos-arm64", macos, "macos", "aarch64"),
        ("windows-x64", windows, "windows", "x86_64"),
    ] {
        platforms.insert(target.into(), native(&path, &source, &version, os, arch)?);
        archive(
            &path,
            &output.join(format!("nddev-browseruse-{version}-{target}.zip")),
        )?;
    }
    for receipt in [
        linux.join("Extension/STORE-TARGETS.json"),
        linux.join("Store-materials/PROVENANCE.json"),
    ] {
        let value = read(&receipt)?;
        if value["source"] != source || value["version"] != version {
            return Err("store packet source or version mismatch".into());
        }
    }
    for channel in ["chrome", "edge"] {
        let name = format!("nddev-browseruse-{version}-{channel}.zip");
        fs::copy(linux.join("Extension").join(&name), output.join(name))?;
    }
    archive(
        &linux.join("Store-materials"),
        &output.join(format!("nddev-browseruse-{version}-store-materials.zip")),
    )?;
    let mut assets = Vec::new();
    for entry in fs::read_dir(output)? {
        let path = entry?.path();
        assets.push(json!({"name":path.file_name().and_then(|s|s.to_str()).ok_or("invalid asset name")?,"sha256":digest(&path)?,"size":fs::metadata(path)?.len()}));
    }
    assets.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    write(
        &output.join("QUALIFICATION.json"),
        &json!({
            "version":version,"source":source,"platforms":platforms,"workflowEvidence":evidence,
            "artifacts":assets,"applicationRebuiltDuringPromotion":false,
            "limits":["Physical Android and actual Edge/catalog admission require separate qualification.","NDS adapter/telemetry composition remains downstream.","Owner loads the extension and submits store packets.","Existing unmarked groups are not deleted by guessing their titles."]
        }),
    )?;
    assets.push(json!({"name":"QUALIFICATION.json","sha256":digest(&output.join("QUALIFICATION.json"))?,"size":fs::metadata(output.join("QUALIFICATION.json"))?.len()}));
    let checksums = assets
        .iter()
        .map(|asset| {
            format!(
                "{}  {}\n",
                asset["sha256"].as_str().unwrap_or_default(),
                asset["name"].as_str().unwrap_or_default()
            )
        })
        .collect::<String>();
    fs::write(output.join("SHA256SUMS"), checksums)?;
    assets.push(json!({"name":"SHA256SUMS","sha256":digest(&output.join("SHA256SUMS"))?,"size":fs::metadata(output.join("SHA256SUMS"))?.len()}));
    write(
        &input.join("publication.json"),
        &json!({"source":source,"version":version,"tag":format!("v{version}"),"directory":output,"assets":assets}),
    )?;
    fs::write(
        input.join("release-notes.md"),
        fs::read_to_string("distribution/RELEASE-NOTES.md")?.replace("{{source}}", &source),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_or_incomplete_browser_reports_cannot_qualify_a_packet() {
        let report = json!({"success":true,"numFailedTests":0,"numTotalTests":2,"testResults":[{"assertionResults":[{"status":"passed","fullName":"fixture"}]}]});
        assert!(browser_summary(&report).is_err());
        let mut report = report;
        report["numTotalTests"] = 1.into();
        assert_eq!(browser_summary(&report).unwrap()["passed"], 1);
        report["testResults"][0]["assertionResults"][0]["status"] = "failed".into();
        assert!(browser_summary(&report).is_err());
    }

    #[test]
    fn declared_platform_skips_remain_visible_in_measured_evidence() {
        let report = json!({"success":true,"numFailedTests":0,"numTotalTests":2,"testResults":[{"assertionResults":[{"status":"passed","fullName":"common behavior"},{"status":"pending","fullName":"platform fixture"}]}]});
        let summary = browser_summary(&report).unwrap();
        assert_eq!(summary["passed"], 1);
        assert_eq!(summary["skipped"], json!(["platform fixture"]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn packet_archives_repeat_with_hidden_files_and_executable_modes() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = tempfile::tempdir().unwrap();
        let input = fixture.path().join("packet");
        fs::create_dir_all(input.join("bin")).unwrap();
        fs::create_dir(input.join(".hidden")).unwrap();
        fs::write(input.join(".hidden/lock"), "hidden fixture").unwrap();
        let binary = input.join("bin/fixture");
        fs::write(&binary, "executable fixture").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        let first = fixture.path().join("first.zip");
        let second = fixture.path().join("second.zip");
        archive(&input, &first).unwrap();
        archive(&input, &second).unwrap();
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
        let extracted = fixture.path().join("extracted");
        unpack(&first, &extracted).unwrap();
        assert_eq!(
            fs::read(extracted.join(".hidden/lock")).unwrap(),
            b"hidden fixture"
        );
        assert_ne!(
            fs::metadata(extracted.join("bin/fixture"))
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0
        );
    }

    #[test]
    fn another_source_cannot_be_relabelled_as_this_native_packet() {
        let fixture = tempfile::tempdir().unwrap();
        for name in ["CANDIDATE.json", "BROWSER-ACCEPTANCE.json"] {
            write(&fixture.path().join(name),&json!({"source":"other","version":"0.2.7","platform":{"os":"linux","arch":"x86_64"}})).unwrap();
        }
        assert!(native(fixture.path(), "expected", "0.2.7", "linux", "x86_64").is_err());
    }

    #[test]
    fn release_rejects_a_pr_run_a_failed_job_and_an_unrelated_artifact() {
        let source = "a".repeat(40);
        let mut evidence = json!({"source":source,"runs":{},"artifacts":{}});
        let needed = requirements(&source, "0.2.7");
        for (index, workflow) in needed["workflows"].as_array().unwrap().iter().enumerate() {
            let key = workflow["key"].as_str().unwrap();
            evidence["runs"][key] = json!({"id":index+1,"head_sha":source,"head_branch":"dev","event":"push","conclusion":"success","jobs":workflow["jobs"].as_array().unwrap().iter().map(|name|json!({"name":name,"conclusion":"success"})).collect::<Vec<_>>()});
            for artifact in workflow["artifacts"].as_array().unwrap() {
                evidence["artifacts"][artifact["key"].as_str().unwrap()] = json!({"id":10+index,"workflow_run_id":index+1,"name":artifact["name"],"expired":false});
            }
        }
        workflow_evidence(&evidence, &source, "0.2.7").unwrap();
        let mut wrong = evidence.clone();
        wrong["runs"]["check"]["event"] = "pull_request".into();
        assert!(workflow_evidence(&wrong, &source, "0.2.7").is_err());
        let mut wrong = evidence.clone();
        wrong["runs"]["native"]["jobs"][0]["conclusion"] = "failure".into();
        assert!(workflow_evidence(&wrong, &source, "0.2.7").is_err());
        evidence["artifacts"]["macos"]["workflow_run_id"] = 999.into();
        assert!(workflow_evidence(&evidence, &source, "0.2.7").is_err());
    }
}
