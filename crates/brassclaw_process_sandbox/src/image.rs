//! Canonical Docker image reference validation for the process sandbox.
//!
//! This module is the single source of truth for Docker image reference
//! safety across all Reborn host entry points that shell out to `docker run`:
//!
//! - `brassclaw_process_sandbox::DockerProcessSandboxBackend` (runtime)
//! - `brassclaw_extensions::v2` manifest validator (which historically had a
//!   duplicate; the duplicate was deleted in Phase 4 and the v2 validator now
//!   calls through here)
//!
//! The validation rules defend the same attack patterns `docker run` exposes
//! when an untrusted image reference is passed verbatim to a shell line:
//!
//! - empty reference
//! - leading `-` (parsed by `docker run` as a CLI flag, e.g. `--network=host`,
//!   `--privileged`, `--pid=host`)
//! - internal whitespace (lets an attacker split the argument and slip an
//!   extra flag in)
//! - control or NUL bytes (parsed by `docker` itself or by some intermediate
//!   shell)
//! - shell metacharacters that indicate injection attempts (e.g. `'`, `"`,
//!   `\`, `$`, backtick, `;`, `&`, `|`, `>`, `<`, `?`)
//!
//! Valid references match the canonical Docker grammar stripped to the
//! registry/repository:tag@digest subset:
//!
//! `[registry[:port]/]repository[:tag][@digest]`
//!
//! Letters, digits, dot, hyphen, underscore, colon, forward slash, plus the
//! digest separator `@` are allowed. Forward-slash-separated components must
//! not be empty or `..`.

const ALLOWED_BYTES: &[u8] = b"abcdefghijklmnopqrstuvwxyz\
ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_:/@";

/// Validates a Docker image reference for use in `docker run` invocations.
///
/// Returns `Ok(())` for every canonical `[registry[:port]/]repo[:tag][@digest]`
/// reference that does not contain a control character, leading `-`,
/// whitespace, or shell metacharacter. Returns `Err(reason)` otherwise.
///
/// The validator is intentionally conservative: anything the upstream Docker
/// daemon accepts but that the operating-system shell could reinterpret is
/// rejected. This narrows the attack surface for the caller-to-daemon
/// command line, including argument injection through metadata inside the
/// image tag or repository path.
pub fn validate_reference(image: &str) -> std::result::Result<(), String> {
    if image.is_empty() {
        return Err("Docker image reference must not be empty".to_string());
    }
    if image.starts_with('-') {
        return Err("Docker image reference must not start with '-'".to_string());
    }
    if image.chars().any(char::is_whitespace) {
        return Err("Docker image reference must not contain whitespace".to_string());
    }
    if image.chars().any(|ch| ch.is_control()) {
        return Err("Docker image reference must not contain control characters".to_string());
    }
    for byte in image.bytes() {
        if !ALLOWED_BYTES.contains(&byte) {
            return Err(format!(
                "Docker image reference contains disallowed character: {:?}",
                byte as char
            ));
        }
    }
    reject_empty_or_dot_component(image)?;
    Ok(())
}

/// Returns `Err` if any forward-slash-separated component is empty or `..`.
/// Docker rejects `..` (path traversal), and an empty component
/// (`registry//repo` or `repo/` with a trailing slash) is a typo or an
/// injection attempt.
fn reject_empty_or_dot_component(image: &str) -> std::result::Result<(), String> {
    for component in image.split('/') {
        if component.is_empty() {
            return Err(format!(
                "Docker image reference contains an empty component: {image:?}"
            ));
        }
        if component == ".." {
            return Err(format!(
                "Docker image reference contains '..' path traversal: {image:?}"
            ));
        }
    }
    Ok(())
}

/// Convenience shim: returns `Err(ProcessSandboxPlanError::InvalidDockerImageReference)`
/// wrapping `reason`. Callers in plan-validating contexts use this so the
/// plan-error context is preserved alongside the canonical reason string.
#[allow(dead_code)]
pub fn validate_reference_for_plan(
    image: &str,
) -> std::result::Result<(), super::ProcessSandboxPlanError> {
    validate_reference(image)
        .map_err(|reason| super::ProcessSandboxPlanError::InvalidDockerImageReference { reason })
}

#[cfg(test)]
mod tests {
    use super::validate_reference;

    #[test]
    fn accepts_canonical_reference() {
        validate_reference("alpine:latest").unwrap();
    }

    #[test]
    fn accepts_registry_port_repository_tag() {
        validate_reference("registry.example.com:5000/team/app:1.2.3").unwrap();
    }

    #[test]
    fn accepts_digest_pinned_reference() {
        validate_reference(
            "alpine@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();
    }

    #[test]
    fn accepts_underscore_and_dot_in_repo() {
        validate_reference("ghcr.io/team_name/app_v2.0:rc.1").unwrap();
    }

    #[test]
    fn rejects_empty_string() {
        let err = validate_reference("").unwrap_err();
        assert!(err.contains("must not be empty"));
    }

    #[test]
    fn rejects_leading_dash() {
        let err = validate_reference("--network=host").unwrap_err();
        assert!(err.contains("must not start with '-'"));
    }

    #[test]
    fn rejects_internal_whitespace() {
        let err = validate_reference("alpine --privileged").unwrap_err();
        assert!(err.contains("must not contain whitespace"));
    }

    #[test]
    fn rejects_tab_and_newline() {
        assert!(validate_reference("alpine\t3.20").is_err());
        assert!(validate_reference("alpine\n3.20").is_err());
    }

    #[test]
    fn rejects_shell_metacharacters() {
        for bad in [
            "alpine;rm",
            "alpine|host",
            "alpine&rm",
            "alpine$IFS",
            "alpine`id`",
            "alpine\"x",
            "alpine\\x",
            "alpine'x",
        ] {
            assert!(
                validate_reference(bad).is_err(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_double_dot_component() {
        let err = validate_reference("registry/../escape:latest").unwrap_err();
        assert!(err.contains("'..'"));
    }

    #[test]
    fn rejects_double_slash() {
        let err = validate_reference("registry//team/app:latest").unwrap_err();
        assert!(err.contains("empty component"));
    }

    #[test]
    fn rejects_trailing_slash() {
        let err = validate_reference("registry/team/app/").unwrap_err();
        assert!(err.contains("empty component"));
    }

    #[test]
    fn rejects_query_string() {
        let err = validate_reference("alpine:latest?pull=always").unwrap_err();
        assert!(err.contains("disallowed character"));
    }
}
