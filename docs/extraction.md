# Standalone extraction

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

The first increment replaces the NDS I/O dependency with bounded local socket
exchanges and adds an explicitly installed local metadata sink. It preserves
session ownership, generation checks, execution limits and shutdown behavior.
It is not a qualified release until native and browser acceptance pass.

Remaining acceptance includes the screenshot CSP regression, Chrome input/frame
stalls and coexistence, phase-specific deadlines, collection consent on upgrade,
five-language UI, generated configuration and store packages. Edge's store ID
must be admitted explicitly after its item exists. Store screenshots and claims
must match the tested candidate. Browser loading and store upload are the owner's
steps after the candidate and evidence are ready.

Track the extraction in
[upstream issue 142](https://github.com/NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-browser-use/issues/142).
Historical issue descriptions and merged PRs are evidence of earlier work,
not acceptance of the new product.
