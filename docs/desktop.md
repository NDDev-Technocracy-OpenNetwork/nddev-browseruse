# Desktop browsers

On a computer, agents work in the owner's own Chrome, Chrome Beta, Chrome for Testing or Chromium,
inside the tab groups that the owner grants them. Phones are described in [mobile.md](mobile.md).

## Layout

| Path | What it is |
|---|---|
| `packages/desktop/extension` | The MV3 extension, in strict TypeScript, built to the checked-in `*.js` files |
| `src/host` | The Rust native host that Chrome starts for the extension (native messaging on stdin and stdout) |
| `packages/executor` | The Bun executor that runs agent code, shared with phones |
| `src/mobile` | The phone link, which takes the extension's place on phones |

## How it works

- **The extension** attaches `chrome.debugger` only to tabs in agent tab groups. It relays their CDP
  traffic to the host through native messaging, and answers the host's `claim`, `open`, `close`,
  `release`, `cdp`, `tabs` and `forget` requests.
- **The host** serves agents on its private control socket and runs each session's code in an
  executor, connected to a session-scoped CDP endpoint.
- **The owner's controls** are in the extension's popup:
  - the switch, which turns agent control on or off;
  - the observation profile, Full, Standard or Off ([observability.md](observability.md));
  - the prominent disclosure on first use or an upgrade that needs renewed acceptance.
- **Owner actions.** The owner hands a tab to an agent by moving it into the agent's group, and
  pins an element with the context menu.

Every agent group contains an owner service tab. It shows the group's state, uptime, site-tab
count, inactivity countdown and aggregate execution counts/durations. Off discards and hides
the analytics. The service tab is excluded from agent browser targets. Its controls end the
group either by closing its pages or by keeping them as ordinary ungrouped owner tabs.

Explicit session close removes the service tab and the group's pages. `--keep-tabs` retains and
ungroups the pages. An abandoned session expires after five minutes without work; a running
execution or active recording suspends that lease. Merely looking at analytics does not renew it.
Losing the native host also ends its groups. Closing, moving or navigating the service tab ends
its old group while preserving a tab the owner took back.

Recovery uses a bounded local registry together with a unique service tab, including after
browser group IDs change. Conflicting or duplicated markers preserve the browser area and refuse
replacement until ownership is unambiguous. Groups left by older builds without a marker cannot
be identified safely after extension reload/update or a whole-browser restart clears their old
session-storage mapping; a title alone never authorizes deletion.

