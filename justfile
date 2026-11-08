set shell := ["bash", "-euo", "pipefail", "-c"]

tools := ".tools/bin/nddev-browseruse-tools"

# Lists the project commands.
default:
    @just --list

# Installs pinned tooling and dependencies; run on a build host.
setup:
    cargo install --locked --path tools --root .tools --force
    {{ tools }} setup

# Runs every check that needs no build, offline and without writing files.
check:
    just --fmt --check
    {{ tools }} check

# Applies the project's automatic formatting and lint fixes.
fix:
    just --fmt --unstable
    {{ tools }} fix

# Builds, lints with clippy and runs native and TypeScript tests.
test:
    {{ tools }} test

# Runs the complete validation, including isolated real-browser acceptance.
verify: check test build
    {{ tools }} verify-release

# Tests the exact staged release host, executor and extension on a build host.
verify-staged output: check test build
    {{ tools }} verify-staged {{ quote(output) }}

# Builds the release host and generated browser code.
build:
    {{ tools }} build

# Generates versioned extension metadata and compiled browser scripts.
[group('browser')]
generate:
    {{ tools }} generate

# Runs acceptance with NDS_TEST_CHROME in an isolated browser profile.
[group('browser')]
browser-check:
    {{ tools }} browser-check

# Runs the same isolated Android/ADB acceptance fixtures on a build host.
[group('browser')]
phone-check:
    {{ tools }} phone-check

# Stages a built host and local executor dependencies without registering a browser.
[group('browser')]
stage output:
    {{ tools }} stage {{ quote(output) }}

# Creates unpacked and Chrome/Edge ZIP packets from a clean source commit, without publishing.
[group('browser')]
package-store output:
    {{ tools }} package-store {{ quote(output) }}

# Renders store assets from authentic isolated popup captures, without registering a browser.
[group('browser')]
store-materials captures output:
    {{ tools }} store-materials {{ quote(captures) }} {{ quote(output) }}
