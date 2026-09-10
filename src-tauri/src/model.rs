use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCapabilities {
    pub sms_receive: bool,
    pub sms_send: bool,
    pub native_esim: bool,
    pub euicc_bridge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellularDevice {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub model: Option<String>,
    pub operator: Option<String>,
    pub signal: Option<u8>,
    pub network_class: Option<String>,
    pub status: String,
    pub capabilities: DeviceCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmsMessage {
    pub id: String,
    pub sender: String,
    pub body: String,
    pub received_at: String,
    pub unread: bool,
    pub archived: bool,
    pub category: String,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EsimProfile {
    pub id: String,
    pub name: String,
    pub country_code: String,
    pub operator: String,
    pub phone_number: Option<String>,
    pub iccid: Option<String>,
    pub plan: Option<String>,
    pub keep_alive_cost: Option<f64>,
    pub currency: Option<String>,
    pub billing_cycle: Option<String>,
    pub renewal_date: Option<String>,
    pub expiry_date: Option<String>,
    pub auto_renew: bool,
    pub status: String,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub native_lpa_available: bool,
    pub euicc_bridge_available: bool,
    pub lpac_found: bool,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub devices: Vec<CellularDevice>,
    pub selected_device_id: Option<String>,
    pub active_sms_listener_device_id: Option<String>,
    pub messages: Vec<SmsMessage>,
    pub esim_profiles: Vec<EsimProfile>,
    pub runtime: RuntimeCapabilities,
}
