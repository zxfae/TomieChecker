use futures::future::join_all;
use reqwest;
use semver::Version;
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use tokio;
use toml;
mod utils;
use crate::utils::*;

type AnalysisResult = Result<Option<DependencyAnalysis>, Box<dyn Error>>;

async fn normalize_version(version: &str) -> String {
    let parts: Vec<&str> = version.trim_start_matches('^').split('.').collect();

    match parts.len() {
        1 => format!("{}.0.0", parts[0]),
        2 => format!("{}.{}.0", parts[0], parts[1]),
        _ => version.trim_start_matches('^').to_string(),
    }
}

async fn get_crate_versions(name: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let url = format!("https://crates.io/api/v1/crates/{}", name);
    println!("Requête API pour {}: {}", name, url);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "cargo-deps-analyzer")
        .send()
        .await?;

    if !response.status().is_success() {
        println!(
            "Erreur HTTP {} pour {}: {}",
            response.status(),
            name,
            response
                .status()
                .canonical_reason()
                .unwrap_or("Unknown error")
        );
        return Ok(vec![]);
    }

    let json = response.json::<serde_json::Value>().await?;

    let versions = json["versions"]
        .as_array()
        .ok_or_else(|| {
            println!("Pas de versions trouvées pour {}", name);
            "No versions found"
        })?
        .iter()
        .filter_map(|v| v["num"].as_str().map(String::from))
        .collect();

    Ok(versions)
}

fn parse_dependency_version(dep: &Dependency) -> Option<String> {
    match dep {
        Dependency::Simple(version) => Some(version.clone()),
        Dependency::Detailed(detail) => detail.version.clone(),
    }
}

async fn analyze_dependency(name: String, current_version: String) -> AnalysisResult {
    println!(
        "Analyse de la dépendance {} version {}",
        name, current_version
    );
    let versions = get_crate_versions(&name).await?;

    if versions.is_empty() {
        println!("Aucune version trouvée pour {}", name);
        return Ok(None);
    }

    let latest = versions.first().unwrap();
    let normalized_current = normalize_version(&current_version).await;
    let normalized_latest = normalize_version(latest).await;

    println!(
        "Version normalisée pour {} : {} -> {}",
        name, normalized_current, normalized_latest
    );

    match (
        Version::parse(&normalized_current),
        Version::parse(&normalized_latest),
    ) {
        (Ok(current), Ok(latest)) => Ok(Some(DependencyAnalysis {
            name,
            current_version: current_version.trim_start_matches('^').to_string(),
            latest_version: latest.to_string(),
            is_outdated: latest > current,
        })),
        (Err(e), _) | (_, Err(e)) => {
            println!("Erreur de parsing de version pour {}: {}", name, e);
            Ok(None)
        }
    }
}

async fn analyze_dependencies(
    cargo_toml_path: &PathBuf,
) -> Result<Vec<DependencyAnalysis>, Box<dyn Error>> {
    let content = fs::read_to_string(cargo_toml_path)?;
    println!("Contenu du fichier Cargo.toml lu avec succès");

    let cargo_toml: Tomie = toml::from_str(&content)?;
    println!("Parsing du fichier Cargo.toml réussi");

    let dependencies = match cargo_toml.dependencies {
        Some(deps) => deps,
        None => return Ok(vec![]),
    };

    println!("\nDépendances trouvées dans Cargo.toml:");
    for (name, dep) in dependencies.iter() {
        println!("- {}: {:?}", name, dep);
    }

    let mut futures = Vec::new();
    for (name, dep) in dependencies {
        if let Some(current_version) = parse_dependency_version(&dep) {
            futures.push(analyze_dependency(name, current_version));
        }
    }

    let results = join_all(futures).await;
    let analyses: Vec<_> = results
        .into_iter()
        .filter_map(|r| r.ok().and_then(|o| o))
        .collect();

    Ok(analyses)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    let cargo_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("Cargo.toml")
    };

    if !cargo_path.exists() {
        return Err("Cargo.toml file does not exist".into());
    }

    println!("File analysis : {}", cargo_path.display());
    let analyses = analyze_dependencies(&cargo_path).await?;

    println!("\nAnalysis :");
    println!("------------------------");

    if analyses.is_empty() {
        println!("Aucune dépendance analysée avec succès.");
        return Ok(());
    }

    for analysis in analyses {
        println!(
            "{}: {} -> {} {}",
            analysis.name,
            analysis.current_version,
            analysis.latest_version,
            if analysis.is_outdated {
                "(obsolete)"
            } else {
                "(Up to date)"
            }
        );
    }

    Ok(())
}
