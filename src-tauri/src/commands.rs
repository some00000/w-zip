use std::path::PathBuf;

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::archive::{self, ExportReport};
use crate::error::Error;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackResponse {
    destination: String,
    file_count: usize,
    byte_count: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnpackResponse {
    destination: String,
    files: Vec<String>,
}

fn path_string(path: FilePath) -> Result<String, String> {
    path.into_path()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|err| err.to_string())
}

fn paths_from(files: Option<Vec<FilePath>>) -> Result<Vec<String>, String> {
    files
        .unwrap_or_default()
        .into_iter()
        .map(path_string)
        .collect()
}

#[tauri::command]
pub async fn pick_files(app: AppHandle) -> Result<Vec<String>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Select files to seal")
        .blocking_pick_files();
    paths_from(picked)
}

#[tauri::command]
pub async fn pick_save_path(app: AppHandle) -> Result<Option<String>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Export .wzip")
        .add_filter("w+zip archive", &["wzip"])
        .set_file_name("archive.wzip")
        .blocking_save_file();
    match picked {
        Some(path) => Ok(Some(path_string(path)?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn pick_wzip(app: AppHandle) -> Result<Option<String>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Open a .wzip archive")
        .add_filter("w+zip archive", &["wzip"])
        .blocking_pick_file();
    match picked {
        Some(path) => Ok(Some(path_string(path)?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Choose a folder for the extracted files")
        .blocking_pick_folder();
    match picked {
        Some(path) => Ok(Some(path_string(path)?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn pack_wzip(sources: Vec<String>, password: String, destination: String) -> Result<PackResponse, String> {
    let sources: Vec<PathBuf> = sources.into_iter().map(PathBuf::from).collect();
    let report: ExportReport =
        archive::export_wzip(&sources, &password, PathBuf::from(destination).as_path())
            .map_err(|err: Error| err.to_string())?;
    Ok(PackResponse {
        destination: report.destination.display().to_string(),
        file_count: report.file_count,
        byte_count: report.byte_count,
    })
}

#[tauri::command]
pub fn unpack_wzip(
    archive: String,
    password: String,
    destination: String,
) -> Result<UnpackResponse, String> {
    let destination_path = PathBuf::from(&destination);
    let files = archive::import_wzip(
        PathBuf::from(archive).as_path(),
        &password,
        destination_path.as_path(),
    )
    .map_err(|err: Error| err.to_string())?;
    Ok(UnpackResponse {
        destination: destination_path.display().to_string(),
        files,
    })
}
