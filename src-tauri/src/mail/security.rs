//! Email security analysis: phishing detection, homograph attacks, auth headers.

use std::collections::HashSet;
use std::sync::{LazyLock, OnceLock};

use regex::Regex;
use serde::{Deserialize, Serialize};

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhishingWarning {
    pub displayed_text: String,
    pub actual_url: String,
    pub reason: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub spf: Option<String>,
    pub dkim: Option<String>,
    pub dmarc: Option<String>,
    pub is_authenticated: bool,
}

// ── Compiled regexes ─────────────────────────────────────────────────

fn re_anchor() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?is)<a\s[^>]*href\s*=\s*["']([^"']+)["'][^>]*>(.*?)</a>"#)
            .expect("invalid regex: re_anchor")
    })
}

fn re_strip_tags() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]+>").expect("invalid regex: re_strip_tags"))
}

fn re_url_like() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^https?://([^/]+)")
            .expect("invalid regex: re_url_like")
    })
}

fn re_ip_host() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^https?://\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}")
            .expect("invalid regex: re_ip_host")
    })
}

fn re_auth_spf() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)spf=(pass|fail|softfail|neutral|none|temperror|permerror)").expect("re_auth_spf"))
}

fn re_auth_dkim() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)dkim=(pass|fail|none|temperror|permerror)").expect("re_auth_dkim"))
}

fn re_auth_dmarc() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)dmarc=(pass|fail|none|temperror|permerror)").expect("re_auth_dmarc"))
}

// ── URL shorteners (O(1) lookup via HashSet) ────────────────────────

static URL_SHORTENERS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "bit.ly", "tinyurl.com", "goo.gl", "t.co", "ow.ly", "is.gd",
        "buff.ly", "adf.ly", "j.mp", "rb.gy", "shorturl.at", "cutt.ly",
        "tiny.cc", "lnkd.in", "soo.gd", "s.id",
    ]
    .into_iter()
    .collect()
});

// ── 1. Phishing link detection ───────────────────────────────────────

/// Analyze links in HTML email body for phishing indicators.
/// Returns a list of suspicious links with reasons.
pub fn detect_phishing_links(html: &str) -> Vec<PhishingWarning> {
    let mut warnings = Vec::new();

    for cap in re_anchor().captures_iter(html) {
        let href = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let inner_html = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let display_text = re_strip_tags().replace_all(inner_html, "").trim().to_string();

        if href.is_empty() || display_text.is_empty() {
            continue;
        }

        // Skip mailto: and tel: links
        if href.starts_with("mailto:") || href.starts_with("tel:") {
            continue;
        }

        // Check 1: IP address link
        if re_ip_host().is_match(href) {
            warnings.push(PhishingWarning {
                displayed_text: display_text.clone(),
                actual_url: href.to_string(),
                reason: "IP address link".to_string(),
                severity: "medium".to_string(),
            });
        }

        // Check 2: URL shortener
        if let Some(href_domain) = extract_domain(href) {
            let lower = href_domain.to_lowercase();
            if URL_SHORTENERS.contains(lower.as_str()) || URL_SHORTENERS.iter().any(|s| lower.ends_with(&format!(".{}", s))) {
                warnings.push(PhishingWarning {
                    displayed_text: display_text.clone(),
                    actual_url: href.to_string(),
                    reason: "URL shortener".to_string(),
                    severity: "medium".to_string(),
                });
            }
        }

        // Check 3: Display text looks like a URL but domain doesn't match href
        // Only trigger if the display text actually looks like a URL (starts with http/https/www)
        // This avoids false positives on usernames like "pernelle.music" or "john.doe"
        let text_lower = display_text.to_lowercase();
        let looks_like_url = text_lower.starts_with("http://")
            || text_lower.starts_with("https://")
            || text_lower.starts_with("www.");
        if looks_like_url {
            if let Some(display_domain) = extract_domain(&display_text) {
                if let Some(href_domain) = extract_domain(href) {
                    let d1 = root_domain(&display_domain);
                    let d2 = root_domain(&href_domain);
                    if !d1.is_empty() && !d2.is_empty() && d1 != d2 {
                        warnings.push(PhishingWarning {
                            displayed_text: display_text.clone(),
                            actual_url: href.to_string(),
                            reason: "URL mismatch".to_string(),
                            severity: "high".to_string(),
                        });
                    }
                }
            }
        }
    }

    warnings
}

