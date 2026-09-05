from gen import W, svg

def c(core):
    pad = W - len(core)
    left = pad // 2
    return "." * left + core + "." * (pad - left)

def overlay(base_rows, prop_rows):
    out = []
    for i, row in enumerate(base_rows):
        if i < len(prop_rows) and prop_rows[i]:
            p = c(prop_rows[i])
            row = "".join(pc if pc != "." else bc for pc, bc in zip(p, row))
        out.append(row)
    return out

# ---------------------------------------------------------------- head ----
# The skull is 10 units of skin wide, with hair sides and an outline either
# side. Rounding at the crown and jaw is what stops it reading as a brick.
def head(hair="short", eyes="open", brow=True, beard=False, hair_c=("h", "g", "i")):
    h, g, i = hair_c
    crown = {
      "short": [f"...####{'#'*4}####...",
                f".##{g*12}##.",
                f"#{g}{h*12}{g}#",
                f"#{g}{h*3}{i*6}{h*3}{g}#",
                f"#{g}{h*12}{g}#"],
      "long":  [f"...####{'#'*4}####...",
                f".##{g*12}##.",
                f"#{g}{h*12}{g}#",
                f"#{g}{h*3}{i*6}{h*3}{g}#",
                f"#{h}{h*12}{h}#"],
      "cap":   [f"...####{'#'*4}####...",
                f".##nnnnnnnnnnnn##.",
                f"#nnnnnnnnnnnnnn#",
                f"##nnnnnnnnnnnn##",
                f"#{g}{h*12}{g}#"],
    }[hair]
    crown = [r[:16] if len(r) >= 16 else r for r in crown]
    crown[0] = "...##########..."
    crown[1] = f".##{g*10}##."
    crown[2] = f"#{g}{h*10}{g}#"
    crown[3] = f"#{g}{h*2}{i*6}{h*2}{g}#"
    if hair == "long":
        crown[4] = f"#{h}{h*2}##########{h}"[:16]
        crown[4] = f"#{h}{h}##########{h}{h}"
    elif hair == "cap":
        crown[1] = ".##nnnnnnnnnn##."
        crown[2] = "#nnnnnnnnnnnn#"
        crown[2] = "#n" + "n"*10 + "nn#"
        crown[2] = "#" + "n"*12 + "#"
        crown[3] = "##" + "n"*10 + "##"
        crown[4] = f"#{g}##########{g}#"
    else:
        crown[4] = f"#{g}##########{g}#"

    side = f"#{g}#"
    eyerow = {"open":  "sswesswess",
              "glass": "s##ee##ee#s"[:10],
              "shut":  "ss##ss##ss"}[eyes]
    if eyes == "glass":
        eyerow = "s#we##we#s"
    rows = [
        f"{side}llssssss ll".replace(" ", "")[:13] + "#g#",
        f"{side}" + ("ss##ss##ss" if brow else "ssssssssss") + "#g#",
        f"{side}{eyerow}#g#",
        f"{side}ssssssssss#g#",
        f"{side}ssssddssss#g#",
        f"{side}ssssssssss#g#",
        f"{side}" + ("ggmmmmmmgg" if beard else "sssmmmmsss") + "#g#",
        ".##" + ("gggggggggg" if beard else "ssssssssss") + "##.",
        "..#" + ("ggggggggg" if beard else "sssssssss") + "s#..",
        "..##ssssssss##..",
        "...##dddddd##...",
    ]
    rows[0] = f"{side}llssssssll#g#"
    return ["." * 16] + crown + rows

# --------------------------------------------------------------- torso ----
def torso(cloth=("c", "v", "b"), badge=None):
    """Arms are the difference between a person and a bar of colour: two units
    of shadowed cloth either side, ending in hands."""
    m, s, l = cloth
    rows = [
        "...##########...",
        f".##{m*10}##.",
        f"#{s*2}{m*10}{s*2}#",
        f"#{s*2}{m*2}{l*6}{m*2}{s*2}#",
        f"#{s*2}{m}{l*8}{m}{s*2}#",
        f"#{s*2}{m*2}{l*6}{m*2}{s*2}#",
        f"#{s*2}{m*10}{s*2}#",
        f"#{s*2}{m*10}{s*2}#",
        f".#{s}{m*10}{s}#.",
        f".#d{m*10}d#.",
        f".#{m*12}#.",
        "..############..",
    ]
    if badge:
        rows[6] = f"#{s*2}{m}{badge*3}{m*6}{s*2}#"
    return rows

LEGS = ["...####..####...", "...#pp#..#pp#...", "...#pp#..#pp#...",
        "...#qq#..#qq#...", "..##oo####oo##.."]

def person(hair="short", eyes="open", cloth=("c","v","b"), hair_c=("h","g","i"),
           beard=False, brow=True, badge=None, prop=None):
    rows = [c(r) for r in head(hair, eyes, brow, beard, hair_c) + torso(cloth, badge) + LEGS]
    return overlay(rows, prop) if prop else rows
