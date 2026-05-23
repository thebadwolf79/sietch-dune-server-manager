//! Vendor server package detection and SSH key preparation helpers.

mod package_detection;
mod ssh_key;
mod ssh_key_paths;

pub use package_detection::{
    detect_server_package_layout, ServerPackageLayout, ServerPackageLayoutInfo,
};
pub use ssh_key::{
    prepare_vendor_ssh_key, prepare_vendor_ssh_key_candidates,
    prepare_vendor_ssh_key_candidates_for_vm,
};
