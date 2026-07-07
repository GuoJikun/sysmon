use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct SystemInfo {
    pub cpu: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub mem_pct: f32,
    pub gpu: f32,
    pub net_down: f64,
    pub net_up: f64,
}

#[derive(Clone, Serialize)]
pub struct NetSpeedInfo {
    pub down: f64,
    pub up: f64,
}
