# Embedding and NDS integration

The Rust `BrowserUse` port exposes bounded browser operations. Its local
`BrowserUseClient` owns a concurrency limit and cancellable socket exchanges.
The host owns tab authority, session generations, executors and drained close.
An owned-session handle is private control material and is never a UI DTO or
metadata field. Protocol version 2 remains compatible with the existing host.

The embedding application constructs the client and passes cancellation into
each operation. Create an owned generation, open it, execute against it, and
close it only after outstanding work has drained. A timeout or cancellation
does not prove that a browser action was rolled back. Observe before retrying
an action that can change external state.

Importing the library initializes no subscriber, exporter, process or browser.
The standalone executable explicitly installs its local diagnostics sink.
An NDS adapter belongs in the NDS contour and supplies its context, observation
policy, consent and transport through this public port. The standalone Cargo
graph must not require NDS just to resolve or build the product.

One common extension can serve either composition. Native messaging admits
explicit extension origins only; Edge's catalog origin must be added after its
assigned item ID is known. No credential custody, cloud relay or central content
export is implemented by this interface.
