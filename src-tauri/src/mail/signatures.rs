//! Email signature and template helpers.
//!
//! These utilities are kept for future use when the compose UI supports
//! signature insertion and template selection.

use crate::app_state::MailTemplate;

const SIG_DELIMITER: &str = "\n\n-- \n";

#[allow(dead_code)]
pub fn apply_signature(body: &str, signature: &str) -> String {
    if signature.is_empty() {
        return body.to_string();
    }
    format!("{}{}{}", body, SIG_DELIMITER, signature)
}

#[allow(dead_code)]
pub fn strip_signature(body: &str) -> &str {
    if let Some(pos) = body.find("\n-- \n") {
        &body[..pos]
    } else {
        body
    }
}

#[allow(dead_code)]
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
