//! Prompt matching for the vendor Hyper-V setup script.

use super::models::{VendorHyperVSetupRequest, VendorPromptAnswer};
use super::scripts::{memory_choice, non_empty_or, vendor_region_choice};

#[derive(Debug, Clone)]
pub(super) struct VendorPromptDriver {
    request: VendorHyperVSetupRequest,
    answered: Vec<&'static str>,
}

impl VendorPromptDriver {
    pub(super) fn new(request: VendorHyperVSetupRequest) -> Self {
        Self {
            request,
            answered: Vec::new(),
        }
    }

    pub(super) fn observe(&mut self, stdout: &str, stderr: &str) -> Vec<VendorPromptAnswer> {
        let mut answers = Vec::new();
        let combined = tail(&(stdout.to_string() + stderr), 8_000).to_ascii_lowercase();
        let stdout_tail = tail(stdout, 8_000);
        let req = self.request.clone();
        let static_choice = if req.static_network { "2" } else { "1" };
        let player_choice = if req.player_ip.trim().is_empty() {
            "1"
        } else {
            "3"
        };
        let swap_choice = if req.enable_swap { "Y" } else { "N" };
        self.maybe_answer(
            &combined,
            "drive",
            "select drive (1-",
            drive_answer(&stdout_tail, req.preferred_drive_name().as_deref()),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "remove-existing-vm",
            "do you want to remove it and continue? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "turn-off-existing-vm",
            "turn off the vm now? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "continue-incompatible-vm",
            "incompatibilities detected. continue anyway? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "external-switch",
            "add external switch? [y/n]",
            "Y",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "adapter",
            "select adapter (1-",
            adapter_answer(&stdout_tail, &req.adapter_name),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "memory-choice",
            "enter choice [1/2/3/4/5]",
            memory_choice(req.memory_gb),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "manual-memory",
            "enter memory in gb",
            req.memory_gb.max(1).to_string(),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "change-password",
            "would you like to change the default password? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "network-mode",
            "choice [1/2]",
            static_choice,
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "static-mode",
            "static ip configuration:",
            static_choice,
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "static-ip",
            "enter the static ip for the vm",
            non_empty_or(&req.static_ip, ""),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "cidr",
            "enter the cidr suffix",
            "/24",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "gateway",
            "enter the gateway ip",
            non_empty_or(&req.gateway, ""),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "dns",
            "enter the dns server",
            non_empty_or(&req.dns, "1.1.1.1"),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "player-ip-choice",
            "select the ip that players will connect to",
            player_choice,
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "player-ip-manual",
            "enter ip",
            non_empty_or(&req.player_ip, ""),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "steam-retry",
            "steam download failed. retry? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "world-name",
            "world name",
            non_empty_or(&req.world_name, "Arrakis"),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "region",
            "region",
            vendor_region_choice(&req.region),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "self-host-token",
            "self-host",
            req.self_host_token.clone(),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "experimental-swap",
            "enable experimental swap memory now? [y/n]",
            swap_choice,
            &mut answers,
        );
        answers
    }

    fn maybe_answer(
        &mut self,
        haystack: &str,
        id: &'static str,
        pattern: &str,
        answer: impl Into<String>,
        answers: &mut Vec<VendorPromptAnswer>,
    ) {
        if self.answered.contains(&id) || !haystack.contains(pattern) {
            return;
        }
        self.answered.push(id);
        answers.push(VendorPromptAnswer {
            prompt_id: id,
            answer: answer.into(),
        });
    }
}

pub(super) fn drive_answer(stdout: &str, preferred: Option<&str>) -> String {
    preferred
        .and_then(|drive| drive_line_index(stdout, drive))
        .unwrap_or(1)
        .to_string()
}

fn drive_line_index(stdout: &str, drive: &str) -> Option<usize> {
    let drive = drive.trim().trim_end_matches(':').to_ascii_lowercase();
    if drive.is_empty() {
        return None;
    }
    stdout.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let (number, rest) = trimmed.split_once('.')?;
        let index = number.trim().parse::<usize>().ok()?;
        let rest = rest.trim_start().to_ascii_lowercase();
        (rest == drive || rest.starts_with(&format!("{drive} "))).then_some(index)
    })
}

pub(super) fn adapter_answer(stdout: &str, adapter_name: &str) -> String {
    numbered_line_index(stdout, adapter_name.trim())
        .unwrap_or(1)
        .to_string()
}

fn numbered_line_index(stdout: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    stdout.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let (number, rest) = trimmed.split_once('.')?;
        let index = number.trim().parse::<usize>().ok()?;
        rest.to_ascii_lowercase()
            .contains(&needle.to_ascii_lowercase())
            .then_some(index)
    })
}

fn tail(value: &str, max_chars: usize) -> String {
    let len = value.chars().count();
    value.chars().skip(len.saturating_sub(max_chars)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn request() -> VendorHyperVSetupRequest {
        VendorHyperVSetupRequest {
            vm_destination: PathBuf::from("F:\\DuneAwakeningServer"),
            adapter_name: "Ethernet".to_string(),
            memory_gb: 24,
            static_network: true,
            static_ip: "192.168.1.50".to_string(),
            gateway: "192.168.1.1".to_string(),
            dns: "1.1.1.1".to_string(),
            player_ip: "203.0.113.10".to_string(),
            world_name: "Arrakis".to_string(),
            region: "Europe".to_string(),
            self_host_token: "secret-token".to_string(),
            enable_swap: false,
        }
    }

    #[test]
    fn answers_vendor_hyperv_prompts_from_transcript() {
        let mut driver = VendorPromptDriver::new(request());
        let transcript = r#"
Multiple drives with enough free space (>100GB) detected.
  1. C (120 GB free)
  2. F (500 GB free)
Select drive (1-2)
Multiple network adapters detected.
  1. Wi-Fi (Wireless)
  2. Ethernet (Intel)
Select adapter (1-2)
Enter choice [1/2/3/4/5]
Enter memory in GB (e.g. 16)
Would you like to change the default password? [Y/N]
How do you want the VM to be assigned an IP?
Choice [1/2]
Static IP configuration:
Choice [1/2]
Enter the static IP for the VM [192.168.1.10]
Enter the CIDR suffix (e.g. /24) [/24]
Enter the gateway IP [192.168.1.1]
Enter the DNS server [1.1.1.1]
Select the IP that players will connect to
Choice
Enter IP
World name
Region
Enable experimental swap memory now? [Y/N]
"#;
        let answers = driver.observe(transcript, "");
        let rows = answers
            .iter()
            .map(|answer| (answer.prompt_id, answer.answer.as_str()))
            .collect::<Vec<_>>();
        assert!(rows.contains(&("drive", "2")));
        assert!(rows.contains(&("adapter", "2")));
        assert!(rows.contains(&("memory-choice", "5")));
        assert!(rows.contains(&("manual-memory", "24")));
        assert!(rows.contains(&("network-mode", "2")));
        assert!(rows.contains(&("static-mode", "2")));
        assert!(rows.contains(&("player-ip-choice", "3")));
        assert!(rows.contains(&("player-ip-manual", "203.0.113.10")));
        assert!(rows.contains(&("world-name", "Arrakis")));
        assert!(rows.contains(&("region", "2")));
        assert!(rows.contains(&("experimental-swap", "N")));
    }
}
