# Spotter

A fast Spotlight-style launcher for Linux, written in Rust.

## Features

- Native GTK command palette UI.
- Searches installed desktop apps from `.desktop` files.
- Searches executable commands from `PATH`.
- Indexes configured home folders and supports absolute paths.
- Parallel fuzzy search with a small bounded result list for responsive typing.
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

Default config:

```toml
max_results = 9
max_result_height = 420
max_indexed_items = 60000
index_depth = 5
include_hidden = false

index_dirs = [
  "Desktop",
  "Documents",
  "Downloads",
  "Pictures",
  "Music",
  "Videos",
]

[ui]
position = "top-left"
x = 96
y = 72
window_width = 720
result_max_height = 420
shell_margin = 24
shell_padding = 18
shell_radius = 18
search_height = 54
search_radius = 12
search_font_size = 24
result_margin_top = 12
result_row_padding_y = 10
result_row_padding_x = 14
result_row_radius = 10
title_font_size = 16
subtitle_font_size = 12
icon_font_size = 20

[ui.colors]
window_background = "transparent"
shell_background = "rgba(28, 31, 36, 0.96)"
shell_border = "rgba(255, 255, 255, 0.14)"
search_background = "rgba(255, 255, 255, 0.1)"
search_text = "#f5f7fa"
results_background = "transparent"
row_background = "transparent"
row_selected_background = "rgba(108, 160, 255, 0.28)"
icon = "#9fb7ff"
title = "#f5f7fa"
subtitle = "#aeb6c2"
```

`index_dirs` accepts paths relative to your home directory, `~/...` paths, or absolute paths.
`position`, `x`, and `y` store the preferred placement. On Sway, Spotter uses `swaymsg` to make the launcher floating and move it to the configured coordinates. Other Wayland compositors may ignore exact placement unless they expose a compositor-specific positioning command.

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

The first launch builds the in-memory index on a background thread. Search is available immediately and results fill in as indexing completes.