/// Extract domain from a URL string.
fn extract_domain(url: &str) -> Option<String> {
    // Handle plain domain-like text (e.g. "www.example.com")
    let url = if !url.contains("://") {
        if url.starts_with("www.") || url.contains('.') {
            format!("http://{}", url)
        } else {
            return None;
        }
    } else {
        url.to_string()
    };

    re_url_like().captures(&url).and_then(|c| {
        c.get(1).map(|m| m.as_str().to_lowercase())
    })
}

/// Extract root domain (last two labels): "mail.example.com" -> "example.com"
fn root_domain(domain: &str) -> String {
    // Use rsplit_once to avoid allocating a Vec for split().collect()
    match domain.rsplit_once('.') {
        Some((prefix, tld)) => match prefix.rsplit_once('.') {
            Some((_, sld)) => format!("{}.{}", sld, tld),
            None => domain.to_string(), // already two labels like "example.com"
        },
        None => domain.to_string(), // single label, no dots
    }
}

// ── 2. Homograph attack detection ────────────────────────────────────

/// Cyrillic characters that look like Latin letters.
const CYRILLIC_LOOKALIKES: &[(char, char)] = &[
    ('\u{0430}', 'a'), // а → a
    ('\u{0435}', 'e'), // е → e
    ('\u{043E}', 'o'), // о → o
    ('\u{0440}', 'p'), // р → p
    ('\u{0441}', 'c'), // с → c
    ('\u{0443}', 'y'), // у → y
    ('\u{0445}', 'x'), // х → x
    ('\u{042C}', 'b'), // Ь (uppercase soft sign, looks like lowercase b)
    ('\u{0456}', 'i'), // і → i (Ukrainian)
];

/// Greek characters that look like Latin letters.
const GREEK_LOOKALIKES: &[(char, char)] = &[
    ('\u{03BF}', 'o'), // ο → o
    ('\u{03B1}', 'a'), // α → a (close)
    ('\u{03C1}', 'p'), // ρ → p
];

/// Check if a domain contains mixed scripts (Latin + Cyrillic/Greek lookalikes).
pub fn detect_homograph(domain: &str) -> bool {
    let mut has_latin = false;
    let mut has_non_latin_lookalike = false;

    for ch in domain.chars() {
        if ch == '.' || ch == '-' || ch.is_ascii_digit() {
            continue;
        }
        if ch.is_ascii_alphabetic() {
            has_latin = true;
        } else if CYRILLIC_LOOKALIKES.iter().any(|(c, _)| *c == ch)
            || GREEK_LOOKALIKES.iter().any(|(c, _)| *c == ch)
        {
            has_non_latin_lookalike = true;
        }
    }

    has_latin && has_non_latin_lookalike
}

/// Check sender email domain for homograph attacks.
/// Returns a warning message if mixed scripts detected.
pub fn check_sender_homograph(from: &str) -> Option<String> {
    let domain = extract_email_domain(from)?;
    if detect_homograph(&domain) {
        Some(format!(
            "Sender domain '{}' contains mixed scripts (possible homograph attack)",
            domain
        ))
    } else {
        None
    }
}

// ── 3. SPF/DKIM/DMARC header analysis ───────────────────────────────

/// Parse Authentication-Results header to extract SPF/DKIM/DMARC status.
pub fn parse_auth_results(headers: &str) -> AuthStatus {
    let spf = re_auth_spf()
        .captures(headers)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_lowercase());

    let dkim = re_auth_dkim()
        .captures(headers)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_lowercase());

    let dmarc = re_auth_dmarc()
        .captures(headers)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_lowercase());

    let is_authenticated = matches!(spf.as_deref(), Some("pass"))
        || matches!(dkim.as_deref(), Some("pass"));

    AuthStatus {
        spf,
        dkim,
        dmarc,
        is_authenticated,
    }
}

