pub mod credentials;
pub mod ipc;
pub mod tokens;
pub mod transport;
pub mod vault;

#[cfg(feature = "stronghold")]
pub mod stronghold_vault;

/// IPC protocol version. Bumped when breaking changes are made to the IPC message format.
/// Proxy and bw shim must match the daemon's major version to connect.
pub const IPC_PROTOCOL_VERSION: &str = "1.0.0";

/// Checks if two version strings are compatible (same major version).
pub fn versions_compatible(client_version: &str, daemon_version: &str) -> bool {
    let client_major = client_version.split('.').next().unwrap_or("0");
    let daemon_major = daemon_version.split('.').next().unwrap_or("0");
    client_major == daemon_major
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compatible_versions() {
        assert!(versions_compatible("1.0.0", "1.0.0"));
        assert!(versions_compatible("1.0.0", "1.2.3"));
        assert!(versions_compatible("1.5.0", "1.0.0"));
    }

    #[test]
    fn test_incompatible_versions() {
        assert!(!versions_compatible("1.0.0", "2.0.0"));
        assert!(!versions_compatible("0.1.0", "1.0.0"));
    }
}
