mod model;
mod providers;
mod sms;
mod state;
mod store;

use std::sync::Arc;
use model::{AppSnapshot, EsimProfile};
use state::AppState;
use tauri::{Manager, State};

#[tauri::command]
fn get_snapshot(state: State<'_, Arc<AppState>>) -> AppSnapshot {
    state.snapshot()
}

#[tauri::command]
fn archive_message(id: String, archived: bool, state: State<'_, Arc<AppState>>) -> Result<AppSnapshot, String> {
    state.archive_message(&id, archived)
}

#[tauri::command]
fn mark_sender_read(sender: String, state: State<'_, Arc<AppState>>) -> Result<AppSnapshot, String> {
    state.mark_sender_read(&sender)
}

#[tauri::command]
fn upsert_esim_profile(profile: EsimProfile, state: State<'_, Arc<AppState>>) -> Result<AppSnapshot, String> {
    state.upsert_profile(profile)
}

#[tauri::command]
fn delete_esim_profile(id: String, state: State<'_, Arc<AppState>>) -> Result<AppSnapshot, String> {
    state.delete_profile(&id)
}

#[tauri::command]
fn refresh_devices(state: State<'_, Arc<AppState>>) -> AppSnapshot {
    state.refresh_devices()
}

#[tauri::command]
fn install_esim_activation_code(code: String) -> Result<String, String> {
    providers::launch_native_lpa(&code)
}

#[tauri::command]
fn decode_sms_pdu(pdu: String) -> Result<sms::pdu::DecodedPdu, String> {
    sms::pdu::decode_deliver_pdu(&pdu)
}

#[tauri::command]
fn set_window_mode(mode: String, app: tauri::AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or_else(|| "main window not found".to_string())?;
    match mode.as_str() {
        "mini" => {
            window.set_always_on_top(true).map_err(|e| e.to_string())?;
            window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(386.0, 520.0))).map_err(|e| e.to_string())?;
        }
        "full" => {
            window.set_always_on_top(false).map_err(|e| e.to_string())?;
            window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(1240.0, 790.0))).map_err(|e| e.to_string())?;
            window.center().map_err(|e| e.to_string())?;
        }
        _ => return Err("unknown window mode".into()),
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let store = store::Store::new(app.handle())?;
            let state = Arc::new(AppState::new(store)?);
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            archive_message,
            mark_sender_read,
            upsert_esim_profile,
            delete_esim_profile,
            refresh_devices,
            install_esim_activation_code,
            decode_sms_pdu,
            set_window_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CellularHub");
}
