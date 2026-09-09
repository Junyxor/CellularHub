use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;

use crate::{conversation_store::ConversationStore, model::SmsMessage, reveal_window};

pub fn show_sms(app: &AppHandle, sms: &SmsMessage) {
    let display = app
        .state::<Arc<ConversationStore>>()
        .get(&sms.sender)
        .and_then(|meta| (!meta.alias.is_empty()).then_some(meta.alias))
        .unwrap_or_else(|| sms.sender.clone());
    let code = verification_code(&sms.body);

    #[cfg(target_os = "windows")]
    if windows_native::show(app, sms, &display, code.as_deref()).is_ok() {
        return;
    }

    let preview = truncate(&sms.body, 96);
    let body = match code {
        Some(code) => format!("{preview}\nCode: {code}"),
        None => preview,
    };
    let _ = app
        .notification()
        .builder()
        .title(format!("SMS - {display}"))
        .body(body)
        .show();
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        format!("{}...", value.chars().take(max).collect::<String>())
    }
}

pub fn verification_code(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let keyword = body.contains("\u{9a8c}\u{8bc1}\u{7801}")
        || body.contains("\u{767b}\u{5f55}\u{7801}")
        || body.contains("\u{52a8}\u{6001}\u{5bc6}\u{7801}")
        || body.contains("\u{5b89}\u{5168}\u{7801}")
        || lower.contains("otp")
        || lower.contains("verification code")
        || lower.contains("security code")
        || lower.contains("passcode");
    if !keyword {
        return None;
    }

    let mut run = String::new();
    for ch in body.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() {
            run.push(ch);
            continue;
        }
        if (4..=8).contains(&run.len()) {
            return Some(run);
        }
        run.clear();
    }
    None
}

fn handle_open(app: &AppHandle, sender: &str) {
    reveal_window(app, "full", Some("messages"));
    let _ = app.emit("navigate-conversation", sender.to_string());
}

fn handle_copy(app: &AppHandle, sender: &str, code: &str) {
    if app.clipboard().write_text(code).is_ok() {
        let _ = app.emit("verification-code-copied", code.to_string());
    } else {
        handle_open(app, sender);
    }
}

#[cfg(target_os = "windows")]
mod windows_native {
    use std::sync::{Mutex, OnceLock};

    use super::{handle_copy, handle_open};
    use crate::model::SmsMessage;
    use tauri::{AppHandle, Manager};
    use windows::{
        core::{HSTRING, IInspectable, Interface},
        Data::Xml::Dom::XmlDocument,
        Foundation::TypedEventHandler,
        UI::Notifications::{ToastActivatedEventArgs, ToastNotification, ToastNotificationManager},
    };

    static ACTIVE: OnceLock<Mutex<Vec<ToastNotification>>> = OnceLock::new();

    pub fn show(app: &AppHandle, sms: &SmsMessage, display: &str, code: Option<&str>) -> windows::core::Result<()> {
        let title = xml_escape(&format!("SMS - {display}"));
        let body = xml_escape(&super::truncate(&sms.body, 120));
        let copy_label = "\u{590d}\u{5236}";
        let open_label = "\u{6253}\u{5f00}\u{4f1a}\u{8bdd}";
        let copy_action = code
            .map(|code| format!(r#"<action content="{copy_label} {}" arguments="copy" activationType="foreground"/>"#, xml_escape(code)))
            .unwrap_or_default();
        let xml = format!(
            r#"<toast launch="open"><visual><binding template="ToastGeneric"><text>{title}</text><text>{body}</text></binding></visual><actions>{copy_action}<action content="{open_label}" arguments="open" activationType="foreground"/></actions></toast>"#
        );

        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;

        let app_handle = app.clone();
        let sender = sms.sender.clone();
        let copy_code = code.map(ToOwned::to_owned);
        toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
            move |_sender, args| {
                let arguments = args
                    .as_ref()
                    .and_then(|args| args.cast::<ToastActivatedEventArgs>().ok())
                    .and_then(|args| args.Arguments().ok())
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "open".into());
                if arguments == "copy" {
                    if let Some(code) = copy_code.as_deref() {
                        handle_copy(&app_handle, &sender, code);
                    } else {
                        handle_open(&app_handle, &sender);
                    }
                } else {
                    handle_open(&app_handle, &sender);
                }
                Ok(())
            },
        ))?;

        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app.config().identifier.clone()))?;
        notifier.Show(&toast)?;

        let active = ACTIVE.get_or_init(|| Mutex::new(Vec::new()));
        if let Ok(mut active) = active.lock() {
            active.push(toast);
            if active.len() > 32 {
                let drain = active.len() - 32;
                active.drain(0..drain);
            }
        }
        Ok(())
    }

    fn xml_escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
}
