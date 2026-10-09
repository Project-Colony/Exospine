//! HTML email sanitization using ammonia for robust tag/attribute filtering,
//! plus tracking removal and remote content blocking.
//!
//! Every HTML body sent to the webview goes through [`serialize_sanitized`],
//! so no window ever receives the raw HTML of a message.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use ammonia::Builder;
use regex::Regex;
use serde::{Serialize, Serializer};

/// `src` of an `<img>` that points at the network. It is renamed to
/// `data-remote-src` before ammonia runs, so the image does not load but the
/// reading pane can restore it when the user clicks "Show images".
static REMOTE_IMG_SRC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(<img\b[^>]*?[\s/])src(\s*=\s*["']?\s*https?://)"#).expect("valid regex")
});

/// Build an ammonia sanitizer configured for safe email display.
fn email_sanitizer() -> Builder<'static> {
    let mut builder = Builder::default();

    // Allowed tags. `<style>` is left out on purpose: ammonia drops it with
    // its content, because its CSS is not sanitized and would apply to the
    // whole compose window. `class` and `id` go with it, so message markup
    // cannot pick up the app's own styles or shadow its element ids.
    let tags: HashSet<&str> = [
        "p", "div", "span", "a", "img", "table", "tr", "td", "th", "thead", "tbody", "tfoot",
        "br", "hr", "ul", "ol", "li", "b", "i", "u", "strong", "em",
        "h1", "h2", "h3", "h4", "h5", "h6",
        "blockquote", "pre", "code", "sup", "sub", "dl", "dt", "dd",
        "caption", "colgroup", "col", "abbr", "address",
        "center", "font", "big", "small", "strike", "s",
    ]
    .into_iter()
    .collect();
    builder.tags(tags);

    // Allowed attributes per tag
    let generic_attrs: HashSet<&str> = [
        "style", "title", "align", "valign", "bgcolor", "dir", "lang",
    ]
    .into_iter()
    .collect();
    builder.generic_attributes(generic_attrs);

    let mut tag_attrs: HashMap<&str, HashSet<&str>> = HashMap::new();
    tag_attrs.insert("a", ["href"].into_iter().collect());
    tag_attrs.insert(
        "img",
        ["src", "data-remote-src", "alt", "width", "height"]
            .into_iter()
            .collect(),
    );
    tag_attrs.insert(
        "td",
        ["colspan", "rowspan", "width", "height"]
            .into_iter()
            .collect(),
    );
    tag_attrs.insert(
        "th",
        ["colspan", "rowspan", "width", "height"]
            .into_iter()
            .collect(),
    );
    tag_attrs.insert(
        "table",
        ["width", "cellpadding", "cellspacing", "border"]
            .into_iter()
            .collect(),
    );
    tag_attrs.insert("font", ["color", "face", "size"].into_iter().collect());
    tag_attrs.insert("tr", ["height"].into_iter().collect());
    tag_attrs.insert("col", ["width", "span"].into_iter().collect());
    builder.tag_attributes(tag_attrs);

    // Strip javascript: and other schemes from URL attributes
    builder.url_schemes(
        ["http", "https", "mailto", "data", "cid"]
            .into_iter()
            .collect(),
    );
    builder.attribute_filter(filter_attribute);

    // Link rel — force noopener noreferrer on links
    builder.link_rel(Some("noopener noreferrer"));

    builder
}

/// Per-attribute rules on top of ammonia's allowlist: images only load
/// inline data, links only go to the web or a mail address, and inline CSS
/// cannot fetch anything.
fn filter_attribute<'u>(element: &str, attribute: &str, value: &'u str) -> Option<Cow<'u, str>> {
    let keep = match (element, attribute) {
        ("img", "src") => has_scheme(value, &["data:", "cid:"]),
        ("img", "data-remote-src") => has_scheme(value, &["http:", "https:"]),
        ("a", "href") => has_scheme(value, &["http:", "https:", "mailto:"]),
        (_, "style") => {
            let style = filter_style(value);
            return (!style.trim().is_empty()).then_some(Cow::Owned(style));
        }
        _ => true,
    };
    keep.then_some(Cow::Borrowed(value))
}

