//! Anti-tracking: strip tracking pixels, tracking domains, and UTM parameters
//! from HTML emails before display.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

/// Known tracking / pixel domains (O(1) substring check still requires iteration,
/// but the HashSet is used for exact-match fast paths).
static TRACKING_DOMAINS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "open.convertkit.com",
        "tracking.tldmail.com",
        "pixel.mailchimp.com",
        "click.mailerlite.com",
        "trk.klclick.com",
        "ea.pstmrk.it",
        "links.m.redditmail.com",
        "list-manage.com/track",
        "mandrillapp.com/track",
        "sendgrid.net/wf/open",
        "t.sendinblue.com",
        "t.dripemail2.com",
        "track.hubspot.com",
        "ct.sendgrid.net",
        "email.mg.",
        "pixel.monitor1.returnpath.net",
        "beacon.krxd.net",
        "r.entitydef.com",
        "o.ss2.us",
        "open.spotify.com/track",
    ]
    .into_iter()
    .collect()
});

/// URL parameters that are used for tracking (O(1) lookup via HashSet).
static TRACKING_PARAMS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
        "utm_id", "fbclid", "gclid", "mc_eid", "mc_cid", "mkt_tok",
        "_hsenc", "_hsmi", "oly_enc_id", "oly_anon_id", "vero_id",
        "s_cid", "icid", "igshid", "spm", "trk", "trkCampaign", "trkInfo",
        "si", "ref_src", "ref_url",
    ]
    .into_iter()
    .collect()
});

/// Remove tracking pixels from HTML email content.
///
/// Strips:
/// - `<img>` tags that are 1x1 or 0x0 pixels (common tracking pixels)
/// - `<img>` tags whose `src` matches known tracking domains
/// - UTM and other tracking parameters from all `href` URLs
pub fn strip_tracking(html: &str) -> String {
    let result = strip_tracking_pixels(html);
    strip_tracking_params_from_links(&result)
}

/// Remove `<img>` tags that look like tracking pixels.
fn strip_tracking_pixels(html: &str) -> String {
    // Match <img> tags that have width="1" height="1" or width="0" height="0"
    // or style containing width:0/1px, height:0/1px
    let re_tiny = Regex::new(
        r#"(?i)<img\b[^>]*?(?:(?:width\s*=\s*["']?[01](?:px)?["']?\s+height\s*=\s*["']?[01](?:px)?["']?)|(?:height\s*=\s*["']?[01](?:px)?["']?\s+width\s*=\s*["']?[01](?:px)?["']?))[^>]*/?\s*>"#,
    )
    .expect("valid regex");

    let result = re_tiny.replace_all(html, "");

    // Also match <img> with style containing display:none
    let re_hidden = Regex::new(
        r#"(?i)<img\b[^>]*?style\s*=\s*["'][^"']*display\s*:\s*none[^"']*["'][^>]*/?\s*>"#,
    )
    .expect("valid regex");

    let result = re_hidden.replace_all(&result, "");

    // Remove <img> tags whose src matches known tracking domains
    let re_img = Regex::new(r#"(?i)<img\b[^>]*?src\s*=\s*["']([^"']*)["'][^>]*/?\s*>"#)
        .expect("valid regex");

    re_img
        .replace_all(&result, |caps: &regex::Captures| {
            let src = caps.get(1).map_or("", |m| m.as_str());
            if is_tracking_url(src) {
                String::new()
            } else {
                caps[0].to_string()
            }
        })
        .into_owned()
}

/// Check if a URL belongs to a known tracking domain.
fn is_tracking_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    TRACKING_DOMAINS.iter().any(|domain| lower.contains(domain))
}

