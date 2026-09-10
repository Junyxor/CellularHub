use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    model::SmsMessage,
    sms::pdu::Concatenation,
};

const PART_TTL: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Debug, Clone)]
pub struct IncomingSms {
    pub source_key: String,
    pub sender: String,
    pub body: String,
    pub received_at: Option<String>,
    pub concatenation: Option<Concatenation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteSms {
    pub source_key: String,
    pub sender: String,
    pub body: String,
    pub received_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MultipartKey {
    sender: String,
    reference: u16,
    total: u8,
}

struct PendingMultipart {
    created_at: Instant,
    parts: BTreeMap<u8, IncomingSms>,
}

#[derive(Default)]
pub struct MultipartAssembler {
    pending: HashMap<MultipartKey, PendingMultipart>,
}

impl MultipartAssembler {
    pub fn push(&mut self, incoming: IncomingSms) -> Result<Option<CompleteSms>, String> {
        self.expire_stale();
        let Some(concat) = incoming.concatenation.clone() else {
            return Ok(Some(CompleteSms {
                source_key: incoming.source_key,
                sender: incoming.sender,
                body: incoming.body,
                received_at: incoming.received_at,
            }));
        };

        if concat.total == 0 || concat.sequence == 0 || concat.sequence > concat.total {
            return Err("invalid multipart SMS sequence metadata".into());
        }

        let key = MultipartKey {
            sender: incoming.sender.clone(),
            reference: concat.reference,
            total: concat.total,
        };
        let pending = self.pending.entry(key.clone()).or_insert_with(|| PendingMultipart {
            created_at: Instant::now(),
            parts: BTreeMap::new(),
        });
        pending.parts.insert(concat.sequence, incoming);

        if pending.parts.len() != concat.total as usize
            || !(1..=concat.total).all(|sequence| pending.parts.contains_key(&sequence))
        {
            return Ok(None);
        }

        let pending = self.pending.remove(&key).expect("multipart entry disappeared");
        let mut body = String::new();
        let mut source_keys = Vec::with_capacity(pending.parts.len());
        let mut received_at = None;
        for (_, part) in pending.parts {
            if received_at.is_none() {
                received_at = part.received_at.clone();
            }
            source_keys.push(part.source_key);
            body.push_str(&part.body);
        }

        Ok(Some(CompleteSms {
            source_key: source_keys.join("|"),
            sender: key.sender,
            body,
            received_at,
        }))
    }

    fn expire_stale(&mut self) {
        self.pending.retain(|_, pending| pending.created_at.elapsed() < PART_TTL);
    }
}

pub fn message_from_complete(complete: CompleteSms) -> SmsMessage {
    let received_at = complete
        .received_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let code = extract_code(&complete.body);
    let category = classify_message(&complete.body, code.is_some()).to_string();
    let identity = format!(
        "{}\n{}\n{}",
        complete.source_key, complete.sender, complete.body
    );
    let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()).to_string();

    SmsMessage {
        id,
        sender: complete.sender,
        body: complete.body,
        received_at,
        unread: true,
        archived: false,
        category,
        code,
    }
}

pub fn extract_code(body: &str) -> Option<String> {
    let lower = body.to_lowercase();
    const HINTS: &[&str] = &[
        "验证码",
        "校验码",
        "动态码",
        "安全码",
        "一次性密码",
        "otp",
        "verification code",
        "verify code",
        "passcode",
        "security code",
        "one-time password",
        "one time password",
    ];
    if !HINTS.iter().any(|hint| lower.contains(hint)) {
        return None;
    }

    let bytes = body.as_bytes();
    let mut start = 0usize;
    while start < bytes.len() {
        if !bytes[start].is_ascii_digit() {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        let len = end - start;
        if (4..=8).contains(&len) {
            return Some(body[start..end].to_string());
        }
        start = end;
    }
    None
}

pub fn classify_message(body: &str, has_code: bool) -> &'static str {
    if has_code {
        return "code";
    }
    let lower = body.to_lowercase();
    const BILLING: &[&str] = &[
        "续费", "到期", "账单", "余额", "扣费", "充值", "renew", "bill", "balance",
        "payment", "expire", "subscription",
    ];
    if BILLING.iter().any(|keyword| lower.contains(keyword)) {
        return "billing";
    }
    const USAGE: &[&str] = &[
        "流量", "套餐", "已使用", "剩余", "data", "usage", "quota", "gb", "mb",
    ];
    if USAGE.iter().any(|keyword| lower.contains(keyword)) {
        return "usage";
    }
    "notice"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(sequence: u8, body: &str) -> IncomingSms {
        IncomingSms {
            source_key: format!("source-{sequence}"),
            sender: "10086".into(),
            body: body.into(),
            received_at: Some("2026-09-10T00:00:00Z".into()),
            concatenation: Some(Concatenation {
                reference: 0x77,
                total: 2,
                sequence,
            }),
        }
    }

    #[test]
    fn assembles_parts_in_sequence_order_even_when_arrival_is_reversed() {
        let mut assembler = MultipartAssembler::default();
        assert!(assembler.push(part(2, "world")).unwrap().is_none());
        let complete = assembler.push(part(1, "hello ")).unwrap().unwrap();
        assert_eq!(complete.body, "hello world");
        assert_eq!(complete.source_key, "source-1|source-2");
    }

    #[test]
    fn code_extraction_requires_semantic_hint() {
        assert_eq!(extract_code("验证码 284193，5 分钟内有效"), Some("284193".into()));
        assert_eq!(extract_code("联系电话 13800138000"), None);
    }

    #[test]
    fn categories_are_local_and_conservative() {
        assert_eq!(classify_message("验证码 123456", true), "code");
        assert_eq!(classify_message("套餐剩余流量 3GB", false), "usage");
        assert_eq!(classify_message("Your plan will renew tomorrow", false), "billing");
        assert_eq!(classify_message("欢迎使用本服务", false), "notice");
    }

    #[test]
    fn generated_ids_are_stable_for_duplicate_provider_records() {
        let complete = CompleteSms {
            source_key: "at:COM3:12".into(),
            sender: "10086".into(),
            body: "验证码 123456".into(),
            received_at: None,
        };
        let a = message_from_complete(complete.clone());
        let b = message_from_complete(complete);
        assert_eq!(a.id, b.id);
    }
}
