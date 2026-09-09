use std::{collections::{BTreeMap, HashMap}, time::{Duration, Instant}};

#[derive(Debug, Clone)]
pub struct ConcatInfo {
    pub reference: u16,
    pub total: u8,
    pub sequence: u8,
}

#[derive(Debug, Clone)]
pub struct DecodedSms {
    pub sender: String,
    pub body: String,
    pub concat: Option<ConcatInfo>,
}

struct PendingMultipart {
    sender: String,
    total: u8,
    parts: BTreeMap<u8, String>,
    created_at: Instant,
}

pub struct MultipartAssembler {
    pending: HashMap<String, PendingMultipart>,
}

impl Default for MultipartAssembler {
    fn default() -> Self { Self { pending: HashMap::new() } }
}

impl MultipartAssembler {
    pub fn push(&mut self, sms: DecodedSms) -> Option<(String, String)> {
        self.pending.retain(|_, value| value.created_at.elapsed() < Duration::from_secs(60 * 60));
        let Some(concat) = sms.concat else { return Some((sms.sender, sms.body)); };
        if concat.total <= 1 { return Some((sms.sender, sms.body)); }

        let key = format!("{}:{}:{}", sms.sender, concat.reference, concat.total);
        let entry = self.pending.entry(key.clone()).or_insert_with(|| PendingMultipart {
            sender: sms.sender.clone(),
            total: concat.total,
            parts: BTreeMap::new(),
            created_at: Instant::now(),
        });
        entry.parts.insert(concat.sequence, sms.body);
        if entry.parts.len() < entry.total as usize { return None; }

        let mut body = String::new();
        for sequence in 1..=entry.total {
            body.push_str(entry.parts.get(&sequence)?);
        }
        let sender = entry.sender.clone();
        self.pending.remove(&key);
        Some((sender, body))
    }
}

pub fn decode_sms_deliver(pdu: &str) -> Result<DecodedSms, String> {
    let bytes = decode_hex(pdu)?;
    if bytes.len() < 2 { return Err("PDU 太短".into()); }

    let smsc_len = bytes[0] as usize;
    let mut index = 1usize.checked_add(smsc_len).ok_or("SMSC 长度溢出")?;
    let first = *bytes.get(index).ok_or("缺少 TPDU 首字节")?;
    index += 1;
    if first & 0x03 != 0 { return Err("当前仅支持 SMS-DELIVER PDU".into()); }
    let udhi = first & 0x40 != 0;

    let address_len = *bytes.get(index).ok_or("缺少发送方长度")? as usize;
    index += 1;
    let toa = *bytes.get(index).ok_or("缺少发送方类型")?;
    index += 1;
    let address_octets = (address_len + 1) / 2;
    let address_end = index.checked_add(address_octets).ok_or("发送方长度溢出")?;
    let address_bytes = bytes.get(index..address_end).ok_or("发送方地址不完整")?;
    let sender = decode_address(address_bytes, address_len, toa);
    index = address_end;

    let _pid = *bytes.get(index).ok_or("缺少 PID")?;
    let dcs = *bytes.get(index + 1).ok_or("缺少 DCS")?;
    index += 2;
    index = index.checked_add(7).ok_or("SCTS 长度溢出")?;
    if index >= bytes.len() { return Err("缺少用户数据长度".into()); }
    let udl = bytes[index] as usize;
    index += 1;
    let user_data = bytes.get(index..).ok_or("用户数据不完整")?;

    let (header_octets, concat) = if udhi { parse_udh(user_data)? } else { (0, None) };

    let alphabet = dcs & 0x0c;
    let body = match alphabet {
        0x08 => decode_ucs2(user_data, header_octets, udl),
        0x04 => decode_8bit(user_data, header_octets, udl),
        _ => decode_gsm7(user_data, header_octets, udl),
    }?;

    Ok(DecodedSms { sender, body, concat })
}

fn parse_udh(user_data: &[u8]) -> Result<(usize, Option<ConcatInfo>), String> {
    let udhl = *user_data.first().ok_or("UDH 为空")? as usize;
    let header_octets = udhl + 1;
    if header_octets > user_data.len() { return Err("UDH 长度超出用户数据".into()); }

    let mut cursor = 1usize;
    let mut concat = None;
    while cursor + 1 < header_octets {
        let iei = user_data[cursor];
        let len = user_data[cursor + 1] as usize;
        cursor += 2;
        if cursor + len > header_octets { break; }
        match (iei, len) {
            (0x00, 3) => concat = Some(ConcatInfo { reference: user_data[cursor] as u16, total: user_data[cursor + 1], sequence: user_data[cursor + 2] }),
            (0x08, 4) => concat = Some(ConcatInfo { reference: u16::from_be_bytes([user_data[cursor], user_data[cursor + 1]]), total: user_data[cursor + 2], sequence: user_data[cursor + 3] }),
            _ => {}
        }
        cursor += len;
    }
    Ok((header_octets, concat))
}

fn decode_address(bytes: &[u8], semi_octets: usize, toa: u8) -> String {
    if toa & 0x70 == 0x50 {
        let septets = semi_octets * 4 / 7;
        return decode_gsm7(bytes, 0, septets).unwrap_or_else(|_| "未知发送方".into());
    }
    let mut out = String::new();
    for byte in bytes {
        for nibble in [byte & 0x0f, byte >> 4] {
            if out.len() >= semi_octets { break; }
            if nibble <= 9 { out.push(char::from(b'0' + nibble)); }
        }
    }
    if toa & 0x70 == 0x10 && !out.starts_with('+') { format!("+{out}") } else { out }
}

