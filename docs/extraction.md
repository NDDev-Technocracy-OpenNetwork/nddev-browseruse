# Standalone product and NDS boundary

The accepted direction is one browser extension and host named nddev-browseruse,
usable directly and through an NDS adapter. The existing Chrome Web Store item
is updated with its ID and users preserved. The first store targets are Chrome
Web Store and Microsoft Edge Add-ons.

The import preserves the public NDS browser-use history through commit
`94346d42a7d9cbc30894b3313cd8359f2c1837d1`. Original product code is relicensed
under Apache-2.0 with MIT notices retained. NDS integration descriptors and
mandatory NDS runtime/telemetry dependencies leave the standalone product.
The local protocol remains version 2. Existing IDs and local data paths stay
compatible; branding does not imply a new wire protocol or data reset.

The host uses bounded local socket exchanges and an explicitly installed local
metadata sink. It preserves session ownership, generation checks, execution
limits and drained shutdown. The public port is ready for NDS composition;
the NDS adapter and its telemetry exporter remain separate contour work.

Native/browser checks, strict-CSP capture, collection revocation, input coexistence,
deadline diagnostics, generated configuration and the five-language UI are part of
validation. Qualification evidence and platform limits accompany each release.
An unreproduced historical stall is not described as a root-cause fix.

Edge's store ID must be admitted explicitly after its item exists. Store images and
claims must match the tested build. Browser loading and store upload remain the
owner's steps. Packaging never registers a browser or changes its preferences.

The original NDS source remains in its repository until that contour adopts the
standalone engine. A standalone fix does not silently rewrite the old implementation.

Track the extraction in
[issue 1](https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse/issues/1).
Historical issue descriptions and merged PRs are evidence of earlier work,
not acceptance of the new product.
