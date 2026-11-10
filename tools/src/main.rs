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

mod architecture;
mod configuration;
mod design;
mod legal;
mod licenses;
mod native_licenses;
mod release;
mod store;
mod store_materials;

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
            architecture::check()?;
            design::check()?;
            configuration::check()?;
            legal::check()?;
            release::check()?;
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
                &[
                    "run",
                    "biome",
                    "check",
                    "--write",
                    "packages",
                    "e2e",
                    "vitest.config.ts",
                    "vitest.e2e.config.ts",
                ],
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
            run("cargo", &["nextest", "run", "--locked", "--workspace"])?;
            run("cargo", &["test", "--locked", "--workspace", "--doc"])?;
            run(tool("bun"), &["run", "test"])
        }
        "generate" => generate(),
        "vendor-design" => design::vendor(),
        "contracts" => configuration::generate(),
        "package-store" => {
            let output = std::env::args()
                .nth(2)
                .ok_or("package-store requires an absolute new output directory")?;
            store::package(Path::new(&output))
        }
        "store-materials" => {
            let captures = std::env::args()
                .nth(2)
                .ok_or("store-materials requires captures")?;
            let output = std::env::args()
                .nth(3)
                .ok_or("store-materials requires a new output directory")?;
            store_materials::prepare(Path::new(&captures), Path::new(&output))
        }
        "stage" => {
            let output = std::env::args()
                .nth(2)
                .ok_or("stage requires an absolute output directory")?;
            let profile = std::env::args().nth(3).unwrap_or_else(|| "release".into());
            stage(Path::new(&output), &profile)
        }
        "verify-staged" => {
            let output = std::env::args()
                .nth(2)
                .ok_or("verify-staged requires an absolute new native directory")?;
            let output = Path::new(&output);
            stage(output, "release")?;
            release::accept(output, None).map(|_| ())
        }
        "verify-release" => release::prepare(),
        "release-requirements" => release::write_requirements(),
        "release-assemble" => {
            let input = std::env::args()
                .nth(2)
                .ok_or("release-assemble requires input")?;
            let output = std::env::args()
                .nth(3)
                .ok_or("release-assemble requires output")?;
            release::assemble(Path::new(&input), Path::new(&output))
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
        "browser-check" | "phone-check" => {
            generate()?;
            run(
                "cargo",
                &["build", "--locked", "--package", "nddev-browseruse"],
            )?;
            let target = target_directory()?;
            let mut browser = Command::new(tool("bun"));
            if command == "phone-check" {
                browser.args(["run", "e2e", "e2e/phone.test.ts"]);
            } else {
                browser.args(["run", "e2e"]);
            }
            if let Some(pattern) = std::env::args().nth(2) {
                browser.args(["--testNamePattern", &pattern]);
            }
            browser_environment(&mut browser)?;
            browser.env(
                "NDS_TEST_HOST",
                target.join("debug").join(executable("nddev-browseruse")),
            );
            complete(browser)
        }
        _ => Err("unknown project command".into()),
    }
}

