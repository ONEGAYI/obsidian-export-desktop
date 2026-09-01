# Installation

## Pre-built binaries

Pre-compiled CLI binaries, as well as the graphical desktop installer for Windows, are available at <https://github.com/ONEGAYI/obsidian-export-desktop/releases>

The desktop app bundles the CLI as its sidecar: installing the desktop app is enough, a separate CLI install is only needed if you want to use it from a terminal.

## Building from source

When binary releases are unavailable for your platform, or you do not trust the pre-built binaries, then _obsidian-export_ can be compiled from source with relatively little effort.
This is done through [Cargo], the official package manager for Rust, with the following steps:

1. Install the Rust toolchain from <https://www.rust-lang.org/tools/install>
2. Clone this repository
3. Run `cargo install --path .` from the repository root

> It is expected that you successfully configured the PATH variable correctly while installing the Rust toolchain, as described under _"Configuring the PATH environment variable"_ on <https://www.rust-lang.org/tools/install>.

## Upgrading from earlier versions

If you downloaded a pre-built binary, upgrade by downloading the latest version to replace the old one — or let the CLI fetch it for you with `obsidian-export update --download`.

If you built from source, upgrade by pulling the latest changes and running `cargo install --path .` again.

[Cargo]: https://doc.rust-lang.org/cargo/
