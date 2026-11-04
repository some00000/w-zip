# w+zip

Desktop app for sealing files into a single `.wzip` archive.

Choose files, zip them, derive a key with **Argon2id**, and encrypt the zip with **AES-256-GCM**. Opening an archive checks the passphrase and writes the original files back out.

## Run

```sh
cargo run -p w-zip
```

On Linux the Tauri shell needs WebKitGTK 4.1 and the usual GTK build packages (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`).

## Test

```sh
cargo test
```

## Windows installer

The Windows build uses Tauri's NSIS bundler. It produces a per-user installer named `w+zip_<version>_x64-setup.exe`. If WebView2 is missing, the installer downloads the Evergreen bootstrapper.

On a Windows machine with Rust (1.90 or newer) and the Microsoft C++ build tools:

```powershell
cargo install tauri-cli --version 2.12.4 --locked
cargo tauri build --bundles nsis
```

Run that from the repository root. The installer is written to `src-tauri/target/release/bundle/nsis/`. NSIS itself is fetched by the Tauri bundler; you do not install it separately. Windows 11 already includes the WebView2 runtime used to compile and smoke-test the app.

GitHub Actions builds the same installer on `windows-latest` (`.github/workflows/windows-installer.yml`) for pull requests, pushes to `main`, tags named `v*`, and manual runs. Download `w-zip-windows-nsis` from the workflow run's artifacts. A `v*` tag also attaches the `.exe` to a GitHub Release.

## `.wzip` layout

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | Magic `WZIP` |
| 4 | 1 | Version `1` |
| 5 | 4 | Argon2id memory (KiB, little-endian) |
| 9 | 4 | Argon2id iterations |
| 13 | 4 | Argon2id lanes |
| 17 | 16 | Salt |
| 33 | 12 | AES-GCM nonce |
| 45 | rest | Ciphertext and 16-byte tag |

The plaintext is a zip of the selected files (basenames only). The bytes before the nonce are AES-GCM associated data, so the header is bound to the ciphertext. Interactive archives use 19 MiB of memory, 2 iterations, and 1 lane. Memory, time, and lane values outside a fixed range are rejected before key derivation.

There is no recovery if the passphrase is lost.
