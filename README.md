# Rustfin [![CI](https://github.com/chrisby4563/rustfin/actions/workflows/ci.yml/badge.svg)](https://github.com/chrisby4563/rustfin/actions/workflows/ci.yml)

A native Jellyfin client for the Linux desktop, written in Rust with GTK4,
libadwaita and an embedded mpv video player.

Rustfin is a fork of [Delfin](https://codeberg.org/avery42/delfin) by Avery.
Many thanks for the solid foundation. It is not affiliated with the Delfin or
Jellyfin projects; please report Rustfin issues here, not upstream.

> **An AI-assisted project.** The changes on top of Delfin were written almost
> entirely by AI coding assistants (mainly Claude Code, some help from Codex),
> directed, tested and reviewed by a human on real hardware and a real Jellyfin
> server. Expect the occasional rough edge, and read the code with that in mind.
>
> **Forks are welcome.** Take it, change it, make it your own: the license
> (GPL-3.0-or-later) only asks that your changes stay open as well.

## Features

Everything Delfin offers, plus:

- **Watch together (SyncPlay)**: create or join groups, compatible with the
  Jellyfin web client
- **Redesigned interface**: home page with a rotating "continue watching" hero,
  detail pages with backdrop, season pills and episode cards, library grid with
  filter bar and A–Z letter bar, skeleton placeholders and fade-ins
- **Library tools**: sorting, filters (unwatched, genres), search, mark as
  watched, delete media (with confirmation)
- **Playback**: chapters with thumbnails, multiple versions per item, resume or
  start over, media info card (video, audio, subtitles, file, transcoding),
  server-side audio track switching, quality selection, mini player (Hyprland)
- **Segments**: skip or ask for intro, recap, outro and preview separately
- **Jellyfin account settings**: audio and subtitle languages, subtitle mode,
  home sections, next-up options, hidden libraries, password change and Quick
  Connect, all stored on the server like in the web client
- Colors follow the desktop theme (libadwaita named colors; live reload of the
  Omarchy theme when present)
- Access tokens are masked in all log output

Not planned: music, live TV, server administration.

## Building

Dependencies: Rust (stable), meson, GTK4, libadwaita ≥ 1.4, mpv ≥ 2.1,
libepoxy, appstream.

```sh
meson setup _build -Dprofile=release
ninja -C _build
./_build/delfin/rustfin
```

The [mold](https://github.com/rui314/mold/) linker is used automatically when
available and speeds up builds considerably.

### Arch Linux

```sh
cd packaging/arch
makepkg -si
```

### Migrating from Delfin

On first start Rustfin copies `~/.config/delfin/config.toml` to
`~/.config/rustfin/`, so your servers and accounts carry over. Delfin's file is
left untouched.

## Fonts

The interface uses [Inter](https://rsms.me/inter/) and
[Space Grotesk](https://fonts.google.com/specimen/Space+Grotesk) when
installed and falls back to the system font otherwise.

## Jellyfin API

`jellyfin_api/` contains Rust types generated from the Jellyfin 12.4 OpenAPI
description; see [jellyfin_api/README.md](jellyfin_api/README.md) for how to
regenerate them.

## License

GPL-3.0-or-later, like Delfin. Original code © Avery and Delfin contributors;
fork changes © chrisby4563.
