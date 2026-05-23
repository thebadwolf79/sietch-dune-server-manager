//! Guest bootstrap plan, validators, and self-host token helpers.

use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{errors::failure, models::CommandResult};

/// User-selected and token-derived inputs for guest-side bootstrap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestBootstrapPlan {
    /// Player-facing address written into guest settings.
    pub player_ip: String,
    /// Human-readable world name.
    pub world_name: String,
    /// Vendor region label for the world.
    pub world_region: String,
    /// Self-host JWT used to create the world. Treat as secret.
    pub self_host_token: String,
    /// Lowercase host identifier decoded from the self-host token.
    pub host_id: String,
    /// Six-letter lowercase suffix used in the unique world resource name.
    pub world_suffix: String,
}

impl GuestBootstrapPlan {
    /// Builds a bootstrap plan from a self-host token and generated world suffix.
    pub fn from_self_host_token(
        player_ip: impl Into<String>,
        world_name: impl Into<String>,
        world_region: impl Into<String>,
        self_host_token: impl Into<String>,
    ) -> CommandResult<Self> {
        let token = self_host_token.into();
        Ok(Self {
            player_ip: player_ip.into(),
            world_name: world_name.into(),
            world_region: world_region.into(),
            host_id: host_id_from_self_host_token(&token)?,
            world_suffix: random_lowercase_suffix(),
            self_host_token: token,
        })
    }

    /// Returns the Kubernetes-safe vendor world identifier.
    pub fn world_unique_name(&self) -> String {
        format!("sh-{}-{}", self.host_id, self.world_suffix)
    }

    /// Validates addresses, world naming, region choice, and token presence.
    pub fn validate(&self) -> CommandResult<()> {
        validate_ipv4ish(&self.player_ip, "player-facing IP")?;
        validate_world_name(&self.world_name)?;
        validate_region(&self.world_region)?;
        validate_host_id(&self.host_id)?;
        validate_world_suffix(&self.world_suffix)?;
        if self.self_host_token.trim().is_empty()
            || self.self_host_token.contains('\n')
            || self.self_host_token.contains('\r')
        {
            return Err(failure("Self-host token is required"));
        }
        Ok(())
    }
}

/// Validates a vendor-supported world region label.
pub fn validate_region(value: &str) -> CommandResult<()> {
    match value {
        "Asia" | "Europe" | "North America" | "Oceania" | "South America" => Ok(()),
        _ => Err(failure(
            "Region must be Asia, Europe, North America, Oceania, or South America",
        )),
    }
}

/// Validates the six-letter suffix used in generated world names.
pub fn validate_world_suffix(value: &str) -> CommandResult<()> {
    if value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_lowercase()) {
        Ok(())
    } else {
        Err(failure(
            "World suffix must be exactly six lowercase letters",
        ))
    }
}

/// Extracts the lowercase host id from a self-host JWT payload.
pub fn host_id_from_self_host_token(token: &str) -> CommandResult<String> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| failure("Self-host token must be a JWT-like token"))?;
    let decoded = base64url_decode(payload)?;
    let value: Value = serde_json::from_slice(&decoded)
        .map_err(|err| failure(format!("Self-host token payload was not JSON: {err}")))?;
    let host_id = value["HostId"]
        .as_str()
        .ok_or_else(|| failure("Self-host token did not contain HostId"))?
        .to_ascii_lowercase();
    validate_host_id(&host_id)?;
    Ok(host_id)
}

/// Generates a six-letter lowercase suffix for a world identifier.
pub fn random_lowercase_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let mut state = nanos ^ ((std::process::id() as u64) << 32) ^ 0xa5a5_5a5a_d3c7_b901;
    let mut suffix = String::with_capacity(6);
    for _ in 0..6 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        suffix.push((b'a' + (state % 26) as u8) as char);
    }
    suffix
}

fn base64url_decode(value: &str) -> CommandResult<Vec<u8>> {
    let mut bits = 0u32;
    let mut bit_count = 0u8;
    let mut decoded = Vec::new();
    for byte in value.bytes() {
        if byte == b'=' {
            break;
        }
        let next = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(failure("Self-host token payload is not base64url")),
        };
        bits = (bits << 6) | u32::from(next);
        bit_count += 6;
        while bit_count >= 8 {
            bit_count -= 8;
            decoded.push(((bits >> bit_count) & 0xff) as u8);
        }
    }
    Ok(decoded)
}

/// Validates a decoded host id for use in Kubernetes resource names.
pub fn validate_host_id(value: &str) -> CommandResult<()> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(failure(
            "HostId must contain only lowercase letters and digits",
        ))
    }
}

/// Validates a user-facing world name.
pub fn validate_world_name(value: &str) -> CommandResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > 50
        || trimmed.contains('\n')
        || trimmed.contains('\r')
    {
        Err(failure(
            "World name must be 1-50 characters and single-line",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn validate_ipv4ish(value: &str, label: &str) -> CommandResult<()> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() == 4 && parts.iter().all(|part| part.parse::<u8>().is_ok()) {
        Ok(())
    } else {
        Err(failure(format!("{label} must be an IPv4 address")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_vendor_suffix_shape() {
        assert!(validate_world_suffix("52d16d").is_err());
        assert!(validate_world_suffix("abcdef").is_ok());
    }

    #[test]
    fn builds_plan_from_self_host_token_host_id() {
        let plan = GuestBootstrapPlan::from_self_host_token(
            "10.0.0.4",
            "Adain",
            "Europe",
            "e30.eyJIb3N0SWQiOiJBQkMxMjMifQ.sig",
        )
        .unwrap();

        assert_eq!(plan.host_id, "abc123");
        assert_eq!(plan.world_suffix.len(), 6);
        assert!(plan
            .world_suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase()));
        assert!(plan.world_unique_name().starts_with("sh-abc123-"));
    }

    #[test]
    fn rejects_token_without_host_id() {
        assert!(host_id_from_self_host_token("e30.e30.sig").is_err());
    }
}
