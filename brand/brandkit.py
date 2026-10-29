# SPDX-License-Identifier: AGPL-3.0-or-later
"""The export half of a falkOSS brand kit, from falkoss/starterpack – the same set of files for every project.

generate.py draws the project's own marks as SVG strings and hands them to export(), which writes:

    svg/<name>-<key>.svg     every master, plus svg/favicon.svg
    png/icon/                rounded icon, 64–1024 px        png/icon-square/  edge to edge, 64–1024 px
    png/icon-small/          16–48 px                        png/mark/         marks, 256–1024 px
    png/wordmark/, lockup/   1200 px wide                    png/social/       GitHub avatar (500), social preview (1280×640)
    web/                     favicon.svg, favicon.ico, apple-touch-icon.png, icon-192/512.png, social.png, site.webmanifest

Copy web/ into the website repository, the lockups into the organisation profile. Needs rsvg-convert (Homebrew:
librsvg). Keep this file as it is; changes that help every project go back to falkoss/starterpack.
"""
import os, shutil, struct, subprocess

# Keys export() expects in the kit, and the PNG sizes it renders from them.
KEYS = ("icon", "icon-square", "icon-small", "mark-on-dark", "mark-on-light", "mark-black", "mark-white",
        "wordmark-on-dark", "wordmark-on-light", "lockup-on-dark", "lockup-on-light", "social", "favicon")


def svg(view_box, body, size=None):
    dims = f' width="{size[0]}" height="{size[1]}"' if size else ""
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}"{dims}>\n{body}\n</svg>\n'


def write(path, content):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(content)


def render(src, dst, w, h=None):
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    args = ["rsvg-convert", "-w", str(w)] + (["-h", str(h)] if h else []) + ["-o", dst, src]
    subprocess.run(args, check=True)


def ico(dst, pngs):
    """favicon.ico with PNG entries; pngs is a list of (size, path)."""
    data = [open(p, "rb").read() for _, p in pngs]
    out = struct.pack("<HHH", 0, 1, len(data))
    offset = 6 + 16 * len(data)
    for (size, _), d in zip(pngs, data):
        out += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(d), offset)
        offset += len(d)
    with open(dst, "wb") as f:
        f.write(out + b"".join(data))


def manifest(name, color):
    return ('{\n'
            f'  "name": "{name}",\n  "short_name": "{name}",\n  "icons": [\n'
            '    {"src": "/icon-192.png", "sizes": "192x192", "type": "image/png"},\n'
            '    {"src": "/icon-512.png", "sizes": "512x512", "type": "image/png"}\n  ],\n'
            f'  "theme_color": "{color}",\n  "background_color": "{color}",\n  "display": "standalone"\n}}\n')


def export(here, name, kit, theme_color):
    """Writes the whole kit below `here` (svg/, png/, web/ are generated only and replaced every time)."""
    missing = [k for k in KEYS if k not in kit]
    if missing:
        raise SystemExit(f"brand kit is missing: {', '.join(missing)}")
    SVG, PNG, WEB = (os.path.join(here, d) for d in ("svg", "png", "web"))
    for d in (SVG, PNG, WEB):
        if os.path.isdir(d):
            shutil.rmtree(d)

    def s(key):
        return os.path.join(SVG, "favicon.svg" if key == "favicon" else f"{name}-{key}.svg")

    for key in KEYS:
        write(s(key), kit[key])
    png = lambda folder, file: os.path.join(PNG, folder, file)
    for size in (64, 128, 256, 512, 1024):
        render(s("icon"), png("icon", f"{name}-icon-{size}.png"), size)
        render(s("icon-square"), png("icon-square", f"{name}-icon-square-{size}.png"), size)
    for size in (16, 32, 48):
        render(s("icon-small"), png("icon-small", f"{name}-icon-small-{size}.png"), size)
    for size in (256, 512, 1024):
        for v in ("on-dark", "on-light", "black", "white"):
            render(s(f"mark-{v}"), png("mark", f"{name}-mark-{v}-{size}.png"), size)
    for v in ("on-dark", "on-light"):
        render(s(f"wordmark-{v}"), png("wordmark", f"{name}-wordmark-{v}-1200.png"), 1200)
        render(s(f"lockup-{v}"), png("lockup", f"{name}-lockup-{v}-1200.png"), 1200)
    render(s("icon-square"), png("social", "github-avatar-500.png"), 500)
    render(s("social"), png("social", f"{name}-social-1280x640.png"), 1280, 640)

    os.makedirs(WEB, exist_ok=True)
    shutil.copy(s("favicon"), os.path.join(WEB, "favicon.svg"))
    render(s("icon-square"), os.path.join(WEB, "apple-touch-icon.png"), 180)
    render(s("icon"), os.path.join(WEB, "icon-192.png"), 192)
    render(s("icon"), os.path.join(WEB, "icon-512.png"), 512)
    shutil.copy(png("social", f"{name}-social-1280x640.png"), os.path.join(WEB, "social.png"))
    ico(os.path.join(WEB, "favicon.ico"), [(n, png("icon-small", f"{name}-icon-small-{n}.png")) for n in (16, 32, 48)])
    write(os.path.join(WEB, "site.webmanifest"), manifest(name, theme_color))
    print("brand kit written to", here)
