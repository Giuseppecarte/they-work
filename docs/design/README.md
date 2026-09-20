# Design source

The current design direction is [Modern campus](Campus.dc.html). It supplies
all current reference surfaces through its `surface` and `theme` parameters.
It is an independent schematic, not a screenshot or terminal certification.
Earlier individual boards remain here as historical design material.

The current renderer review is reproducible with:

```sh
env -u NO_COLOR COLORTERM=truecolor ./scripts/cargo run --example campus_review -- target/campus-review/ui
env -u NO_COLOR COLORTERM=truecolor ./scripts/cargo run --example studio_gallery -- target/campus-review/art
scripts/render-design.sh
make shot
```

The first command exports 19 real UI routes at 80×24 and 120×36 cells, in both
themes and with graphics or native cells. It records physical RGBA layers and
native text separately; it is not terminal playback. The second exports both
authored character grids, furniture variants, and decorative motion frames.
Inspect the assembled views and exercise the native app before landing.


## Historical boards

These earlier artboards document the former direction. They are retained for
context and are no longer the source of the current reference images.

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

`docs/references/` holds a rendered PNG of each campus surface, which the review contact
sheet places beside the real output. To refresh them after editing a board:

~~~bash
scripts/render-design.sh
~~~

It needs Chrome or Chromium and writes only into `docs/references/`.
If no browser is found, the script exits with:

~~~text
no browser found; install Chrome or Chromium, or set THEYWORK_SVG_RASTERIZER
~~~

If the browser exits without writing one of the expected PNGs, the script
reports the source board and target path and stops before reporting success.
