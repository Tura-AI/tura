# Native macOS application packaging

**Status:** Implemented by this change; signing and notarization credentials remain a release-pipeline responsibility.

## Problem

A copied or installed native `.app` must reliably locate the gateway, runtime assets, and its writable application home. Development-only paths and Finder-driven DMG steps make the result fragile.

## Work

- Bundle or deterministically locate the gateway and runtime assets from the app bundle.
- Default installed state to `~/Library/Application Support/Tura`.
- Make the packaged app independent of a Tura source checkout and Homebrew-specific paths.
- Replace interactive Finder packaging with a reproducible command-line build.
- Document signing and notarization requirements.

## Acceptance

- The `.app` launches after being copied to `/Applications` on a clean Mac account.
- Chat, tools, restart, and session restoration work without a source checkout.
- Packaging is non-interactive and produces a signed, reproducible artifact.
