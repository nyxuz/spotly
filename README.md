# Spotly

A lightweight terminal Spotify lyrics client written in Rust.

Spotly displays synchronized lyrics directly in your terminal while you listen to music with Spotify.

[![CI](https://github.com/nyxuz/spotly/actions/workflows/ci.yml/badge.svg)](https://github.com/nyxuz/spotly/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/nyxuz/spotly)](LICENSE)

![Spotly screenshot](assets/Spotly.png)

## Features

* Spotify track detection through MPRIS
* Synchronized lyrics from LRCLIB
* Word-by-word lyric display
* Local playback clock for smoother synchronization
* Automatic handling of pause, resume, seek, and track changes
* `♪` fallback when lyrics are unavailable
* Optional synchronization debug output
* Lightweight terminal-only interface

## Requirements

* Linux
* Spotify
* MPRIS support
* Rust and Cargo

## Installation

Clone the repository and install Spotly with Cargo:

```bash
git clone https://github.com/nyxuz/spotly.git
cd spotly
cargo install --path . --locked
```

> If `spotly` is not found after installation, make sure `$HOME/.cargo/bin` is in your `PATH`.

## Usage

Start Spotify and play a song, then run:

```bash
spotly
```

Press `q` or `Q` to quit.

## Debugging

Enable synchronization debugging:

```bash
spotly --debug
```

For playback clock diagnostics:

```bash
spotly --debug-clock
```

To keep debug output separate from the terminal UI:

```bash
spotly --debug 2> /tmp/spotly-debug.log
tail -f /tmp/spotly-debug.log
```

## Lyrics

Spotly retrieves lyrics from LRCLIB.

Lyrics availability and synchronization accuracy depend on the data provided by LRCLIB. Some tracks may only have plain lyrics, while others may not have lyrics available.

## Development

Run tests:

```bash
cargo test
```

Check the project:

```bash
cargo check
```

Check formatting:

```bash
cargo fmt --check
```

## License

Spotly is licensed under the MIT License.