fn has_scheme(value: &str, schemes: &[&str]) -> bool {
    let value = value.trim_start();
    schemes.iter().any(|scheme| {
        value
            .get(..scheme.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
    })
}

/// Drop inline CSS declarations that fetch a resource (`url()`,
/// `image-set()`, `src()`, or a backslash escape that could spell one) or
/// take the element out of the message flow (`position`), so a quoted
/// message cannot cover the compose window.
fn filter_style(style: &str) -> String {
    style
        .split(';')
        .filter(|declaration| {
            let declaration = declaration.to_ascii_lowercase();
            let property = declaration.split(':').next().unwrap_or_default().trim();
            property != "position"
                && !["url(", "image-set(", "src(", "\\"]
                    .iter()
                    .any(|needle| declaration.contains(needle))
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Sanitize HTML for safe display in any window: removes scripts, event
/// handlers and unsafe URLs, strips tracking pixels and tracking parameters
/// from links, and blocks remote images (kept as `data-remote-src`).
pub fn sanitize_html(html: &str) -> String {
    let html = crate::mail::anti_tracking::strip_tracking(html);
    let html = REMOTE_IMG_SRC.replace_all(&html, "${1}data-remote-src${2}");
    email_sanitizer().clean(&html).to_string()
}

/// `serialize_with` hook for HTML bodies sent to the webview.
pub fn serialize_sanitized<S: Serializer>(
    html: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    html.as_deref().map(sanitize_html).serialize(serializer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script() {
        let out = sanitize_html("<p>Hi</p><script>alert(1)</script>");
        assert_eq!(out, "<p>Hi</p>");
    }

    #[test]
    fn strips_img_onerror() {
        let out = sanitize_html(r#"<img src=x onerror="alert(1)">"#);
        assert!(!out.contains("onerror"), "{out}");
        assert!(!out.contains("alert"), "{out}");
    }

    #[test]
    fn strips_javascript_href() {
        let out = sanitize_html(
            r#"<a href="javascript:alert(1)">x</a><a href=" JaVaScRiPt:alert(2)">y</a>"#,
        );
        assert!(!out.to_ascii_lowercase().contains("javascript"), "{out}");
        assert!(out.contains(">x</a>") && out.contains(">y</a>"), "{out}");
    }

    #[test]
    fn strips_svg_onload() {
        let out = sanitize_html(r#"<svg onload="alert(1)"><circle r="1"/></svg><p>ok</p>"#);
        assert!(!out.contains("svg") && !out.contains("onload"), "{out}");
        assert!(out.contains("<p>ok</p>"), "{out}");
    }

    #[test]
    fn strips_tracking_pixel() {
        let out = sanitize_html(
            r#"<p>Hello</p><img src="https://t.example.com/open.gif" width="1" height="1">"#,
        );
        assert_eq!(out, "<p>Hello</p>");
    }

    #[test]
    fn blocks_remote_images_but_keeps_the_url() {
        let out = sanitize_html(r#"<img alt="logo" SRC = 'https://example.com/logo.png'>"#);
        assert!(!out.contains(" src="), "{out}");
        assert!(
            out.contains(r#"data-remote-src="https://example.com/logo.png""#),
            "{out}"
        );
    }

    #[test]
    fn never_keeps_a_remote_src() {
        let out =
            sanitize_html(r#"<img src="//example.com/a.png"><img/src="http://example.com/b.png">"#);
        assert!(!out.contains("example.com/a.png"), "{out}");
        assert!(!out.contains(r#" src="http"#), "{out}");
    }

    #[test]
    fn drops_non_web_remote_src() {
        let out = sanitize_html(r#"<img data-remote-src="javascript:alert(1)">"#);
        assert!(!out.contains("javascript"), "{out}");
    }

    #[test]
    fn keeps_inline_images_and_links() {
        let out = sanitize_html(
            r#"<img src="data:image/png;base64,AAAA"><a href="https://example.com/?id=1&utm_source=x">l</a>"#,
        );
        assert!(out.contains(r#"src="data:image/png;base64,AAAA""#), "{out}");
        assert!(out.contains(r#"href="https://example.com/?id=1""#), "{out}");
        assert!(out.contains(r#"rel="noopener noreferrer""#), "{out}");
    }

    #[test]
    fn strips_style_blocks_and_remote_css() {
        let out = sanitize_html(
            r#"<style>body{display:none}</style><p class="overlay" id="compose-body" style="color:red; background:url(https://t.example.com/p.gif); position:fixed">x</p>"#,
        );
        assert_eq!(out, r#"<p style="color:red">x</p>"#);
    }

    #[test]
    fn serialize_hook_sanitizes() {
        #[derive(Serialize)]
        struct Body {
            #[serde(serialize_with = "serialize_sanitized")]
            html: Option<String>,
        }
        let json = serde_json::to_string(&Body {
            html: Some("<b onclick=x>hi</b>".into()),
        })
        .unwrap();
        assert_eq!(json, r#"{"html":"<b>hi</b>"}"#);
        let json = serde_json::to_string(&Body { html: None }).unwrap();
        assert_eq!(json, r#"{"html":null}"#);
    }
}
