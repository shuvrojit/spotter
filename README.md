# Spotter

A fast application launcher for Linux, written in Rust.

## Features

- Native GTK command palette UI.
- Searches installed desktop apps from `.desktop` files.
- Shows each application's original themed or file-based icon when available.
- Searches executable commands from `PATH`.
- Indexes configured home folders and supports absolute paths.
- Parallel fuzzy search with a small bounded result list for responsive typing.
- Persists recent searches and shows them newest-first when the input is empty.
- Opens apps, shell commands, files, and directories with `Enter`.
- Loads configuration from `~/.config/spotter/config.toml`.

## Run

```sh
cargo run --release
```

## Build

```sh
make
```

The binary is created at:

```sh
target/release/spotter
```

You can also build it directly with `cargo build --release --bin spotter`.

## Install

Install system-wide under `/usr/local/bin`:

```sh
sudo make install
```

For a per-user installation, use:

```sh
make install PREFIX="$HOME/.local"
```

`PREFIX`, `BINDIR`, and `DESTDIR` can be overridden for packaging or custom
installation layouts.

## Configuration

Spotter creates this file on first launch:

```sh
~/.config/spotter/config.toml
```

The annotated [config.example.toml](config.example.toml) is also compiled into
the binary as the first-launch default. Copy it to start from a fresh config:

```sh
mkdir -p "$HOME/.config/spotter"
cp config.example.toml "$HOME/.config/spotter/config.toml"
```

Sway supports all documented anchors, including `top-right`, and applies
`window_width` exactly. The result pane grows naturally up to
`result_max_height`. Other Wayland compositors control placement unless they
provide a compatible positioning API.

Invalid TOML cannot be applied. Spotter shows the parse error in the launcher
and uses defaults until the file is corrected. Restart Spotter after editing
configuration values.

Set `max_recent_searches` to control how many queries are retained, or set it
to `0` to disable search history. History is stored locally at
`~/.local/share/spotter/recent-searches.json` by default. Selecting a recent
query restores it so you can review the results before launching anything.

## Global Shortcut

On Linux, global shortcuts are desktop-environment specific, especially under Wayland. Bind your preferred shortcut to:

```sh
spotter
```

Examples:

- GNOME: Settings -> Keyboard -> View and Customize Shortcuts -> Custom Shortcuts.
- KDE Plasma: System Settings -> Shortcuts -> Custom Shortcuts.
- i3/sway: bind a key to `exec spotter`.

## Notes

The first launch builds the in-memory index on a background thread. Application
and command results are published first, followed by filesystem entries. An
active query refreshes automatically as each indexing stage completes.