// ── 4. Reply-To mismatch ─────────────────────────────────────────────

/// Check if Reply-To header differs from From header (potential spoofing).
pub fn check_reply_to_mismatch(from: &str, reply_to: &str) -> Option<String> {
    let from_domain = extract_email_domain(from)?;
    let reply_domain = extract_email_domain(reply_to)?;

    let r1 = root_domain(&from_domain);
    let r2 = root_domain(&reply_domain);

    if r1 != r2 {
        Some(format!(
            "Reply-To domain ({}) differs from sender domain ({})",
            reply_domain, from_domain
        ))
    } else {
        None
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Extract the domain part from an email address or "Name <email>" format.
fn extract_email_domain(addr: &str) -> Option<String> {
    // Handle "Name <email@domain>" format
    let email = if let Some(start) = addr.find('<') {
        let end = addr.find('>')?;
        &addr[start + 1..end]
    } else {
        addr.trim()
    };

    let at = email.rfind('@')?;
    Some(email[at + 1..].to_lowercase())
}

// ── Run full security analysis on parsed email ───────────────────────

/// Combined security analysis result for a single email.
#[derive(Debug, Clone, Serialize)]
pub struct SecurityAnalysis {
    pub phishing_warnings: Vec<PhishingWarning>,
    pub auth_status: Option<AuthStatus>,
    pub sender_warnings: Vec<String>,
}

/// Run all security checks on a parsed email.
pub fn analyze_email(
    from: &str,
    reply_to: Option<&str>,
    body_html: Option<&str>,
    raw_headers: &str,
) -> SecurityAnalysis {
    let mut sender_warnings = Vec::new();

    // Phishing link detection
    let phishing_warnings = body_html
        .map(detect_phishing_links)
        .unwrap_or_default();

    // Homograph check on sender
    if let Some(warning) = check_sender_homograph(from) {
        sender_warnings.push(warning);
    }

    // Reply-To mismatch
    if let Some(rt) = reply_to {
        if !rt.is_empty() {
            if let Some(warning) = check_reply_to_mismatch(from, rt) {
                sender_warnings.push(warning);
            }
        }
    }

    // Auth headers
    let auth_status = if raw_headers.contains("Authentication-Results") || raw_headers.contains("authentication-results") {
        Some(parse_auth_results(raw_headers))
    } else {
        None
    };

    SecurityAnalysis {
        phishing_warnings,
        auth_status,
        sender_warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_mismatch() {
        let html = r#"<a href="https://evil.com/login">https://paypal.com/login</a>"#;
        let warnings = detect_phishing_links(html);
        assert!(!warnings.is_empty());
        assert_eq!(warnings[0].reason, "URL mismatch");
        assert_eq!(warnings[0].severity, "high");
    }

    #[test]
    fn test_ip_link() {
        let html = r#"<a href="http://192.168.1.1/page">Click here</a>"#;
        let warnings = detect_phishing_links(html);
        assert!(warnings.iter().any(|w| w.reason == "IP address link"));
    }

    #[test]
    fn test_url_shortener() {
        let html = r#"<a href="https://bit.ly/abc123">Click here</a>"#;
        let warnings = detect_phishing_links(html);
        assert!(warnings.iter().any(|w| w.reason == "URL shortener"));
    }

    #[test]
    fn test_no_false_positive() {
        let html = r#"<a href="https://example.com/page">Click here</a>"#;
        let warnings = detect_phishing_links(html);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_homograph_mixed() {
        // Mix Latin 'g' with Cyrillic 'о' (U+043E) and Cyrillic 'а' (U+0430)
        assert!(detect_homograph("g\u{043E}\u{043E}gle.c\u{043E}m"));
    }

    #[test]
    fn test_homograph_clean() {
        assert!(!detect_homograph("google.com"));
    }

    #[test]
    fn test_auth_results_pass() {
        let hdr = "Authentication-Results: mx.google.com; spf=pass; dkim=pass; dmarc=pass";
        let auth = parse_auth_results(hdr);
        assert_eq!(auth.spf.as_deref(), Some("pass"));
        assert_eq!(auth.dkim.as_deref(), Some("pass"));
        assert_eq!(auth.dmarc.as_deref(), Some("pass"));
        assert!(auth.is_authenticated);
    }

    #[test]
    fn test_auth_results_fail() {
        let hdr = "Authentication-Results: mx.google.com; spf=fail; dkim=fail; dmarc=fail";
        let auth = parse_auth_results(hdr);
        assert!(!auth.is_authenticated);
    }

    #[test]
    fn test_reply_to_mismatch() {
        let result = check_reply_to_mismatch("user@company.com", "user@phisher.com");
        assert!(result.is_some());
    }

    #[test]
    fn test_reply_to_match() {
        let result = check_reply_to_mismatch("user@company.com", "support@company.com");
        assert!(result.is_none());
    }

    #[test]
    fn test_sender_homograph_check() {
        let result = check_sender_homograph("user <user@g\u{043E}\u{043E}gle.com>");
        assert!(result.is_some());
    }

    #[test]
    fn test_sender_homograph_clean_domain() {
        let result = check_sender_homograph("User <user@google.com>");
        assert!(result.is_none());
    }

    #[test]
    fn test_homograph_greek_lookalike() {
        // Mix Latin 'g' with Greek 'ο' (U+03BF)
        assert!(detect_homograph("g\u{03BF}\u{03BF}gle.com"));
    }

    #[test]
    fn test_homograph_pure_cyrillic() {
        // Pure Cyrillic (no Latin) should NOT trigger
        assert!(!detect_homograph("\u{0433}\u{043E}\u{043E}\u{0433}\u{043B}\u{0435}.\u{043A}\u{043E}\u{043C}"));
    }

    #[test]
    fn test_auth_results_partial() {
        let hdr = "Authentication-Results: mx.example.com; spf=pass; dkim=none";
        let auth = parse_auth_results(hdr);
        assert_eq!(auth.spf.as_deref(), Some("pass"));
        assert_eq!(auth.dkim.as_deref(), Some("none"));
        assert!(auth.dmarc.is_none());
        assert!(auth.is_authenticated); // spf=pass is enough
    }

    #[test]
    fn test_auth_results_softfail() {
        let hdr = "Authentication-Results: mx.example.com; spf=softfail; dkim=pass";
        let auth = parse_auth_results(hdr);
        assert_eq!(auth.spf.as_deref(), Some("softfail"));
        assert!(auth.is_authenticated); // dkim=pass is enough
    }

    #[test]
    fn test_auth_results_empty_headers() {
        let auth = parse_auth_results("");
        assert!(auth.spf.is_none());
        assert!(auth.dkim.is_none());
        assert!(auth.dmarc.is_none());
        assert!(!auth.is_authenticated);
    }

    #[test]
    fn test_mailto_link_not_flagged() {
        let html = r#"<a href="mailto:user@example.com">Contact Us</a>"#;
        let warnings = detect_phishing_links(html);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_multiple_phishing_links() {
        let html = r#"
            <a href="http://192.168.1.1/login">Login</a>
            <a href="https://bit.ly/xyz">Click</a>
            <a href="https://evil.com/page">https://bank.com/page</a>
        "#;
        let warnings = detect_phishing_links(html);
        assert!(warnings.len() >= 3);
    }

    #[test]
    fn test_analyze_email_combined() {
        let html = r#"<a href="http://192.168.1.1/steal">Login here</a>"#;
        let headers = "Authentication-Results: mx.example.com; spf=fail; dkim=fail";
        let analysis = analyze_email(
            "user <user@g\u{043E}\u{043E}gle.com>",
            Some("reply@phisher.com"),
            Some(html),
            headers,
        );
        assert!(!analysis.phishing_warnings.is_empty());
        assert!(analysis.auth_status.is_some());
        // Sender warnings: homograph + reply-to mismatch
        assert!(analysis.sender_warnings.len() >= 2);
    }
}
