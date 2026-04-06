use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct LoggerConfig {
    pub logged_metadata_keys: HashSet<String>,
}

impl LoggerConfig {
    pub fn with_logged_metadata_keys<I, S>(keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            logged_metadata_keys: keys.into_iter().map(Into::into).collect(),
        }
    }

    pub fn should_log_metadata_key(&self, key: &str) -> bool {
        self.logged_metadata_keys.contains(key)
    }
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self::with_logged_metadata_keys([
            "node_id",
            "device_id",
            "capability_id",
            "capability_kind",
            "device_class",
            "manufacturer",
            "model",
            "unit",
        ])
    }
}
