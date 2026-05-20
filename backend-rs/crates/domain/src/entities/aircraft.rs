use serde::{Deserialize, Serialize};

use crate::value_objects::Icao24;

/// Origin of an aircraft metadata record. Tracking provenance lets the
/// crawler prefer higher-confidence sources and audit fills.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AircraftSource(String);

impl AircraftSource {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Aircraft master data, keyed by ICAO24.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aircraft {
    pub icao24: Icao24,
    pub registration: Option<String>,
    pub type_code: Option<String>,
    pub type_description: Option<String>,
    pub operator: Option<String>,
    pub designator: Option<String>,
    pub source: Option<AircraftSource>,
}

impl Aircraft {
    pub fn new(icao24: Icao24) -> Self {
        Self {
            icao24,
            registration: None,
            type_code: None,
            type_description: None,
            operator: None,
            designator: None,
            source: None,
        }
    }

    pub fn has_type(&self) -> bool {
        self.type_code.is_some() && self.type_description.is_some()
    }

    pub fn is_complete(&self) -> bool {
        self.has_type() && self.registration.is_some()
    }

    pub fn is_complete_with_operator(&self) -> bool {
        self.is_complete() && self.operator.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.registration.is_none()
            && self.type_code.is_none()
            && self.type_description.is_none()
            && self.operator.is_none()
            && self.designator.is_none()
    }

    /// Fill any `None` field on `self` from `other` (if they refer to the
    /// same aircraft). Returns `true` if at least one field changed.
    pub fn merge_from(&mut self, other: &Self) -> bool {
        if self.icao24 != other.icao24 {
            return false;
        }
        let mut changed = false;
        fill_if_empty(
            &mut self.registration,
            other.registration.as_ref(),
            &mut changed,
        );
        fill_if_empty(&mut self.type_code, other.type_code.as_ref(), &mut changed);
        fill_if_empty(
            &mut self.type_description,
            other.type_description.as_ref(),
            &mut changed,
        );
        fill_if_empty(&mut self.operator, other.operator.as_ref(), &mut changed);
        fill_if_empty(
            &mut self.designator,
            other.designator.as_ref(),
            &mut changed,
        );
        fill_if_empty(&mut self.source, other.source.as_ref(), &mut changed);
        changed
    }
}

fn fill_if_empty<T: Clone>(dst: &mut Option<T>, src: Option<&T>, changed: &mut bool) {
    if dst.is_none() {
        if let Some(value) = src {
            *dst = Some(value.clone());
            *changed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ac(hex: &str) -> Aircraft {
        Aircraft::new(Icao24::new(hex).unwrap())
    }

    #[test]
    fn empty_when_no_fields_set() {
        assert!(ac("ABCDEF").is_empty());
    }

    #[test]
    fn merge_fills_missing_fields_only() {
        let mut a = ac("ABCDEF");
        a.registration = Some("HB-ABC".into());

        let mut b = ac("ABCDEF");
        b.registration = Some("OTHER".into());
        b.type_code = Some("A320".into());

        let changed = a.merge_from(&b);
        assert!(changed);
        assert_eq!(a.registration.as_deref(), Some("HB-ABC")); // preserved
        assert_eq!(a.type_code.as_deref(), Some("A320")); // filled
    }

    #[test]
    fn merge_rejects_different_icao24() {
        let mut a = ac("ABCDEF");
        let mut b = ac("123456");
        b.registration = Some("HB-XYZ".into());
        assert!(!a.merge_from(&b));
        assert!(a.registration.is_none());
    }

    #[test]
    fn completeness_flags() {
        let mut a = ac("ABCDEF");
        assert!(!a.has_type());
        a.type_code = Some("A320".into());
        a.type_description = Some("Airbus A320".into());
        assert!(a.has_type());
        assert!(!a.is_complete());
        a.registration = Some("HB-ABC".into());
        assert!(a.is_complete());
        assert!(!a.is_complete_with_operator());
        a.operator = Some("Swiss".into());
        assert!(a.is_complete_with_operator());
    }
}
