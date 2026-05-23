use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use dune_manager_core::orchestration::LocalForwarder;

use crate::dto::ServerTunnelStatus;

#[derive(Default, Clone)]
pub struct TunnelRegistry {
    pub tunnels: Arc<Mutex<HashMap<String, ManagedTunnel>>>,
}

pub struct ManagedTunnel {
    pub forwarder: LocalForwarder,
    pub status: ServerTunnelStatus,
}

impl TunnelRegistry {
    pub fn stop_all(&self) {
        let Ok(mut tunnels) = self.tunnels.lock() else {
            return;
        };
        for (_, tunnel) in tunnels.drain() {
            tunnel.forwarder.stop();
        }
    }
}
