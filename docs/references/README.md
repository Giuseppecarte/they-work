# Reference images

The contact sheet compares the four surfaces that currently have renderer
output. Use these exact filenames for their intended designs:

| Surface | File name |
| --- | --- |
| Office floor | floor.png |
| Guard office | guard-office.png |
| Desk detail | desk.png |
| Phone | phone.png |

The source of truth for these images is [docs/design](../design/README.md).
scripts/render-design.sh renders those boards into this directory; do not edit
the generated PNGs by hand. The contact sheet scales each reference to fit
beside the output without stretching it. PNG at the native design dimensions
is preferred.

## Design-only boards

These six supplied boards have no matching renderer surface yet, so they stay
in the repository as design material and are intentionally left out of
docs/shots/index.html. A row without a rendered counterpart would not be a
meaningful visual comparison:

| Board | File name |
| --- | --- |
| Cast | cast.png |
| Cameras | cameras.png |
| Settings | settings.png |
| Identity | identity.png |
| First run | first-run.png |
| Themes | themes.png |

The renderer script also writes floor-dark.png, floor-light.png, and
titles.png as supporting source boards. The dark floor is an alias of the
canonical floor.png, the light floor is its light design variant, and the
title board is not a separate rendered surface; none adds a contact-sheet row.

## Regenerating

After editing a source board, regenerate the reference images with:

~~~bash
scripts/render-design.sh
~~~

Then use the single review command:

~~~bash
make shot
~~~

The design renderer and the shot exporter both require Google Chrome or
Chromium. They search THEYWORK_SVG_RASTERIZER, google-chrome, chromium, and
chromium-browser; when no browser is available, they stop with an actionable
error rather than silently producing an incomplete image set.
