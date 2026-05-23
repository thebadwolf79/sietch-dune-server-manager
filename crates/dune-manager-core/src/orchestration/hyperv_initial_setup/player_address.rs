use crate::{
    models::CommandResult,
    orchestration::{emit_hyperv_event, GuestProvider, OperationSink, StepAction, StepDomain},
};

use super::request::PlayerAddressCandidates;

/// Detects LAN and optional public player-facing address candidates.
pub fn detect_player_address_candidates(
    guest: &impl GuestProvider,
    guest_ip: &str,
    sink: &mut impl OperationSink,
) -> CommandResult<PlayerAddressCandidates> {
    emit_hyperv_event(
        sink,
        "guest.detect-public-ip",
        "Detecting public player-facing IP.",
        StepDomain::Guest,
        StepAction::Detect,
    );
    Ok(PlayerAddressCandidates {
        guest_lan_ip: guest_ip.to_string(),
        public_ip: guest.detect_public_ip(guest_ip)?,
    })
}
