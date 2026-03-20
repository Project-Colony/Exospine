//! Calendar event types and ICS (iCalendar) parsing/serialization.
//!
//! Provides a basic ICS parser that extracts VEVENT blocks and a
//! serializer that produces RFC 5545 compliant output. This is
//! scaffolding — full CalDAV support would require an HTTP client.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use uuid::Uuid;

/// A single calendar event.
#[derive(Debug, Clone)]
pub struct CalendarEvent {
    /// Unique identifier for the event.
    pub id: String,
    /// Event title / summary.
    pub title: String,
    /// Longer description of the event.
    pub description: String,
    /// Start date/time in UTC.
    pub start: DateTime<Utc>,
    /// End date/time in UTC.
    pub end: DateTime<Utc>,
    /// Location string (free-form).
    pub location: String,
    /// Whether this is an all-day event.
    pub all_day: bool,
    /// Email addresses of attendees.
    pub attendees: Vec<String>,
}

impl Default for CalendarEvent {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: String::new(),
            description: String::new(),
            start: Utc::now(),
            end: Utc::now(),
            location: String::new(),
            all_day: false,
            attendees: Vec::new(),
        }
    }
}

/// Parse an ICS datetime string (e.g. "20240315T120000Z" or "20240315") into a `DateTime<Utc>`.
///
/// Returns the datetime and a boolean indicating whether it was a date-only (all-day) value.
fn parse_ics_datetime(s: &str) -> Result<(DateTime<Utc>, bool)> {
    let s = s.trim();
    // Full datetime with Z suffix: 20240315T120000Z
    if s.len() >= 15 && s.contains('T') {
        let clean = s.trim_end_matches('Z');
        let naive = NaiveDateTime::parse_from_str(clean, "%Y%m%dT%H%M%S")
            .context("Failed to parse ICS datetime")?;
        return Ok((Utc.from_utc_datetime(&naive), false));
    }
    // Date only: 20240315
    if s.len() == 8 {
        let naive = chrono::NaiveDate::parse_from_str(s, "%Y%m%d")
            .context("Failed to parse ICS date")?;
        let dt = naive
            .and_hms_opt(0, 0, 0)
            .context("Invalid date")?;
        return Ok((Utc.from_utc_datetime(&dt), true));
    }
    // Fallback: try full datetime without Z
    let naive = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
        .context("Failed to parse ICS datetime (no Z)")?;
    Ok((Utc.from_utc_datetime(&naive), false))
}

