#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Builds the documentation into docs/book: the book (mdBook) with the brand's fonts and icons, and the description of
# the API (api/openapi.json) to download beside it.
set -eu
cd "$(dirname "$0")"

mkdir -p src/fonts
cp ../brand/web/fonts/*.woff2 ../brand/web/fonts/OFL.txt src/fonts/
cp ../brand/web/favicon.svg theme/favicon.svg
cp ../brand/web/icon-192.png theme/favicon.png
cp ../api/openapi.json src/openapi.json

mdbook build
