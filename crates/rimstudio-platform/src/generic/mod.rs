//! Fallbacks for operating systems without a dedicated module, and the credential stub.

use rimstudio_core::ports::{CredentialStore, LinkSupport, PortError, PortErrorKind, PortResult};

/// A credential store that stores nothing: every call fails with `Unsupported`. 0.1.0 has no
/// keyring dependency; secrets are never written to disk by the app.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCredentialStore;

fn unsupported<T>() -> PortResult<T> {
    Err(PortError::new(
        PortErrorKind::Unsupported,
        "no credential store in this build",
    ))
}

impl CredentialStore for NoCredentialStore {
    fn get(&self, _service: &str, _account: &str) -> PortResult<Option<String>> {
        unsupported()
    }
    fn set(&self, _service: &str, _account: &str, _secret: &str) -> PortResult<()> {
        unsupported()
    }
    fn delete(&self, _service: &str, _account: &str) -> PortResult<()> {
        unsupported()
    }
}

/// The link support of a platform that can create no links at all.
pub fn no_link_support() -> LinkSupport {
    LinkSupport {
        symlink: false,
        junction: false,
        needs_privilege: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_credential_call_is_unsupported() {
        let s = NoCredentialStore;
        assert_eq!(
            s.get("svc", "acc").unwrap_err().kind,
            PortErrorKind::Unsupported
        );
        assert_eq!(
            s.set("svc", "acc", "x").unwrap_err().code(),
            "port.unsupported"
        );
        assert_eq!(
            s.delete("svc", "acc").unwrap_err().kind,
            PortErrorKind::Unsupported
        );
    }

    #[test]
    fn no_links_are_reported() {
        let s = no_link_support();
        assert!(!s.symlink && !s.junction);
    }
}
