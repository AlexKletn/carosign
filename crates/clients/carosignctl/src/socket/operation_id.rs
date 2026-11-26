use carosignctl::client::ClientError;

pub(super) fn parse_operation_id(input: &str) -> Result<[u8; 16], ClientError> {
    if input.len() != 32 || !input.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ClientError::Refused {
            message: "укажите полный идентификатор операции: 32 шестнадцатеричных символа".into(),
        });
    }
    let mut id = [0u8; 16];
    for (index, chunk) in input.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(chunk).map_err(|_| ClientError::Unexpected)?;
        id[index] = u8::from_str_radix(pair, 16).map_err(|_| ClientError::Unexpected)?;
    }
    if id.iter().all(|byte| *byte == 0) {
        return Err(ClientError::Refused {
            message: "нулевой идентификатор операции недопустим".into(),
        });
    }
    Ok(id)
}
