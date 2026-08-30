# Design references

The contact sheet compares the four surfaces that currently have renderer
output. Use these exact filenames for their intended designs:

| Surface | File name |
| --- | --- |
| Office floor | `floor.png` |
| Guard office | `guard-office.png` |
| Desk detail | `desk.png` |
| Phone | `phone.png` |

PNG, JPEG, WebP, and SVG are supported. Keep the supplied image at its design
dimensions; the contact sheet scales it to fit beside the corresponding
rendered surface without stretching it. The rendered outputs are labelled
Dark and Light beside the same intended-design slot at the fixed demo
timestamp. PNG at the native design dimensions is preferred.

## Supplied design-only boards

These six supplied boards have no matching renderer surface yet, so they are
kept here as design material and intentionally left out of
`docs/shots/index.html`. A row without a rendered counterpart would not be a
meaningful visual pass/fail comparison:

| Board | File name |
| --- | --- |
| Cast | `cast.png` |
| Cameras | `cameras.png` |
| Settings | `settings.png` |
| Identity | `identity.png` |
| First run | `first-run.png` |
| Alternate light floor | `floor-light.png` |

`floor-dark.png` is byte-for-byte identical to `floor.png`, so it is an alias
of the canonical floor reference rather than another board. The comparison
script only consumes the four canonical names above; it never overwrites
anything in this directory.

To refresh the review page after adding or replacing a reference, run:

~~~bash
make shot
~~~

This regenerates `docs/shots/index.html` and both dark/light PNG and SVG
outputs. Open the contact sheet to compare each rendered surface with its
reference. Regeneration requires Google Chrome or Chromium; set
`THEYWORK_SVG_RASTERIZER` when the browser is installed at a non-standard
path. If no browser is available, the exporter stops with a useful error
instead of claiming that the PNG review bundle was produced.
