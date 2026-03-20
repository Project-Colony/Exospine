//! HTML email sanitization and image blocking.
//!
//! Uses simple string scanning (split by '<' and '>') instead of regex
//! to strip dangerous tags and block external images.

/// Tags that should be completely removed (along with their content).
const DANGEROUS_TAGS: &[&str] = &["script", "style", "iframe", "object", "embed", "form"];

/// Event handler attribute prefixes to strip.
const EVENT_PREFIXES: &[&str] = &[
    "onclick",
    "onerror",
    "onload",
    "onmouseover",
    "onmouseout",
    "onfocus",
    "onblur",
    "onsubmit",
    "onchange",
    "oninput",
    "onkeydown",
    "onkeyup",
    "onkeypress",
    "ondblclick",
    "oncontextmenu",
    "onresize",
    "onscroll",
    "onunload",
    "onbeforeunload",
];

/// The placeholder prefix used when blocking external images.
const IMAGE_BLOCKED_PREFIX: &str = "[Image blocked: ";
const IMAGE_BLOCKED_SUFFIX: &str = "]";

/// Sanitize HTML for safe display.
///
/// - Strips dangerous tags (`<script>`, `<style>`, `<iframe>`, `<object>`, `<embed>`, `<form>`)
///   and all content between their opening and closing tags.
/// - Removes event handler attributes (onclick, onerror, etc.).
/// - Blocks external images by replacing `<img src="http...">` with placeholder text.
pub fn sanitize_html(html: &str) -> String {
    let after_dangerous = strip_dangerous_tags(html);
    let after_events = strip_event_handlers(&after_dangerous);
    block_external_images(&after_events)
}

/// Re-enable images by replacing `[Image blocked: URL]` placeholders
/// back with `<img src="URL">` tags.
pub fn allow_images(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut remaining = html;

    while let Some(start) = remaining.find(IMAGE_BLOCKED_PREFIX) {
        result.push_str(&remaining[..start]);
        let after_prefix = &remaining[start + IMAGE_BLOCKED_PREFIX.len()..];
        if let Some(end) = after_prefix.find(IMAGE_BLOCKED_SUFFIX) {
            let url = &after_prefix[..end];
            result.push_str("<img src=\"");
            result.push_str(url);
            result.push_str("\">");
            remaining = &after_prefix[end + IMAGE_BLOCKED_SUFFIX.len()..];
        } else {
            // Malformed placeholder — keep as-is
            result.push_str(&remaining[start..]);
            remaining = "";
        }
    }
    result.push_str(remaining);
    result
}

/// Strip dangerous tags and everything between their open/close pairs.
fn strip_dangerous_tags(html: &str) -> String {
    let mut result = html.to_string();
    for tag in DANGEROUS_TAGS {
        result = remove_tag_with_content(&result, tag);
    }
    result
}

/// Remove all occurrences of `<tag ...>...</tag>` (case-insensitive).
/// Also removes self-closing variants like `<tag .../>`.
fn remove_tag_with_content(html: &str, tag: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let lower = html.to_lowercase();
    let open_pattern = format!("<{}", tag);
    let close_pattern = format!("</{}", tag);

    let mut pos = 0;
    while pos < html.len() {
        let lower_remaining = &lower[pos..];
        if let Some(open_start) = lower_remaining.find(&open_pattern) {
            let abs_open = pos + open_start;
            // Verify next char after tag name is whitespace, '>', or '/'
            let after_tag = abs_open + open_pattern.len();
            if after_tag < html.len() {
                let next_char = html.as_bytes()[after_tag];
                if next_char != b' '
                    && next_char != b'>'
                    && next_char != b'/'
                    && next_char != b'\t'
                    && next_char != b'\n'
                    && next_char != b'\r'
                {
                    // Not actually our tag (e.g. <scripting>)
                    result.push_str(&html[pos..=abs_open]);
                    pos = abs_open + 1;
                    continue;
                }
            }

            // Add everything before this tag
            result.push_str(&html[pos..abs_open]);

            // Find the closing tag
            let search_from = abs_open + 1;
            let lower_after = &lower[search_from..];
            if let Some(close_start) = lower_after.find(&close_pattern) {
                let abs_close = search_from + close_start;
                // Find the '>' that ends the closing tag
                if let Some(close_end) = html[abs_close..].find('>') {
                    pos = abs_close + close_end + 1;
                } else {
                    pos = html.len();
                }
            } else {
                // No closing tag — just skip to end of opening tag
                if let Some(open_end) = html[abs_open..].find('>') {
                    pos = abs_open + open_end + 1;
                } else {
                    pos = html.len();
                }
            }
        } else {
            result.push_str(&html[pos..]);
            break;
        }
    }
    result
}

