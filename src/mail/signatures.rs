//! Email signature and template helpers.
//!
//! Provides utilities for appending/stripping the standard email signature
//! delimiter (`-- `) and a set of built-in mail templates.

use crate::state::MailTemplate;

/// Standard email signature delimiter (RFC 3676).
const SIG_DELIMITER: &str = "\n\n-- \n";

/// Append a signature to a message body using the standard delimiter.
///
/// If `signature` is empty the body is returned unchanged.
pub fn apply_signature(body: &str, signature: &str) -> String {
    if signature.is_empty() {
        return body.to_string();
    }
    format!("{}{}{}", body, SIG_DELIMITER, signature)
}

/// Strip everything after the standard signature delimiter.
///
/// Returns the body up to (but not including) the first `\n-- \n` sequence.
/// If no delimiter is found the entire body is returned.
pub fn strip_signature(body: &str) -> &str {
    if let Some(pos) = body.find("\n-- \n") {
        &body[..pos]
    } else {
        body
    }
}

/// Return the built-in set of mail templates.
pub fn default_templates() -> Vec<MailTemplate> {
    vec![
        MailTemplate {
            name: "Meeting request".to_string(),
            subject: "Meeting Request".to_string(),
            body: "Hi,\n\n\
                   I would like to schedule a meeting to discuss the following topic.\n\n\
                   Proposed time: \n\
                   Location: \n\n\
                   Please let me know if this works for you.\n\n\
                   Best regards"
                .to_string(),
        },
        MailTemplate {
            name: "Thank you".to_string(),
            subject: "Thank You".to_string(),
            body: "Hi,\n\n\
                   Thank you for your time and assistance. I really appreciate it.\n\n\
                   Best regards"
                .to_string(),
        },
        MailTemplate {
            name: "Follow-up".to_string(),
            subject: "Following Up".to_string(),
            body: "Hi,\n\n\
                   I wanted to follow up on our previous conversation regarding the matter below.\n\n\
                   Please let me know if there are any updates.\n\n\
                   Best regards"
                .to_string(),
        },
        MailTemplate {
            name: "Out of office".to_string(),
            subject: "Out of Office".to_string(),
            body: "Hi,\n\n\
                   Thank you for your email. I am currently out of the office and will return on [date].\n\n\
                   For urgent matters, please contact [alternate contact].\n\n\
                   Best regards"
                .to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_signature_appends_delimiter() {
        let result = apply_signature("Hello", "-- John");
        assert_eq!(result, "Hello\n\n-- \n-- John");
    }

    #[test]
    fn apply_signature_empty_is_noop() {
        let result = apply_signature("Hello", "");
        assert_eq!(result, "Hello");
    }

    #[test]
    fn strip_signature_removes_sig() {
        let body = "Hello\n-- \nJohn";
        assert_eq!(strip_signature(body), "Hello");
    }

    #[test]
    fn strip_signature_no_delimiter() {
        let body = "Hello world";
        assert_eq!(strip_signature(body), "Hello world");
    }

    #[test]
    fn default_templates_has_four() {
        assert_eq!(default_templates().len(), 4);
    }
}
