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

Dependency update configuration now covers the Rust workspace and both Bun manifests as well
as GitHub Actions, with daily proposals after three days and no automatic branch rebasing.
The existing daily shared security scan remains separate. A strict-mode diagnostic fixture now
initializes the locator engine and uses a bounded diagnostic budget before checking the complete
three-match error; it is not a latency assertion. These changes do not alter product runtime files.

Official sources reviewed include Chrome's storage and tabGroups contracts, React createRoot,
Node's AsyncLocalStorage, nextest's test runner/documentation-test boundary, and Chrome/Edge store
requirements. Context history from available harnesses informed provenance; incomplete indexes
were not represented as an exhaustive audit. Raw transcripts and owner-browser data remain out
of the public tree. Tooling compatibility review remains part of the PR's completion criteria.

Track the work in #19, #20, #21 and PR #22. Loading the new extension and submitting store packets
remain owner actions. No package is a production release merely because candidate tests pass.

The subsequent dependency review accounted for the complete Rust and both Bun lock graphs,
including required older companion APIs. The selected graph now takes Playwright 1.65.0,
unbash 6.0.3, serde_spanned 1.1.2 and toml_datetime 1.1.2+spec-1.1.0. Other lock records remain
unchanged. Lightning CSS remains an unchanged test-tool dependency, excluded from the shipped
runtime and browser bundle. Setup now reuses a matching Bun on PATH before its separate native
bootstrap, without writing through an old project symlink into the owner's runtime.
Repeating setup exposed cargo-shear's different version-output label and an existing-binary
update failure. Exact version tokens now handle that format, and mismatched tools are replaced
inside the project without following external symlinks.

The delivery review also found that release assembly still depended on individual agent
commands. Follow-up #24 makes the full Linux check retain its qualified packet and makes
main publication consume the successful dev/native/security artifacts of that exact commit.
Browser acceptance now records measured results separately from build facts. The publisher
reconciles immutable tags and draft assets and downloads uploaded bytes before publication.
Browser loading, store submission and contour-side deployment remain their existing owners.

The first merged dev commit 1574d4d4f6b12aeac7da168dd870dbb3c6921930 passed the complete PC
packet qualification: 147 nextest, 116 Vitest and all 84 browser/ADB-fixture cases. Full native
macOS/Windows and security dev jobs succeeded. The public Linux dev job lacked FFmpeg, skipped
three media cases and correctly failed the new zero-skip packet guard. Main and live installations
therefore remained at 0.2.6. The source-consistent candidate was prepared with explicit hold
instructions; #25 tracks the native encoder prerequisite and #24 the first automated publication.

A further investigation distinguished tab-strip cleanup from Chrome's saved/synced group model.
The unsigned-in sync-internals model was empty and supplied no proof. Chromium's file-backed
loopback sync made actual Saved Tab Group records observable in an isolated profile without an
account or remote sync service. Positive controls appeared before removal. Ordinary removal deleted the saved record while its
window remained open, but a corrected separate-window probe reproduced retention when the last
grouped tab closed. The product reproduction also failed against the previous staged extension
after successful session close: its saved GUID remained. Retirement now ungroups the verified
control tab before deletion and rechecks its marker, window and ungrouped state after that await.
Owner navigation or movement during that boundary preserves the tab. Moving a whole group across
windows emits intermediate group removal events; the regression instead creates the session in
its initial test window and verifies that window before removing its ungrouped seed tab.
Acceptance checks real
saved GUIDs for normal close, retained pages,
host loss and five-minute expiry, while keeping the active recording's saved group until close.
The owner-assisted legacy workflow and the cross-device effect of Chrome's Ungroup are in #23.

The final provider audit found the test browser still pinned to the previous line. Official
Chrome-for-Testing metadata supplies the selected stable fixture for Linux x64, macOS arm64 and
Windows x64. Each archive was downloaded on the build PC, measured, SHA-256 pinned and ZIP-CRC
checked before updating provider.lock.json. This changes test artifacts, not an installed browser.

The final otherwise-empty-window fixture was red against the original staged extension and green
with the retirement fix on Chrome-for-Testing 157. It validates the initial window and saved GUID
before closing, rather than moving the group through Chrome's intermediate removal events.
Privacy policy, five-language store copy and reviewer notes now distinguish local product
analytics from Chrome's independent saving/sync of group titles and tab URLs. Externally closing
a whole window remains the scoped recovery investigation in #27.
