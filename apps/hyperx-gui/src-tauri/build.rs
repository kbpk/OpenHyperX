fn main() {
    #[cfg(feature = "desktop")]
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "gui_snapshot",
            "gui_edit",
            "gui_set_local_draft",
            "gui_reset",
            "gui_open_profile",
            "gui_save_profile",
        ]),
    ))
    .expect("Tauri configuration/permissions build failed");
}
