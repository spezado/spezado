# SPDX-License-Identifier: AGPL-3.0-or-later
# The image of spezado for servers – a relay, or `spezado serve --listen` –, built by the release workflow from the
# Linux binaries it builds anyway: the build context holds them as spezado-amd64 and spezado-arm64. No RUN step, so
# that every platform builds without emulation. The base is Debian trixie (its glibc is new enough for binaries built
# on Ubuntu 24.04), pinned by digest.
FROM debian:trixie-slim@sha256:f6b08b09a05725f9431502d3b5714f5c4576e82c1a84fa1debe0c24a7e0c3ec4

ARG TARGETARCH
ARG VERSION=dev
LABEL org.opencontainers.image.title="spezado" \
      org.opencontainers.image.description="Free accounting for everyone – the relay and the server" \
      org.opencontainers.image.source="https://github.com/spezado/spezado" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later" \
      org.opencontainers.image.version="${VERSION}"

COPY --chmod=0755 spezado-${TARGETARCH} /usr/local/bin/spezado
COPY LICENSE README.md /usr/share/doc/spezado/
COPY LICENSES /usr/share/doc/spezado/LICENSES

# Not root; the data in a volume the user owns.
COPY --chown=65532:65532 --chmod=0700 data /data
USER 65532:65532
VOLUME /data
EXPOSE 8049
ENTRYPOINT ["spezado"]
CMD ["relay", "--data", "/data", "--listen", "0.0.0.0:8049"]
