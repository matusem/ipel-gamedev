use dioxus::prelude::*;

/// Lightweight syntax-colored JSON display (CSS token classes, no parser).
#[component]
pub fn JsonConsole(content: String, max_height: Option<&'static str>) -> Element {
    let height = max_height.unwrap_or("max-h-64");
    let colored = colorize_json(&content);
    rsx! {
        pre {
            class: "json-console {height} overflow-auto",
            dangerous_inner_html: "{colored}"
        }
    }
}

fn colorize_json(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() * 2);
    let mut i = 0;
    let bytes = raw.as_bytes();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let slice = &raw[start..i];
            let is_key = raw[i..].trim_start().starts_with(':');
            let class = if is_key { "json-key" } else { "json-string" };
            out.push_str(&format!("<span class=\"{class}\">"));
            out.push_str(&html_escape(slice));
            out.push_str("</span>");
        } else if c.is_ascii_digit() || (c == '-' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            if c == '-' {
                i += 1;
            }
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            out.push_str("<span class=\"json-number\">");
            out.push_str(&html_escape(&raw[start..i]));
            out.push_str("</span>");
        } else if raw[i..].starts_with("true") || raw[i..].starts_with("false") {
            let len = if raw[i..].starts_with("true") { 4 } else { 5 };
            out.push_str("<span class=\"json-bool\">");
            out.push_str(&raw[i..i + len]);
            out.push_str("</span>");
            i += len;
        } else if raw[i..].starts_with("null") {
            out.push_str("<span class=\"json-null\">null</span>");
            i += 4;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