/// Remove event handler attributes from all tags.
fn strip_event_handlers(html: &str) -> String {
    let mut result = String::with_capacity(html.len());

    let parts: Vec<&str> = html.split('<').collect();

    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            // Text before the first '<'
            result.push_str(part);
            continue;
        }

        // Find the end of the tag
        if let Some(gt_pos) = part.find('>') {
            let tag_content = &part[..gt_pos];
            let after_tag = &part[gt_pos + 1..];

            let cleaned_tag = remove_event_attrs_from_tag(tag_content);
            result.push('<');
            result.push_str(&cleaned_tag);
            result.push('>');
            result.push_str(after_tag);
        } else {
            // No closing '>' found — might be malformed, keep as-is
            result.push('<');
            result.push_str(part);
        }
    }

    result
}

/// Remove event handler attributes from a single tag's content (between < and >).
fn remove_event_attrs_from_tag(tag_content: &str) -> String {
    let lower = tag_content.to_lowercase();

    // Check if any event prefix is present before doing expensive work
    let has_event = EVENT_PREFIXES.iter().any(|p| lower.contains(p));
    if !has_event {
        return tag_content.to_string();
    }

    // Parse attributes more carefully
    let mut result = String::with_capacity(tag_content.len());
    let mut remaining = tag_content;

    // Get the tag name first (everything up to first space)
    let first_space = remaining
        .find(|c: char| c.is_whitespace())
        .unwrap_or(remaining.len());
    result.push_str(&remaining[..first_space]);
    remaining = &remaining[first_space..];

    // Now process attributes
    let lower_remaining_full = remaining.to_lowercase();
    let mut i = 0;
    let bytes = remaining.as_bytes();

    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            // Check if next non-whitespace starts an event handler
            let trimmed_start = remaining[i..]
                .find(|c: char| !c.is_whitespace())
                .map(|off| i + off)
                .unwrap_or(bytes.len());

            if trimmed_start < bytes.len() {
                let lower_from_here = &lower_remaining_full[trimmed_start..];
                let is_event = EVENT_PREFIXES
                    .iter()
                    .any(|p| lower_from_here.starts_with(p));

                if is_event {
                    // Skip this attribute: find the end of its value
                    // Attribute format: name="value" or name='value' or name=value
                    let eq_pos = remaining[trimmed_start..]
                        .find('=')
                        .map(|off| trimmed_start + off);
                    if let Some(eq) = eq_pos {
                        let after_eq = eq + 1;
                        if after_eq < bytes.len() {
                            let quote = bytes[after_eq];
                            if quote == b'"' || quote == b'\'' {
                                // Find matching closing quote
                                if let Some(end_quote) =
                                    remaining[after_eq + 1..].find(quote as char)
                                {
                                    i = after_eq + 1 + end_quote + 1;
                                    continue;
                                }
                            }
                            // Unquoted value: skip to next whitespace
                            let end = remaining[after_eq..]
                                .find(|c: char| c.is_whitespace())
                                .map(|off| after_eq + off)
                                .unwrap_or(bytes.len());
                            i = end;
                            continue;
                        }
                    }
                    // No '=' found, just skip the attribute name
                    let end = remaining[trimmed_start..]
                        .find(|c: char| c.is_whitespace())
                        .map(|off| trimmed_start + off)
                        .unwrap_or(bytes.len());
                    i = end;
                    continue;
                }
            }

            result.push(bytes[i] as char);
            i += 1;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

/// Replace external image tags with placeholder text.
/// Matches `<img` tags whose `src` starts with `http://` or `https://`.
fn block_external_images(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let lower = html.to_lowercase();
    let mut pos = 0;

    while pos < html.len() {
        let remaining_lower = &lower[pos..];
        if let Some(img_start) = remaining_lower.find("<img") {
            let abs_img = pos + img_start;
            // Verify it's actually an <img tag
            let after_tag = abs_img + 4;
            if after_tag < html.len() {
                let next = html.as_bytes()[after_tag];
                if next != b' ' && next != b'>' && next != b'/' && next != b'\t' && next != b'\n'
                {
                    result.push_str(&html[pos..after_tag]);
                    pos = after_tag;
                    continue;
                }
            }

            // Find the end of this img tag
            let tag_end = html[abs_img..]
                .find('>')
                .map(|off| abs_img + off)
                .unwrap_or(html.len() - 1);

            let tag_content = &html[abs_img..=tag_end];
            // Extract the src attribute
            if let Some(url) = extract_src_url(tag_content) {
                let url_lower = url.to_lowercase();
                if url_lower.starts_with("http://") || url_lower.starts_with("https://") {
                    // Block this external image
                    result.push_str(&html[pos..abs_img]);
                    result.push_str(IMAGE_BLOCKED_PREFIX);
                    result.push_str(&url);
                    result.push_str(IMAGE_BLOCKED_SUFFIX);
                    pos = tag_end + 1;
                    continue;
                }
            }

            // Not an external image, keep as-is
            result.push_str(&html[pos..=tag_end]);
            pos = tag_end + 1;
        } else {
            result.push_str(&html[pos..]);
            break;
        }
    }

    result
}

/// Extract the value of the `src` attribute from a tag string.
fn extract_src_url(tag: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let src_pos = lower.find("src=")?;
    let after_src = src_pos + 4;
    let bytes = tag.as_bytes();

    if after_src >= bytes.len() {
        return None;
    }

    let quote = bytes[after_src];
    if quote == b'"' || quote == b'\'' {
        let start = after_src + 1;
        let end = tag[start..].find(quote as char)?;
        Some(tag[start..start + end].to_string())
    } else {
        // Unquoted src
        let start = after_src;
        let end = tag[start..]
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(tag.len() - start);
        Some(tag[start..start + end].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_script_tags() {
        let html = "<p>Hello</p><script>alert('xss')</script><p>World</p>";
        let result = sanitize_html(html);
        assert!(!result.contains("script"));
        assert!(!result.contains("alert"));
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn test_strip_style_tags() {
        let html = "<style>body { display: none; }</style><p>Visible</p>";
        let result = sanitize_html(html);
        assert!(!result.contains("style"));
        assert!(!result.contains("display"));
        assert!(result.contains("Visible"));
    }

    #[test]
    fn test_strip_event_handlers() {
        let html = r#"<div onclick="alert('xss')" class="safe">Content</div>"#;
        let result = sanitize_html(html);
        assert!(!result.contains("onclick"));
        assert!(!result.contains("alert"));
        assert!(result.contains("class=\"safe\""));
        assert!(result.contains("Content"));
    }

    #[test]
    fn test_block_external_images() {
        let html = r#"<p>Hi</p><img src="https://tracker.example.com/pixel.gif"><p>Bye</p>"#;
        let result = sanitize_html(html);
        assert!(!result.contains("<img"));
        assert!(result.contains("[Image blocked: https://tracker.example.com/pixel.gif]"));
        assert!(result.contains("Hi"));
        assert!(result.contains("Bye"));
    }

    #[test]
    fn test_allow_images_roundtrip() {
        let html = r#"<p>Hi</p><img src="https://example.com/photo.jpg"><p>Bye</p>"#;
        let sanitized = sanitize_html(html);
        assert!(sanitized.contains("[Image blocked:"));

        let restored = allow_images(&sanitized);
        assert!(restored.contains(r#"<img src="https://example.com/photo.jpg">"#));
        assert!(!restored.contains("[Image blocked:"));
    }

    #[test]
    fn test_keep_data_uri_images() {
        let html = r#"<img src="data:image/png;base64,abc123">"#;
        let result = sanitize_html(html);
        assert!(result.contains("data:image/png;base64,abc123"));
        assert!(!result.contains("[Image blocked:"));
    }

    #[test]
    fn test_strip_iframe() {
        let html = r#"<p>Before</p><iframe src="https://evil.com"></iframe><p>After</p>"#;
        let result = sanitize_html(html);
        assert!(!result.contains("iframe"));
        assert!(result.contains("Before"));
        assert!(result.contains("After"));
    }
}
