mod backend;
mod conversation_store;
mod esim_store;
mod manager;
mod model;
mod notification_center;
mod storage;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use conversation_store::ConversationStore;
use esim_store::EsimStore;
use manager::CellularManager;
use model::{
    AtConnectRequest, CellularSnapshot, ConversationMeta, DeviceInfo, EsimCompatInstallRequest, EsimCompatReport,
    EsimProfile, SmsMessage, HealthCheckItem, ReleaseDiagnostics,
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, State,
};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;

#[derive(Default)]
struct RuntimePrefs {
    close_to_tray: AtomicBool,
}


fn apply_window_mode<R: Runtime>(window: &tauri::WebviewWindow<R>, mode: &str) -> Result<(), String> {
    match mode {
        "mini" => {
            const W: u32 = 386;
            const H: u32 = 520;
            window.set_resizable(false).map_err(|e| e.to_string())?;
            window.set_decorations(false).map_err(|e| e.to_string())?;
            window.set_always_on_top(true).map_err(|e| e.to_string())?;
            window.set_size(PhysicalSize::new(W, H)).map_err(|e| e.to_string())?;
            if let Ok(Some(monitor)) = window.current_monitor() {
                let size = monitor.size();
                let origin = monitor.position();
                let x = origin.x + size.width as i32 - W as i32 - 18;
                // Reserve a little room for the Windows taskbar.
                let y = origin.y + size.height as i32 - H as i32 - 66;
                let _ = window.set_position(PhysicalPosition::new(x.max(origin.x), y.max(origin.y)));
            }
        }
        "full" => {
            window.set_always_on_top(false).map_err(|e| e.to_string())?;
            window.set_decorations(true).map_err(|e| e.to_string())?;
            window.set_resizable(true).map_err(|e| e.to_string())?;
            window.set_size(PhysicalSize::new(1240, 790)).map_err(|e| e.to_string())?;
            let _ = window.center();
        }
        _ => return Err("未知窗口模式".into()),
    }
    let _ = window.emit("window-mode-changed", mode.to_string());
    Ok(())
}

pub(crate) fn reveal_window<R: Runtime>(app: &tauri::AppHandle<R>, mode: &str, section: Option<&str>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = apply_window_mode(&window, mode);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        if let Some(section) = section {
            let _ = window.emit("navigate-section", section.to_string());
        }
    }
}

#[tauri::command]
fn get_snapshot(manager: State<'_, Arc<CellularManager>>) -> CellularSnapshot {
    manager.snapshot()
}

#[tauri::command]
fn list_devices() -> Vec<DeviceInfo> {
    let mut devices = backend::windows_mbn::discover();
    devices.extend(backend::at::discover());
    devices
}

#[tauri::command]
fn list_messages(manager: State<'_, Arc<CellularManager>>) -> Vec<SmsMessage> {
    manager.messages()
}

#[tauri::command]
fn list_conversation_meta(store: State<'_, Arc<ConversationStore>>) -> Vec<ConversationMeta> {
    store.list()
}

#[tauri::command]
fn save_conversation_meta(
    store: State<'_, Arc<ConversationStore>>,
    meta: ConversationMeta,
) -> Result<ConversationMeta, String> {
    store.save(meta)
}

#[tauri::command]
fn copy_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
fn mark_message_read(manager: State<'_, Arc<CellularManager>>, id: String) {
    manager.mark_read(&id);
}

#[tauri::command]
fn mark_all_messages_read(manager: State<'_, Arc<CellularManager>>) {
    manager.mark_all_read();
}

#[tauri::command]
fn mark_conversation_read(manager: State<'_, Arc<CellularManager>>, sender: String) {
    manager.mark_sender_read(&sender);
}

#[tauri::command]
fn set_conversation_archived(
    manager: State<'_, Arc<CellularManager>>,
    sender: String,
    archived: bool,
) {
    manager.set_sender_archived(&sender, archived);
}

#[tauri::command]
fn set_message_archived(manager: State<'_, Arc<CellularManager>>, id: String, archived: bool) {
    manager.set_archived(&id, archived);
}

