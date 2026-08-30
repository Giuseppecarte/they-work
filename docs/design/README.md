# Design source

The intended design for every surface, one file per artboard. These are the
authority: where the code and a board disagree, the board is what we meant.

| File | Surface |
| --- | --- |
| `Main.dc.html` | the office floor, dark |
| `Light.dc.html` | the office floor, light |
| `Tabs.dc.html` | office tabs and the guard office |
| `Identity.dc.html` | one worker drawn on all six surfaces |
| `Devs.dc.html` | the wardrobe and the assembled cast |
| `Views.dc.html` | isometric, top-down and side cameras |
| `Settings.dc.html` | the settings screen |
| `FirstRun.dc.html` | the first screen |
| `Offices.dc.html` | per-project office themes |
| `Titles.dc.html` | the project sign and its letterforms |
| `Phone.dc.html` | the phone overlay |
| `Messages.dc.html` | one worker's thread |

`canvas.json` lays them out and carries the design notes as sticky annotations —
the colour law, why amber is reserved, and the questions still open.

Each file is plain HTML: open one in a browser to read it. The `<x-dc>` and
`<helmet>` wrappers are inert outside the canvas editor and do not affect how it
renders locally.

## Regenerating the reference images

`docs/references/` holds a rendered PNG of each board, which the review contact
sheet places beside the real output. To refresh them after editing a board:

~~~bash
scripts/render-design.sh
~~~

It needs Chrome or Chromium and writes only into `docs/references/`.
