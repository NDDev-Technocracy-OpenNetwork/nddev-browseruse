//! Repeatable project operations. Build this helper on a build host before running checks on USA.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    ffi::OsStr,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("browseruse tools: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute() -> Result {
    let command = std::env::args().nth(1).ok_or("choose a project command")?;
    let root = std::env::current_dir()?;
    if !root.join("tools/versions.json").is_file() {
        return Err("run from the repository root".into());
    }
    match command.as_str() {
        "setup" => setup(),
        "check" => {
            check_version()?;
            run("cargo", &["fmt", "--all", "--", "--check"])?;
            run(tool("cargo-shear"), &[])?;
            run(
                tool("cargo-deny"),
                &["--offline", "--locked", "check", "licenses", "sources"],
            )?;
            run(tool("bun"), &["run", "check"])?;
            run("git", &["diff", "--check"])
        }
        "fix" => {
            run("cargo", &["fmt", "--all"])?;
            run(
                tool("bun"),
                &["run", "biome", "check", "--write", "packages", "e2e"],
            )
        }
        "test" => {
            check_generated()?;
            run(
                "cargo",
                &[
                    "clippy",
                    "--locked",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            run("cargo", &["test", "--locked", "--workspace"])?;
            run(tool("bun"), &["run", "test"])
        }
        "generate" => generate(),
        "stage" => {
            let output = std::env::args()
                .nth(2)
                .ok_or("stage requires an absolute output directory")?;
            let profile = std::env::args().nth(3).unwrap_or_else(|| "release".into());
            stage(Path::new(&output), &profile)
        }
        "build" => {
            generate()?;
            let mut build = Command::new("cargo");
            build.args([
                "build",
                "--locked",
                "--release",
                "--package",
                "nddev-browseruse",
            ]);
            build.env("BROWSERUSE_SOURCE_COMMIT", source_commit()?);
            complete(build)
        }
        "browser-check" => {
            generate()?;
            run(
                "cargo",
                &["build", "--locked", "--package", "nddev-browseruse"],
            )?;
            let target = target_directory()?;
            let mut browser = Command::new(tool("bun"));
            browser.args(["run", "e2e"]);
            if let Some(pattern) = std::env::args().nth(2) {
                browser.args(["--test-name-pattern", &pattern]);
            }
            let chrome = test_browser()?;
            if cfg!(target_os = "linux")
                && std::env::var_os("NDS_TEST_CHROME").is_none()
                && std::env::var_os("CHROME_DEVEL_SANDBOX").is_none()
            {
                browser.env(
                    "CHROME_DEVEL_SANDBOX",
                    chrome
                        .parent()
                        .ok_or("invalid browser executable")?
                        .join("chrome_sandbox"),
                );
            }
            browser.env("NDS_TEST_CHROME", chrome);
            browser.env(
                "NDS_TEST_HOST",
                target.join("debug").join(executable("nddev-browseruse")),
            );
            complete(browser)
        }
        _ => Err("unknown project command".into()),
    }
}

fn executable(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

fn tool(name: &str) -> PathBuf {
    PathBuf::from(".tools/bin").join(executable(name))
}

fn run(program: impl AsRef<OsStr>, args: &[&str]) -> Result {
    let mut command = Command::new(program);
    command.args(args);
    complete(command)
}

fn complete(mut command: Command) -> Result {
    prepared_path(&mut command)?;
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}").into())
    }
}