fn unread_tray_icon(unread: usize) -> Option<tauri::image::Image<'static>> {
    let bytes: &'static [u8] = match unread {
        1 => include_bytes!("../icons/tray-unread-1.png"),
        2 => include_bytes!("../icons/tray-unread-2.png"),
        3 => include_bytes!("../icons/tray-unread-3.png"),
        4 => include_bytes!("../icons/tray-unread-4.png"),
        5 => include_bytes!("../icons/tray-unread-5.png"),
        6 => include_bytes!(".../icons/tray-unread-6.png"),
        7 => include_bytes!("../icons/tray-unread-7.png"),
        8 => include_bytes!("../icons/tray-unread-8.png"),
        9 => include_bytes!("../icons/tray-unread-9.png"),
        _ => include_bytes!("../icons/tray-unread-9plus.png"),
    };
    tauri::image::Image::from_bytes(bytes).ok().map(|image| image.to_owned())
}

#[tauri::command]
fn update_tray_unread(app: tauri::AppHandle, unread: usize) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let tooltip = if unread == 0 {
            "CellularHub · ��'��史络控制中式".to_string()
        } else {
            format!("CellularHub · {unread} 条旪诿短信�)
        };
        let _ = tray.set_tooltip(Some(tooltip));
        if unread == 0 {
            if let Some(icon) = app.default_window_icon() {
                let _ = tray.set_icon(Some(icon.clone()));
            }
        } else if let Some(icon) = unread_tray_icon(unread) {
            let _ = tray.set_icon(Some(icon));
        }
    }
}

#[tauri::command]
fn disconnect_backend(manager: State<'_, Arc<CellularManager>>) {
    manager.disconnect();
}

#[tauri::command]
fn connect_at(
    app: tauri::AppHandle,
    manager: State<'_, Arc<CellularManager>>,
    request: AtConnectRequest,
) -> Result<(), String> {
    manager.disconnect();
    let stop = Arc::new(AtomicBool::new(false));
    manager.replace_stop_flag(stop.clone());
    match backend::at::spawn_listener(
        app,
        manager.inner().clone(),
        request.port,
        request.baud_rate,
        stop,
    ) {
        Ok(()) => Ok(()),
        Err(error) => { manager.set_error(error.clone()); Err(error) }
    }
}

#[tauri::command]
fn connect_windows_mbn(
    app: tauri::AppHandle,
    manager: State<'_, Arc<CellularManager>>,
    device_id: String,
) -> Result<(), String> {
    manager.disconnect();
    let stop = Arc::new(AtomicBool::new(false));
    manager.replace_stop_flag(stop.clone());
    match backend::windows_mbn::spawn_listener(app, manager.inner().clone(), device_id, stop) {
        Ok(()) => Ok(()),
        Err(error) => { manager.set_error(error.clone()); Err(error) }
    }
}

#[tauri::command]
fn probe_esim_compat() -> EsimCompatReport {
    backend::esim_compat::probe()
}

#[tauri::command]
fn install_esim_compat(
    manager: State<'_, Arc<CellularManager>>,
    request: EsimCompatInstallRequest,
) -> Result<String, String> {
    // lpac and the AT SMS receiver cannot safely own the same COM port at once.
    manager.disconnect();
    backend::esim_compat::install(request)
}

#[tauri::command]
fn inject_demo_sms(
    app: tauri::AppHandle,
    manager: State<'_, Arc<CellularManager>>,
) -> Result<(), String> {
    let sms = backend::mock::sample_sms();
    manager.push_message(sms.clone());
    let _ = app.emit("sms-received", sms.clone());
    notification_center::show_sms(&app, &sms);
    Ok(())
}

