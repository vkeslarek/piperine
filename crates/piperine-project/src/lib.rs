use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiperineToml {
    pub project: Project,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub edition: String,
}

impl PiperineToml {
    pub fn new(name: &str) -> Self {
        Self {
            project: Project {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                authors: vec![],
                edition: "2024".to_string(),
            },
        }
    }

    pub fn to_string_pretty(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}
