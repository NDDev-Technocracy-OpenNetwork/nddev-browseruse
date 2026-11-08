History. This records work on this date. Current guides, issue state and release receipts take precedence.

# Session lifecycle and module boundaries

The owner requested standards alignment and a standalone product that can become an NDS native
module without duplicating the engine. The owner also reported abandoned tab groups and chose a
dedicated service tab with local group analytics, plus cleanup after five idle minutes. Actual
execution and recording must remain protected. The owner authorized autonomous investigation
within this product, with unresolved choices preserved as issues and no interactive questions.

The client SDK, serializable control contract and platform local transport now have independent
crates. Compiler visibility and a checked crate graph keep engine/installer dependencies out of
the SDK. Dependency versions have one workspace home. Capability discovery preserves the old
Status reply and wire version. NDS retains its own identity, storage, synchronization, module
host and telemetry composition; no contour adapter or device pin was changed here.

Group cleanup uses native generation fencing, drained close and explicit terminal release proof.
The extension records a bounded persistent identity and a unique owner service tab. Recovery
handles new numeric group IDs and delayed window restoration. Duplicate/conflicting markers fail
closed. Owner-moved pages and unrelated groups are preserved. A legacy session-storage mapping
survives worker suspension but is lost on extension reload/update and browser restart; a title
never grants authority. Already-unmarked legacy groups remain a separate owner decision in #23.

React feature folders replace direct DOM mutation in the popup and group page. The shared locale
and theme boundary uses five ICU locales, RTL and four design-system modes. The canonical public
design-system pin remains unchanged. Production browser chunks share React instead of duplicating
it. All TypeScript behavior/component tests now use Vitest, and native tests move to nextest with
a separate documentation-test pass. Store copy and authentic screenshots cover the service tab.

Real acceptance found that an ongoing video did not protect its session after the idle lease.
A session-local native recording reservation now expires without live encoder renewals. Stop,
executor loss and capture expiry revoke protection. Heartbeats neither count as agent work nor
reset the idle clock. A full five-minute reproduction failed before the fix; the green scenario
preserved the real video while removing a plain idle session, then stopped and closed it cleanly.

At checkpoint c779676bd2babefd2c8df1f3d493abd04db47ce6, Linux passed 116 unit/component tests and
83 real Chromium/phone-fixture scenarios. Visual inspection covered real popup/consent captures
and the service page; its layout checks exercised five languages, four themes and two widths.
Platform CI and the exact final packet remain required before promotion. Physical Android and
actual Edge/catalog admission are still outside this qualification and tracked in #18.

The release-profile Linux candidate at 5231feb594281512884fb96fe594b76919544b48 subsequently
passed 142 nextest cases, 116 Vitest tests and all 83 Chromium/ADB-fixture scenarios against its
exact staged host/executor/extension. Store ZIP hashes and 27 rendered assets were checked;
the store artwork was also inspected through browser-use in ordinary Chrome. The packet was
prepared as a candidate and did not replace an installed host or loaded extension.

A later macOS run conservatively redacted an ordinary fill after its bounded type lookup.
The acceptance expectation now permits this documented outcome while still requiring sensitive
values to be redacted. A separate real-browser scenario removes the filled field before lookup
and confirms that its unverified value and snapshot are omitted. The three focused recorder
scenarios also passed against the same staged Linux packet. This changed tests and documentation,
not the runtime's privacy policy or its classification deadline.

Official sources reviewed include Chrome's storage and tabGroups contracts, React createRoot,
Node's AsyncLocalStorage, nextest's test runner/documentation-test boundary, and Chrome/Edge store
requirements. Context history from available harnesses informed provenance; incomplete indexes
were not represented as an exhaustive audit. Raw transcripts and owner-browser data remain out
of the public tree. Tooling compatibility review remains part of the PR's completion criteria.

Track the work in #19, #20, #21 and PR #22. Loading the new extension and submitting store packets
remain owner actions. No package is a production release merely because candidate tests pass.
