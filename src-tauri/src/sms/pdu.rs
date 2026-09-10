use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Concatenation {
    pub reference: u16,
    pub total: u8,
    pub sequence: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DecodedPdu {
    pub sender: String,
    pub body: String,
    pub timestamp: Option<String>,
    pub concatenation: Option<Concatenation>,
    pub dcs: u8,
}

pub fn decode_deliver_pdu(hex: &str) -> Result<DecodedPdu, String> {
    let bytes = hex_to_bytes(hex)?;
    let mut cursor = 0usize;

    let smsc_len = take(&bytes, &mut cursor, 1)?[0] as usize;
    take(&bytes, &mut cursor, smsc_len)?;

    let first_octet = take(&bytes, &mut cursor, 1)?[0];
    if first_octet & 0x03 != 0 {
        return Err("PDU is not an SMS-DELIVER TPDU".into());
    }
    let has_udh = first_octet & 0x40 != 0;

    let address_len = take(&bytes, &mut cursor, 1)?[0] as usize;
    let toa = take(&bytes, &mut cursor, 1)?[0];
    let address_octets = (address_len + 1) / 2;
    let address = take(&bytes, &mut cursor, address_octets)?;
    let sender = if toa & 0x70 == 0x50 {
        let septets = address_len.saturating_mul(4) / 7;
        decode_gsm7(address, septets, 0)?
    } else {
        let mut value = decode_semi_octets(address, address_len);
        if toa & 0x70 == 0x10 {
            value.insert(0, '+');
        }
        value
    };

    // TP-PID is currently informational only.
    take(&bytes, &mut cursor, 1)?;
    let dcs = take(&bytes, &mut cursor, 1)?[0];
    let scts = take(&bytes, &mut cursor, 7)?;
    let timestamp = decode_timestamp(scts);
    let user_data_len = take(&bytes, &mut cursor, 1)?[0] as usize;
    let user_data = &bytes[cursor..];

    let (header_octets, concatenation) = if has_udh {
        parse_udh(user_data)?
    } else {
        (0, None)
    };

    let body = match dcs & 0x0C {
        0x08 => {
            if user_data_len < header_octets {
                return Err("UCS-2 user data length is shorter than its UDH".into());
            }
            let octets = user_data_len - header_octets;
            decode_ucs2(take_slice(user_data, header_octets, octets)?)?
        }
        0x04 => {
            if user_data_len < header_octets {
                return Err("8-bit user data length is shorter than its UDH".into());
            }
            let octets = user_data_len - header_octets;
            let payload = take_slice(user_data, header_octets, octets)?;
            format!("[binary] {}", encode_hex(payload))
        }
        _ => {
            let header_septets = if has_udh {
                (header_octets * 8 + 6) / 7
            } else {
                0
            };
            if user_data_len < header_septets {
                return Err("GSM 7-bit user data length is shorter than its UDH".into());
            }
            decode_gsm7(user_data, user_data_len - header_septets, header_septets * 7)?
        }
    };

    Ok(DecodedPdu {
        sender,
        body,
        timestamp,
        concatenation,
        dcs,
    })
}

fn hex_to_bytes(input: &str) -> Result<Vec<u8>, String> {
    let compact: String = input.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.len() % 2 != 0 {
        return Err("PDU hex has an odd number of digits".into());
    }
    compact
        .as_bytes()
        .chunks_exact(2)
        .enumerate()
        .map(|(index, pair)| {
            let text = std::str::from_utf8(pair).map_err(|_| "PDU contains non-ASCII hex".to_string())?;
            u8::from_str_radix(text, 16)
                .map_err(|_| format!("invalid hex byte at offset {}", index * 2))
        })
        .collect()
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8], String> {
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| "PDU cursor overflow".to_string())?;
    if end > bytes.len() {
        return Err("truncated PDU".into());
    }
    let slice = &bytes[*cursor..end];
    *cursor = end;
    Ok(slice)
}

fn take_slice(bytes: &[u8], start: usize, len: usize) -> Result<&[u8], String> {
    let end = start
        .checked_add(len)
        .ok_or_else(|| "PDU slice overflow".to_string())?;
    bytes.get(start..end).ok_or_else(|| "truncated user data".into())
}

fn decode_semi_octets(bytes: &[u8], digits: usize) -> String {
    let mut output = String::with_capacity(digits);
    for byte in bytes {
        for nibble in [byte & 0x0F, byte >> 4] {
            if output.len() >= digits {
                return output;
            }
            if nibble <= 9 {
                output.push((b'0' + nibble) as char);
            }
        }
    }
    output
}

fn decode_timestamp(bytes: &[u8]) -> Option<String> {
    if bytes.len() != 7 {
        return None;
    }
    let bcd = |byte: u8| -> Option<u8> {
        let low = byte & 0x0F;
        let high = byte >> 4;
        (low <= 9 && high <= 9).then_some(low * 10 + high)
    };
    let year = bcd(bytes[0])?;
    let month = bcd(bytes[1])?;
    let day = bcd(bytes[2])?;
    let hour = bcd(bytes[3])?;
    let minute = bcd(bytes[4])?;
    let second = bcd(bytes[5])?;
    Some(format!("20{year:02}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"))
}