Retirement ungroups its verified service tab before deleting it, so the last agent group of a
window does not remain as a saved group. Chrome's independent saving/sync behavior also depends
on browser/account settings. Externally closing the whole window can retain a saved record;
that recovery boundary is tracked in [#27](https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse/issues/27).
For an explicitly owner-selected old group, Chrome's Ungroup keeps its local pages and removes
the group across the account's devices. Closing a group alone retains its saved record.

Video recording holds a separate, bounded native reservation. Only a live encoder renews it;
stopping the recording, losing its executor or missing renewals removes that protection.
Renewals do not count as agent work or reset the idle countdown. The host also expires a person
recording's capture at its configured maximum duration even if no more frames arrive.

## Browsers

`install --browser` registers the host for one browser's user profile root:

| Name | Browser |
|---|---|
| `chrome`, `chrome-beta`, `chrome-dev`, `chrome-canary` | Google Chrome and its channels |
| `chrome-for-testing`, `chromium` | Unbranded builds |
| `edge`, `edge-beta`, `edge-dev` | Microsoft Edge and its channels |
| `brave` | Brave |

Other Chromium browsers can use `--host-dir <their NativeMessagingHosts folder>`.
Registration support does not qualify a browser: it must expose compatible debugger,
tab-group and native-messaging APIs. Firefox has no implementation in this product.

## Build and validation

On a build host, run `just setup`, then `just verify`. Setup builds the Rust
project helper and installs pinned dependencies. It first reuses a matching Bun
from the project or PATH; npm is only a native-runtime bootstrap when neither is
available. Both JavaScript dependency graphs are installed and run with Bun.
Check is read-only and offline;
its prepared helper and dependencies can be copied to USA from the build host.
Tests, builds and isolated browser acceptance run on build hosts only.

The browser check uses `NDS_TEST_CHROME` when explicitly supplied, or downloads
the pinned Chrome for Testing archive after checking its size and SHA-256.
Chromium's sandbox remains enabled. The isolated Linux SUID sandbox helper
requires permission to set its owner and mode; failure stops the check.
No owner profile or native-host registration is used for acceptance.
The isolated browser uses Chromium's file-backed local sync backend to check actual saved group
GUIDs, with positive controls before retirement. It uses no account or remote sync service.

`just verify-staged /absolute/new/native-packet` runs the checks, builds the release
host, stages its executor/dependencies/extension and runs acceptance against those
exact files. Full dev macOS/Windows CI uploads that tested directory. A new output
directory is required; an existing packet is never overwritten.

The shared full Linux check leaves `dist/qualified-linux.zip`: its exact staged native
packet, measured browser report, extension ZIPs, authentic captures and rendered store
material. The archive contains hidden runtime files and retains executable modes.
Other supported platforms still run native acceptance through `just verify`;
the Linux CI result supplies the common store packet.
Pull requests run `just check` and `just test` on each native target; the full
dev run qualifies its staged browser/native packet once before promotion.

After promotion to `main`, `release.yml` selects successful check, native and security
runs from the same `dev` push commit. It assembles their packets without rebuilding the
application, records sources, artifact identities, measured skips and hashes in
`QUALIFICATION.json`, then verifies every uploaded asset by downloading it before
publishing. Repeated publication reconciles the same tag and bytes; conflicting tags
or published assets are refused. Failure leaves an issue with the failed run.
This publication does not replace a running native host or submit a store listing.

## Install

Keep the downloaded native packet in a stable directory. Use the Bun version pinned
by `packageManager` in its `package.json`, installed separately for the same platform.
The packet does not install Bun globally. Video additionally needs ffmpeg on PATH.

Check the build, then register the host for the browser you use:

```sh
/absolute/native-packet/bin/nddev-browseruse --version
/absolute/native-packet/bin/nddev-browseruse install --browser chrome --bun /absolute/bun --executor /absolute/native-packet/executor/executor.ts
```

Use `chrome-beta` for Chrome Beta. Windows uses `bin/nddev-browseruse.exe`.
Compare the source/version with the release's qualification evidence. GitHub artifact
ZIPs normalize Unix executable permissions; after extracting a macOS/Linux packet,
restore `chmod +x bin/nddev-browseruse` if the executable bit is absent.

Load the matching unpacked extension or install its store build, open the popup and
choose Full, Standard or Off. An update that requires renewed disclosure stays paused
until that choice. `Update needed` means the native host and extension need a compatible
pair; replacing only the extension does not replace the host. Close existing browser-use
sessions before changing the host, then reconnect it without restarting the browser.
Never stop another task's native host while its sessions are active.

Run the CLI by its absolute path, or put its `bin` directory on PATH. `status` must
report `enabled: true` before a task starts. The public skill is included under
`skills/nddev-browseruse`; registration does not change global skills.

The native host can register one browser through `nddev-browseruse install`.
Loading the unpacked candidate and uploading a store ZIP are the owner's steps.
The Chrome Web Store build uses the same source with the development key omitted.
Edge's assigned store ID must also be admitted and tested before its release.

## Versions

- `nddev-browseruse status` names the host's build (`host.version` and the source commit
  `host.commit`) beside the connected extension's version.
- The host and the extension speak one contract. When they do not match, for example a store
  extension older than the host, the host refuses it, `status` reports `compatible: false`, and
  the extension shows `!` on its icon and explains in its popup which side to update.

## Native configuration and migration

The installer delivers `common.toml` and `profile.toml` under the existing `nds-browser-use`
configuration namespace. Valid operator edits survive reinstallation. An invalid file stops
installation/startup; it is never silently replaced with defaults. `config` validates a complete
composition before writing and supports `--runtime-dir`, `--default-device`, `--default-phone`
and `--adb-server`. Use an empty string to clear runtime/device/phone overrides; ADB needs a
valid endpoint. This command edits common-source fields; manually edit the profile if it owns
the native values instead. OS `HOME`/XDG/LOCALAPPDATA locations
remain platform conventions; application values no longer come from environment variables.

Reinstallation adds a newly introduced execution-budget section only when neither installed
source declares it. Defaults come from the compiled common configuration; all prior operator
values stay. An explicitly malformed or incomplete section is rejected instead of replaced.

