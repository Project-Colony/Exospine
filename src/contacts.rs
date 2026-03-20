//! Contact types and vCard 3.0 parsing/serialization.
//!
//! Provides a basic vCard parser that extracts FN, EMAIL, TEL, ORG, NOTE
//! properties and a serializer. Also includes a utility to extract contacts
//! from mail entries by deduplicating sender/recipient addresses.

use std::collections::HashMap;

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::state::MailEntry;

/// A single contact record.
#[derive(Debug, Clone)]
pub struct Contact {
    /// Unique identifier for the contact.
    pub id: String,
    /// Full display name.
    pub name: String,
    /// Primary email address.
    pub email: String,
    /// Phone number.
    pub phone: Option<String>,
    /// Organization / company.
    pub organization: Option<String>,
    /// Free-form notes.
    pub notes: Option<String>,
}

impl Default for Contact {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::new(),
            email: String::new(),
            phone: None,
            organization: None,
            notes: None,
        }
    }
}

/// Parse raw vCard 3.0 text into a list of `Contact` structs.
///
/// Handles VCARD blocks and extracts FN, EMAIL, TEL, ORG, NOTE, and UID properties.
pub fn parse_vcard(raw: &str) -> Result<Vec<Contact>> {
    let mut contacts = Vec::new();
    let mut in_vcard = false;
    let mut contact = Contact::default();

    // Unfold continuation lines (lines starting with space or tab).
    let unfolded = raw.replace("\r\n ", "").replace("\r\n\t", "");

    for line in unfolded.lines() {
        let line = line.trim_end();

        if line.eq_ignore_ascii_case("BEGIN:VCARD") {
            in_vcard = true;
            contact = Contact::default();
            continue;
        }

        if line.eq_ignore_ascii_case("END:VCARD") {
            in_vcard = false;
            if !contact.email.is_empty() || !contact.name.is_empty() {
                contacts.push(contact.clone());
            }
            continue;
        }

        if !in_vcard {
            continue;
        }

        // Split on first colon: property (with optional parameters) : value
        if let Some(colon_pos) = line.find(':') {
            let prop_part = &line[..colon_pos];
            let value = &line[colon_pos + 1..];
            let prop_name = prop_part
                .split(';')
                .next()
                .unwrap_or(prop_part)
                .to_uppercase();

            match prop_name.as_str() {
                "FN" => {
                    contact.name = unescape_vcard(value);
                }
                "EMAIL" => {
                    // Take the first EMAIL property encountered.
                    if contact.email.is_empty() {
                        contact.email = value.trim().to_string();
                    }
                }
                "TEL" => {
                    if contact.phone.is_none() {
                        contact.phone = Some(value.trim().to_string());
                    }
                }
                "ORG" => {
                    // ORG can have multiple components separated by semicolons.
                    contact.organization = Some(unescape_vcard(value).replace(';', ", "));
                }
                "NOTE" => {
                    contact.notes = Some(unescape_vcard(value));
                }
                "UID" => {
                    contact.id = value.trim().to_string();
                }
                _ => {} // Ignore other properties.
            }
        }
    }

    Ok(contacts)
}

/// Unescape vCard text values.
fn unescape_vcard(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

/// Escape a string for vCard text values.
fn escape_vcard(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(',', "\\,")
        .replace(';', "\\;")
}

/// Serialize a list of contacts to vCard 3.0 format.
pub fn format_vcard(contacts: &[Contact]) -> String {
    let mut out = String::new();

    for contact in contacts {
        out.push_str("BEGIN:VCARD\r\n");
        out.push_str("VERSION:3.0\r\n");
        out.push_str(&format!("UID:{}\r\n", contact.id));
        out.push_str(&format!("FN:{}\r\n", escape_vcard(&contact.name)));
        out.push_str(&format!("EMAIL:{}\r\n", contact.email));

        if let Some(ref phone) = contact.phone {
            out.push_str(&format!("TEL:{}\r\n", phone));
        }
        if let Some(ref org) = contact.organization {
            out.push_str(&format!("ORG:{}\r\n", escape_vcard(org)));
        }
        if let Some(ref notes) = contact.notes {
            out.push_str(&format!("NOTE:{}\r\n", escape_vcard(notes)));
        }

        out.push_str("END:VCARD\r\n");
    }

    out
}

/// Extract a deduplicated contact list from mail entries.
///
/// Pulls sender (`from`) and recipient (`to`) addresses from each mail entry,
/// deduplicates by email address (case-insensitive), and returns a sorted list.
pub fn extract_contacts_from_mails(entries: &[MailEntry]) -> Vec<Contact> {
    let mut seen: HashMap<String, Contact> = HashMap::new();

    for entry in entries {
        // Extract email from "Name <email>" format or plain email.
        let from_email = extract_email(&entry.from);
        let from_name = extract_name(&entry.from);

        if !from_email.is_empty() {
            let key = from_email.to_lowercase();
            seen.entry(key).or_insert_with(|| Contact {
                id: Uuid::new_v4().to_string(),
                name: from_name,
                email: from_email,
                phone: None,
                organization: None,
                notes: None,
            });
        }

        for to_addr in &entry.to {
            let to_email = extract_email(to_addr);
            let to_name = extract_name(to_addr);

            if !to_email.is_empty() {
                let key = to_email.to_lowercase();
                seen.entry(key).or_insert_with(|| Contact {
                    id: Uuid::new_v4().to_string(),
                    name: to_name,
                    email: to_email,
                    phone: None,
                    organization: None,
                    notes: None,
                });
            }
        }
    }

    let mut contacts: Vec<Contact> = seen.into_values().collect();
    contacts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    contacts
}

/// Extract the bare email address from a string like "Name <user@example.com>" or "user@example.com".
fn extract_email(addr: &str) -> String {
    if let Some(start) = addr.find('<') {
        if let Some(end) = addr.find('>') {
            return addr[start + 1..end].trim().to_string();
        }
    }
    // Might just be a plain email address.
    addr.trim().to_string()
}

/// Extract the display name from "Name <email>" format. Returns the email if no name part.
fn extract_name(addr: &str) -> String {
    if let Some(start) = addr.find('<') {
        let name = addr[..start].trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }
    // No name part; use the email itself.
    extract_email(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_vcard() {
        let contacts = vec![Contact {
            id: "test-uid-1".to_string(),
            name: "Alice Smith".to_string(),
            email: "alice@example.com".to_string(),
            phone: Some("+1-555-1234".to_string()),
            organization: Some("Acme Corp".to_string()),
            notes: None,
        }];

        let vcard = format_vcard(&contacts);
        let parsed = parse_vcard(&vcard).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "Alice Smith");
        assert_eq!(parsed[0].email, "alice@example.com");
        assert_eq!(parsed[0].phone.as_deref(), Some("+1-555-1234"));
    }

    #[test]
    fn extract_contacts_deduplicates() {
        let entries = vec![
            MailEntry {
                from: "Alice <alice@example.com>".to_string(),
                to: vec!["bob@example.com".to_string()],
                ..MailEntry::default()
            },
            MailEntry {
                from: "Alice <ALICE@example.com>".to_string(),
                to: vec!["bob@example.com".to_string()],
                ..MailEntry::default()
            },
        ];

        let contacts = extract_contacts_from_mails(&entries);
        assert_eq!(contacts.len(), 2); // alice + bob, deduplicated
    }
}
