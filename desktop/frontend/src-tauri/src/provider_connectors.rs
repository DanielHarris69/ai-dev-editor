// Provider connectors for the spend ledger. Each kind declares what a live
// puller will need (credential shape, environment hints) and reports whether a
// configured provider is ready. No network calls happen here: until a live
// puller is wired, `pull` is reported as not connected rather than returning
// fabricated usage.

use serde::Serialize;

use crate::providers::ProviderRecord;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Usage is pushed to `/v1/usage` by the integration.
    Reported,
    /// The app pulls usage from the provider's API (not yet implemented).
    Pulled,
}

#[derive(Clone, Copy, Debug)]
pub struct Connector {
    pub kind: &'static str,
    pub source: Source,
    pub credential_hint: &'static str,
    pub env: &'static [&'static str],
}

pub const CONNECTORS: &[Connector] = &[
    Connector {
        kind: "reported",
        source: Source::Reported,
        credential_hint: "Your integration posts usage with this provider's identifier.",
        env: &[],
    },
    Connector {
        kind: "hermes",
        source: Source::Reported,
        credential_hint: "Not needed: Hermes posts usage to the local proxy.",
        env: &[],
    },
    Connector {
        kind: "vertex",
        source: Source::Pulled,
        credential_hint:
            "Service-account JSON path, or blank to use gcloud application-default credentials.",
        env: &["ASTRA_VERTEX_PROJECT", "ASTRA_VERTEX_SA_PATH"],
    },
    Connector {
        kind: "devpass",
        source: Source::Pulled,
        credential_hint: "DevPass API token.",
        env: &["ASTRA_DEVPASS_ENDPOINT", "ASTRA_DEVPASS_TOKEN"],
    },
];

pub fn connector(kind: &str) -> Option<&'static Connector> {
    CONNECTORS.iter().find(|c| c.kind == kind)
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Readiness {
    /// Usage arrives by push; nothing to check.
    Reported,
    /// No credentials saved yet.
    NeedsCredentials,
    /// The provider is disabled on the dashboard.
    Disabled,
    /// Credentials are saved, but the live puller for this kind is not built,
    /// so no usage can be fetched yet.
    NotConnected,
}

/// Decides what the dashboard shows for a configured provider. Pure, so the
/// dashboard and tests use the same rule.
pub fn readiness(record: &ProviderRecord, has_credentials: bool) -> Readiness {
    let Some(c) = connector(&record.kind) else {
        return Readiness::NotConnected;
    };
    if !record.enabled {
        return Readiness::Disabled;
    }
    match c.source {
        Source::Reported => Readiness::Reported,
        Source::Pulled if !has_credentials => Readiness::NeedsCredentials,
        Source::Pulled => Readiness::NotConnected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(kind: &str, enabled: bool) -> ProviderRecord {
        ProviderRecord {
            id: "p".into(),
            label: "P".into(),
            kind: kind.into(),
            enabled,
            model_allowlist: vec![],
        }
    }

    #[test]
    fn every_known_kind_has_a_connector() {
        for kind in ["hermes", "vertex", "devpass"] {
            assert!(connector(kind).is_some(), "{kind}");
        }
        assert!(connector("openai").is_none());
    }

    #[test]
    fn hermes_is_always_reported() {
        assert_eq!(readiness(&rec("hermes", true), false), Readiness::Reported);
    }

    #[test]
    fn pulled_provider_without_credentials_needs_them() {
        assert_eq!(readiness(&rec("vertex", true), false), Readiness::NeedsCredentials);
        assert_eq!(readiness(&rec("devpass", true), false), Readiness::NeedsCredentials);
    }

    #[test]
    fn pulled_provider_with_credentials_is_not_connected_until_puller_exists() {
        assert_eq!(readiness(&rec("vertex", true), true), Readiness::NotConnected);
    }

    #[test]
    fn disabled_wins_over_everything_else() {
        assert_eq!(readiness(&rec("vertex", false), true), Readiness::Disabled);
        assert_eq!(readiness(&rec("hermes", false), false), Readiness::Disabled);
    }

    #[test]
    fn unknown_kind_is_not_connected() {
        assert_eq!(readiness(&rec("openai", true), true), Readiness::NotConnected);
    }
}