/// Format a `DateTime<Utc>` as an ICS datetime string (e.g. "20240315T120000Z").
fn format_ics_datetime(dt: &DateTime<Utc>) -> String {
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Format a `DateTime<Utc>` as an ICS date string for all-day events (e.g. "20240315").
fn format_ics_date(dt: &DateTime<Utc>) -> String {
    dt.format("%Y%m%d").to_string()
}

/// Parse raw ICS/iCalendar text and extract VEVENT blocks into `CalendarEvent` structs.
///
/// This is a basic line-by-line parser that handles unfolded lines (RFC 5545 Section 3.1)
/// and extracts SUMMARY, DTSTART, DTEND, LOCATION, DESCRIPTION, UID, and ATTENDEE properties.
pub fn parse_ics(raw: &str) -> Result<Vec<CalendarEvent>> {
    let mut events = Vec::new();
    let mut in_event = false;
    let mut event = CalendarEvent::default();

    // Unfold continuation lines (lines starting with space or tab are continuations).
    let unfolded = raw.replace("\r\n ", "").replace("\r\n\t", "");

    for line in unfolded.lines() {
        let line = line.trim_end();

        if line == "BEGIN:VEVENT" {
            in_event = true;
            event = CalendarEvent::default();
            continue;
        }

        if line == "END:VEVENT" {
            in_event = false;
            events.push(event.clone());
            continue;
        }

        if !in_event {
            continue;
        }

        // Split on first colon to get property name and value.
        // Handle parameters like DTSTART;VALUE=DATE:20240315
        if let Some(colon_pos) = line.find(':') {
            let prop_part = &line[..colon_pos];
            let value = &line[colon_pos + 1..];
            // Property name is before any ';' parameters.
            let prop_name = prop_part
                .split(';')
                .next()
                .unwrap_or(prop_part)
                .to_uppercase();

            match prop_name.as_str() {
                "SUMMARY" => {
                    event.title = unescape_ics(value);
                }
                "DESCRIPTION" => {
                    event.description = unescape_ics(value);
                }
                "LOCATION" => {
                    event.location = unescape_ics(value);
                }
                "UID" => {
                    event.id = value.to_string();
                }
                "DTSTART" => {
                    if let Ok((dt, all_day)) = parse_ics_datetime(value) {
                        event.start = dt;
                        if all_day {
                            event.all_day = true;
                        }
                    }
                }
                "DTEND" => {
                    if let Ok((dt, _)) = parse_ics_datetime(value) {
                        event.end = dt;
                    }
                }
                "ATTENDEE" => {
                    // ATTENDEE can look like: mailto:user@example.com
                    // or ATTENDEE;CN=Name:mailto:user@example.com
                    let email = value
                        .strip_prefix("mailto:")
                        .or_else(|| value.strip_prefix("MAILTO:"))
                        .unwrap_or(value);
                    if !email.is_empty() {
                        event.attendees.push(email.to_string());
                    }
                }
                _ => {} // Ignore unknown properties.
            }
        }
    }

    Ok(events)
}

/// Unescape ICS text values (backslash-escaped commas, semicolons, newlines).
fn unescape_ics(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

/// Escape a string for ICS text values.
fn escape_ics(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(',', "\\,")
        .replace(';', "\\;")
}

/// Serialize a list of calendar events to ICS/iCalendar format (RFC 5545).
pub fn format_ics(events: &[CalendarEvent]) -> String {
    let mut out = String::new();
    out.push_str("BEGIN:VCALENDAR\r\n");
    out.push_str("VERSION:2.0\r\n");
    out.push_str("PRODID:-//Exospine//EN\r\n");

    for event in events {
        out.push_str("BEGIN:VEVENT\r\n");
        out.push_str(&format!("UID:{}\r\n", event.id));
        out.push_str(&format!("SUMMARY:{}\r\n", escape_ics(&event.title)));

        if event.all_day {
            out.push_str(&format!(
                "DTSTART;VALUE=DATE:{}\r\n",
                format_ics_date(&event.start)
            ));
            out.push_str(&format!(
                "DTEND;VALUE=DATE:{}\r\n",
                format_ics_date(&event.end)
            ));
        } else {
            out.push_str(&format!(
                "DTSTART:{}\r\n",
                format_ics_datetime(&event.start)
            ));
            out.push_str(&format!(
                "DTEND:{}\r\n",
                format_ics_datetime(&event.end)
            ));
        }

        if !event.location.is_empty() {
            out.push_str(&format!("LOCATION:{}\r\n", escape_ics(&event.location)));
        }
        if !event.description.is_empty() {
            out.push_str(&format!(
                "DESCRIPTION:{}\r\n",
                escape_ics(&event.description)
            ));
        }
        for attendee in &event.attendees {
            out.push_str(&format!("ATTENDEE:mailto:{}\r\n", attendee));
        }

        out.push_str("END:VEVENT\r\n");
    }

    out.push_str("END:VCALENDAR\r\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_ics() {
        let events = vec![CalendarEvent {
            id: "test-uid-1".to_string(),
            title: "Team Meeting".to_string(),
            description: "Weekly sync".to_string(),
            start: Utc.with_ymd_and_hms(2024, 3, 15, 14, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2024, 3, 15, 15, 0, 0).unwrap(),
            location: "Room 42".to_string(),
            all_day: false,
            attendees: vec!["alice@example.com".to_string()],
        }];

        let ics = format_ics(&events);
        let parsed = parse_ics(&ics).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].title, "Team Meeting");
        assert_eq!(parsed[0].location, "Room 42");
        assert_eq!(parsed[0].attendees, vec!["alice@example.com"]);
    }
}
