use anchor_lang::prelude::*;

/// Parameters for reconstructing clientDataJSON on-chain.
/// Packed into a single byte + optional port to minimize storage.
///
/// Only used by the precompile verification path for P256/WebAuthn signers.
#[derive(
    AnchorSerialize,
    AnchorDeserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Default,
)]
pub struct ClientDataJsonReconstructionParams {
    /// High 4 bits: type (0x00 = create, 0x10 = get)
    /// Low 4 bits: flags (cross_origin, http, google_extra)
    pub type_and_flags: u8,
    /// Optional port number (0 means no port)
    pub port: u16,
}

impl ClientDataJsonReconstructionParams {
    pub const TYPE_CREATE: u8 = 0x00;
    pub const TYPE_GET: u8 = 0x10;
    pub const FLAG_CROSS_ORIGIN: u8 = 0x01;
    pub const FLAG_HTTP_ORIGIN: u8 = 0x02;
    pub const FLAG_GOOGLE_EXTRA: u8 = 0x04;

    pub fn new(
        is_create: bool,
        cross_origin: bool,
        http_origin: bool,
        google_extra: bool,
        port: Option<u16>,
    ) -> Self {
        let type_bits = if is_create { Self::TYPE_CREATE } else { Self::TYPE_GET };
        let mut flags = 0u8;
        if cross_origin { flags |= Self::FLAG_CROSS_ORIGIN; }
        if http_origin { flags |= Self::FLAG_HTTP_ORIGIN; }
        if google_extra { flags |= Self::FLAG_GOOGLE_EXTRA; }

        Self {
            type_and_flags: type_bits | flags,
            port: port.unwrap_or(0),
        }
    }

    /// Returns true if the type nibble is valid (TYPE_CREATE or TYPE_GET)
    /// and no undefined flag bits are set (only bits 0-2 are valid flags).
    pub fn is_valid_type(&self) -> bool {
        let type_nibble = self.type_and_flags & 0xF0;
        let flags_nibble = self.type_and_flags & 0x0F;
        let valid_type = type_nibble == Self::TYPE_CREATE || type_nibble == Self::TYPE_GET;
        let valid_flags = flags_nibble & 0x08 == 0; // bit 3 is undefined
        valid_type && valid_flags
    }

    pub fn is_create(&self) -> bool {
        (self.type_and_flags & 0xF0) == Self::TYPE_CREATE
    }

    pub fn is_cross_origin(&self) -> bool {
        (self.type_and_flags & Self::FLAG_CROSS_ORIGIN) != 0
    }

    pub fn is_http(&self) -> bool {
        (self.type_and_flags & Self::FLAG_HTTP_ORIGIN) != 0
    }

    pub fn has_google_extra(&self) -> bool {
        (self.type_and_flags & Self::FLAG_GOOGLE_EXTRA) != 0
    }

    pub fn get_port(&self) -> Option<u16> {
        if self.port == 0 { None } else { Some(self.port) }
    }
}

/// Base64URL alphabet for encoding
const BASE64URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encode bytes as base64url (no padding)
fn base64url_encode(input: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity((input.len() * 4 + 2) / 3);

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        output.push(BASE64URL_ALPHABET[b0 >> 2]);
        output.push(BASE64URL_ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)]);

        if chunk.len() > 1 {
            output.push(BASE64URL_ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)]);
        }
        if chunk.len() > 2 {
            output.push(BASE64URL_ALPHABET[b2 & 0x3f]);
        }
    }

    output
}

/// Reconstruct clientDataJSON from compact parameters
///
/// This reconstructs the full clientDataJSON that was hashed to produce clientDataHash.
/// The format is:
/// {"type":"webauthn.get","challenge":"<base64url>","origin":"<origin>","crossOrigin":<bool>}
pub fn reconstruct_client_data_json(
    params: &ClientDataJsonReconstructionParams,
    rp_id: &[u8],
    challenge: &[u8],
) -> Result<Vec<u8>> {
    // Validate rp_id bytes are safe for JSON embedding (ASCII printable, no quotes/backslash).
    // RP IDs are domain names per RFC 6454 and cannot contain these characters.
    for &b in rp_id {
        if b < 0x20 || b == b'"' || b == b'\\' || b > 0x7e {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
    }

    let mut json = Vec::with_capacity(256);

    // Start JSON object
    json.extend_from_slice(b"{\"type\":\"webauthn.");

    // Type: "create" or "get"
    if params.is_create() {
        json.extend_from_slice(b"create");
    } else {
        json.extend_from_slice(b"get");
    }

    // Challenge (base64url encoded)
    json.extend_from_slice(b"\",\"challenge\":\"");
    let encoded_challenge = base64url_encode(challenge);
    json.extend_from_slice(&encoded_challenge);

    // Origin
    json.extend_from_slice(b"\",\"origin\":\"");
    if params.is_http() {
        json.extend_from_slice(b"http://");
    } else {
        json.extend_from_slice(b"https://");
    }
    json.extend_from_slice(rp_id);

    // Optional port
    if let Some(port) = params.get_port() {
        json.push(b':');
        // Convert port to string bytes
        let port_str = port.to_string();
        json.extend_from_slice(port_str.as_bytes());
    }

    // Cross-origin
    json.extend_from_slice(b"\",\"crossOrigin\":");
    if params.is_cross_origin() {
        json.extend_from_slice(b"true");
    } else {
        json.extend_from_slice(b"false");
    }

    // Google extra field (some authenticators add this)
    if params.has_google_extra() {
        json.extend_from_slice(b",\"androidPackageName\":\"com.google.android.gms\"");
    }

    // Close JSON object
    json.push(b'}');

    Ok(json)
}
