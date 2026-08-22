# Changelog

All notable changes to Spotter are documented in this file.

## [Unreleased]

### Added

- Optional StatusNotifierItem system tray mode with a search icon, click-to-open
  behavior, and Open/Quit menu actions.
- An `include_path_binaries` option for indexing executable files from `PATH`,
  disabled by default.

### Fixed

- Prevent the launcher from briefly appearing at the compositor's default
  position before moving to its configured Sway position.

### Changed

- Show recently launched applications instead of query strings when the input
  is empty, while retaining query history for Readline navigation.
- Keep launched web fallbacks in the recent list with an explicit web-search
  label.

## [v0.2] - 2026-08-16

### Added

- Make targets for release builds and configurable installation paths.
- Annotated `config.example.toml` with supported UI values and color formats.
- Original application icons from desktop entries, with a glyph fallback when
  an icon is unavailable.
- Persistent, deduplicated recent searches shown newest-first when the search
  input is empty.
- GNU Readline-style cursor movement, deletion, kill/yank, transpose, and
  recent-history keyboard commands in the search input.

### Fixed

- Publish application and command indexes before filesystem scanning completes,
  and refresh an active query automatically instead of showing a premature web
  fallback.
- Apply anchored Sway positions, exact configured width, and updated placement
  after the result pane changes height.
- Prevent GTK theme backgrounds from overriding configured UI colors.
- Clip the GTK window to the configured shell radius to remove square corner
  residue around the launcher.
- Show malformed configuration errors in the launcher instead of silently
  ignoring the entire file.

### Changed

- Split the application into focused configuration, search, AI, platform, and
  GTK UI modules.

### Removed

- Multiple build name references

## [v0.1] - 2026-08-12

### Added

- Native GTK4 launcher interface with configurable layout and colors.
- Indexing for desktop applications, executable commands, files, and directories.
- Parallel ranked search with keyboard navigation and a bounded result list.
- Google search fallback when a query has no local matches.
- OpenAI-compatible AI prompts for queries beginning with `/`, with support for
  configured API keys or the `OPENAI_API_KEY` environment variable.
- Configurable Sway window positioning.
