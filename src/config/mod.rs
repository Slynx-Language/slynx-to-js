use color_eyre::eyre::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StojsConfig {
    pub target: String,
    pub output: String,
    #[serde(default)]
    pub include: Vec<String>,
}

impl StojsConfig {
    pub fn load() -> Result<Option<Self>> {
        let Some(content) = std::fs::read_to_string("Stojs.toml").ok() else {
            return Ok(None);
        };
        toml::from_str(&content).map_err(|e| e.into())
    }
}