fn capture(program: impl AsRef<OsStr>, args: &[&str]) -> Result<String> {
    let mut command = Command::new(program);
    command.args(args);
    prepared_path(&mut command)?;
    let output = command.output()?;
    if !output.status.success() {
        return Err("project tool query failed".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn prepared_path(command: &mut Command) -> Result {
    let mut paths = vec![std::env::current_dir()?.join(".tools/bin")];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    command.env("PATH", std::env::join_paths(paths)?);
    Ok(())
}

fn setup() -> Result {
    fs::create_dir_all(".tools/bin")?;
    let package: Value = serde_json::from_slice(&fs::read("package.json")?)?;
    let version = package["packageManager"]
        .as_str()
        .and_then(|p| p.strip_prefix("bun@"))
        .ok_or("packageManager must pin Bun")?;
    if capture(tool("bun"), &["--version"]).ok().as_deref() != Some(version) {
        let platform = match std::env::consts::OS {
            "linux" => "linux",
            "macos" => "darwin",
            "windows" => "windows",
            _ => return Err("unsupported Bun bootstrap platform".into()),
        };
        let architecture = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "aarch64",
            _ => return Err("unsupported Bun bootstrap architecture".into()),
        };
        // npm installs the pinned native binary package without running package lifecycle scripts.
        // It is only a bootstrap: all project TypeScript tooling runs with Bun.
        let name = format!("@oven/bun-{platform}-{architecture}");
        let specification = format!("{name}@{version}");
        let mut npm = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/D", "/C", "npm"]);
            command
        } else {
            Command::new("npm")
        };
        npm.args([
            "install",
            "--prefix",
            ".tools/bun",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            &specification,
        ]);
        complete(npm)?;
        fs::copy(
            Path::new(".tools/bun/node_modules")
                .join(name)
                .join("bin")
                .join(executable("bun")),
            tool("bun"),
        )?;
        if capture(tool("bun"), &["--version"])? != version {
            return Err("Bun bootstrap version mismatch".into());
        }
    }
    run(
        tool("bun"),
        &["install", "--frozen-lockfile", "--ignore-scripts"],
    )?;
    let versions: serde_json::Map<String, Value> =
        serde_json::from_slice(&fs::read("tools/versions.json")?)?;
    for (name, value) in versions {
        let version = value.as_str().ok_or("invalid quality-tool version")?;
        let expected = format!("{name} {version}");
        if capture(tool(&name), &["--version"])
            .ok()
            .as_deref()
            .is_some_and(|v| v.starts_with(&expected))
        {
            continue;
        }
        run(
            "cargo",
            &[
                "install",
                "--locked",
                "--root",
                ".tools",
                "--version",
                version,
                &name,
            ],
        )?;
    }
    run("cargo", &["fetch", "--locked"])
}

fn product_version() -> Result<String> {
    let cargo: toml::Value = toml::from_str(&fs::read_to_string("Cargo.toml")?)?;
    Ok(cargo["package"]["version"]
        .as_str()
        .ok_or("product version is missing")?
        .into())
}

fn check_version() -> Result {
    let manifest: Value =
        serde_json::from_slice(&fs::read("packages/desktop/extension/manifest.json")?)?;
    if manifest["version"].as_str() != Some(product_version()?.as_str()) {
        return Err("extension version drift; run just generate on a build host".into());
    }
    Ok(())
}

fn copy_tree(source: &Path, output: &Path) -> Result {
    fs::create_dir(output)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        if name == "test" || name == "src" || name == "tsconfig.json" {
            continue;
        }
        let target = output.join(name);
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err("a source artifact is not a regular file or directory".into());
        }
    }
    Ok(())
}

fn stage(output: &Path, profile: &str) -> Result {
    if !output.is_absolute() || !matches!(profile, "debug" | "release") {
        return Err("stage requires an absolute new directory and debug or release profile".into());
    }
    check_version()?;
    let binary = target_directory()?
        .join(profile)
        .join(executable("nddev-browseruse"));
    if !binary.is_file() {
        return Err("build the requested native profile before staging".into());
    }
    // Never remove or overwrite a caller's existing directory.
    fs::create_dir(output)?;
    fs::create_dir(output.join("bin"))?;
    fs::copy(
        binary,
        output.join("bin").join(executable("nddev-browseruse")),
    )?;
    copy_tree(Path::new("packages/executor"), &output.join("executor"))?;
    copy_tree(
        Path::new("packages/desktop/extension"),
        &output.join("extension"),
    )?;
    for file in ["package.json", "bun.lock", "LICENSE", "NOTICE"] {
        fs::copy(file, output.join(file))?;
    }
    for file in ["LICENSE", "NOTICE"] {
        fs::copy(file, output.join("extension").join(file))?;
    }
    let mut install = Command::new(std::env::current_dir()?.join(tool("bun")));
    install.current_dir(output).args([
        "install",
        "--frozen-lockfile",
        "--production",
        "--ignore-scripts",
        "--linker",
        "hoisted",
    ]);
    complete(install)?;
    fs::write(
        output.join("CANDIDATE.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "product": "nddev-browseruse", "version": product_version()?, "source": source_commit()?,
            "profile": profile, "qualified": false, "nativeHostRegistered": false
        }))?,
    )?;
    fs::write(
        output.join("INSTALL.txt"),
        "Candidate only. Loading the unpacked extension is the owner's action.\nNative-host registration is separate: run bin/nddev-browseruse install with the absolute installed Bun and executor/executor.ts paths and the chosen --browser.\nStaging does not change a browser profile, settings or native-host registration.\n",
    )?;
    println!("{}", output.display());
    Ok(())
}

fn generate() -> Result {
    let path = Path::new("packages/desktop/extension/manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(path)?)?;
    manifest["version"] = product_version()?.into();
    let mut encoded = serde_json::to_vec_pretty(&manifest)?;
    encoded.push(b'\n');
    fs::write(path, encoded)?;
    run(tool("bun"), &["run", "generate"])
}

fn check_generated() -> Result {
    let generated = Path::new(".tools/generated-browser");
    if generated.exists() {
        fs::remove_dir_all(generated)?;
    }
    run(
        tool("bun"),
        &[
            "run",
            "tsc",
            "-p",
            "tsconfig.extension.json",
            "--outDir",
            ".tools/generated-browser",
        ],
    )?;
    fn compare(source: &Path, checked: &Path) -> Result {
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let target = checked.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                compare(&entry.path(), &target)?;
            } else if fs::read(entry.path())? != fs::read(&target)? {
                return Err(format!("generated browser code drift: {}", target.display()).into());
            }
        }
        Ok(())
    }
    compare(generated, Path::new("packages/desktop/extension"))
}

