//! Rule-based spam scorer for incoming mail.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::app_state::MailEntry;

/// Spam trigger words (O(1) lookup via HashSet).
static SPAM_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "viagra", "casino", "lottery", "winner", "prince", "urgent",
        "free money", "click here", "unsubscribe", "act now", "limited time",
        "congratulations", "you've won", "you have won", "claim your",
        "100% free", "buy now", "order now", "risk free", "no obligation",
        "double your", "earn extra cash", "million dollars", "nigerian",
        "inheritance", "wire transfer",
    ]
    .into_iter()
    .collect()
});

/// Known spammy TLDs (O(1) lookup via HashSet).
static SPAMMY_TLDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [".xyz", ".top", ".club", ".work", ".buzz", ".gq", ".tk", ".ml", ".ga", ".cf"]
        .into_iter()
        .collect()
});

/// Compute a spam score for the given mail entry.
/// Score > 3.0 is considered likely spam.
pub fn spam_score(mail: &MailEntry) -> f32 {
    let mut score: f32 = 0.0;

    // ── Subject-based checks ────────────────────────────────────────
    let subject_lower = mail.subject.to_lowercase();

    for word in SPAM_WORDS.iter() {
        if subject_lower.contains(word) {
            score += 1.0;
        }
    }

    // ALL CAPS subject (over 10 chars)
    if mail.subject.len() > 10 && mail.subject == mail.subject.to_uppercase() {
        score += 2.0;
    }

    // Excessive punctuation in subject
    let punct_count = mail
        .subject
        .chars()
        .filter(|c| *c == '!' || *c == '?')
        .count();
    if punct_count > 3 {
        score += 1.5;
    }

    // ── Sender-based checks ─────────────────────────────────────────
    let from_lower = mail.from.to_lowercase();

    // Known spammy TLDs
    for tld in SPAMMY_TLDS.iter() {
        if from_lower.ends_with(tld) || from_lower.ends_with(&format!("{}>", tld)) {
            score += 1.5;
            break;
        }
    }

    // ── Body-based checks ───────────────────────────────────────────
    let preview_lower = mail.preview.to_lowercase();

    if preview_lower.contains("click here") || preview_lower.contains("act now") {
        score += 0.5;
    }

    // Many exclamation marks in preview
    let body_punct = preview_lower.chars().filter(|c| *c == '!').count();
    if body_punct > 5 {
        score += 1.0;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mail(subject: &str, from: &str, preview: &str) -> MailEntry {
        MailEntry {
            subject: subject.to_string(),
            from: from.to_string(),
            preview: preview.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_clean_email_zero_score() {
        let mail = make_mail("Meeting tomorrow", "alice@company.com", "See you at 3pm.");
        let score = spam_score(&mail);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_spam_word_in_subject() {
        let mail = make_mail("You've won the lottery!", "spammer@example.com", "Claim now.");
        let score = spam_score(&mail);
        // "lottery" (+1.0) and "you've won" (+1.0) = at least 2.0
        assert!(score >= 2.0, "Score was {}", score);
    }

    #[test]
    fn test_all_caps_subject() {
        let mail = make_mail(
            "IMPORTANT ANNOUNCEMENT HERE",
            "user@example.com",
            "Details inside.",
        );
        let score = spam_score(&mail);
        // All caps = +2.0
        assert!(score >= 2.0, "Score was {}", score);
    }

    #[test]
    fn test_excessive_punctuation() {
        let mail = make_mail(
            "Buy now!!! Act fast!!!!",
            "user@example.com",
            "Limited offer.",
        );
        let score = spam_score(&mail);
        // "buy now" (+1.0) + "act now"-ish words + >3 exclamation marks (+1.5)
        assert!(score >= 1.5, "Score was {}", score);
    }

    #[test]
    fn test_spammy_tld() {
        let mail = make_mail("Hello", "user@scammer.xyz", "Normal body.");
        let score = spam_score(&mail);
        assert!(score >= 1.5, "Score was {}", score);
    }

    #[test]
    fn test_spammy_tld_with_brackets() {
        let mail = make_mail("Hello", "Scammer <user@scammer.tk>", "Normal body.");
        let score = spam_score(&mail);
        assert!(score >= 1.5, "Score was {}", score);
    }

    #[test]
    fn test_body_click_here() {
        let mail = make_mail(
            "Check this out",
            "user@example.com",
            "Please click here to verify your account.",
        );
        let score = spam_score(&mail);
        assert!(score >= 0.5, "Score was {}", score);
    }

    #[test]
    fn test_body_excessive_exclamations() {
        let mail = make_mail(
            "Great news",
            "user@example.com",
            "Amazing! Incredible! Wow! Fantastic! Unbelievable! Super!",
        );
        let score = spam_score(&mail);
        // 6 exclamation marks in body (+1.0)
        assert!(score >= 1.0, "Score was {}", score);
    }

    #[test]
    fn test_combined_spam_signals() {
        let mail = make_mail(
            "CONGRATULATIONS! YOU HAVE WON!!!",
            "prince@nigerian.xyz",
            "Click here to claim your prize! Act now! Don't miss out!!!!!",
        );
        let score = spam_score(&mail);
        // Should be well above 3.0 threshold
        assert!(score > 3.0, "Score was {}", score);
    }

    #[test]
    fn test_spam_threshold() {
        // Scores > 3.0 are likely spam per the doc
        let legit = make_mail("Weekly report", "boss@company.com", "Please review attached.");
        assert!(spam_score(&legit) <= 3.0);

        let spam = make_mail(
            "You have won the lottery - claim your inheritance",
            "winner@scam.tk",
            "Click here now!",
        );
        assert!(spam_score(&spam) > 3.0);
    }
}
