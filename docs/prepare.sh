#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Brings into docs/ what the documentation shows but has its one home elsewhere: the changelog and the description of
# the API. Run before Jekyll builds docs/ – by .github/workflows/docs.yml, or by hand for a local build. The files it
# writes are not tracked.
set -eu
cd "$(dirname "$0")"

printf -- '---\ntitle: Changelog\nnav_order: 99\nedit_path: CHANGELOG.md\n---\n\n' > changelog.md
cat ../CHANGELOG.md >> changelog.md
cp ../api/openapi.json openapi.json