#[tauri::command]
fn launch_esim_uri(uri: String) -> Result<(), String> {
    let raw = uri.trim();
    let normalized = if raw.to_ascii_lowercase().starts_with("lpa:") {
        raw.to_string()
    } else if raw.starts_with("1$") {
        format!("lpa:{raw}")
    } else {
        return Err("请输入 LPA: URI，或二维码中的完整 activation code（通常以 1$ 开头）".into());
    };
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(&normalized)
            .spawn()
            .map_err(|e| format!("无法调用 Windows LPA: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("eSIM LPA handoff 仅在 Windows 上可用".into())
    }
}

#[tauri::command]
fn list_esim_profiles(store: State<'_, Arc<EsimStore>>) -> Vec<EsimProfile> {
    store.list()
}

#[tauri::command]
fn save_esim_profile(
    store: State<'_, Arc<EsimStore>>,
    profile: EsimProfile,
) -> Result<EsimProfile, String> {
    store.save(profile)
}

#[tauri::command]
fn delete_esim_profile(store: State<'_, Arc<EsimStore>>, id: String) -> Result<(), String> {
    store.delete(&id)
}

#[tauri::command]
fn check_esim_reminders(
    app: tauri::AppHandle,
    store: State<'_, Arc<EsimStore>>,
) -> Result<usize, String> {
    let due = store.due_within(7);
    for (profile, kind, days) in &due {
        let when = match *days {
            0 => format!("今天{kind}"),
            1 => format!("明天{kind}"),
            n => format!("{n} 天后{kind}"),
        };
        let cost = if profile.retention_cost > 0.0 {
            format!(" · {} {:.2}", profile.currency, profile.retention_cost)
        } else {
            String::new()
        };
        let _ = app
            .notification()
            .builder()
            .title(format!("eSIM 提醒 · {}", profile.display_name))
            .body(format!(
                "{when}{cost}{}",
                if profile.operator_name.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", profile.operator_name)
                }
            ))
            .show();
    }
    Ok(due.len())
}


