//! HTML email sanitization using ammonia for robust tag/attribute filtering,
//! plus external image blocking.

use std::collections::{HashMap, HashSet};

use ammonia::Builder;

/// Build an ammonia sanitizer configured for safe email display.
/// If `remote_images` is false, `src` attributes pointing to http/https are
/// stripped (only `data:` and `cid:` are kept).
fn email_sanitizer(remote_images: bool) -> Builder<'static> {
    let mut builder = Builder::default();

    // Allowed tags
    let tags: HashSet<&str> = [
        "p", "div", "span", "a", "img", "table", "tr", "td", "th", "thead", "tbody", "tfoot",
        "br", "hr", "ul", "ol", "li", "b", "i", "u", "strong", "em",
        "h1", "h2", "h3", "h4", "h5", "h6",
        "blockquote", "pre", "code", "sup", "sub", "dl", "dt", "dd",
        "caption", "colgroup", "col", "abbr", "address",
    ]
    .into_iter()
    .collect();
    builder.tags(tags);

    // Allowed attributes per tag
    let generic_attrs: HashSet<&str> = ["class", "style", "title"].into_iter().collect();
    builder.generic_attributes(generic_attrs);

    let mut tag_attrs: HashMap<&str, HashSet<&str>> = HashMap::new();
    tag_attrs.insert(
        "a",
        ["href", "title", "class", "style"].into_iter().collect(),
    );
    tag_attrs.insert(
        "img",
        ["src", "alt", "title", "width", "height", "class", "style"]
            .into_iter()
            .collect(),
    );
    tag_attrs.insert(
        "td",
        ["colspan", "rowspan", "class", "style", "width", "height"]
            .into_iter()
            .collect(),
    );
    tag_attrs.insert(
        "th",
        ["colspan", "rowspan", "class", "style", "width", "height"]
            .into_iter()
            .collect(),
    );
    tag_attrs.insert(
        "table",
        ["class", "style", "width", "cellpadding", "cellspacing", "border"]
            .into_iter()
            .collect(),
    );
    builder.tag_attributes(tag_attrs);

    // Strip javascript: from URL attributes
    let url_schemes: HashSet<&str> = if remote_images {
        ["http", "https", "mailto", "data", "cid"]
            .into_iter()
            .collect()
    } else {
        ["mailto", "data", "cid"].into_iter().collect()
    };
    builder.url_schemes(url_schemes);

    // Link rel — force noopener noreferrer on links
    builder.link_rel(Some("noopener noreferrer"));

    builder
}

/// Sanitize HTML for safe display, blocking remote images by default.
/// Also strips tracking pixels and UTM parameters from links.
#[allow(dead_code)]
pub fn sanitize_html(html: &str) -> String {
    let cleaned = crate::mail::anti_tracking::strip_tracking(html);
    email_sanitizer(false).clean(&cleaned).to_string()
}

/// Re-sanitize allowing remote images (http/https src kept).
/// Also strips tracking pixels and UTM parameters from links.
#[allow(dead_code)]
pub fn allow_images(original_html: &str) -> String {
    let cleaned = crate::mail::anti_tracking::strip_tracking(original_html);
    email_sanitizer(true).clean(&cleaned).to_string()
}
