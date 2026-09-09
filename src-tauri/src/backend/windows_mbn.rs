//! Native Windows Mobile Broadband (MBN) SMS backend.
//!
//! On Windows this uses the desktop Mobile Broadband COM API directly. The
//! frontend sees the same `CellularManager` contract as the AT compatibility
//! backend, so native MBN can be preferred without tying the UI to COM.

use std::sync::{atomic::AtomicBool, Arc};

use tauri::AppHandle;

use crate::{
    manager::CellularManager,
    model::{BackendKind, CapabilitySet, DeviceInfo},
};

#[cfg(not(target_os = "windows"))]
pub fn discover() -> Vec<DeviceInfo> { Vec::new() }

#[cfg(not(target_os = "windows"))]
pub fn spawn_listener(
    _app: AppHandle,
    _manager: Arc<CellularManager>,
    _interface_id: String,
    _stop: Arc<AtomicBool>,
) -> Result<(), String> {
    Err("Windows MBN 后端仅在 Windows 上可用".into())
}

#[cfg(target_os = "windows")]
mod imp {
    use std::{
        ffi::c_void,
        mem::ManuallyDrop,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread,
        time::Duration,
    };

    use chrono::Utc;
    use tauri::{AppHandle, Emitter};
    use uuid::Uuid;
    use windows::{
        core::{implement, Interface, IUnknown, PCWSTR, Ref, Result as WinResult, HRESULT},
        Win32::{
            Foundation::{SAFEARRAY, VARIANT_BOOL},
            NetworkManagement::MobileBroadband::{
                IMbnInterface, IMbnInterfaceManager, IMbnRegistration, IMbnSignal, IMbnSms,
                IMbnSmsEvents, IMbnSmsEvents_Impl, IMbnSmsReadMsgPdu, MbnInterfaceManager,
                MBN_INTERFACE_CAPS, MBN_SMS_CAPS_PDU_RECEIVE,
                MBN_SMS_FILTER, MBN_SMS_FLAG_NEW, MBN_SMS_FORMAT, MBN_SMS_FORMAT_PDU,
            },
            System::{
                Com::{
                    CoCreateInstance, CoInitializeEx, CoUninitialize, IConnectionPointContainer,
                    CLSCTX_ALL, COINIT_MULTITHREADED,
                },
                Ole::{SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound},
            },
        },
    };

    use crate::{
        backend::pdu::{decode_sms_deliver, MultipartAssembler},
        manager::CellularManager,
        model::{BackendKind, CapabilitySet, CellularSnapshot, DeviceInfo, SmsMessage},
    };

    pub fn discover() -> Vec<DeviceInfo> {
        thread::spawn(discover_worker).join().unwrap_or_default()
    }

    fn discover_worker() -> Vec<DeviceInfo> {
        let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok().is_ok();
        if !initialized { return Vec::new(); }
        let result = unsafe { enumerate_interfaces() }.unwrap_or_default();
        unsafe { CoUninitialize(); }
        result
    }

    unsafe fn enumerate_interfaces() -> WinResult<Vec<DeviceInfo>> {
        let manager: IMbnInterfaceManager = unsafe { CoCreateInstance(&MbnInterfaceManager, None, CLSCTX_ALL)? };
        let array = unsafe { manager.GetInterfaces()? };
        let interfaces = unsafe { safe_array_unknowns(array as *const SAFEARRAY) };
        let _ = unsafe { SafeArrayDestroy(array) };

        let mut devices = Vec::new();
        for unknown in interfaces? {
            let Ok(interface) = unknown.cast::<IMbnInterface>() else { continue; };
            let interface_id = unsafe { interface.InterfaceID() }
                .map(|value| value.to_string())
                .unwrap_or_else(|_| "unknown".into());

            let mut caps = MBN_INTERFACE_CAPS::default();
            if unsafe { interface.GetInterfaceCapability(&mut caps) }.is_err() { continue; }

            let sms_caps = caps.smsCaps;
            let manufacturer = bstr_field(&caps.manufacturer);
            let model = bstr_field(&caps.model);
            let name = if !model.is_empty() {
                model.clone()
            } else if !manufacturer.is_empty() {
                format!("{manufacturer} Mobile Broadband")
            } else {
                "Windows Mobile Broadband".into()
            };
            unsafe { drop_caps_strings(&mut caps); }

            let receive_mask = MBN_SMS_CAPS_PDU_RECEIVE.0 as u32;
            devices.push(DeviceInfo {
                id: format!("mbn:{interface_id}"),
                name,
                backend: BackendKind::WindowsMbn,
                port: None,
                manufacturer: (!manufacturer.is_empty()).then_some(manufacturer),
                product: (!model.is_empty()).then_some(model),
                capabilities: CapabilitySet {
                    data: true,
                    sms_receive: sms_caps & receive_mask != 0,
                    sms_send: false,
                    esim: false,
                    signal: true,
                    operator: true,
                },
            });
        }
        Ok(devices)
    }

