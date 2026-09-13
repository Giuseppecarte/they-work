# Source chooser: executable PTY verification

This check launches the native `they-work --setup` executable in a real pseudoterminal. It is separate from the TestBackend compositor inventory. The PNGs replay the captured ANSI cells using Menlo/Pillow at 10×18 pixels per cell; they are not screenshots of a terminal window. Terminal.app is not controlled.

The [harness](review_sources_pty.py) creates fresh directories beneath ignored audit scratch. Each run has its own config, HOME, empty `.codex` and `.claude`, and provider-name stubs that only record capability probes and exit. It inherits no provider credentials or real conversation folders. No login or real provider is launched.

**Result: 9/9 passed** against native release SHA-256 `44100edbd7e6b4fa34d2c796d42e6281cd463a1d5c0ae72491b466d2f773c8dd`. [Machine-readable results](evidence/sources-pty/results.json) include 387 verified evidence-file hashes. All nine runs restored terminal flags and left the alternate screen; no provider stub was invoked, so actual and fake provider calls were both zero.

| Case | Initial chooser |
|---|---|
| 32×14 | [Compact](evidence/sources-pty/32x14-dark/01-chooser.png) |
| 80×24 | [Standard](evidence/sources-pty/80x24-dark/01-chooser.png) |
| 120×36 | [Medium](evidence/sources-pty/120x36-dark/01-chooser.png) |
| 192×58 | [Large](evidence/sources-pty/192x58-dark/01-chooser.png) |
| 110×80 | [Tall](evidence/sources-pty/110x80-dark/01-chooser.png) |
| 240×70 | [Wide](evidence/sources-pty/240x70-dark/01-chooser.png) |
| 80×24 light | [Saved light theme](evidence/sources-pty/80x24-light/01-chooser.png) |
| 80×24 monochrome | [Bold focus without color](evidence/sources-pty/80x24-no-color/01-chooser.png) |
| 80×24 indexed | [Explicit 256 colors](evidence/sources-pty/80x24-256/01-chooser.png) |

Visual critique found no blocking clipping in the reviewed states. At 32×14 all switches, paths, editing guidance and three actions remain visible. [Editing](evidence/sources-pty/32x14-dark/02-editing.png) has complete Apply/Cancel actions, and the [empty tower](evidence/sources-pty/32x14-dark/05-empty-tower.png) explains the consequence and offers Connect sources. Long paths retain the `.codex`/`.claude` ends. Wide/tall cases center a bounded form instead of stretching its controls across the screen. Light text remains readable; monochrome retains a bold focused Connect button; explicit 256 colors emit indexed ANSI rather than truecolor.

The requested matrix is 32×14, 80×24, 120×36, 192×58, 110×80 and 240×70 in dark mode, plus 80×24 with saved light appearance, `NO_COLOR=1` and explicit `THEYWORK_COLOR=256`.

Every case checks:

1. Both providers, Remember and Connect/Demo/Back are visible, with Connect focused initially.
2. Eight forward Tab transitions and eight reverse Shift+Tab transitions each redraw the chooser and expose the expected focused field/button. Every frame has PNG and TXT evidence.
3. Editing a folder, clearing it, typing a synthetic path and pressing Escape restores the exact prior form text. The cancelled path is not persisted.
4. Disabling both sources and pressing F5 opens an empty tower, persists both switches as false and preserves the two original isolated folder paths.
5. Exiting restores the PTY's canonical/echo flags and returns status zero.
6. Saved light appearance produces a light background. Monochrome and explicit 256-color startup ANSI must contain no truecolor `38;2` or `48;2` SGR.

Each case retains `session.ansi`, initial/edit/cancel/off/empty frames, all focus-transition frames and a result containing their hashes. The matrix result records the actual executable SHA-256 and refuses to continue if that binary changes mid-run. Provider probe records are included in the result.

## Reproduce

With Python, Pillow and pyte available, from the repository root:

```sh
python3 docs/design-audit/iteration-6/review_sources_pty.py --binary target/native-macos/release/they-work
```

`--cases 32x14-dark 80x24-256` selects a bounded subset. `--output` chooses an evidence directory inside the repository. The default full output is [sources-pty](evidence/sources-pty/).

The scope is native macOS execution and captured ANSI behavior. These results do not claim Windows/Linux execution or a physical font/terminal-window test.
