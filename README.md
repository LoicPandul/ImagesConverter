<p align="center">
  <img src="assets/logo.png" width="90" alt="ImagesConverter logo" />
</p>

<h1 align="center">ImagesConverter</h1>

<p align="center">
  Convert, compress and clean metadata from your images — fast, private, fully offline.
</p>

<p align="center">
  <img src="assets/screenshot.png" width="720" alt="ImagesConverter screenshot" />
</p>

## Download

Grab the latest installer from the [Releases](https://github.com/LoicPandul/ImagesConverter/releases) page. These builds are not signed (code signing certificates cost money and add nothing to the code), so your OS will warn you on first launch:

| Platform | File | First launch |
|---|---|---|
| Windows | `*-setup.exe` or `.msi` | SmartScreen warns: click "More info", then "Run anyway" |
| macOS | `.dmg` | Right-click the app, then "Open" |
| Linux | `.AppImage` (portable), `.deb` or `.rpm` | Nothing special, `chmod +x` the AppImage |

## Features

- **Convert** JPEG, PNG, WEBP, GIF, BMP and TIFF images to **JPEG**, **WEBP** or **PNG** — drop files anywhere in the window, or browse.
- **Metadata always removed** (EXIF, GPS, XMP, ICC, comments). When the file is already in the target format, metadata is stripped **losslessly** — pixels are never re-encoded.
- **Compress to a size budget**: give a max size in KB and the app finds the best quality that fits (binary search on quality, then downscaling as a last resort). Lossy PNG uses built-in palette quantization — no external tools.
- **EXIF orientation applied** before stripping, so rotated phone photos come out upright.
- **Batch & parallel**: every file is processed on its own CPU core.
- **Never overwrites**: existing files get a numbered suffix; originals are only deleted after the output is fully written (and only if you keep "Delete originals" on).
- Native app on Windows, macOS and Linux — a few MB, starts instantly, no network access at all.

## Build from source

Requires [Rust](https://rustup.rs/).

```powershell
cd src-tauri
cargo run              # development
cargo build --release  # target/release/imagesconverter(.exe)
```

The frontend (`ui/`) is plain HTML, CSS and JavaScript: no Node, no build step.

## License

Released into the public domain under the [Unlicense](LICENSE).