    fn bstr_field(value: &ManuallyDrop<windows::core::BSTR>) -> String { value.to_string() }

    unsafe fn drop_caps_strings(caps: &mut MBN_INTERFACE_CAPS) {
        unsafe {
            ManuallyDrop::drop(&mut caps.customDataClass);
            ManuallyDrop::drop(&mut caps.customBandClass);
            ManuallyDrop::drop(&mut caps.deviceID);
            ManuallyDrop::drop(&mut caps.manufacturer);
            ManuallyDrop::drop(&mut caps.model);
            ManuallyDrop::drop(&mut caps.firmwareInfo);
        }
    }

    pub fn spawn_listener(
        app: AppHandle,
        manager: Arc<CellularManager>,
        interface_id: String,
        stop: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let id = interface_id.strip_prefix("mbn:").unwrap_or(&interface_id).to_string();
        if id.trim().is_empty() { return Err("缺少 Windows MBN interface id".into()); }

        manager.set_snapshot(CellularSnapshot {
            backend: BackendKind::WindowsMbn,
            device_name: "Windows Mobile Broadband".into(),
            operator_name: "正在读取运营商".into(),
            network_type: "Mobile Broadband".into(),
            signal_percent: 0,
            rssi_dbm: None,
            connected: true,
            sms_ready: true,
            esim_ready: false,
            last_error: None,
        });

        thread::spawn(move || {
            if let Err(error) = run_worker(app, manager.clone(), id, stop) { manager.set_error(error); }
        });
        Ok(())
    }

