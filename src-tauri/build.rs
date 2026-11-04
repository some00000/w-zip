fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "pick_files",
                "pick_save_path",
                "pick_wzip",
                "pick_folder",
                "pack_wzip",
                "unpack_wzip",
            ]),
        ),
    )
    .expect("failed to run tauri-build");
}
