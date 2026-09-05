import cast, gen

HEAD = '''<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Press+Start+2P&family=JetBrains+Mono:wght@400;700&display=swap">
  <style>
    body { margin: 0; background: #0d0b14; }
    a { color: #f0b429; } a:hover { color: #e8342c; }
    .h1 { font-family: 'Press Start 2P', monospace; font-size: 17px; color: #e8342c;
          text-shadow: 2px 2px 0 #8e1a15; }
    .sub { font-size: 11px; color: #6f6885; }
    .card { background: #131126; border: 1px solid #241f3d; }
    .lab { font-size: 10px; font-weight: 700; letter-spacing: 1.5px; }
    .body { font-size: 11px; color: #b4aec2; line-height: 1.85; }
    .cap { font-size: 10px; color: #6f6885; line-height: 1.7; }
  </style>
</helmet>
'''
TAIL = "</x-dc>\n</body>\n</html>\n"

def page(w, h, inner):
    return (HEAD + f'<div style="width:{w}px;height:{h}px;background:#0d0b14;'
            f"font-family:'JetBrains Mono',ui-monospace,monospace;color:#e8e2d6;"
            f'padding:28px 32px;box-sizing:border-box">' + inner + "</div>\n" + TAIL)

CAST = [
  ("Dev 1", "Codex",  dict(hair="short", cloth=("c","v","b"))),
  ("Dev 2", "Claude", dict(hair="long", eyes="glass", cloth=("r","R","L"), hair_c=("j","J","N"))),
  ("Dev 3", "Codex",  dict(hair="cap", beard=True, cloth=("c","v","b"))),
  ("Collector", "Claude", dict(hair="short", eyes="shut", cloth=("r","R","L"), hair_c=("G","Q","U"))),
  ("Orchestrator", "Claude", dict(hair="long", cloth=("r","R","L"), hair_c=("h","g","i"))),
  ("sub:explore", "contractor", dict(hair="short", cloth=("y","Y","U"), hair_c=("G","Q","U"))),
]

def figure(spec, scale=7, bg="#0f0d1a"):
    return gen.svg(cast.person(**spec), scale=scale, bg=bg)