Migrate `NDS_BROWSER_USE_RUNTIME_DIR`, `NDS_BROWSER_DEVICE`, `NDS_BROWSER_PHONE` to these fields
and clear the old variables. A nonempty old setting refuses an operation with
`invalid_configuration`. The config and local `--version` commands remain available for repair.
Phone operations also reject legacy ADB endpoint variables. Runtime/ADB changes require finishing
existing sessions and restarting the host; CLI defaults apply to the next CLI process. Explicit
`--device`/`--phone` wins over a default, and `--local` bypasses both defaults. Forwarding needs a
peer CLI at least 0.2.5 and cannot inherit that peer's routing defaults.

The host sends only validated executor budgets to its own executor before observation starts;
operator changes apply to new executor connections. A missing/invalid budget handshake stops
Full collection. Profiles and collection-generation tokens are read atomically, and late replies
cannot restore an older collection generation after Off or a private-site change.

After Edge assigns its listing ID, run the same installer with
`--browser edge --extension-id <assigned ID>`, plus the candidate's Bun/executor paths.
It adds an exact explicit native admission while preserving existing CWS/unpacked IDs. No Edge
listing ID is needed to prepare the package. Loading it and checking the installed Edge build
remain owner steps after the card exists.


## Public UI and contracts

The popup's Licenses link opens a local screen in the selected language, with the
original Apache/MIT, bundled npm, design-system and font notices. It works without
a native-host connection and follows the system theme. Native-host/executor notices
are included in their separate installation packet.

The popup uses the canonical Apache OpenNetwork DS release pinned in
`design-system.lock.json`. The vendored web package is copied unchanged and
checked by digest. Product font faces come from that release's verified font
manifest. Chromium badge colors are projected from the same tokens, rather than
maintained as a second palette. All fonts are local; the extension loads no UI
code, fonts or message catalogs from a CDN.

The UI has English, Russian, Kazakh, Arabic and Simplified Chinese catalogs.
Arabic uses RTL; dynamic messages use i18next with ICU. Chromium's Chinese locale
directory is `zh_CN`, while the UI language identifier is `zh-CN`.

`crates/contract` declares public settings and owner-facing browser types. Utoipa
produces `contracts/openapi.json`; Hey API produces the TypeScript types. The
native TS 7 typechecker stays separate from the TS 6 compiler API package needed
by the schema producer. `config/common.toml`, `dev.toml` and `production.toml`
are validated by the same typed loader, which rejects duplicate and unknown
values. `just generate` produces the shipped public projection. Check reads
only; test rebuilds generated files in an isolated output directory to catch drift.

The build-only schema generator uses the unchanged library files of js-yaml 4.3.2.
Its unused CLI/argparse dependency is omitted from a private local library distribution.
The original MIT notice, upstream npm integrity and per-file digests are retained under
`tools/codegen/vendor/js-yaml`; check validates them and test proves identical generated
contracts. This build-only directory is excluded from native and store packets.

## Preparing a distribution without registering a browser

On the Ubuntu PC/build runner, use a clean Git checkout:

```sh
just setup
just verify
just build
just stage /absolute/new/native-candidate
just package-store /absolute/new/store-packet
```

`stage` checks that the release binary reports this exact source commit through
`--version`. It copies the executor, production dependencies, unpacked extension,
bundled agent skill and original Rust/npm/DS/font license notices. Bun 1.4.4 is an
external prerequisite. Neither command changes browser registration or global
skills. Finish existing sessions before the owner applies an update.

Store packaging runs on Linux with the system `zip` and `unzip` tools. It sorts
entries, fixes timestamps/modes and omits ZIP extra fields. Chrome and Edge ZIPs
have identical engine bytes and omit the unpacked public key; `unpacked/` retains
that key and its existing ID. SHA256SUMS and source/DS metadata accompany them.
A partial directory retains INCOMPLETE.txt. Build metadata records source and
destinations; qualification is recorded separately in the release receipt after
the acceptance and visual checks. Edge's future assigned extension ID needs explicit native
admission and browser acceptance before its listing can become operational.

`just store-materials /absolute/isolated-captures /absolute/new/materials` prepares
five localized listings, permission/privacy/reviewer field copy, 1280×800 actual-popup
compositions, 440×280 promos, optional 1400×560 marquees and 128/300 px logos. Captures
come from `NDS_E2E_SHOTS` on an isolated browser acceptance run. Rust owns the input
bounds, copy, provenance and output hashes; a thin sandboxed browser renderer uses the
canonical DS/font package and does not load network resources. Inspect the PNGs before
marking the packet reviewed. Store acceptance remains each store's decision.
