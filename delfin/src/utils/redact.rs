//! Remove access tokens from text before it is logged.

/// Replaces the value of `ApiKey=`, `api_key=` and `X-Emby-Token=` query
/// parameters (and `Token="…"` header fragments) with `***`.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for key in ["ApiKey=", "api_key=", "X-Emby-Token=", "Token=\""] {
        let mut search_from = 0;
        while let Some(found) = out[search_from..].find(key) {
            let start = search_from + found + key.len();
            let end = out[start..]
                .find(|c: char| c == '&' || c == '"' || c == '\'' || c.is_whitespace() || c == ')')
                .map(|i| start + i)
                .unwrap_or(out.len());
            out.replace_range(start..end, "***");
            search_from = start + 3;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn redacts_query_and_header_tokens() {
        assert_eq!(
            redact("http://h/Videos/1/stream?static=true&ApiKey=abc123&x=1"),
            "http://h/Videos/1/stream?static=true&ApiKey=***&x=1"
        );
        assert_eq!(redact("ws://h/socket?api_key=zz"), "ws://h/socket?api_key=***");
        assert_eq!(redact("MediaBrowser Token=\"secret\""), "MediaBrowser Token=\"***\"");
        assert_eq!(redact("nothing here"), "nothing here");
    }
}
