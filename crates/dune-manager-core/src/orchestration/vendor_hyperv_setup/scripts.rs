//! PowerShell wrapper and remote setup script generation for the vendor flow.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::{errors::failure, models::CommandResult, shell::ps_single_quoted};

use super::models::VendorHyperVSetupRequest;

const WRAPPER_TEMPLATE: &str = include_str!("wrapper.ps1");
const REMOTE_SETUP_SCRIPT: &str = include_str!("remote_setup.sh");

pub(super) fn write_wrapper_script(wrapper: &str) -> CommandResult<PathBuf> {
    let path = env::temp_dir().join(format!(
        "dune-vendor-hyperv-setup-{}.ps1",
        std::process::id()
    ));
    fs::write(&path, wrapper)
        .map_err(|err| failure(format!("Failed to write vendor setup wrapper: {err}")))?;
    Ok(path)
}

pub(super) fn vendor_powershell_wrapper(
    request: &VendorHyperVSetupRequest,
    script_dir: &Path,
) -> String {
    let static_network = if request.static_network {
        "$true"
    } else {
        "$false"
    };
    let enable_swap = if request.enable_swap {
        "$true"
    } else {
        "$false"
    };
    WRAPPER_TEMPLATE
        .replace(
            "__SCRIPT_DIR__",
            &ps_single_quoted(&script_dir.to_string_lossy()),
        )
        .replace(
            "__DRIVE__",
            &ps_single_quoted(
                &request
                    .preferred_drive_name()
                    .unwrap_or_else(|| "C".to_string()),
            ),
        )
        .replace("__ADAPTER__", &ps_single_quoted(&request.adapter_name))
        .replace(
            "__MEMORY_CHOICE__",
            &ps_single_quoted(&memory_choice(request.memory_gb)),
        )
        .replace(
            "__MEMORY_GB__",
            &ps_single_quoted(&request.memory_gb.max(1).to_string()),
        )
        .replace("__STATIC_NETWORK__", static_network)
        .replace("__STATIC_IP__", &ps_single_quoted(&request.static_ip))
        .replace("__GATEWAY__", &ps_single_quoted(&request.gateway))
        .replace(
            "__DNS__",
            &ps_single_quoted(non_empty_or(&request.dns, "1.1.1.1").as_ref()),
        )
        .replace("__PLAYER_IP__", &ps_single_quoted(&request.player_ip))
        .replace("__ENABLE_SWAP__", enable_swap)
        .replace(
            "__REMOTE_SETUP_INPUT_B64__",
            &ps_single_quoted(&base64_text(&remote_setup_answers(request))),
        )
        .replace(
            "__REMOTE_SETUP_SCRIPT_B64__",
            &ps_single_quoted(&base64_text(remote_setup_script())),
        )
}

pub(super) fn base64_text(value: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = value.as_bytes();
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        encoded.push(TABLE[(b0 >> 2) as usize] as char);
        encoded.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

pub(super) fn remote_setup_script() -> &'static str {
    REMOTE_SETUP_SCRIPT
}

fn remote_setup_answers(request: &VendorHyperVSetupRequest) -> String {
    let region_choice = vendor_region_choice(&request.region);
    [
        non_empty_or(&request.world_name, "Arrakis"),
        region_choice.to_string(),
        request.self_host_token.trim().to_string(),
    ]
    .join("\n")
        + "\n"
}

pub(super) fn vendor_region_choice(region: &str) -> &'static str {
    match region.trim().to_ascii_lowercase().as_str() {
        "asia" => "1",
        "north america" => "3",
        "oceania" => "4",
        "south america" => "5",
        _ => "2",
    }
}

pub(super) fn memory_choice(memory_gb: u64) -> String {
    match memory_gb {
        10 => "1".to_string(),
        20 => "2".to_string(),
        30 => "3".to_string(),
        40 => "4".to_string(),
        _ => "5".to_string(),
    }
}

pub(super) fn non_empty_or(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_release_region_menu_choices() {
        assert_eq!(vendor_region_choice("Asia"), "1");
        assert_eq!(vendor_region_choice("Europe"), "2");
        assert_eq!(vendor_region_choice("North America"), "3");
        assert_eq!(vendor_region_choice("Oceania"), "4");
        assert_eq!(vendor_region_choice("South America"), "5");
    }
}
