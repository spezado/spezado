# Synchronisation

A set of books starts on one device. Through a **relay** it reaches the person's other devices – end-to-end
encrypted after [librotenado 0006](https://librotenado.dev/spec/0006-encryption.html): every device encrypts what it
sends with the key of the books, which only the devices of the books hold. The relay stores and passes on what it
cannot read. It sees which books it holds, which device sends when and how much, and the names of the devices –
not the books.

Books are shared the same way with other people – a tax adviser who reads, a bookkeeper who books –, each with
what they may do.

## Starting

On the device that holds the books:

```sh
spezado sync start https://relay.example.org --name Laptop
```

spezado founds the books' member list on the relay – this device and a **recovery code** – and sends every event
and original. It prints the recovery code once: 53 characters in groups of four. **Write it down and keep it safe.**
If every device is lost, it brings the books back to a new one; without it and without a device, the books are lost.
In the terminal it is `y` on *Overview*, in the web app the panel *Synchronisation* on the overview.

## Another device

On the device that holds the books, make an invitation – good for a day:

```sh
spezado sync invite
# https://relay.example.org#join=01a1….rm-H3_gG…
```

On the new device – a workspace of its own, with a device name the books do not have yet:

```sh
spezado init --device phone
spezado sync join 'https://relay.example.org#join=01a1…' --name Phone
```

The next synchronisation on the first device adds it – it checks that the request was made with its invitation –,
and the next one on the new device brings the books. `spezado serve` synchronises every minute by itself; the web app
offers *join* when setting up, the terminal shows the invitation with `v` on *Overview*.

```sh
spezado sync            # every set of books that is synchronised
spezado sync devices    # the devices of the books
```

## Other people and what they may do

An invitation says what the device it brings may do:

| Rights | |
|---|---|
| `manage` | everything: read, book, invite and remove – the person's own devices (the default) |
| `write` | read and book – a bookkeeper |
| `read` | only read – a tax adviser; on such a device, spezado refuses to book |

```sh
spezado sync invite --rights read      # for the tax adviser
```

The web app offers the same when inviting and shows what every device may do.

## Removing a device or a person

```sh
spezado sync remove Steuerberaterin    # by name or device name
```

The books get a new key, which the device removed does not receive: nothing written afterwards reaches it. What it
has read, it may have kept – removing protects the future, not the past. In the web app, every device but this one
has *remove*.

## Where the keys are

Each device keeps its key of the books in the key store of the system – the macOS keychain, the Windows credential
manager, the Secret Service on Linux. Where there is none – a server, a container –, or with `SPEZADO_KEYCHAIN=0`, it
stays in the workspace; `spezado sync start` says which. Protect the workspace's folder then.

## Recovery

When no device is left:

```sh
spezado init --device tablet
spezado --books 01a1… sync recover https://relay.example.org WFXE-TNQP-…
```

The recovery code adds the new device to the books, which then receives everything. Keep the identity of the books
with the code – `spezado books` shows it.

## Running a relay

A relay is the same program:

```sh
spezado relay --data /var/lib/spezado-relay --listen 127.0.0.1:8049
```

Or as a container – every release after 0.3.0 is also an image for Linux on amd64 and arm64,
`ghcr.io/spezado/spezado`, which runs the relay unless told otherwise. With
[deploy/compose.yaml](https://github.com/spezado/spezado/blob/main/deploy/compose.yaml) next to you and the version
of a release:

```sh
SPEZADO_VERSION=0.4.0 docker compose up -d   # the version of the release to run
```

It speaks HTTP and is meant to run behind a reverse proxy that speaks TLS, at the root of its address. With
[Caddy](https://caddyserver.com/docs/quick-starts/reverse-proxy), which fetches certificates by itself:

```text
relay.example.org {
    reverse_proxy 127.0.0.1:8049
}
```

The relay keeps its data in one SQLite database. It accepts new books from anyone who founds them, and from then on
only what their members sign; a request to join waits at most a day.
