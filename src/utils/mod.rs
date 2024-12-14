use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Tomie {
    pub dependencies: Option<std::collections::BTreeMap<String, Dependency>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Simple(String),
    Detailed(DependencyDetail),
}

#[derive(Debug, Deserialize)]
pub struct DependencyDetail {
    pub version: Option<String>,
    #[serde(skip)]
    pub _path: Option<String>,
    #[serde(skip)]
    pub _git: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DependencyAnalysis {
    pub name: String,
    pub current_version: String,
    pub latest_version: String,
    pub is_outdated: bool,
}
