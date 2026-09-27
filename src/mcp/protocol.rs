//! MCP protocol revisions, version negotiation, and per-revision feature gates.
//!
//! Two eras of the protocol are supported (see
//! <https://modelcontextprotocol.io/specification/versioning>):
//!
//! * **Legacy** revisions (`2025-11-25` and earlier) open a session with an
//!   `initialize` handshake. The server answers with the client's requested
//!   version when it supports it, otherwise with the newest legacy version.
//! * **Modern** revisions (`2026-07-28` and later) are stateless: every
//!   request carries its version in
//!   `params._meta["io.modelcontextprotocol/protocolVersion"]`, and
//!   `server/discover` advertises the supported versions. An unsupported
//!   version is answered with `UnsupportedProtocolVersionError` (`-32022`).
//!
//! Fields that only exist in newer revisions (tool `title`, `annotations`,
//! `outputSchema`, `structuredContent`, ...) are emitted only when the
//! negotiated version defines them, so strict clients on older revisions never
//! see unknown fields.

/// Modern (stateless, per-request `_meta`) revisions, newest first.
pub const MODERN_VERSIONS: &[&str] = &["2026-07-28"];

/// Legacy (`initialize` handshake) revisions, newest first.
pub const LEGACY_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Version assumed for requests that arrive without an `initialize` handshake
/// and without per-request `_meta`: the oldest revision, whose feature set
/// every client understands.
pub const FALLBACK_VERSION: &str = "2024-11-05";

/// `_meta` key carrying the protocol version of a modern request.
pub const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
/// `_meta` key carrying the server identity on modern results.
pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// JSON-RPC error code for `UnsupportedProtocolVersionError` (2026-07-28).
pub const UNSUPPORTED_PROTOCOL_VERSION: i32 = -32022;

/// The protocol revision a request is served under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Protocol {
    pub version: &'static str,
}

impl Protocol {
    pub const fn new(version: &'static str) -> Self {
        Self { version }
    }

    /// Revisions are `YYYY-MM-DD` strings, so lexical order is date order.
    fn at_least(&self, version: &str) -> bool {
        self.version >= version
    }

    /// Tool `annotations` (readOnlyHint, destructiveHint, ...): 2025-03-26+.
    pub fn tool_annotations(&self) -> bool {
        self.at_least("2025-03-26")
    }

    /// Top-level `title` on tools, resources, and `serverInfo`: 2025-06-18+.
    pub fn titles(&self) -> bool {
        self.at_least("2025-06-18")
    }

    /// Tool `outputSchema` and result `structuredContent`: 2025-06-18+.
    pub fn structured_output(&self) -> bool {
        self.at_least("2025-06-18")
    }

    /// `serverInfo.description`: 2025-11-25+.
    pub fn implementation_description(&self) -> bool {
        self.at_least("2025-11-25")
    }

    /// Stateless revision: results carry `resultType`, `_meta.serverInfo`,
    /// and (for list/read results) caching hints.
    pub fn is_modern(&self) -> bool {
        MODERN_VERSIONS.contains(&self.version)
    }
}

impl Default for Protocol {
    fn default() -> Self {
        Self::new(FALLBACK_VERSION)
    }
}

/// Pick the legacy version to answer an `initialize` request with: the
/// client's requested version when supported, otherwise the newest legacy
/// version (per the lifecycle spec, the client then decides whether to
/// continue).
pub fn negotiate_legacy(requested: Option<&str>) -> &'static str {
    requested
        .and_then(|r| LEGACY_VERSIONS.iter().find(|v| **v == r).copied())
        .unwrap_or(LEGACY_VERSIONS[0])
}

/// Look up a modern version by name.
pub fn modern_version(requested: &str) -> Option<&'static str> {
    MODERN_VERSIONS.iter().find(|v| **v == requested).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negotiate_echoes_supported_legacy_versions() {
        for v in LEGACY_VERSIONS {
            assert_eq!(negotiate_legacy(Some(v)), *v);
        }
    }

    #[test]
    fn test_negotiate_unknown_or_missing_uses_newest_legacy() {
        assert_eq!(negotiate_legacy(None), "2025-11-25");
        assert_eq!(negotiate_legacy(Some("2099-01-01")), "2025-11-25");
        assert_eq!(negotiate_legacy(Some("2024-01-01")), "2025-11-25");
        // A modern version is not negotiated through `initialize`.
        assert_eq!(negotiate_legacy(Some("2026-07-28")), "2025-11-25");
    }

    #[test]
    fn test_feature_gates_by_version() {
        let old = Protocol::new("2024-11-05");
        assert!(!old.tool_annotations() && !old.titles() && !old.structured_output());
        let march = Protocol::new("2025-03-26");
        assert!(march.tool_annotations() && !march.titles() && !march.structured_output());
        let june = Protocol::new("2025-06-18");
        assert!(june.tool_annotations() && june.titles() && june.structured_output());
        assert!(!june.implementation_description());
        let nov = Protocol::new("2025-11-25");
        assert!(nov.implementation_description() && !nov.is_modern());
        let modern = Protocol::new("2026-07-28");
        assert!(modern.is_modern() && modern.structured_output());
        assert_eq!(Protocol::default().version, "2024-11-05");
    }

    #[test]
    fn test_version_lists_are_sorted_newest_first() {
        for list in [MODERN_VERSIONS, LEGACY_VERSIONS] {
            let mut sorted = list.to_vec();
            sorted.sort_unstable_by(|a, b| b.cmp(a));
            assert_eq!(sorted, list);
        }
        assert!(MODERN_VERSIONS[MODERN_VERSIONS.len() - 1] > LEGACY_VERSIONS[0]);
        assert_eq!(modern_version("2026-07-28"), Some("2026-07-28"));
        assert_eq!(modern_version("2025-06-18"), None);
    }
}
