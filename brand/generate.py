#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Generates the spezado brand kit: master SVGs, PNG exports, favicon.ico, the website's head icons and web fonts.

The mark: two folder spines of equal size side by side – ink and copper bronze as equals, debit and credit, two
poles. Each spine has a label window with a perforated edge (a receipt torn off a pad) and a grip hole with a metal
ring; the rings swap colours. The right spine stands a little higher. A small variant (three coarse perforation
dots, heavier ring) serves 16–48 px. The wordmark is "spezado" in Domitian Bold (a Palatino design, SIL OFL 1.1,
fonts/), shaped with HarfBuzz for kerning and converted to outlines – no font is needed where it is shown.

Needs rsvg-convert and hb-shape (Homebrew: librsvg, harfbuzz) and fontTools with brotli (pip install fonttools
brotli). Run from anywhere:
    python3 brand/generate.py
Edit this file, not the exports.
"""
import json, os, shutil, subprocess

from fontTools import subset
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

from brandkit import export, svg

HERE = os.path.dirname(os.path.abspath(__file__))
FONTS = os.path.join(HERE, "fonts")
NAME = "spezado"
TAGLINE = "Free accounting for everyone"

INK = "#0A2C33"           # deep-sea petrol – the left spine, text on light
NIGHT = "#071F25"         # backgrounds in dark mode, the icon tile
SURFACE = "#0D2A31"       # panels on night
COPPER = "#985A34"        # copper bronze – the right spine and accents on light (4.7 : 1 on paper)
COPPER_LIGHT = "#D59A70"  # copper bronze on dark (7.1 : 1 on night)
PAPER = "#F4EEE8"         # light backgrounds, the left spine and text on dark
MUTED = "#4E6266"         # secondary text on light (5.6 : 1 on paper)
MUTED_LIGHT = "#9FB2B4"   # secondary text on dark (7.7 : 1 on night)

DARK = dict(face=PAPER, accent=COPPER_LIGHT, bg=NIGHT, muted=MUTED_LIGHT)
LIGHT = dict(face=INK, accent=COPPER, bg=PAPER, muted=MUTED)

BOLD, ITALIC, ROMAN = (os.path.join(FONTS, f"Domitian-{s}.otf") for s in ("Bold", "Italic", "Roman"))

# --- mark (drawn in a 100×100 box) -----------------------------------------------------------------------------------

MARK_BOX = "0 0 100 100"
SPINES = ((14, 13), (54, 5))   # x and top of the left (ink) and right (copper) spine; both 32 × 80


def rounded(x, y, w, h, r):
    return (f"M{x + r},{y} H{x + w - r} A{r},{r} 0 0 1 {x + w},{y + r} V{y + h - r} A{r},{r} 0 0 1 {x + w - r},{y + h} "
            f"H{x + r} A{r},{r} 0 0 1 {x},{y + h - r} V{y + r} A{r},{r} 0 0 1 {x + r},{y} Z")


def circle(cx, cy, r):
    return f"M{cx - r},{cy} A{r},{r} 0 1 0 {cx + r},{cy} A{r},{r} 0 1 0 {cx - r},{cy} Z"


def spine(x, top, color, ring, small=False, w=32, h=80):
    """One spine with the label window and the grip hole cut out (transparent), perforation, lines and ring."""
    if small:
        label, hole_r, ring_w = (x + 6, top + 8, 20, 28), 8, 4.5
        dots = [(x + 10 + i * 6, top + 15, 2.2) for i in range(3)]
        lines = []
    else:
        label, hole_r, ring_w = (x + 5, top + 8, w - 10, h * 0.42), 7, 3.2
        dots = [(x + 8.5 + i * 5, top + 14, 1.5) for i in range(4)]
        lines = [(x + 9, top + 21, 14, 3.5), (x + 9, top + 28, 8.4, 3.5)]
    cx, cy = x + w / 2, top + h - (19 if small else 17)
    d = rounded(x, top, w, h, 5 if small else 4) + rounded(*label, 2 if small else 1.5) + circle(cx, cy, hole_r)
    out = f'<path d="{d}" fill-rule="evenodd" fill="{color}"/>'
    out += "".join(f'<circle cx="{a}" cy="{b}" r="{r}" fill="{color}"/>' for a, b, r in dots)
    out += "".join(f'<path d="{rounded(*l, 1)}" fill="{color}"/>' for l in lines)
    return out + f'<circle cx="{cx}" cy="{cy}" r="{hole_r}" fill="none" stroke="{ring}" stroke-width="{ring_w}"/>'


def mark(p=DARK, small=False):
    (lx, lt), (rx, rt) = SPINES
    return spine(lx, lt, p["face"], p["accent"], small) + spine(rx, rt, p["accent"], p["face"], small)


def flat_mark(color, small=False):
    return mark(dict(face=color, accent=color), small)


def placed(body, cx, cy, width, box=100):
    k = width / box
    return f'<g transform="translate({cx} {cy}) scale({k:.4f}) translate({-box / 2} {-box / 2})">{body}</g>'


def icon(rounded_tile=True, small=False):
    r = 224 if rounded_tile else 0
    return svg("0 0 1024 1024", f'<rect width="1024" height="1024" rx="{r}" fill="{NIGHT}"/>'
               + placed(mark(DARK, small), 512, 512, 800 if small else 740))


# --- type: Domitian, shaped by HarfBuzz, as outlines ------------------------------------------------------------------

_fonts = {}


def outline(text, font_path, size, x=0, y=0, tracking=0):
    """Text as one SVG path (baseline at y) and its advance width; kerning from the font's GPOS via hb-shape."""
    font = _fonts.setdefault(font_path, TTFont(font_path))
    glyphs, k = font.getGlyphSet(), size / font["head"].unitsPerEm
    shaped = json.loads(subprocess.run(["hb-shape", "--output-format=json", font_path, text],
                                       check=True, capture_output=True, text=True).stdout)
    pen_x, d = 0, ""
    for g in shaped:
        pen = SVGPathPen(glyphs)
        glyphs[g["g"]].draw(TransformPen(pen, (k, 0, 0, -k, x + pen_x + g["dx"] * k, y - g["dy"] * k)))
        d += pen.getCommands()
        pen_x += g["ax"] * k + tracking
    return d, pen_x - tracking