fn build_release_diagnostics(
    app: &tauri::AppHandle,
    manager: &CellularManager,
    esim_store: &EsimStore,
    conversation_store: &ConversationStore,
) -> Result<ReleaseDiagnostics, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    let probe = data_dir.join(".write-test");
    let writable = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    let devices = list_devices();
    let snapshot = manager.snapshot();
    let mut checks = vec![
        HealthCheckItem {
            id: "platform".into(),
            label: "Windows 平台".into(),
            status: if cfg!(target_os = "windows") { "pass".into() } else { "warn".into() },
            detail: format!("{} / {}", std::env::consts::OS, std::env::consts::ARCH),
        },
        HealthCheckItem {
            id: "storage".into(),
            label: "本地数据目录".into(),
            status: if writable { "pass".into() } else { "fail".into() },
            detail: if writable { "可写；已启用 .bak 恢复".into() } else { "不可写，短信/eSIM 档案无法可靠保存".into() },
        },
        HealthCheckItem {
            id: "sms".into(),
            label: "SMS 接收后端".into(),
            status: if devices.iter().any(|d| d.capabilities.sms_receive) { "pass".into() } else { "warn".into() },
            detail: format!("发现 {} 个候选蜂窝/AT 设备", devices.len()),
        },
        HealthCheckItem {
            id: "send".into(),
            label: "SMS 发送".into(),
            status: "info".into(),
            detail: "1.0 默认关闭；兼容层只承诺接收".into(),
        },
    ];
    if let Some(error) = snapshot.last_error.as_ref() {
        checks.push(HealthCheckItem {
            id: "last_error".into(),
            label: "最近 Provider 错误".into(),
            status: "warn".into(),
            detail: error.clone(),
        });
    }
    Ok(ReleaseDiagnostics {
        app_version: app.package_info().version.to_string(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        data_dir: data_dir.display().to_string(),
        message_count: manager.messages().len(),
        unread_count: manager.unread_count(),
        esim_profile_count: esim_store.list().len(),
        conversation_meta_count: conversation_store.list().len(),
        snapshot,
        device_count: devices.len(),
        checks,
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
fn run_release_diagnostics(
    app: tauri::AppHandle,
    manager: State<'_, Arc<CellularManager>>,
    esim_store: State<'_, Arc<EsimStore>>,
    conversation_store: State<'_, Arc<ConversationStore>>,
) -> Result<ReleaseDiagnostics, String> {
    build_release_diagnostics(
        &app,
        manager.inner().as_ref(),
        esim_store.inner().as_ref(),
        conversation_store.inner().as_ref(),
    )
}

#[tauri::command]
fn export_release_diagnostics(
    app: tauri::AppHandle,
    manager: State<'_, Arc<CellularManager>>,
    esim_store: State<'_, Arc<EsimStore>>,
    conversation_store: State<'_, Arc<ConversationStore>>,
) -> Result<String, String> {
    let report = build_release_diagnostics(
        &app,
        manager.inner().as_ref(),
        esim_store.inner().as_ref(),
        conversation_store.inner().as_ref(),
    )?;
    let dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("CellularHub-diagnostics-{stamp}.json"));
    let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn set_window_mode(window: tauri::WebviewWindow, mode: String) -> Result<(), String> {
    apply_window_mode(&window, &mode)
}

#[tauri::command]
fn hide_main_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
fn show_quick_panel_if_hidden(app: tauri::AppHandle) -> Result<bool, String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("主窗口不存在".into());
    };
    let visible = window.is_visible().map_err(|e| e.to_string())?;
    let minimized = window.is_minimized().unwrap_or(false);
    if visible && !minimized {
        return Ok(false);
    }
    apply_window_mode(&window, "mini")?;
    let _ = window.unminimize();
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(true)
}


#[tauri::command]
fn set_close_to_tray(prefs: State<'_, Arc<RuntimePrefs>>, enabled: bool) {
    prefs.close_to_tray.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
fn get_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart_enabled(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    manager.is_enabled().map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered first so a second launch cannot race the modem/SMS providers.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !args.iter().any(|arg| arg == "--background") {
                reveal_window(app, "full", Some("overview"));
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .manage(Arc::new(RuntimePrefs {
            close_to_tray: AtomicBool::new(true),
        }))
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(Arc::new(CellularManager::load(
                data_dir.join("messages.json"),
            )));
            app.manage(Arc::new(EsimStore::load(
                data_dir.join("esim-profiles.json"),
            )));
            app.manage(Arc::new(ConversationStore::load(
                data_dir.join("conversation-meta.json"),
            )));

            let open_i = MenuItem::with_id(app, "open", "打开 CellularHub", true, None::<&str>)?;
            let mini_i = MenuItem::with_id(app, "mini", "显示迷你面板", true, None::<&str>)?;
            let sms_i = MenuItem::with_id(app, "messages", "短信", true, None::<&str>)?;
            let esim_i = MenuItem::with_id(app, "esim", "eSIM 管理", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出 CellularHub", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&open_i, &mini_i, &sms_i, &esim_i, &separator, &quit_i],
            )?;

            let mut tray_builder = TrayIconBuilder::with_id("main-tray")
                .tooltip("CellularHub · 蜂窝网络控制中心")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => reveal_window(app, "full", Some("overview")),
                    "mini" => reveal_window(app, "mini", None),
                    "messages" => reveal_window(app, "full", Some("messages")),
                    "esim" => reveal_window(app, "full", Some("esim")),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let visible = window.is_visible().unwrap_or(false);
                            let minimized = window.is_minimized().unwrap_or(false);
                            if visible && !minimized {
                                let _ = window.set_focus();
                            } else {
                                reveal_window(app, "mini", None);
                            }
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }
            let _tray = tray_builder.build(app)?;

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                // Windows 11 native material. If unavailable, the CSS surface remains a safe fallback.
                let _ = window_vibrancy::apply_mica(&window, Some(true));
            }

            if std::env::args().any(|arg| arg == "--background") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window
                    .state::<Arc<RuntimePrefs>>()
                    .close_to_tray
                    .load(Ordering::Relaxed);
                api.prevent_close();
                if close_to_tray {
                    let _ = window.hide();
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            list_devices,
            list_messages,
            list_conversation_meta,
            save_conversation_meta,
            copy_text,
            mark_message_read,
            mark_all_messages_read,
            mark_conversation_read,
            set_conversation_archived,
            set_message_archived,
            update_tray_unread,
            disconnect_backend,
            connect_at,
            connect_windows_mbn,
            probe_esim_compat,
            install_esim_compat,
            inject_demo_sms,
            launch_esim_uri,
            list_esim_profiles,
            save_esim_profile,
            delete_esim_profile,
            check_esim_reminders,
            set_window_mode,
            hide_main_window,
            show_quick_panel_if_hidden,
            set_close_to_tray,
            get_autostart_enabled,
            set_autostart_enabled,
            run_release_diagnostics,
            export_release_diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("error while running CellularHub");
}
