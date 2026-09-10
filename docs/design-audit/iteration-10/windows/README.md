# WIN-01 native gate remains open

No Windows runner executed in this local verification. A read-only query found no remote `audit/design-and-usability` branch. The earlier upload was rejected by automatic approval review pending explicit destination authorization; no workaround upload was attempted.

Run the checked-in native workflow on the exact candidate, on `windows-2022` x64 and `windows-11-arm` ARM64. Inspect both jobs and download their `windows-storage-*` evidence plus actual native distribution artifacts. The structured requirements are in [status.json](status.json). Skipped jobs, absent artifacts, mismatched commits/architecture and zero matching tests block closure.

The six replacement unit results include a child-process helper. Four shared recovery tests, one live-host fault test and four native process cases are required separately on each architecture. Preserve logs even when a runner cannot create a disposable symlink; that case fails rather than silently skips.

Native macOS tests and previous Windows cross-Clippy cannot close this item. Physical power loss, non-NTFS and network filesystems, real terminal rendering and authenticated providers remain untested beyond this bounded native-runtime gate.
