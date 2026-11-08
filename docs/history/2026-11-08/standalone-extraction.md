History. This records the extraction and qualification work on this date. It is not the current product state; use the current guides, releases and linked issues.

# Standalone extraction

The owner requested a standalone OpenNetwork product named nddev-browseruse, following NDDev standards, with one extension/engine ready for future NDS integration. The accepted license is Apache-2.0 for original code, retaining existing third-party licenses. Chrome updates its existing Web Store item. Edge gets material for a future new card. Browser loading and store uploads remain owner actions.

Prior implementation sessions, issues and pull requests were reviewed. The retained source history runs through 94346d42a7d9cbc30894b3313cd8359f2c1837d1; the runtime audit also compared 9089610bc7f50fd3daa9b57b5134b331f93414d5. Raw transcripts, device details and owner-browser data were kept out of the public repository. The research distinguished implementation evidence from usage and coordination, and retained the limits of available history.

The standalone graph removed mandatory NDS I/O and telemetry dependencies. Work added bounded cancellable IPC/ADB, public owned-session control, generated typed configuration/contracts, privacy-generation checks in Rust retention, private child-frame capture exclusion, explicit consent, five ICU languages/RTL, the public canonical OpenNetwork design system, license screens and repeatable native/store packaging.

Version 0.2.5 passed Linux/macOS/Windows checks and security scanning. The exact Linux release packet passed 77 browser/phone-fixture scenarios, including video with ffmpeg. Other platform skips were recorded. A real installation check found an upgraded extension paired with the old native host; registering the qualified host/executor restored compatible control without resetting the observation choice or restarting the browser.

The final cleanup addresses the redundant screenshot-style guide tip (#13), raw browser CDP subtree release (#14), bounded execution-step timing context (#15), and post-execution metadata delaying an otherwise completed/aborted reply (#16). New execution settings receive declared defaults only when that entire section is absent; existing operator values survive and malformed configurations still stop the install. Native CI now runs browser acceptance on the exact staged release files. Main promotion reuses the checks from dev.

The historical Chrome input/frame stall did not reproduce in the isolated two-extension checks. A live comparison with Playwriter 0.9.0 / extension 0.0.170 also completed wheel and screenshots before, during and after a browser-use session with preview active. This is qualification evidence, not a claimed root cause. The original NDS repository is not silently patched or declared fixed by a standalone release.

The parent task is #1. Its final release evidence records exact source, checks and artifact digests. Actual Edge catalog admission, physical Android acceptance and NDS adapter/telemetry composition remain separate work; credential custody and a cloud relay are not implemented.
