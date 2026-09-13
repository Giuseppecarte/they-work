# Connect once, with a clear choice

The source screen now explains the local data it reads, names the app data folders,
and offers **Remember on this computer** without requiring a configuration flag.
Paths can be edited at the cursor, and a connection error selects the source that
needs attention. A temporary session remains available through the checkbox or
`--no-save`.

## Observed problems and changes

| Observation | Change and reason |
| --- | --- |
| The old editor appended text and removed only the last character. Both source paths were cut off on the right, hiding the part being edited. | Arrow keys, Home/End, Backspace/Delete and Ctrl+U work at the insertion point. The viewport follows the cursor; display paths preserve their ending. UTF-8 paths are retained. |
| Connect detected a missing Codex folder but left Claude Code selected. The suggested `e` action edited the wrong source. | Validation focuses the failing source before explaining `e` to repair or Space to disconnect. |
| “Folder found” gave little guidance about which directory the application expected. | The screen identifies `.codex` / `.claude` as app data folders and explains local titles, messages, tool activity and approval in the original app. Folder status refreshes while the dialog is open. |
| Choices were forgotten unless the user learned and repeated `--config-dir`. | The visible remember checkbox uses the standard user settings location. `--config-dir` remains an override; temporary choices leave an existing saved configuration unchanged. |
| `--once --project --config-dir` created a settings folder and wrote the selected project. Unconfigured noninteractive commands implicitly selected both providers. | Read-only commands do not create preferences. Unconfigured scripts explain source selection; unconfigured doctor checks only candidate folder metadata. |
| The setup form emitted RGB colors even under Apple Terminal’s 256-color environment. | Source UI uses indexed colors; explicit no-color and NO_COLOR reset its colors. |

The native PTY before capture confirms the wrong-source repair: Codex is missing,
but Claude remains selected when `e` opens the editor. Compare
[before](evidence/entry-before-editor.png) with
[after](evidence/entry-after-editor.png). Long-path editing is visible in
[the cursor viewport](evidence/entry-after-long-path.png).

## Persistence and access contract

- macOS, Linux and WSL use `$XDG_CONFIG_HOME/they-work`, with
  `$HOME/.config/they-work` as the fallback. Windows uses `%APPDATA%\they-work`,
  then the home `.config\they-work` directory. Relative environment settings
  paths are ignored. Computing the location does not create it.
- Enter on Connect saves provider switches and folder paths only when Remember is
  enabled. The interactive app also saves appearance and the selected real floor.
  No messages or transcript records are copied into preferences.
- `--no-save` locks the remember choice off for the entire process. Turning the
  checkbox off makes the current choices temporary. Neither option deletes or
  replaces previously saved choices.
- Demo does not read saved preferences or conversation sources. Opening `c` from
  demo is an explicit request to choose local sources; confirming that dialog is
  what starts collection. Demo does not overwrite real appearance or floor choices.
- `--once`, `--doctor`, `--headless`, and redirected ordinary output never write
  preferences. Without saved source consent or `--sources`, no conversation store
  is opened. Home-path flags alone do not grant consent. Doctor may stat candidate
  folders and examine terminal capabilities, then explains how to choose sources.
- CLI source flags apply to that run. The connection dialog records a reusable
  choice. `--sources none` is valid, including a successful doctor result.

The setup screen inspects folder metadata before consent. A found directory may
still contain no compatible or recent conversations. After connecting, doctor
reports store-level errors and gives direct instructions for `--setup`, `e`, and
Space. The empty tower avoids claiming that zero workers means zero configured
sources.

## Self-critique and corrections

A final cross-directory restart caught a relative environment override being
saved literally, which would reconnect to a different folder from another
project. Both source homes are now resolved before the choice is remembered.
[The reproduction](evidence/entry-relative-before.json) records the original
relative value; [the regression](evidence/entry-paths-results.json) confirms the
absolute saved location and a restart from a different working directory.

