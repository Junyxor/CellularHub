use chrono::Utc;
use uuid::Uuid;

use crate::model::{BackendKind, SmsMessage};

pub fn sample_sms() -> SmsMessage {
    SmsMessage {
        id: Uuid::new_v4().to_string(),
        sender: "10695500".into(),
        body: "【CellularHub】测试短信已通过统一 Provider 注入，验证码 482631。".into(),
        received_at: Utc::now().to_rfc3339(),
        unread: true,
        archived: false,
        backend: BackendKind::Mock,
        slot: None,
    }
}
