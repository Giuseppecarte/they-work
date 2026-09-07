# Preserve current requests before rendering

The inspector's native PTY fixture exposed a data loss before layout: a Codex
pending approval command was shortened to 120 characters twice, and the prefix
consumed part of that budget. The displayed command ended in `customer`, losing
`-account-id-index` without any visible indication of truncation.

Pending Codex commands and Claude questions now use the existing 2,000-character
timeline budget. Bounded detail and timeline text use a visible ellipsis only
when content was actually omitted. Control characters remain sanitized and no
UTF-8 code point is split. Short office captions remain bounded to 120 characters.

A SQLite-backed regression verifies the exact suffix of a pending command longer
than a caption and a bounded 3,000-character Unicode request. Claude's matching
test preserves long question context; limit tests distinguish exactly-at-limit
text from omitted content. The final inspector PTY capture displays the original
request suffix. This does not imply that complete transcripts are retained: core
history is still bounded, and requests are handled in the source conversation.
