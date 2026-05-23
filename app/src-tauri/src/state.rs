use std::{
    collections::HashMap,
    process::Child,
    sync::{Arc, Mutex},
};

use crate::dto::ServerTunnelStatus;

#[derive(Default, Clone)]
pub struct TunnelRegistry {
    pub tunnels: Arc<Mutex<HashMap<String, ManagedTunnel>>>,
}

pub struct ManagedTunnel {
    pub child: Child,
    pub status: ServerTunnelStatus,
}

impl TunnelRegistry {
    pub fn stop_all(&self) {
        let Ok(mut tunnels) = self.tunnels.lock() else {
            return;
        };
        for (_, mut tunnel) in tunnels.drain() {
            let _ = tunnel.child.kill();
            let _ = tunnel.child.wait();
        }
    }
}
