# nddev-browseruse

Local browser automation for agents you run. A Rust host and CLI connect a thin
Bun executor to a Chromium extension through native messaging and local sockets.
Each task works inside its own tab group in your existing browser.
Its owner control tab shows session state and local analytics, and lets you close the group or
keep its pages. Abandoned idle sessions are reclaimed automatically.

The standalone product preserves the public history of
[NDS browser-use](https://github.com/NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-browser-use).
Existing native host, extension and local data identities remain compatible.
Download the native packet and extension from [Releases](https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse/releases),
then follow [installation](docs/desktop.md#install). Chrome is qualified; the Edge
packet awaits its assigned catalog ID and actual Edge acceptance.

```sh
nddev-browseruse skill
nddev-browseruse status
nddev-browseruse session new docs
nddev-browseruse -s docs -e 'state.page = await context.newPage(); await state.page.goto("https://example.com"); return await snapshot({ page: state.page })'
nddev-browseruse session close docs
```

Read the full guide printed by `skill` before controlling a browser. The owner
installs the local host, loads the extension and enables access. Agents control
their own groups; disabling access drains their work. Interrupted actions can
have an uncertain result and must be observed before retrying.

Observation is local and configurable: Full keeps a redacted activity journal,
Standard keeps operational metadata, and Off disables collection. Agents may
send what they read to their own model provider. Review the
[privacy policy](docs/privacy.md) and [observation model](docs/observability.md).

The standalone library initializes no telemetry subscriber and depends on no
NDS runtime. Embedding systems use the independent client SDK and
[public port](docs/integration.md); [architecture](docs/architecture.md) describes the boundaries.
Android support connects through the owner's ADB server; it does not install a
mobile browser extension. Credential custody and cloud relay are not implemented.

Build and acceptance instructions are in [desktop.md](docs/desktop.md).
The project uses Apache-2.0 for its original code and retains MIT adaptations;
see [LICENSE](LICENSE), [NOTICE](NOTICE) and [provenance](docs/provenance.md).
