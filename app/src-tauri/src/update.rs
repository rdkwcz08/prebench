// app/src-tauri/src/update.rs — Mises à jour de l'app (section 12.5).
//
// Le plugin updater de Tauri lit latest.json dans la dernière GitHub Release du
// dépôt public et vérifie la signature de la mise à jour avec la clé publique
// du projet (tauri.conf.json) avant toute installation. Hors ligne, la
// vérification échoue sans bruit : l'app reste utilisable (D10).

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::error::{CoreError, CoreResult};

#[derive(Serialize)]
pub struct UpdateInfo {
    version: String,
    notes: Option<String>,
}

fn update_error(e: tauri_plugin_updater::Error) -> CoreError {
    CoreError::Update(e.to_string())
}

/// Version plus récente disponible, ou None.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> CoreResult<Option<UpdateInfo>> {
    let update = app
        .updater()
        .map_err(update_error)?
        .check()
        .await
        .map_err(update_error)?;
    Ok(update.map(|u| UpdateInfo {
        version: u.version,
        notes: u.body,
    }))
}

/// Télécharge, vérifie la signature, installe, puis redémarre l'app.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> CoreResult<()> {
    let update = app
        .updater()
        .map_err(update_error)?
        .check()
        .await
        .map_err(update_error)?
        .ok_or_else(|| CoreError::Update("aucune mise à jour disponible".into()))?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(update_error)?;
    app.restart();
}