fn decode_ucs2(data: &[u8], header_octets: usize, udl: usize) -> Result<String, String> {
    let total_octets = udl.min(data.len());
    if header_octets > total_octets { return Err("UCS2 UDH 超出 UDL".into()); }
    let payload = &data[header_octets..total_octets];
    let units: Vec<u16> = payload.chunks_exact(2).map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]])).collect();
    String::from_utf16(&units).map_err(|_| "无效 UCS2 数据".into())
}

fn decode_8bit(data: &[u8], header_octets: usize, udl: usize) -> Result<String, String> {
    let total_octets = udl.min(data.len());
    if header_octets > total_octets { return Err("8-bit UDH 超出 UDL".into()); }
    Ok(String::from_utf8_lossy(&data[header_octets..total_octets]).to_string())
}

fn decode_gsm7(data: &[u8], header_octets: usize, udl_septets: usize) -> Result<String, String> {
    let header_septets = if header_octets == 0 { 0 } else { (header_octets * 8 + 6) / 7 };
    if header_septets > udl_septets { return Err("7-bit UDH 超出 UDL".into()); }
    let text_septets = udl_septets - header_septets;
    let mut values = Vec::with_capacity(text_septets);
    for septet_index in header_septets..header_septets + text_septets {
        let bit = septet_index * 7;
        let byte_index = bit / 8;
        let shift = bit % 8;
        let Some(first) = data.get(byte_index).copied() else { break; };
        let mut value = first >> shift;
        if shift > 1 { if let Some(next) = data.get(byte_index + 1) { value |= next << (8 - shift); } }
        values.push(value & 0x7f);
    }
    Ok(gsm7_to_string(&values))
}

fn gsm7_to_string(values: &[u8]) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for value in values {
        if escaped {
            out.push(match value {
                0x0a => '\u{000c}', 0x14 => '^', 0x28 => '{', 0x29 => '}', 0x2f => '\\', 0x3c => '[', 0x3d => '~', 0x3e => ']', 0x40 => '|', 0x65 => '€', _ => '�',
            });
            escaped = false;
            continue;
        }
        if *value == 0x1b { escaped = true; continue; }
        out.push(match value {
            0x00 => '@', 0x01 => '£', 0x02 => '$', 0x03 => '¥', 0x04 => 'è', 0x05 => 'é', 0x06 => 'ù', 0x07 => 'ì',
            0x08 => 'ò', 0x09 => 'Ç', 0x0a => '\n', 0x0b => 'Ø', 0x0c => 'ø', 0x0d => '\r', 0x0e => 'Å', 0x0f => 'å',
            0x10 => 'Δ', 0x11 => '_', 0x12 => 'Φ', 0x13 => 'Γ', 0x14 => 'Λ', 0x15 => 'Ω', 0x16 => 'Π', 0x17 => 'Ψ',
            0x18 => 'Σ', 0x19 => 'Θ', 0x1a => 'Ξ', 0x1c => 'Æ', 0x1d => 'æ', 0x1e => 'ß', 0x1f => 'É',
            0x20..=0x23 | 0x25..=0x3f | 0x41..=0x5a | 0x61..=0x7a => *value as char,
            0x24 => '¤', 0x40 => '¡',
            0x5b => 'Ä', 0x5c => 'Ö', 0x5d => 'Ñ', 0x5e => 'Ü', 0x5f => '§', 0x60 => '¿',
            0x7b => 'ä', 0x7c => 'ö', 0x7d => 'ñ', 0x7e => 'ü', 0x7f => 'à',
            _ => '�',
        });
    }
    out
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 || !clean.chars().all(|c| c.is_ascii_hexdigit()) { return Err("PDU 不是有效十六进制".into()); }
    clean.as_bytes().chunks_exact(2).map(|chunk| {
        let s = std::str::from_utf8(chunk).map_err(|_| "PDU 编码错误".to_string())?;
        u8::from_str_radix(s, 16).map_err(|_| "PDU 十六进制解析失败".to_string())
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_8bit_concat_udh() {
        let data = [0x05, 0x00, 0x03, 0xcc, 0x02, 0x01, b'H', b'i'];
        let (header, concat) = parse_udh(&data).unwrap();
        assert_eq!(header, 6);
        let concat = concat.unwrap();
        assert_eq!(concat.reference, 0xcc);
        assert_eq!(concat.total, 2);
        assert_eq!(concat.sequence, 1);
    }

    #[test]
    fn decodes_simple_ucs2_deliver() {
        let sms = decode_sms_deliver("00040A912143658709000800000000000000044F60597D").unwrap();
        assert_eq!(sms.sender, "+1234567890");
        assert_eq!(sms.body, "你好");
        assert!(sms.concat.is_none());
    }

    #[test]
    fn decodes_concat_metadata_from_deliver() {
        let sms = decode_sms_deliver("00440A91214365870900080000000000000008050003CC02014F60").unwrap();
        assert_eq!(sms.body, "你");
        let concat = sms.concat.unwrap();
        assert_eq!(concat.reference, 0xcc);
        assert_eq!(concat.total, 2);
        assert_eq!(concat.sequence, 1);
    }

    #[test]
    fn assembler_orders_parts() {
        let mut assembler = MultipartAssembler::default();
        assert!(assembler.push(DecodedSms { sender: "+1".into(), body: "B".into(), concat: Some(ConcatInfo { reference: 7, total: 2, sequence: 2 }) }).is_none());
        let combined = assembler.push(DecodedSms { sender: "+1".into(), body: "A".into(), concat: Some(ConcatInfo { reference: 7, total: 2, sequence: 1 }) }).unwrap();
        assert_eq!(combined.1, "AB");
    }
}
