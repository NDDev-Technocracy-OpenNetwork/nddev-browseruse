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
  - the observation profile, Full, Standard or Off ([observability.md](observability.md)).
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