    fn run_worker(
        app: AppHandle,
        manager: Arc<CellularManager>,
        interface_id: String,
        stop: Arc<AtomicBool>,
    ) -> Result<(), String> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| format!("无法初始化 Windows COM: {error}"))?;
        let result = unsafe { run_worker_inner(&app, &manager, &interface_id, &stop) };
        unsafe { CoUninitialize(); }
        result
    }

    unsafe fn run_worker_inner(
        app: &AppHandle,
        manager: &Arc<CellularManager>,
        interface_id: &str,
        stop: &Arc<AtomicBool>,
    ) -> Result<(), String> {
        let mbn_manager: IMbnInterfaceManager = unsafe { CoCreateInstance(&MbnInterfaceManager, None, CLSCTX_ALL) }
            .map_err(|e| format!("无法创建 Windows Mobile Broadband Manager: {e}"))?;
        let interface_id_wide: Vec<u16> = interface_id.encode_utf16().chain(std::iter::once(0)).collect();
        let interface = unsafe { mbn_manager.GetInterface(PCWSTR(interface_id_wide.as_ptr())) }
            .map_err(|e| format!("找不到 Windows MBN 接口 {interface_id}: {e}"))?;
        let sms: IMbnSms = interface.cast().map_err(|e| format!("该 MBN 设备没有 SMS 接口: {e}"))?;

        update_snapshot(manager, &interface, true);

        let events: IMbnSmsEvents = SmsEvents {
            app: app.clone(),
            manager: manager.clone(),
            multipart: Mutex::new(MultipartAssembler::default()),
        }.into();
        let container: IConnectionPointContainer = mbn_manager.cast()
            .map_err(|e| format!("MBN Manager 不支持 ConnectionPoint: {e}"))?;
        let point = unsafe { container.FindConnectionPoint(&IMbnSmsEvents::IID) }
            .map_err(|e| format!("无法订阅 IMbnSmsEvents: {e}"))?;
        let cookie = unsafe { point.Advise(&events) }
            .map_err(|e| format!("无法注册 IMbnSmsEvents: {e}"))?;

        request_new_messages(&sms);
        let mut ticks = 0u32;
        while !stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));
            ticks = ticks.wrapping_add(1);
            if ticks % 60 == 0 { update_snapshot(manager, &interface, true); }
        }
        let _ = unsafe { point.Unadvise(cookie) };
        Ok(())
    }

    fn request_new_messages(sms: &IMbnSms) {
        let filter = MBN_SMS_FILTER { flag: MBN_SMS_FLAG_NEW, messageIndex: 0 };
        let _ = unsafe { sms.SmsRead(&filter, MBN_SMS_FORMAT_PDU) };
    }

    fn update_snapshot(manager: &CellularManager, interface: &IMbnInterface, connected: bool) {
        let operator = interface.cast::<IMbnRegistration>()
            .ok()
            .and_then(|registration| unsafe { registration.GetProviderName().ok() })
            .map(|name| name.to_string())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "蜂窝网络".into());
        let signal = interface.cast::<IMbnSignal>()
            .ok()
            .and_then(|signal| unsafe { signal.GetSignalStrength().ok() })
            .map(|value| value.min(100) as u8)
            .unwrap_or(0);

        let current = manager.snapshot();
        manager.set_snapshot(CellularSnapshot {
            backend: BackendKind::WindowsMbn,
            device_name: if current.device_name.is_empty() { "Windows Mobile Broadband".into() } else { current.device_name },
            operator_name: operator,
            network_type: "Mobile Broadband".into(),
            signal_percent: signal,
            rssi_dbm: None,
            connected,
            sms_ready: true,
            esim_ready: false,
            last_error: None,
        });
    }

    #[implement(IMbnSmsEvents)]
    struct SmsEvents {
        app: AppHandle,
        manager: Arc<CellularManager>,
        multipart: Mutex<MultipartAssembler>,
    }

    impl IMbnSmsEvents_Impl for SmsEvents_Impl {
        fn OnSmsConfigurationChange(&self, _sms: Ref<'_, IMbnSms>) -> WinResult<()> { Ok(()) }
        fn OnSetSmsConfigurationComplete(&self, _sms: Ref<'_, IMbnSms>, _requestid: u32, _status: HRESULT) -> WinResult<()> { Ok(()) }
        fn OnSmsSendComplete(&self, _sms: Ref<'_, IMbnSms>, _requestid: u32, _status: HRESULT) -> WinResult<()> { Ok(()) }

        fn OnSmsReadComplete(
            &self,
            sms: Ref<'_, IMbnSms>,
            smsformat: MBN_SMS_FORMAT,
            readmsgs: *const SAFEARRAY,
            moremsgs: VARIANT_BOOL,
            _requestid: u32,
            status: HRESULT,
        ) -> WinResult<()> {
            if status.ok().is_ok() && smsformat == MBN_SMS_FORMAT_PDU {
                let _ = unsafe { self.consume_pdu_array(readmsgs) };
            }
            if moremsgs.as_bool() { if let Some(sms) = sms.as_ref() { request_new_messages(sms); } }
            Ok(())
        }

        fn OnSmsNewClass0Message(&self, _sms: Ref<'_, IMbnSms>, smsformat: MBN_SMS_FORMAT, readmsgs: *const SAFEARRAY) -> WinResult<()> {
            if smsformat == MBN_SMS_FORMAT_PDU { let _ = unsafe { self.consume_pdu_array(readmsgs) }; }
            Ok(())
        }

        fn OnSmsDeleteComplete(&self, _sms: Ref<'_, IMbnSms>, _requestid: u32, _status: HRESULT) -> WinResult<()> { Ok(()) }

        fn OnSmsStatusChange(&self, sms: Ref<'_, IMbnSms>) -> WinResult<()> {
            if let Some(sms) = sms.as_ref() { request_new_messages(sms); }
            Ok(())
        }
    }

    impl SmsEvents_Impl {
        unsafe fn consume_pdu_array(&self, array: *const SAFEARRAY) -> WinResult<()> {
            for unknown in unsafe { safe_array_unknowns(array)? } {
                let Ok(message) = unknown.cast::<IMbnSmsReadMsgPdu>() else { continue; };
                let index = unsafe { message.Index() }.ok();
                let Ok(pdu) = (unsafe { message.PduData() }) else { continue; };
                let Ok(decoded) = decode_sms_deliver(&pdu.to_string()) else { continue; };
                let Some((sender, body)) = self.multipart.lock().expect("multipart poisoned").push(decoded) else { continue; };

                let sms = SmsMessage {
                    id: Uuid::new_v4().to_string(), sender, body,
                    received_at: Utc::now().to_rfc3339(), unread: true, archived: false,
                    backend: BackendKind::WindowsMbn, slot: index,
                };
                if !self.manager.push_message(sms.clone()) { continue; }
                let _ = self.app.emit("sms-received", sms.clone());
                crate::notification_center::show_sms(&self.app, &sms);
            }
            Ok(())
        }
    }

    unsafe fn safe_array_unknowns(array: *const SAFEARRAY) -> WinResult<Vec<IUnknown>> {
        if array.is_null() { return Ok(Vec::new()); }
        let lower = unsafe { SafeArrayGetLBound(array, 1)? };
        let upper = unsafe { SafeArrayGetUBound(array, 1)? };
        if upper < lower { return Ok(Vec::new()); }
        let mut result = Vec::with_capacity((upper - lower + 1) as usize);
        for index in lower..=upper {
            let mut raw: *mut c_void = std::ptr::null_mut();
            unsafe { SafeArrayGetElement(array, &index, &mut raw as *mut _ as *mut c_void)?; }
            if !raw.is_null() { result.push(unsafe { IUnknown::from_raw(raw) }); }
        }
        Ok(result)
    }
}

#[cfg(target_os = "windows")]
pub use imp::{discover, spawn_listener};

#[allow(dead_code)]
pub fn planned_capabilities() -> CapabilitySet {
    CapabilitySet { data: true, sms_receive: true, sms_send: false, esim: false, signal: true, operator: true }
}

#[allow(dead_code)]
pub fn kind() -> BackendKind { BackendKind::WindowsMbn }
