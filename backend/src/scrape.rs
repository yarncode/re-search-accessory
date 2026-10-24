//! The public API does not expose a product's specification table, but the
//! product page embeds it in its Next.js flight payload as
//! `\"product_properties\":[{"name":"…","value":"…"}]`. That array is extracted
//! here so the detail drawer can show real specs.

use serde_json::{Value, json};

const MARKERS: [&str; 2] = [r#"\"product_properties\":["#, r#""product_properties":["#];

pub fn extract_specs(html: &str) -> Vec<Value> {
    let Some((payload, escaped)) = find_array(html) else {
        return Vec::new();
    };
    let decoded = format!(
        "[{}]",
        if escaped { unescape(payload) } else { payload.to_string() }
    );
    let Ok(items) = serde_json::from_str::<Vec<Value>>(&decoded) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        let name = item.get("name").and_then(Value::as_str).unwrap_or("").trim();
        let value = item
            .get("value")
            .map(|v| match v {
                Value::String(s) => s.trim().to_string(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => String::new(),
            })
            .unwrap_or_default();
        if !name.is_empty() && !value.is_empty() {
            out.push(json!({ "name": name, "value": value }));
        }
    }
    out
}

/// Locate the array that follows the first matching marker and return the raw
/// slice between the brackets plus whether the payload still carries flight
/// escaping (in which case quotes are written as `\"` and brackets inside
/// strings are impossible).
fn find_array(html: &str) -> Option<(&str, bool)> {
    let mut best: Option<(usize, bool)> = None;
    for (escaped, marker) in MARKERS.iter().enumerate() {
        if let Some(pos) = html.find(marker) {
            let start = pos + marker.len();
            if best.is_none_or(|(current, _)| start < current) {
                best = Some((start, escaped == 0));
            }
        }
    }
    let (start, escaped) = best?;

    let bytes = html.as_bytes();
    let mut depth = 1usize;
    let mut i = start;
    let mut in_string = false;
    while i < bytes.len() {
        let c = bytes[i];
        if escaped {
            // Every quote inside the escaped payload is `\"`, so only brackets
            // matter; skip the character after a backslash.
            match c {
                b'\\' => i += 1,
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((&html[start..i], true));
                    }
                }
                _ => {}
            }
        } else if in_string {
            match c {
                b'\\' => i += 1,
                b'"' => in_string = false,
                _ => {}
            }
        } else {
            match c {
                b'"' => in_string = true,
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((&html[start..i], false));
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Turn the flight-payload escaping (`\"`, `\\`, `\u003c`, …) back into JSON.
fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('b') | Some('f') => {}
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Some(decoded) = u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    out.push(decoded);
                }
            }
            Some(other) => out.push(other),
            None => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_escaped_specs() {
        let html = r#"<script>self.__next_f.push([1,"xx\"product_properties\":[{\"id\":1,\"name\":\"Điện áp cấp\",\"value\":\"5V\",\"sorted\":1},{\"id\":2,\"name\":\"RAM\",\"value\":\"20 KB\",\"sorted\":2}],\"crumbs\":[]" ])</script>"#;
        let specs = extract_specs(html);
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0]["name"], "Điện áp cấp");
        assert_eq!(specs[0]["value"], "5V");
        assert_eq!(specs[1]["value"], "20 KB");
    }

    #[test]
    fn extracts_plain_specs_and_unicode_escape() {
        let html = r#"{"product_properties":[{"name":"Giao tiếp","value":"USB\u003cbr\u003eUART"}]}"#;
        let specs = extract_specs(html);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0]["value"], "USB<br>UART");
    }

    #[test]
    fn missing_marker_is_empty() {
        assert!(extract_specs("<html>nothing here</html>").is_empty());
    }
}