fn browser_environment(browser: &mut Command) -> Result {
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
    Ok(())
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

/// The video cases need ffmpeg, and GitHub's Linux image carries none. On that runner alone,
/// setup installs the build of the image's own distribution, so the Linux packet qualification
/// executes every browser case. Every other host keeps the ffmpeg its owner provides.
fn hosted_ffmpeg() -> Result {
    if std::env::consts::OS != "linux"
        || std::env::var("RUNNER_ENVIRONMENT").as_deref() != Ok("github-hosted")
        || capture("ffmpeg", &["-version"]).is_ok()
    {
        return Ok(());
    }
    run("sudo", &["apt-get", "update", "--quiet"])?;
    run(
        "sudo",
        &[
            "apt-get",
            "install",
            "--yes",
            "--no-install-recommends",
            "ffmpeg",
        ],
    )?;
    println!(
        "ffmpeg for the video cases: {}",
        capture("dpkg-query", &["--show", "ffmpeg"])?
    );
    Ok(())
}

fn setup() -> Result {
    hosted_ffmpeg()?;
    let root = std::env::current_dir()?;
    fs::create_dir_all(".tools/bin")?;
    let package: Value = serde_json::from_slice(&fs::read("package.json")?)?;
    let version = package["packageManager"]
        .as_str()
        .and_then(|p| p.strip_prefix("bun@"))
        .ok_or("packageManager must pin Bun")?;
    if capture(tool("bun"), &["--version"]).ok().as_deref() != Some(version) {
        if let Some(installed) = std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .map(|directory| root.join(directory).join(executable("bun")))
                .find(|candidate| {
                    capture(candidate, &["--version"]).ok().as_deref() == Some(version)
                })
        }) {
            install_bun(&installed)?;
        } else {
            bootstrap_bun(version)?;
        }
        if capture(tool("bun"), &["--version"])? != version {
            return Err("Bun setup version mismatch".into());
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
        let label = if name == "cargo-shear" {
            "Version:"
        } else {
            &name
        };
        if capture(tool(&name), &["--version"])
            .ok()
            .as_deref()
            .is_some_and(|value| {
                let mut fields = value.split_whitespace();
                fields.next() == Some(label) && fields.next() == Some(version)
            })
        {
            continue;
        }
        let installed = tool(&name);
        if fs::symlink_metadata(&installed).is_ok() {
            fs::remove_file(installed)?;
        }
        run(
            "cargo",
            &[
                "install",
                "--locked",
                "--root",
                ".tools",
                "--force",
                "--version",
                version,
                &name,
            ],
        )?;
    }
    run(
        tool("bun"),
        &[
            "install",
            "--cwd",
            "tools/codegen",
            "--frozen-lockfile",
            "--ignore-scripts",
        ],
    )?;
    run("cargo", &["fetch", "--locked"])
}

fn install_bun(source: &Path) -> Result {
    let destination = tool("bun");
    // Never follow an old project symlink and overwrite the owner's installed runtime.
    if fs::symlink_metadata(&destination).is_ok() {
        fs::remove_file(&destination)?;
    }
    fs::copy(source, destination)?;
    Ok(())
}

fn bootstrap_bun(version: &str) -> Result {
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
    install_bun(
        Path::new(".tools/bun/node_modules")
            .join(name)
            .join("bin")
            .join(executable("bun"))
            .as_path(),
    )?;
    Ok(())
}

fn product_version() -> Result<String> {
    let cargo: toml::Value = toml::from_str(&fs::read_to_string("Cargo.toml")?)?;
    Ok(cargo["workspace"]["package"]["version"]
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
        if matches!(
            name.to_str(),
            Some(
                "test"
                    | "src"
                    | "tsconfig.json"
                    | "node_modules"
                    | ".tools"
                    | ".git"
                    | ".nddev-reports"
            )
        ) {
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
    let compiled: nddev_browseruse_contract::Build =
        serde_json::from_str(&capture(&binary, &["--version"])?)?;
    let package: Value = serde_json::from_slice(&fs::read("package.json")?)?;
    let runtime = package["packageManager"]
        .as_str()
        .ok_or("missing pinned executor runtime")?;
    let source = source_commit()?;
    if compiled.version != product_version()?
        || (profile == "release" && (source.len() != 40 || compiled.commit != source))
    {
        return Err(
            "native binary does not match this clean source commit; rebuild before release staging"
                .into(),
        );
    }
    // Never remove or overwrite a caller's existing directory.
    fs::create_dir(output)?;
    fs::write(
        output.join("INCOMPLETE.txt"),
        "Native staging is incomplete. Do not install this directory.\n",
    )?;
    fs::create_dir(output.join("bin"))?;
    fs::copy(
        binary,
        output.join("bin").join(executable("nddev-browseruse")),
    )?;
    copy_tree(Path::new("packages/executor"), &output.join("executor"))?;
    copy_tree(Path::new("packages/skill"), &output.join("skills"))?;
    copy_tree(
        Path::new("packages/desktop/extension"),
        &output.join("extension"),
    )?;
    for file in [
        "package.json",
        "bun.lock",
        "LICENSE",
        "NOTICE",
        "Cargo.lock",
        "design-system.lock.json",
    ] {
        fs::copy(file, output.join(file))?;
    }
    for file in ["LICENSE", "NOTICE"] {
        fs::copy(file, output.join("extension").join(file))?;
    }
    native_licenses::generate(output)?;
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
            "product": "nddev-browseruse", "version": compiled.version, "source": compiled.commit,
            "executorRuntime": runtime,
            "platform": {"os":std::env::consts::OS,"arch":std::env::consts::ARCH},
            "profile": profile, "qualification": "recorded in the release receipt", "nativeHostRegistered": false
        }))?,
    )?;
    fs::write(
        output.join("INSTALL.txt"),
        format!(
            "Build packet. Compare CANDIDATE.json and bin/nddev-browseruse --version with the release qualification receipt. Complete existing agent sessions before changing the installation.\nLoading extension/ as an unpacked extension is the owner's action. Native-host registration is separate: run bin/nddev-browseruse install with the absolute installed {runtime} path, this directory's executor/executor.ts path and the chosen --browser.\nBun is an external prerequisite; its binary is not redistributed here. The bundled agent skill is under skills/nddev-browseruse; it does not replace global skills automatically.\nThe source NOTICE refers to packages/executor and packages/desktop/extension; this distribution maps them to executor and extension. Rust license texts are in licenses/rust and licenses/rust-toolchain; npm and DS/font notices accompany their packages.\nStaging does not change a browser profile, settings or native-host registration.\n"
        ),
    )?;
    fs::remove_file(output.join("INCOMPLETE.txt"))?;
    println!("{}", output.display());
    Ok(())
}