fn source_commit() -> Result<String> {
    let Ok(status) = capture(
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
    ) else {
        return Ok("unknown".into());
    };
    if status.is_empty() {
        capture("git", &["rev-parse", "HEAD"])
    } else {
        Ok("unknown".into())
    }
}

fn target_directory() -> Result<PathBuf> {
    let metadata: Value = serde_json::from_str(&capture(
        "cargo",
        &["metadata", "--format-version", "1", "--no-deps"],
    )?)?;
    Ok(PathBuf::from(
        metadata["target_directory"]
            .as_str()
            .ok_or("Cargo target directory is missing")?,
    ))
}

fn test_browser() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("NDS_TEST_CHROME") {
        let path = PathBuf::from(path);
        if !path.is_absolute() || !path.is_file() {
            return Err("NDS_TEST_CHROME must name an absolute executable path".into());
        }
        return Ok(path);
    }
    let lock: Value = serde_json::from_slice(&fs::read("provider.lock.json")?)?;
    let provider = if cfg!(target_os = "macos") {
        &lock["mac_arm64"]
    } else if cfg!(windows) {
        &lock["win64"]
    } else {
        &lock
    };
    if cfg!(target_os = "macos") && !cfg!(target_arch = "aarch64") {
        return Err("the pinned macOS browser requires arm64".into());
    }
    let directory = std::env::current_dir()?
        .join(".tools/providers")
        .join(lock["version"].as_str().ok_or("missing browser version")?);
    let executable = directory.join(
        provider["executables"]["NDS_TEST_CHROME"]
            .as_str()
            .ok_or("missing browser executable")?,
    );
    if !executable.is_file() {
        fs::create_dir_all(&directory)?;
        let archive = directory.join("chrome.zip");
        let url = provider["asset"]
            .as_str()
            .or_else(|| provider["linux_asset"].as_str())
            .ok_or("missing browser asset")?;
        if !url.starts_with("https://storage.googleapis.com/chrome-for-testing-public/") {
            return Err("unexpected browser asset origin".into());
        }
        run(
            "curl",
            &[
                "--fail",
                "--location",
                "--max-time",
                "300",
                "--output",
                archive.to_str().ok_or("invalid archive path")?,
                url,
            ],
        )?;
        let mut input = fs::File::open(&archive)?;
        if provider["size"].as_u64() != Some(input.metadata()?.len()) {
            fs::remove_file(&archive)?;
            return Err("browser archive size mismatch".into());
        }
        let mut digest = Sha256::new();
        let mut buffer = [0; 65_536];
        loop {
            let size = input.read(&mut buffer)?;
            if size == 0 {
                break;
            }
            digest.update(&buffer[..size]);
        }
        let hash: String = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        drop(input);
        if provider["sha256"].as_str() != Some(hash.as_str()) {
            fs::remove_file(&archive)?;
            return Err("browser archive size or checksum mismatch".into());
        }
        if cfg!(windows) {
            run(
                "7z",
                &[
                    "x",
                    "-bd",
                    "-y",
                    &format!("-o{}", directory.display()),
                    archive.to_str().ok_or("invalid archive path")?,
                ],
            )?;
        } else if cfg!(target_os = "macos") {
            run(
                "ditto",
                &[
                    "-x",
                    "-k",
                    archive.to_str().ok_or("invalid archive path")?,
                    directory.to_str().ok_or("invalid browser path")?,
                ],
            )?;
        } else {
            run(
                "unzip",
                &[
                    "-q",
                    "-o",
                    archive.to_str().ok_or("invalid archive path")?,
                    "-d",
                    directory.to_str().ok_or("invalid browser path")?,
                ],
            )?;
        }
    }
    if cfg!(target_os = "linux") && std::env::var_os("CHROME_DEVEL_SANDBOX").is_none() {
        let sandbox = directory.join(
            provider["executables"]["CHROME_DEVEL_SANDBOX"]
                .as_str()
                .ok_or("missing sandbox executable")?,
        );
        let sandbox = sandbox.to_str().ok_or("invalid sandbox path")?;
        // Only this isolated downloaded helper is changed. A failure never disables Chromium's sandbox.
        run("sudo", &["-n", "chown", "root:root", sandbox])?;
        run("sudo", &["-n", "chmod", "4755", sandbox])?;
    }
    Ok(executable)
}
