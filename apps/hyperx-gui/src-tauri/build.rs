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
            "gui_overwrite_profile",
            "gui_undo_file_edit",
            "gui_redo_file_edit",
            "gui_restore_recovery",
            "gui_discard_recovery",
        ]),
    ))
    .expect("Tauri configuration/permissions build failed");
}
