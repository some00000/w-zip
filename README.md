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
