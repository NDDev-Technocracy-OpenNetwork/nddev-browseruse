# Embedding and NDS integration

The independent `nddev-browseruse-client` crate exposes the Rust `BrowserUse` application port.
Its `BrowserUseClient::new` accepts an explicit absolute local socket path, owns a concurrency
limit and performs cancellable exchanges. `nddev-browseruse-contract` owns the serializable
control requests and replies. Neither crate depends on the native engine or an NDS runtime.
The host owns tab authority, session generations, executors and drained close.
An owned-session handle is private control material and is never a UI DTO or
metadata field. Protocol version 2 remains compatible with the existing host.

The embedding application constructs the client and passes cancellation into
each operation. First call `capabilities` (or `nddev-browseruse capabilities`): it reports the
loaded protocol, host build, browser readiness, lifecycle support and effective limits without
creating a session. Discovery is a separate operation; the existing Status reply shape is
unchanged. An older host can refuse discovery instead of implicitly promising support.

Create an owned generation, open it, execute against it, then request close and retain the handle
until the host acknowledges its drain. Canceling one exchange does not acknowledge that drain.
After an idle lease expires, the old generation is stale; create a new one for later work.
A timeout or cancellation
does not prove that a browser action was rolled back. Observe before retrying
an action that can change external state.

Importing the library initializes no subscriber, exporter, process or browser.
The standalone executable explicitly installs its local diagnostics sink.
An NDS adapter belongs in NDS. It retains the original actor/device authority and maps task
lifecycle and cancellation onto owned generations, with its own telemetry composition. Browser
consent remains an explicit owner decision; SDK discovery or construction never enables control.
This product exposes the native process/RPC boundary; it does not implement the NDS module host,
identity, storage or synchronization. The standalone Cargo
graph must not require NDS just to resolve or build the product.

One common extension can serve either composition. Native messaging admits
explicit extension origins only; Edge's catalog origin must be added after its
assigned item ID is known. No credential custody, cloud relay or central content
export is implemented by this interface.

The installer accepts `--extension-id <assigned ID>` and retains explicit installed IDs on
reinstallation, including the existing CWS and unpacked identities. The typed, bounded local
`admission.json` policy admits only exact 32-character Chromium IDs. It is checked before
executor paths, application configuration or runtime state are read; the browser manifest
and host enforce the same list. Wildcards are rejected. No future Edge identity is guessed.
