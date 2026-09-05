"""Render pixel-art characters from ASCII maps to inline SVG.

The maps are the design source: 24x34 units, one character per pixel. Everything
is authored once at this size and scaled by whole numbers, so the art stays
crisp at every rung instead of being redrawn or smoothed.
"""
W, H = 24, 34

PAL = {
    "#": "#1a1626",  # outline
    "s": "#f0c9a0", "d": "#c99a72", "l": "#ffe3c4",           # skin
    "h": "#3a2a1e", "g": "#241a12", "i": "#5c4430",           # hair
    "e": "#1a1626", "w": "#ffffff",                            # eye
    "m": "#b5705c",                                            # mouth
    "c": "#4f9ee8", "v": "#2f6fae", "b": "#7fbdf2",           # cloth
    "p": "#2b2542", "q": "#1c1830",                            # trousers
    "o": "#4a3a2a",                                            # shoes
    "a": "#e8342c", "n": "#f0b429", "t": "#56c26a",           # accents
    "k": "#c9c2d6",                                            # neutral prop
    "z": "#58d6e8",                                            # glass/screen
    # Claude orange, the second agent hue
    "r": "#e8834a", "R": "#b45f2c", "L": "#ffb07a",
    # neutral grey, for contractors
    "y": "#8a8299", "Y": "#5c566e", "U": "#c9c2d6",
    # warm sand, for variety in tops
    "f": "#d9b26a", "F": "#a3823f", "E": "#f5d79b",
    # deep violet
    "x": "#6b5ca8", "X": "#463a75", "Z": "#9a8bd6",
    # extra hair tones
    "j": "#c9843a", "J": "#8a5320", "N": "#f0b46a",
    "G": "#8a8299", "Q": "#5c566e",
}

def sprite(rows, overrides=None):
    pal = dict(PAL, **(overrides or {}))
    assert len(rows) == H, f"expected {H} rows, got {len(rows)}"
    for i, r in enumerate(rows):
        assert len(r) == W, f"row {i} is {len(r)} wide, expected {W}: {r!r}"
    out = []
    for y, row in enumerate(rows):
        x = 0
        while x < W:
            ch = row[x]
            if ch == ".":
                x += 1
                continue
            run = 1
            while x + run < W and row[x + run] == ch:
                run += 1
            out.append(f'<rect x="{x}" y="{y}" width="{run}" height="1" fill="{pal[ch]}"/>')
            x += run
    return "".join(out)

def svg(rows, scale=8, overrides=None, bg=None):
    body = sprite(rows, overrides)
    back = f'<rect width="{W}" height="{H}" fill="{bg}"/>' if bg else ""
    return (f'<svg viewBox="0 0 {W} {H}" width="{W*scale}" height="{H*scale}" '
            f'shape-rendering="crispEdges" xmlns="http://www.w3.org/2000/svg">{back}{body}</svg>')
