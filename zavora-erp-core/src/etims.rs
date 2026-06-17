//! eTIMS transmission status and lifecycle guards (Kenya KRA, 2026 practice).
//!
//! Under KRA's electronic Tax Invoice Management System, a tax document is either
//! **not yet transmitted** to KRA — in which case it may still be edited, deleted
//! (if draft) or voided — or **already transmitted**, in which case it is
//! immutable and the only legitimate downward correction is a **credit note**
//! that references the original document.
//!
//! This module centralises that state so service-layer guards stay consistent.

use serde::{Deserialize, Serialize};

/// Transmission state of a tax document with respect to KRA eTIMS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EtimsStatus {
    /// Created locally but not yet sent to KRA. Editable / deletable / voidable.
    NotTransmitted,
    /// Accepted by KRA eTIMS. Immutable — correct via a credit note only.
    Transmitted,
    /// A transmission attempt failed. Treated as not-yet-on-record at KRA, so it
    /// may be corrected/retried, but it is NOT a compliant invoice yet.
    TransmissionFailed,
}

impl EtimsStatus {
    /// Parse the database string representation. Unknown values fall back to
    /// `NotTransmitted` (the safe, still-editable default).
    pub fn from_db(s: &str) -> Self {
        match s {
            "transmitted" => EtimsStatus::Transmitted,
            "transmission_failed" => EtimsStatus::TransmissionFailed,
            _ => EtimsStatus::NotTransmitted,
        }
    }

    /// The database string representation.
    pub fn as_db(&self) -> &'static str {
        match self {
            EtimsStatus::NotTransmitted => "not_transmitted",
            EtimsStatus::Transmitted => "transmitted",
            EtimsStatus::TransmissionFailed => "transmission_failed",
        }
    }

    /// Whether the document is on record at KRA (a compliant tax invoice).
    pub fn is_transmitted(&self) -> bool {
        matches!(self, EtimsStatus::Transmitted)
    }

    /// Whether the document may still be voided/deleted locally. Once a document
    /// is transmitted to KRA it must be corrected with a credit note instead.
    pub fn allows_void_or_delete(&self) -> bool {
        !self.is_transmitted()
    }
}

impl Default for EtimsStatus {
    fn default() -> Self {
        EtimsStatus::NotTransmitted
    }
}

impl std::fmt::Display for EtimsStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_db_strings() {
        for s in [
            EtimsStatus::NotTransmitted,
            EtimsStatus::Transmitted,
            EtimsStatus::TransmissionFailed,
        ] {
            assert_eq!(EtimsStatus::from_db(s.as_db()), s);
        }
    }

    #[test]
    fn unknown_db_value_is_not_transmitted() {
        assert_eq!(EtimsStatus::from_db("garbage"), EtimsStatus::NotTransmitted);
    }

    #[test]
    fn only_untransmitted_allows_void() {
        assert!(EtimsStatus::NotTransmitted.allows_void_or_delete());
        assert!(EtimsStatus::TransmissionFailed.allows_void_or_delete());
        assert!(!EtimsStatus::Transmitted.allows_void_or_delete());
    }
}
