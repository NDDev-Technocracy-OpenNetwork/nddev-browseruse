# Build-only js-yaml library distribution

The source files, CJS/ESM library bundles, upstream README and MIT license are byte-for-byte
from the npm js-yaml 4.3.2 release. UPSTREAM.json records the original manifest, npm archive
integrity and every retained upstream file digest. No argparse source is copied or relabeled.

The package manifest omits the standalone YAML CLI, its argparse dependency and upstream
build/development scripts. This private local package supplies only the unchanged library
API used by Hey API. It deliberately does not provide a js-yaml executable. The original
README's CLI instructions therefore describe upstream, not this library-only distribution.

Version 4.3.2 fixes GHSA-2883-xcg3-v3hh, GHSA-52cp-r559-cp3m and GHSA-5p4m-2wfm-xmqj.
Bun's local override removes the old transitive CLI dependency without a license exception
or a changed parser API. The code generator still consumes our Rust-produced OpenAPI JSON;
its generated TypeScript must remain identical. No part of this directory ships in the
browser ZIP or native runtime. The Rust project check validates source and manifest integrity.

Update from a verified upstream npm release, retain its exact library bytes/notices and
receipt, adapt only the described manifest fields, regenerate the frozen codegen lock and
prove contract-output conformance before merging. Do not publish this upstream-named private
package. Report the dependency graph to the ordinary daily security scan.