/// Strip tracking parameters from all `href` attributes in the HTML.
fn strip_tracking_params_from_links(html: &str) -> String {
    let re_href =
        Regex::new(r#"(?i)(href\s*=\s*["'])([^"']+)(["'])"#).expect("valid regex");

    re_href
        .replace_all(html, |caps: &regex::Captures| {
            let prefix = &caps[1];
            let url = &caps[2];
            let suffix = &caps[3];
            let cleaned = strip_tracking_params(url);
            format!("{}{}{}", prefix, cleaned, suffix)
        })
        .into_owned()
}

/// Strip UTM and tracking parameters from a single URL.
pub fn strip_tracking_params(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_string();
    };

    let (query_part, fragment) = if let Some((q, f)) = query.split_once('#') {
        (q, Some(f))
    } else {
        (query, None)
    };

    let filtered: Vec<&str> = query_part
        .split('&')
        .filter(|param| {
            let key = param.split_once('=').map_or(*param, |(k, _)| k);
            let key_lower = key.to_lowercase();
            !TRACKING_PARAMS.contains(key_lower.as_str())
        })
        .collect();

    let mut result = base.to_string();
    if !filtered.is_empty() {
        result.push('?');
        result.push_str(&filtered.join("&"));
    }
    if let Some(f) = fragment {
        result.push('#');
        result.push_str(f);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_1x1_pixel() {
        let html = r#"<p>Hello</p><img src="https://pixel.mailchimp.com/track" width="1" height="1"><p>World</p>"#;
        let result = strip_tracking(html);
        assert!(!result.contains("pixel.mailchimp.com"));
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn test_strip_tracking_params_from_url() {
        let url = "https://example.com/page?utm_source=newsletter&utm_medium=email&id=42&fbclid=abc";
        let cleaned = strip_tracking_params(url);
        assert_eq!(cleaned, "https://example.com/page?id=42");
    }

    #[test]
    fn test_tracking_domain_img_removed() {
        let html = r#"<img src="https://track.hubspot.com/pixel.gif" alt="">"#;
        let result = strip_tracking(html);
        assert!(result.is_empty() || !result.contains("hubspot"));
    }

    #[test]
    fn test_normal_images_kept() {
        let html = r#"<img src="https://example.com/photo.jpg" width="600" height="400">"#;
        let result = strip_tracking(html);
        assert!(result.contains("example.com/photo.jpg"));
    }

    #[test]
    fn test_strip_0x0_pixel() {
        let html = r#"<img src="https://tracker.example.com/open" width="0" height="0">"#;
        let result = strip_tracking(html);
        assert!(!result.contains("tracker.example.com"));
    }

    #[test]
    fn test_strip_hidden_display_none() {
        let html = r#"<img src="https://tracker.example.com/pixel.gif" style="display:none" />"#;
        let result = strip_tracking(html);
        assert!(!result.contains("tracker.example.com"));
    }

    #[test]
    fn test_strip_utm_params_preserves_fragment() {
        let url = "https://example.com/page?id=42&utm_source=news#section1";
        let cleaned = strip_tracking_params(url);
        assert_eq!(cleaned, "https://example.com/page?id=42#section1");
    }

    #[test]
    fn test_strip_all_tracking_params_leaves_no_query() {
        let url = "https://example.com/page?utm_source=news&utm_medium=email";
        let cleaned = strip_tracking_params(url);
        assert_eq!(cleaned, "https://example.com/page");
    }

    #[test]
    fn test_strip_fbclid_and_gclid() {
        let url = "https://example.com/?fbclid=abc123&gclid=xyz789&real=value";
        let cleaned = strip_tracking_params(url);
        assert_eq!(cleaned, "https://example.com/?real=value");
    }

    #[test]
    fn test_url_without_query_unchanged() {
        let url = "https://example.com/page";
        let cleaned = strip_tracking_params(url);
        assert_eq!(cleaned, "https://example.com/page");
    }

    #[test]
    fn test_strip_tracking_from_href_in_html() {
        let html = r#"<a href="https://example.com/page?utm_source=news&id=1">Link</a>"#;
        let result = strip_tracking(html);
        assert!(result.contains("id=1"));
        assert!(!result.contains("utm_source"));
    }

    #[test]
    fn test_multiple_tracking_pixels_removed() {
        let html = r#"<p>Content</p><img src="https://open.convertkit.com/pixel" width="1" height="1"><img src="https://ea.pstmrk.it/open" width="1" height="1"><p>More</p>"#;
        let result = strip_tracking(html);
        assert!(!result.contains("convertkit"));
        assert!(!result.contains("pstmrk"));
        assert!(result.contains("Content"));
        assert!(result.contains("More"));
    }

    #[test]
    fn test_sendgrid_tracking_domain_removed() {
        let html = r#"<img src="https://ct.sendgrid.net/wf/open?u=abc" />"#;
        let result = strip_tracking(html);
        assert!(!result.contains("sendgrid"));
    }
}
