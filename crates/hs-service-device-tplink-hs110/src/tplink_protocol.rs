pub fn encrypt(payload: &[u8]) -> Vec<u8> {
    let mut key = 0xABu8;
    let mut output = Vec::with_capacity(payload.len());

    for byte in payload {
        let encrypted = key ^ *byte;
        key = encrypted;
        output.push(encrypted);
    }

    output
}

pub fn decrypt(payload: &[u8]) -> Vec<u8> {
    let mut key = 0xABu8;
    let mut output = Vec::with_capacity(payload.len());

    for byte in payload {
        let decrypted = key ^ *byte;
        key = *byte;
        output.push(decrypted);
    }

    output
}

pub fn frame_request(plaintext_json: &str) -> Vec<u8> {
    let encrypted = encrypt(plaintext_json.as_bytes());
    let mut frame = Vec::with_capacity(4 + encrypted.len());
    frame.extend_from_slice(&(encrypted.len() as u32).to_be_bytes());
    frame.extend_from_slice(&encrypted);
    frame
}

pub fn parse_response_payload(frame: &[u8]) -> Option<Vec<u8>> {
    if frame.len() < 4 {
        return None;
    }

    let expected = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    if frame.len() < 4 + expected {
        return None;
    }

    Some(decrypt(&frame[4..4 + expected]))
}

#[cfg(test)]
mod tests {
    use super::{decrypt, encrypt, frame_request, parse_response_payload};

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let input = b"{\"system\":{\"get_sysinfo\":{}}}";
        let encrypted = encrypt(input);
        let decrypted = decrypt(&encrypted);
        assert_eq!(decrypted, input);
    }

    #[test]
    fn framed_payload_parses() {
        let frame = frame_request("{\"foo\":\"bar\"}");
        let payload = parse_response_payload(&frame).expect("payload should parse");
        assert_eq!(payload, b"{\"foo\":\"bar\"}");
    }
}