The first implementation passed focused tests but failed a realistic recovery
review. Connecting temporarily after a damaged settings file still caused the
runtime to load that same file. The current process now retains its confirmed
connection in memory. An unreadable/invalid saved floor is treated as a disposable
view preference. Source-consent errors still require the explicit setup path.

Another review found that entering source setup from the demo initially selected
both providers, including a nonexistent folder. Discovered defaults are now
consistent with first launch; explicit or remembered choices still take priority.
The [rejected PTY run](evidence/entry-pty-rejected.log) reached this validation screen and then sent `q`,
which correctly returned to demo. Its harness timeout was caused by assuming
Connect had succeeded, not by a broken exit loop.

At less than 40×16, choices are hidden behind resize guidance. Only cancel and demo
are accepted there, so Enter cannot confirm choices the user cannot see. The form
fits 40×16 with both sources, the remember checkbox and the exit/confirm controls.

Bracketed paste prevents a copied newline from confirming a connection. A
multiline path paste is rejected; single paths with spaces and non-ASCII text are
accepted. Integration review also identified that enabling this mode globally
required the finder to handle paste explicitly; the finder now owns that behavior
rather than treating pasted text as global shortcuts.

## Verification

`review_entry.py` launches the native executable in a real macOS PTY. Every fixture,
HOME, USERPROFILE, XDG_CONFIG_HOME and APPDATA used by this review stays inside the
repository. No personal configuration was modified. The child wrapper compares
terminal modes before/after exit, excluding macOS’s transient PENDIN flag.

The initial nine passing interaction scenarios are recorded in
[entry-after-results.json](evidence/entry-after-results.json). They cover:

1. Missing-source focus, cursor visibility, Unicode/space path repair, live folder
   detection, confirmation and exact saved source paths.
2. Restart without `--config-dir`, cancellation, and live → demo → source selection.
3. An unchecked remember option leaves no settings folder.
4. Demo selected during first launch leaves no settings folder.
5. `--no-save` stays disabled through reconnection.
6. An unwritable settings location (a file where the folder belongs) offers a
   temporary recovery and preserves the original file.
7. `--setup --no-save` repairs a corrupt saved choice in memory.
8. The 40×16 form can deliberately enter an empty tower.
9. Empty and multiline input are rejected; Unicode cursor movement and Home/End
   remain usable.

The two additional boundary scenarios are recorded separately in
[entry-boundaries-results.json](evidence/entry-boundaries-results.json): a terminal
smaller than 40×16 cannot confirm hidden options, and `c` from demo can recover a
corrupt saved connection without selecting a nonexistent provider. Two more
PTY launches cover the relative-folder correction and restart from another
directory in [entry-paths-results.json](evidence/entry-paths-results.json). Across
the three passing result files, **13 sessions exited successfully and restored
terminal modes**.

The integrated native TUI test run passed **55 tests** (21 unit tests and 34 CLI
integration tests), recorded in [entry-tests.log](evidence/entry-tests.log).
Strict Clippy also passed in [entry-clippy.log](evidence/entry-clippy.log).
Focused Rust tests cover the path editor, the visible form at supported sizes,
platform configuration precedence, default saved choices, explicit overrides,
noninteractive consent and filesystem effects. CLI fixtures now isolate all user
settings environment variables. Full workspace verification is recorded by the
integration review; earlier results are not substituted for that final run.

Reproduce the interactions with:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 docs/design-audit/iteration-3/review_entry.py --stage after
sh docs/design-audit/native-cargo.sh test -p theywork-tui
sh docs/design-audit/native-cargo.sh clippy -p theywork-tui --all-targets -- -D warnings
```

Images reconstruct captured ANSI cells with Menlo. They are not Terminal.app
screenshots. The PTYs use the Apple Terminal environment and 256-color palette;
no Windows/Linux terminal UI is claimed as visually tested here. Platform path
selection is unit-tested for both Windows and Unix branches on the available
host; real platform results belong to the integration/release matrix.
