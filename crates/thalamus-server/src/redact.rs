//! Redaction of secret-looking material before operational logs (§3
//! security: redaction before operational logs). Applied to free-text error
//! detail that may echo upstream request/response fragments. Audit events are
//! metadata-only by construction and do not pass through here.

const SECRET_PREFIXES: &[&str] = &[
    "sk-", "rbxsess_", "sess_", "eyJ", // JWT header
];

const SECRET_KEYS: &[&str] = &["api_key=", "api-key=", "apikey=", "token=", "password="];

/// Mask secret-looking tokens in free text. Conservative: masks the token
/// tail, keeps enough prefix to identify the kind. The word after a
/// `Bearer` marker is always masked.
pub fn redact(input: &str) -> String {
    let mut out = Vec::new();
    let mut mask_next = false;
    for word in input.split(' ') {
        if mask_next && !word.is_empty() {
            out.push("[REDACTED]".to_owned());
            mask_next = false;
            continue;
        }
        if word.eq_ignore_ascii_case("bearer") {
            mask_next = true;
            out.push(word.to_owned());
            continue;
        }
        out.push(redact_word(word));
    }
    out.join(" ")
}

fn redact_word(word: &str) -> String {
    for prefix in SECRET_PREFIXES {
        if let Some(rest) = word.strip_prefix(prefix) {
            if rest.len() > 2 {
                return format!("{}[REDACTED]", prefix.trim_end());
            }
        }
    }
    for key in SECRET_KEYS {
        if let Some(pos) = word.to_ascii_lowercase().find(key) {
            let split = pos + key.len();
            return format!("{}[REDACTED]", &word[..split]);
        }
    }
    word.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_bearer_tokens_and_key_material() {
        assert_eq!(
            redact("auth failed: Bearer abc123secret"),
            "auth failed: Bearer [REDACTED]"
        );
        assert_eq!(redact("bad key sk-live-123456"), "bad key sk-[REDACTED]");
        assert_eq!(
            redact("url?api_key=supersecret&x=1"),
            "url?api_key=[REDACTED]"
        );
        assert_eq!(
            redact("session rbxsess_abcdef"),
            "session rbxsess_[REDACTED]"
        );
        assert_eq!(
            redact("jwt eyJhbGciOiJIUzI1NiJ9.payload.sig"),
            "jwt eyJ[REDACTED]"
        );
    }

    #[test]
    fn leaves_ordinary_text_untouched() {
        let text = "backend returned status 502 for model glm-5.2";
        assert_eq!(redact(text), text);
    }
}
