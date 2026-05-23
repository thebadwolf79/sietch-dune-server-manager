//! App-owned external tool installation and discovery.

mod manager;
mod package_detection;
mod package_status;
mod path_defaults;
mod ssh_key;
mod ssh_key_paths;
mod ssh_key_rotation;
mod tool_models;
mod vdf;

pub use manager::Toolchain;
pub use package_detection::{
    detect_server_package_layout, ServerPackageInstallResult, ServerPackageLayout,
    ServerPackageLayoutInfo,
};
pub use package_status::ServerPackageStatus;
pub use path_defaults::{default_server_package_dir, default_tools_root, default_vm_destination};
pub use ssh_key::{
    prepare_vendor_ssh_key, prepare_vendor_ssh_key_candidates,
    prepare_vendor_ssh_key_candidates_for_vm, rotate_vendor_guest_ssh_key,
    rotate_vendor_guest_ssh_key_for_vm, VendorSshKeyRotationResult,
};
pub use tool_models::{ManagedTool, ToolInstallResult, ToolStatus};
pub use vdf::{read_installed_server_build_id, SERVER_APP_ID};
