use encoding_rs::{Encoding, GB18030, GBK, UTF_8, WINDOWS_1252};
use std::fs;
use std::io;
use std::path::Path;

/// Text decoded from a file, plus the encoding and BOM needed to write it back
/// in its original form.
pub struct Decoded {
    pub text: String,
    pub encoding: &'static Encoding,
    pub bom: Vec<u8>,
}

pub fn read_file_auto(path: &Path) -> io::Result<Decoded> {
    let bytes = fs::read(path)?;
    Ok(decode_bytes(&bytes))
}

/// Decode with a degradation chain: BOM -> strict UTF-8 -> chardetng ->
/// GBK -> GB18030 -> WINDOWS-1252 (cannot fail, maps every byte).
pub fn decode_bytes(bytes: &[u8]) -> Decoded {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return Decoded {
            text: String::from_utf8_lossy(rest).into_owned(),
            encoding: UTF_8,
            bom: b"\xEF\xBB\xBF".to_vec(),
        };
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        let (text, _, _) = encoding_rs::UTF_16LE.decode(rest);
        return Decoded {
            text: text.into_owned(),
            encoding: encoding_rs::UTF_16LE,
            bom: b"\xFF\xFE".to_vec(),
        };
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        let (text, _, _) = encoding_rs::UTF_16BE.decode(rest);
        return Decoded {
            text: text.into_owned(),
            encoding: encoding_rs::UTF_16BE,
            bom: b"\xFE\xFF".to_vec(),
        };
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Decoded {
            text: text.to_owned(),
            encoding: UTF_8,
            bom: Vec::new(),
        };
    }
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    let guessed = detector.guess(locale_tld(), true);
    if guessed != UTF_8 {
        let (text, _, had_errors) = guessed.decode(bytes);
        if !had_errors {
            return Decoded {
                text: text.into_owned(),
                encoding: guessed,
                bom: Vec::new(),
            };
        }
    }
    for enc in [GBK, GB18030, WINDOWS_1252] {
        let (text, _, had_errors) = enc.decode(bytes);
        if !had_errors || enc == WINDOWS_1252 {
            return Decoded {
                text: text.into_owned(),
                encoding: enc,
                bom: Vec::new(),
            };
        }
    }
    unreachable!()
}

/// Map the OS locale's region to a chardetng TLD hint. Short CJK texts are
/// often ambiguous between GBK / Big5 / EUC-KR / Shift_JIS; the hint makes
/// detection resolve to the encoding the user's system actually uses.
fn locale_tld() -> Option<&'static [u8]> {
    let loc = sys_locale::get_locale()?;
    let region = loc.rsplit(['-', '_']).next()?.to_ascii_uppercase();
    match region.as_str() {
        "CN" => Some(b"cn"),
        "TW" => Some(b"tw"),
        "HK" | "MO" => Some(b"hk"),
        "JP" => Some(b"jp"),
        "KR" => Some(b"kr"),
        _ => None,
    }
}

/// Re-encode replacement text using the file's original encoding and BOM.
pub fn encode_back(decoded: &Decoded, new_text: &str) -> Vec<u8> {
    let (bytes, _, _) = decoded.encoding.encode(new_text);
    let mut out = decoded.bom.clone();
    out.extend_from_slice(&bytes);
    out
}