def bounds(text, font_path, size):
    """Ink bounds (x0, y0, x1, y1) of the text set at the origin, y down."""
    font = _fonts.setdefault(font_path, TTFont(font_path))
    pen = BoundsPen(None)
    glyphs = font.getGlyphSet()
    k = size / font["head"].unitsPerEm
    shaped = json.loads(subprocess.run(["hb-shape", "--output-format=json", font_path, text],
                                       check=True, capture_output=True, text=True).stdout)
    x = 0
    for g in shaped:
        glyphs[g["g"]].draw(TransformPen(pen, (k, 0, 0, -k, x + g["dx"] * k, -g["dy"] * k)))
        x += g["ax"] * k
    return pen.bounds


WORD_SIZE = 100


def wordmark_body(p=DARK, x=0, y=0):
    d, width = outline(NAME, BOLD, WORD_SIZE, x, y)
    return f'<path d="{d}" fill="{p["face"]}"/>', width


def wordmark(p=DARK):
    x0, y0, x1, y1 = bounds(NAME, BOLD, WORD_SIZE)
    body, _ = wordmark_body(p)
    pad = 6
    return svg(f"{x0 - pad:.1f} {y0 - pad:.1f} {x1 - x0 + 2 * pad:.1f} {y1 - y0 + 2 * pad:.1f}", body)


def lockup(p=DARK):
    """Mark left, wordmark right: the p's descender ends where the spines end."""
    box = 143                       # mark box for a 100 px wordmark
    gap = 18
    baseline = 0.75 * box + 4
    body, width = wordmark_body(p, box + gap, baseline)
    return svg(f"0 0 {box + gap + width + 6:.0f} {box + 8}", placed(mark(p), box / 2, box / 2 + 4, box) + body)


def social():
    """1280×640: GitHub social preview, link previews."""
    word, _ = outline(NAME, BOLD, 150, 520, 330)
    tag, _ = outline(TAGLINE, ITALIC, 44, 524, 410)
    foot, _ = outline("free software · local first · a falkOSS project", ROMAN, 26, 524, 530)
    return svg("0 0 1280 640", f'<rect width="1280" height="640" fill="{NIGHT}"/>'
               + placed(mark(DARK), 290, 320, 330) + f'<path d="{word}" fill="{PAPER}"/>'
               + f'<path d="{tag}" fill="{COPPER_LIGHT}"/><path d="{foot}" fill="{MUTED_LIGHT}"/>', (1280, 640))


def favicon():
    """The small mark, transparent: paper and light copper on dark tabs, ink and copper on light ones."""
    style = (f'<style>.f{{fill:{PAPER}}}.a{{fill:{COPPER_LIGHT}}}.rf{{stroke:{PAPER}}}.ra{{stroke:{COPPER_LIGHT}}}'
             f'@media (prefers-color-scheme: light){{.f{{fill:{INK}}}.a{{fill:{COPPER}}}.rf{{stroke:{INK}}}'
             f'.ra{{stroke:{COPPER}}}}}</style>')
    body = (mark(DARK, small=True).replace(f'fill="{PAPER}"', 'class="f"').replace(f'fill="{COPPER_LIGHT}"', 'class="a"')
            .replace(f'stroke="{PAPER}"', 'class="rf"').replace(f'stroke="{COPPER_LIGHT}"', 'class="ra"'))
    return svg(MARK_BOX, style + body)


LATIN = "U+0000-024F,U+0300-036F,U+1E00-1EFF,U+2000-206F,U+20A0-20CF,U+2100-214F,U+2190-21FF,U+2212"


def web_fonts():
    """WOFF2 copies for the website (headings in Domitian; body text uses the system font), subset to Latin –
    every European language written in Latin script, punctuation, currency signs and arrows."""
    out = os.path.join(HERE, "web", "fonts")
    os.makedirs(out, exist_ok=True)
    for path in (ROMAN, BOLD, ITALIC):
        dst = os.path.join(out, os.path.basename(path).replace(".otf", ".woff2"))
        subset.main([path, f"--unicodes={LATIN}", "--layout-features=*", "--flavor=woff2", f"--output-file={dst}"])
    shutil.copy(os.path.join(FONTS, "OFL.txt"), out)


def main():
    kit = {
        "icon": icon(),
        "icon-square": icon(rounded_tile=False),
        "icon-small": icon(small=True),
        "mark-on-dark": svg(MARK_BOX, mark(DARK)),
        "mark-on-light": svg(MARK_BOX, mark(LIGHT)),
        "mark-black": svg(MARK_BOX, flat_mark("#000000")),
        "mark-white": svg(MARK_BOX, flat_mark("#FFFFFF")),
        "wordmark-on-dark": wordmark(DARK),
        "wordmark-on-light": wordmark(LIGHT),
        "lockup-on-dark": lockup(DARK),
        "lockup-on-light": lockup(LIGHT),
        "social": social(),
        "favicon": favicon(),
    }
    export(HERE, NAME, kit, NIGHT)
    web_fonts()


if __name__ == "__main__":
    main()
