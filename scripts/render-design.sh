#!/usr/bin/env sh
# Render each design board to a reference PNG for the review contact sheet.
#
# The boards are plain HTML, so any browser can rasterise them; we use headless
# Chrome because it is what the shot exporter already depends on.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SRC="$ROOT/docs/design"
OUT="$ROOT/docs/references"

BROWSER=${THEYWORK_SVG_RASTERIZER:-}
if [ -z "$BROWSER" ]; then
    for candidate in google-chrome chromium chromium-browser; do
        if command -v "$candidate" >/dev/null 2>&1; then BROWSER=$candidate; break; fi
    done
fi
if [ -z "$BROWSER" ]; then
    echo "no browser found; install Chrome or Chromium, or set THEYWORK_SVG_RASTERIZER" >&2
    exit 1
fi

mkdir -p "$OUT"
render() {
    target="$OUT/$2.png"
    rm -f "$target"
    log=$(mktemp /tmp/they-work-design-render.XXXXXX)
    if ! "$BROWSER" --headless --disable-gpu --no-sandbox --hide-scrollbars \
        --screenshot="$target" --window-size="$3,$4" "file://$SRC/$1" >"$log" 2>&1
    then
        echo "failed to render $1 with $BROWSER" >&2
        sed -n '1,20p' "$log" >&2
        rm -f "$log"
        exit 1
    fi
    rm -f "$log"
    if [ ! -s "$target" ]; then
        echo "browser produced no reference image for $1: $target" >&2
        exit 1
    fi
    echo "  $2.png"
}

echo "rendering design references into docs/references"
render Main.dc.html     floor         1200 760
render Main.dc.html     floor-dark    1200 760
render Light.dc.html    floor-light   1200 760
render Tabs.dc.html     guard-office  1240 860
render Messages.dc.html desk           900 800
render Phone.dc.html    phone          760 800
render Devs.dc.html     cast          1240 940
render Views.dc.html    cameras       1240 720
render Settings.dc.html settings      1000 760
render Identity.dc.html identity      1240 640
render FirstRun.dc.html first-run     1000 640
render Offices.dc.html  themes        1100 700
render Titles.dc.html   titles        1000 520
