History. Current guides, issues and qualification receipts take precedence.

# Shared CI readiness

The owner requested a fresh check of runner and release readiness. The qualified shared CI
source now contains hosted SDK/native archive preparation, the Windows drive-letter extraction
fix and independent scanner policy for daily reports. Its exact source passed full dev and six
native/reusable checks on Ubuntu, macOS and Windows. Browseruse still selected the earlier
shared source, so #28 updates its check/security pins together. Project-owned setup remains the
authority for its selected tools; no new SDK or media provider is selected by this pin update.

The application's native workflow had already passed its actual macOS/Windows acceptance at the
previous dev checkpoint. It does not use the shared archive bootstrap that had the Windows
defect. Those checks run again for the new proposed source. Product code, native registration,
owner profiles and store submission are unchanged.

The public FFmpeg provider remains unresolved in #25. The generic verified native-asset mechanism
and synthetic codec tests do not establish an accepted provider or the complete product media
gate. The existing qualified PC package is absent from public hosted APT indexes. Linux's
zero-skip requirement stays enforced, and main/live remain at the previous released version
until a complete successful dev check qualifies publication. #19 retains the remaining order;
#24 covers first automatic publication and recovery, and #27/#23 retain saved-group boundaries.
