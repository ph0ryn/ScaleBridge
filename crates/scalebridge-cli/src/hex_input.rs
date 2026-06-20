pub fn decode_hex_packet(packet_hex: &str) -> Result<Vec<u8>, String> {
    let mut cleaned = String::new();

    for token in packet_hex.split(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ',' | ':' | '-')
    }) {
        let normalized = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);
        cleaned.push_str(normalized);
    }

    if cleaned.is_empty() {
        return Err("packet hex must not be empty".to_string());
    }

    if cleaned.len() % 2 != 0 {
        return Err("packet hex must contain an even number of digits".to_string());
    }

    hex::decode(cleaned).map_err(|error| format!("invalid packet hex: {error}"))
}
