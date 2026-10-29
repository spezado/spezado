# spezado – brand kit

**The mark:** two folder spines of equal size side by side – deep-sea petrol and copper bronze as equals: debit and
credit, two poles of one set of books. Each spine has a label window with a perforated edge, like a receipt torn off a
pad, and a grip hole with a metal ring; the rings swap colours. The right spine stands a little higher.

- **Regular** – label with perforation and two lines; 64 px and up.
- **Small** – three coarse perforation dots, no lines, heavier ring; 16–48 px (favicon, tabs, small app slots).

**The wordmark:** "spezado" in **Domitian Bold**, a Palatino design (see Type), shaped with HarfBuzz for kerning and
converted to outlines – no font is needed where it is shown. The tagline is set in Domitian Italic.

Everything here is generated: edit `generate.py` (the drawing) – `brandkit.py` (the export, from
[falkoss/starterpack](https://github.com/falkoss/starterpack)) stays as it is – then run `python3 brand/generate.py`.
It needs `rsvg-convert` and `hb-shape` (Homebrew: `librsvg`, `harfbuzz`) and `fontTools` with `brotli`
(`pip install fonttools brotli`), and writes `svg/`, `png/` and `web/`.

## Colours

| Name | Hex | Use |
|---|---|---|
| Ink (deep-sea petrol) | `#0A2C33` | the left spine and text on light |
| Night | `#071F25` | backgrounds in dark mode, the icon tile |
| Surface | `#0D2A31` | panels on night |
| Copper bronze | `#985A34` | the right spine and accents on light – 4.7 : 1 on paper |
| Copper bronze light | `#D59A70` | the right spine and accents on dark – 7.1 : 1 on night |
| Paper | `#F4EEE8` | light backgrounds, the left spine and text on dark |
| Muted | `#4E6266` | secondary text on light – 5.6 : 1 on paper |
| Muted light | `#9FB2B4` | secondary text on dark – 7.7 : 1 on night |

Text on paper reaches 12.8 : 1, text on night 14.8 : 1 (WCAG 2.2: 4.5 : 1 for body text, AA). Ink and copper are
equals – never one as the "main" colour and the other as a highlight. No gradients, no glow.

## Type

**Domitian** (Daniel Benjamin Miller, based on URW's Palladio from the Core 35 fonts, a Palatino design by Hermann
Zapf) for the wordmark, the tagline and headings. It is offered under the AGPL 3 with a font exception, the LPPL 1.3c
or the SIL Open Font License 1.1; spezado uses it under the **SIL Open Font License 1.1** (`fonts/OFL.txt`). Body text
uses the system sans-serif. Websites bundle the WOFF2 files from `web/fonts/`; no font is loaded from other servers.

## Name

Always written in lowercase – **spezado**, also at the start of a sentence, in titles and in the wordmark; the same in
handles and domains.

## Where to use what

| Use | File |
|---|---|
| Website `<head>` (copy to the website) | `web/` – favicon.svg (follows light/dark), favicon.ico, apple-touch-icon.png, icon-192/512.png, social.png, site.webmanifest; `web/fonts/` for headings |
| GitHub organisation avatar | `png/social/github-avatar-500.png` |
| GitHub social preview, link previews | `png/social/spezado-social-1280x640.png` |
| Headers, documents, profile | `svg/spezado-lockup-on-light.svg`, `-on-dark.svg` (PNG: `png/lockup/`) |
| Wordmark alone | `svg/spezado-wordmark-on-light.svg`, `-on-dark.svg` |
| Mark alone (transparent) | `svg/spezado-mark-on-light.svg`, `-on-dark.svg`; one colour: `-black.svg`, `-white.svg` |
| App icon | `png/icon/` (rounded), `png/icon-square/` (edge to edge, for platforms that round it themselves), `png/icon-small/` (16–48 px) |

## Rights

spezado's name and brand belong to falkOSS – Falcon Lake Open Source Software gUG (haftungsbeschränkt) i. G., the
publisher of spezado. They are **not** covered by the software licence (AGPL-3.0-or-later). The look stays spezado's
own; project pages say "a falkOSS project" at most. The artwork may be used to refer to spezado, not for other
products or in a way that suggests endorsement.
