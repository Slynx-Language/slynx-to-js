use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StojsConfig {
    pub target: String,
    pub output: String,
    #[serde(default)]
    pub include: Vec<String>,
}

impl StojsConfig {
    pub fn load() -> Option<Self> {
        let content = std::fs::read_to_string("Stojs.toml").ok()?;
        toml::from_str(&content).ok()
    }
}
