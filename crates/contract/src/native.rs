use serde::Deserialize;

/// Installed native values. Routing/runtime changes require coordinated host restart;
/// CLI defaults apply to the next CLI process. These fields never reach a browser client.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSettings {
    /// Empty selects the platform's standard per-user runtime directory, preserving its identity.
    pub runtime_directory: String,
    /// Empty selects this device. Explicit --device/--phone choices take precedence.
    pub default_device: String,
    pub default_phone: String,
    /// Default CLI execution deadline in milliseconds, bounded by the wire contract.
    pub execution_timeout_ms: u64,
    pub adb: AdbSettings,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdbSettings {
    /// Existing ADB server host:port; this product opens no listener or forwarding port.
    pub server: String,
    /// Per-connect and whole ADB command deadlines, in milliseconds; fixed per Server instance.
    pub connect_timeout_ms: u64,
    pub exchange_timeout_ms: u64,
    /// Maximum retained shell output bytes per ADB command.
    pub shell_bytes: usize,
}

impl NativeSettings {
    pub fn validate(&self) -> bool {
        let device = &self.default_device;
        let phone = &self.default_phone;
        let runtime = &self.runtime_directory;
        (runtime.is_empty() || (runtime.len() <= 80 && std::path::Path::new(runtime).is_absolute()))
            && (device.is_empty()
                || ((1..=32).contains(&device.len())
                    && device.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
                    && device
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')))
            && (phone.is_empty()
                || ((1..=128).contains(&phone.len())
                    && phone
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))))
            && (device.is_empty() || phone.is_empty())
            && (100..=25_000).contains(&self.execution_timeout_ms)
            && self.adb.validate()
    }
}

impl AdbSettings {
    pub fn validate(&self) -> bool {
        let valid_address = self
            .server
            .parse::<std::net::SocketAddr>()
            .is_ok_and(|address| address.port() != 0)
            || self.server.rsplit_once(':').is_some_and(|(host, port)| {
                !host.is_empty()
                    && host.len() <= 253
                    && host
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
                    && port.parse::<u16>().is_ok_and(|port| port != 0)
            });
        valid_address
            && (10..=60_000).contains(&self.connect_timeout_ms)
            && (self.connect_timeout_ms..=60_000).contains(&self.exchange_timeout_ms)
            && (4096..=8_388_608).contains(&self.shell_bytes)
    }
}
