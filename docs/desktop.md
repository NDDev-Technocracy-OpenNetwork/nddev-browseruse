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

## Browsers

`install --browser` registers the host for one browser's user profile root:

| Name | Browser |
|---|---|
| `chrome`, `chrome-beta`, `chrome-dev`, `chrome-canary` | Google Chrome and its channels |
| `chrome-for-testing`, `chromium` | Unbranded builds |
| `edge`, `edge-beta`, `edge-dev` | Microsoft Edge and its channels |
| `brave` | Brave |

Other Chromium browsers can use `--host-dir <their NativeMessagingHosts folder>`, but two are
known not to work:
- **Vivaldi:** its stable version gives extensions no tab groups. The agent groups depend on
  them.
- **Opera:** it turns tab groups into its own tab islands.

## Candidate validation

The standalone install and store packets are being qualified. Do not replace a
working owner-browser installation with the extraction worktree.

On a build host, run `just setup`, then `just verify`. Setup builds the Rust
project helper and installs pinned dependencies. Check is read-only and offline;
its prepared helper and dependencies can be copied to USA from the build host.
Tests, builds and isolated browser acceptance run on build hosts only.

The browser check uses `NDS_TEST_CHROME` when explicitly supplied, or downloads
the pinned Chrome for Testing archive after checking its size and SHA-256.
Chromium's sandbox remains enabled. The isolated Linux SUID sandbox helper
requires permission to set its owner and mode; failure stops the check.
No owner profile or native-host registration is used for acceptance.

The native host can register one browser through `nddev-browseruse install`.
The candidate staging workflow will supply its absolute Bun and executor paths.
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
A partial directory retains INCOMPLETE.txt. A completed packet still has
`qualified: false` until its acceptance evidence, store copy and owner-browser
checks are complete. Edge's future assigned extension ID needs explicit native
admission and browser acceptance before its listing can become operational.

`just store-materials /absolute/isolated-captures /absolute/new/materials` prepares
five localized listings, permission/privacy/reviewer field copy, 1280×800 actual-popup
compositions, 440×280 promos, optional 1400×560 marquees and 128/300 px logos. Captures
come from `NDS_E2E_SHOTS` on an isolated browser acceptance run. Rust owns the input
bounds, copy, provenance and output hashes; a thin sandboxed browser renderer uses the
canonical DS/font package and does not load network resources. Inspect the PNGs before
marking the packet reviewed. Store acceptance remains each store's decision.