fn generate() -> Result {
    design::generate()?;
    configuration::generate()?;
    let path = Path::new("packages/desktop/extension/manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(path)?)?;
    manifest["version"] = product_version()?.into();
    let mut encoded = serde_json::to_vec_pretty(&manifest)?;
    encoded.push(b'\n');
    fs::write(path, encoded)?;
    emit_browser("packages/desktop/extension")?;
    legal::generate()
}

fn emit_browser(output: &str) -> Result {
    if Path::new(output).is_dir() {
        for entry in fs::read_dir(output)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if entry.file_type()?.is_file() && name.starts_with("chunk-") && name.ends_with(".js") {
                fs::remove_file(entry.path())?;
            }
        }
    }
    // Only bundled browser entry points ship. Remove outputs of the former per-module emitter.
    for stale in ["groups.js", "ui.js", "restore.js", "generated/types.gen.js"] {
        let path = Path::new(output).join(stale);
        if path.is_file() {
            fs::remove_file(path)?;
        }
    }
    let metadata = Path::new(".tools/bundle-metadata.json");
    run(
        tool("bun"),
        &["run", "tsc", "-p", "tsconfig.extension.json", "--noEmit"],
    )?;
    run(
        tool("bun"),
        &[
            "build",
            "packages/desktop/extension/src/background.ts",
            "packages/desktop/extension/src/popup.ts",
            "packages/desktop/extension/src/group.ts",
            "packages/desktop/extension/src/legal.ts",
            "--target",
            "browser",
            "--minify",
            "--splitting",
            "--chunk-naming",
            "chunk-[hash].[ext]",
            "--define",
            "process.env.NODE_ENV=\"production\"",
            "--outdir",
            output,
            "--metafile=.tools/bundle-metadata.json",
        ],
    )?;
    licenses::generate(Path::new(output), metadata)
}

fn check_generated() -> Result {
    configuration::check_generated()?;
    let generated = Path::new(".tools/generated-browser");
    if generated.exists() {
        fs::remove_dir_all(generated)?;
    }
    emit_browser(".tools/generated-browser")?;
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
    let checked = Path::new("packages/desktop/extension");
    compare(generated, checked)?;
    fn scripts(path: &Path) -> Result<Vec<std::ffi::OsString>> {
        let mut names = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "js")
            {
                names.push(entry.file_name());
            }
        }
        names.sort();
        Ok(names)
    }
    if scripts(generated)? != scripts(checked)? {
        return Err("generated browser directory contains missing or obsolete scripts".into());
    }
    Ok(())
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