fn parse_udh(user_data: &[u8]) -> Result<(usize, Option<Concatenation>), String> {
    let udhl = *user_data.first().ok_or_else(|| "missing UDH length".to_string())? as usize;
    let end = 1usize
        .checked_add(udhl)
        .ok_or_else(|| "UDH length overflow".to_string())?;
    if end > user_data.len() {
        return Err("truncated UDH".into());
    }

    let mut cursor = 1usize;
    let mut concatenation = None;
    while cursor < end {
        if cursor + 2 > end {
            return Err("truncated UDH information element".into());
        }
        let iei = user_data[cursor];
        let len = user_data[cursor + 1] as usize;
        cursor += 2;
        if cursor + len > end {
            return Err("truncated UDH information element payload".into());
        }
        let payload = &user_data[cursor..cursor + len];
        match (iei, payload) {
            (0x00, [reference, total, sequence]) => {
                concatenation = Some(Concatenation {
                    reference: *reference as u16,
                    total: *total,
                    sequence: *sequence,
                });
            }
            (0x08, [hi, lo, total, sequence]) => {
                concatenation = Some(Concatenation {
                    reference: u16::from_be_bytes([*hi, *lo]),
                    total: *total,
                    sequence: *sequence,
                });
            }
            _ => {}
        }
        cursor += len;
    }
    Ok((end, concatenation))
}

fn decode_ucs2(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() % 2 != 0 {
        return Err("UCS-2 payload has an odd byte count".into());
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units).map_err(|_| "invalid UCS-2/UTF-16 payload".into())
}

fn decode_gsm7(bytes: &[u8], septet_count: usize, start_bit: usize) -> Result<String, String> {
    let mut output = String::new();
    let mut escaped = false;
    for septet_index in 0..septet_count {
        let bit_index = start_bit + septet_index * 7;
        let byte_index = bit_index / 8;
        let bit_offset = bit_index % 8;
        let first = *bytes
            .get(byte_index)
            .ok_or_else(|| "truncated GSM 7-bit payload".to_string())? as u16;
        let second = bytes.get(byte_index + 1).copied().unwrap_or(0) as u16;
        let septet = ((first | (second << 8)) >> bit_offset) as u8 & 0x7F;

        if escaped {
            output.push(gsm7_extension(septet).unwrap_or('�'));
            escaped = false;
        } else if septet == 0x1B {
            escaped = true;
        } else {
            output.push(gsm7_default(septet));
        }
    }
    if escaped {
        output.push('�');
    }
    Ok(output)
}

fn gsm7_default(value: u8) -> char {
    const TABLE: [char; 128] = [
        '@', '£', '$', '¥', 'è', 'é', 'ù', 'ì', 'ò', 'Ç', '\n', 'Ø', 'ø', '\r', 'Å', 'å',
        'Δ', '_', 'Φ', 'Γ', 'Λ', 'Ω', 'Π', 'Ψ', 'Σ', 'Θ', 'Ξ', '�', 'Æ', 'æ', 'ß', 'É',
        ' ', '!', '"', '#', '¤', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/',
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?',
        '¡', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O',
        'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'Ä', 'Ö', 'Ñ', 'Ü', '§',
        '¿', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o',
        'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'ä', 'ö', 'ñ', 'ü', 'à',
    ];
    TABLE[value as usize]
}

fn gsm7_extension(value: u8) -> Option<char> {
    match value {
        0x0A => Some('\u{000C}'),
        0x14 => Some('^'),
        0x28 => Some('{'),
        0x29 => Some('}'),
        0x2F => Some('\\'),
        0x3C => Some('['),
        0x3D => Some('~'),
        0x3E => Some(']'),
        0x40 => Some('|'),
        0x65 => Some('€'),
        _ => None,
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0F) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_gsm7_hello() {
        assert_eq!(decode_gsm7(&[0xE8, 0x32, 0x9B, 0xFD, 0x06], 5, 0).unwrap(), "hello");
    }

    #[test]
    fn decodes_ucs2_chinese() {
        assert_eq!(decode_ucs2(&[0x4F, 0x60, 0x59, 0x7D]).unwrap(), "你好");
    }

    #[test]
    fn decodes_numeric_semi_octets() {
        assert_eq!(decode_semi_octets(&[0x21, 0x43, 0xF5], 5), "12345");
    }

    #[test]
    fn parses_8_bit_concat_udh() {
        let (len, concat) = parse_udh(&[0x05, 0x00, 0x03, 0x7A, 0x02, 0x01]).unwrap();
        assert_eq!(len, 6);
        assert_eq!(
            concat,
            Some(Concatenation {
                reference: 0x7A,
                total: 2,
                sequence: 1,
            })
        );
    }

    #[test]
    fn parses_16_bit_concat_udh() {
        let (len, concat) = parse_udh(&[0x06, 0x08, 0x04, 0x12, 0x34, 0x02, 0x01]).unwrap();
        assert_eq!(len, 7);
        assert_eq!(
            concat,
            Some(Concatenation {
                reference: 0x1234,
                total: 2,
                sequence: 1,
            })
        );
    }

    #[test]
    fn rejects_non_deliver_tpdu() {
        // Empty SMSC followed by an SMS-SUBMIT first octet.
        assert!(decode_deliver_pdu("0001").is_err());
    }
}
