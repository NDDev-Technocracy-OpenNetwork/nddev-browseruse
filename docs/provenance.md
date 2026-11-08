# Code provenance

This repository preserves the public history of
[NDDev browser-use](https://github.com/NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-browser-use)
through `94346d42a7d9cbc30894b3313cd8359f2c1837d1`. Original NDDev code is
published here under Apache-2.0 by the owner's decision. Earlier revisions
retain the license under which they were published.

NOTICE retains the MIT license and attribution for adapted accessibility,
capture, recording, audio, restoration and redaction code. Source annotations
continue to identify those adaptations. The extension icons retain their
design-system source manifest in `packages/desktop/extension/icons/source.json`.
Dependency licenses are checked against the standalone locked graph.

The native host name `ai.nddev.nds.browser_use`, existing Chrome extension IDs,
local `nds-browser-use` data namespace and `NDS.*` private protocol methods are
compatibility identities, not runtime dependencies on NDS. Their preservation
avoids disconnecting an existing browser or silently discarding preferences.

The initial runtime audit used `9089610bc7f50fd3daa9b57b5134b331f93414d5`.
The imported main also includes upstream PR 141, a source-metadata cleanup.
Its Rust, TypeScript, tests, dependency locks and browser provider pin are
identical to that audited runtime revision.
