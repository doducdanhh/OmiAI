//! Inference bundle packaging (`model.omiai`): a single tar+zstd archive
//! with a versioned manifest declaring schema version, present pillars,
//! and entry-point signatures.
//
//! Spec: docs/format-spec/bundle-v1.md

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use omiai_checkpoint::manifest::{Manifest as CheckpointManifest};
use omiai_checkpoint::fsutil::hash_file;
use omiai_checkpoint::world_bundle::GraphFile;
use omiai_core::logic_engine::Formula;
use omiai_knowledge::graph::{KnowledgeGraph, GraphError};
use omiai_probabilistic::bayesian::BayesianNetwork;
use omiai_causal::dag::CausalDag;
use serde::{Deserialize, Serialize};
use tar::Builder as TarBuilder;
use zstd::stream::Encoder as ZstdEncoder;
use ciborium::{ser::into_writer, de::from_reader};

/// Bundle format version
pub const BUNDLE_FORMAT_VERSION: u32 = 1;
pub const BUNDLE_SCHEMA: &str = "omiai-bundle";

/// Export options controlling what goes into the bundle
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    /// Include world snapshot for world queries
    pub include_world: bool,
    /// Include language model (if available)
    pub include_language_model: bool,
    /// Prune knowledge graph (remove inference history, keep only concepts/relations)
    pub prune_knowledge_graph: bool,
    /// Maximum reservoir size to include (0 = no limit)
    pub max_reservoir_size: usize,
}

/// Errors during export
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Source checkpoint directory not found: {0}")]
    SourceCheckpointNotFound(PathBuf),
    #[error("No data for declared capability: {0}")]
    NoDataForDeclaredCapability(&'static str),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("CBOR serialization error: {0}")]
    CborError(#[from] serde_cbor::Error),
    #[error("CBOR (ciborium) error: {0}")]
    CiboriumError(String),
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Knowledge graph error: {0}")]
    GraphError(#[from] GraphError),
    #[error("Checkpoint verification failed: {0}")]
    CheckpointError(#[from] omiai_checkpoint::error::CheckpointError),
    #[error("Archive creation error: {0}")]
    ArchiveError(String),
}

/// Bundle manifest — matches docs/format-spec/bundle-v1.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    pub format_version: u32,
    pub schema: String,
    pub created_utc: String,
    pub source_checkpoint_step: u64,
    pub git_commit: Option<String>,
    pub capabilities: BundleCapabilities,
    pub language_model_info: Option<LanguageModelInfo>,
    pub entrypoint: EntrypointInfo,
    pub files: Vec<BundleFileRecord>,
}

/// Capability flags for each pillar
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BundleCapabilities {
    pub logic: bool,
    pub knowledge_graph: bool,
    pub probabilistic: bool,
    pub causal: bool,
    pub reservoir: bool,
    pub world_query: bool,
    pub language_model: bool,
}

/// Language model metadata (required if capabilities.language_model = true)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageModelInfo {
    pub name: String,
    pub quantization: String,
    pub license: String,
    pub source_url: String,
    pub sha256: String,
    pub role: String,                    // Must be "surface_realization_only"
    pub may_assert_unverified_facts: bool, // Must be false
}

/// Entry point metadata for runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrypointInfo {
    pub function: String,
    pub input_schema: String,
    pub output_schema: String,
}

/// File record in bundle manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleFileRecord {
    pub path: String,
    pub blake3: String,
}

/// Main export function: reads checkpoint dir, writes .omiai bundle
pub fn export_bundle(
    checkpoint_dir: &Path,
    output_path: &Path,
    options: ExportOptions,
) -> Result<BundleManifest, ExportError> {
    // Verify checkpoint exists
    if !checkpoint_dir.exists() {
        return Err(ExportError::SourceCheckpointNotFound(checkpoint_dir.to_path_buf()));
    }

    // Read checkpoint manifest
    let checkpoint_manifest = CheckpointManifest::read(checkpoint_dir)?;

    // Verify checkpoint integrity
    omiai_checkpoint::verify_dir(checkpoint_dir)?;

    // Create temporary directory for bundle contents
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path();

    // Track files we add to bundle
    let mut bundle_files: Vec<BundleFileRecord> = Vec::new();
    let mut capabilities = BundleCapabilities::default();

    // Always include logic (core formulas)
    capabilities.logic = true;
    export_logic_facts(&checkpoint_manifest, checkpoint_dir, temp_path, &mut bundle_files)?;

    // Knowledge graph
    if has_knowledge_graph(&checkpoint_manifest) {
        capabilities.knowledge_graph = true;
        export_knowledge_graph(&checkpoint_manifest, checkpoint_dir, temp_path, &mut bundle_files, options.prune_knowledge_graph)?;
    }

    // Probabilistic (Bayesian networks)
    if has_probabilistic(&checkpoint_manifest) {
        capabilities.probabilistic = true;
        export_probabilistic(&checkpoint_manifest, checkpoint_dir, temp_path, &mut bundle_files)?;
    }

    // Causal (DAG)
    if has_causal(&checkpoint_manifest) {
        capabilities.causal = true;
        export_causal(&checkpoint_manifest, checkpoint_dir, temp_path, &mut bundle_files)?;
    }

    // Reservoir
    if has_reservoir(&checkpoint_manifest) {
        capabilities.reservoir = true;
        export_reservoir(&checkpoint_manifest, checkpoint_dir, temp_path, &mut bundle_files, options.max_reservoir_size)?;
    }

    // World snapshot
    if options.include_world && has_world(&checkpoint_manifest) {
        capabilities.world_query = true;
        export_world_snapshot(&checkpoint_manifest, checkpoint_dir, temp_path, &mut bundle_files)?;
    }

    // Language model (placeholder - would need actual model files)
    if options.include_language_model {
        capabilities.language_model = true;
        // Note: Actual language model files would need to be provided separately
        // This is a placeholder for the structure
    }

    // IO lexicon (if available from evolved parser)
    if has_lexicon(&checkpoint_manifest) {
        export_lexicon(&checkpoint_manifest, checkpoint_dir, temp_path, &mut bundle_files)?;
    }

    // Create manifest.json
    let manifest = create_manifest(&checkpoint_manifest, capabilities, bundle_files.clone())?;
    write_manifest(temp_path, &manifest)?;
    add_manifest_to_bundle(&manifest, &mut bundle_files);

    // Create tar.zst archive
    create_bundle_archive(temp_path, output_path, &bundle_files)?;

    Ok(manifest)
}

fn create_manifest(
    checkpoint_manifest: &CheckpointManifest,
    capabilities: BundleCapabilities,
    bundle_files: Vec<BundleFileRecord>,
) -> Result<BundleManifest, ExportError> {
    let now = chrono::Utc::now().to_rfc3339();
    let git_commit = option_env!("OMIAI_GIT_COMMIT").map(String::from);

    Ok(BundleManifest {
        format_version: BUNDLE_FORMAT_VERSION,
        schema: BUNDLE_SCHEMA.to_string(),
        created_utc: now,
        source_checkpoint_step: checkpoint_manifest.step,
        git_commit,
        capabilities,
        language_model_info: None,
        entrypoint: EntrypointInfo {
            function: "step".to_string(),
            input_schema: "InferInput_v1".to_string(),
            output_schema: "InferOutput_v1".to_string(),
        },
        files: bundle_files,
    })
}

fn write_manifest(temp_path: &Path, manifest: &BundleManifest) -> Result<(), ExportError> {
    let manifest_path = temp_path.join("manifest.json");
    let json = serde_json::to_vec_pretty(manifest)?;
    std::fs::write(&manifest_path, json)?;
    Ok(())
}

fn add_manifest_to_bundle(_manifest: &BundleManifest, _bundle_files: &mut Vec<BundleFileRecord>) {
    // The manifest itself should be in the files list with its hash
    // We'll compute it after writing
}

fn create_bundle_archive(
    temp_path: &Path,
    output_path: &Path,
    bundle_files: &[BundleFileRecord],
) -> Result<(), ExportError> {
    let output_file = std::fs::File::create(output_path)?;
    let mut zstd_encoder = ZstdEncoder::new(output_file, 3)?;
    {
        let mut tar_builder = TarBuilder::new(&mut zstd_encoder);

        // Add all files to tar
        for record in bundle_files {
            let file_path = temp_path.join(&record.path);
            if file_path.exists() {
                tar_builder.append_path_with_name(&file_path, &record.path)
                    .map_err(|e| ExportError::ArchiveError(e.to_string()))?;
            }
        }

        // Also add manifest.json
        let manifest_path = temp_path.join("manifest.json");
        if manifest_path.exists() {
            tar_builder.append_path_with_name(&manifest_path, "manifest.json")
                .map_err(|e| ExportError::ArchiveError(e.to_string()))?;
        }

        tar_builder.finish()
            .map_err(|e| ExportError::ArchiveError(e.to_string()))?;
    }

    zstd_encoder.finish()
        .map_err(|e| ExportError::ArchiveError(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Per-pillar export functions
// ============================================================================

fn export_logic_facts(
    _manifest: &CheckpointManifest,
    _checkpoint_dir: &Path,
    temp_path: &Path,
    bundle_files: &mut Vec<BundleFileRecord>,
) -> Result<(), ExportError> {
    // Look for logic formulas in checkpoint
    // For now, create minimal seed_facts.cbor with empty array
    let logic_dir = temp_path.join("logic");
    std::fs::create_dir_all(&logic_dir)?;

    let seed_facts: Vec<Formula> = Vec::new();
    let seed_path = logic_dir.join("seed_facts.cbor");
    let cbor = serde_cbor::to_vec(&seed_facts)?;
    std::fs::write(&seed_path, cbor)?;

    let blake3 = hash_file(&seed_path)?;
    bundle_files.push(BundleFileRecord {
        path: "logic/seed_facts.cbor".to_string(),
        blake3,
    });

    Ok(())
}

fn has_knowledge_graph(manifest: &CheckpointManifest) -> bool {
    manifest.files.iter().any(|f| f.path.starts_with("knowledge_graph/") || f.path.contains("graph.cbor"))
}

fn export_knowledge_graph(
    manifest: &CheckpointManifest,
    checkpoint_dir: &Path,
    temp_path: &Path,
    bundle_files: &mut Vec<BundleFileRecord>,
    _prune: bool,
) -> Result<(), ExportError> {
    let kg_dir = temp_path.join("knowledge_graph");
    std::fs::create_dir_all(&kg_dir)?;

    // Try to load and prune knowledge graph using GraphFile format
    let graph = if let Some(graph_file) = manifest.files.iter().find(|f| f.path.ends_with("graph.cbor")) {
        let path = checkpoint_dir.join(&graph_file.path);
        let data = std::fs::read(&path)?;
        let graph_file: GraphFile = from_reader(&data[..])
            .map_err(|e| ExportError::CiboriumError(e.to_string()))?;

        let mut kg = KnowledgeGraph::new();
        for concept in graph_file.concepts {
            kg.add_concept(concept);
        }
        for (from, to, kind) in graph_file.relations {
            kg.add_relation(&from, &to, kind)?;
        }

        if _prune {
            // Pruning would remove inference history - for now just keep as-is
            // TODO: Implement actual pruning when KG has inference history
        }
        kg
    } else {
        KnowledgeGraph::new()
    };

    // Serialize back using GraphFile format for consistency
    let graph_file = GraphFile {
        concepts: graph.concept_ids().map(|id| graph.get(id).unwrap().clone()).collect(),
        relations: graph.relations(),
    };

    let graph_path = kg_dir.join("graph.cbor");
    let mut buf = Vec::new();
    into_writer(&graph_file, &mut buf)
        .map_err(|e| ExportError::CiboriumError(e.to_string()))?;
    std::fs::write(&graph_path, buf)?;

    let blake3 = hash_file(&graph_path)?;
    bundle_files.push(BundleFileRecord {
        path: "knowledge_graph/graph.cbor".to_string(),
        blake3,
    });

    Ok(())
}

fn has_probabilistic(manifest: &CheckpointManifest) -> bool {
    manifest.files.iter().any(|f| f.path.contains("bayesian") || f.path.contains("probabilistic"))
}

fn export_probabilistic(
    _manifest: &CheckpointManifest,
    _checkpoint_dir: &Path,
    temp_path: &Path,
    bundle_files: &mut Vec<BundleFileRecord>,
) -> Result<(), ExportError> {
    let prob_dir = temp_path.join("probabilistic");
    std::fs::create_dir_all(&prob_dir)?;

    let networks: Vec<BayesianNetwork> = Vec::new(); // Placeholder
    let networks_path = prob_dir.join("networks.cbor");
    let cbor = serde_cbor::to_vec(&networks)?;
    std::fs::write(&networks_path, cbor)?;

    let blake3 = hash_file(&networks_path)?;
    bundle_files.push(BundleFileRecord {
        path: "probabilistic/networks.cbor".to_string(),
        blake3,
    });

    Ok(())
}

fn has_causal(manifest: &CheckpointManifest) -> bool {
    manifest.files.iter().any(|f| f.path.contains("causal") || f.path.contains("dag.cbor"))
}

fn export_causal(
    _manifest: &CheckpointManifest,
    _checkpoint_dir: &Path,
    temp_path: &Path,
    bundle_files: &mut Vec<BundleFileRecord>,
) -> Result<(), ExportError> {
    let causal_dir = temp_path.join("causal");
    std::fs::create_dir_all(&causal_dir)?;

    let dag = CausalDag::new(); // Placeholder
    let dag_path = causal_dir.join("dag.cbor");
    let cbor = serde_cbor::to_vec(&dag)?;
    std::fs::write(&dag_path, cbor)?;

    let blake3 = hash_file(&dag_path)?;
    bundle_files.push(BundleFileRecord {
        path: "causal/dag.cbor".to_string(),
        blake3,
    });

    Ok(())
}

fn has_reservoir(manifest: &CheckpointManifest) -> bool {
    manifest.files.iter().any(|f| f.path.contains("reservoir") || f.path.contains("weights"))
}

fn export_reservoir(
    _manifest: &CheckpointManifest,
    _checkpoint_dir: &Path,
    temp_path: &Path,
    bundle_files: &mut Vec<BundleFileRecord>,
    _max_size: usize,
) -> Result<(), ExportError> {
    let res_dir = temp_path.join("reservoir");
    std::fs::create_dir_all(&res_dir)?;

    // Use CBOR for now (safetensors can be added later as a proper dependency)
    let weights_path = res_dir.join("weights.cbor");
    let weights_data = ReservoirExportData {
        size: 50,
        input_dim: 10,
        output_dim: 5,
        spectral_radius: 0.95,
        leak_rate: 0.3,
        seed: 42,
    };
    let mut buf = Vec::new();
    into_writer(&weights_data, &mut buf)
        .map_err(|e| ExportError::CiboriumError(e.to_string()))?;
    std::fs::write(&weights_path, buf)?;

    let blake3 = hash_file(&weights_path)?;
    bundle_files.push(BundleFileRecord {
        path: "reservoir/weights.cbor".to_string(),
        blake3,
    });

    Ok(())
}

/// Serializable reservoir data for export
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ReservoirExportData {
    size: usize,
    input_dim: usize,
    output_dim: usize,
    spectral_radius: f64,
    leak_rate: f64,
    seed: u64,
}

fn has_world(manifest: &CheckpointManifest) -> bool {
    manifest.files.iter().any(|f| f.path.starts_with("world/"))
}

fn export_world_snapshot(
    manifest: &CheckpointManifest,
    checkpoint_dir: &Path,
    temp_path: &Path,
    bundle_files: &mut Vec<BundleFileRecord>,
) -> Result<(), ExportError> {
    let world_dir = temp_path.join("world_snapshot");
    std::fs::create_dir_all(&world_dir)?;

    // Export grid.bin
    if let Some(grid_file) = manifest.files.iter().find(|f| f.path.ends_with("grid.bin")) {
        let src = checkpoint_dir.join(&grid_file.path);
        let dst = world_dir.join("grid.bin");
        std::fs::copy(&src, &dst)?;
        let blake3 = hash_file(&dst)?;
        bundle_files.push(BundleFileRecord {
            path: "world_snapshot/grid.bin".to_string(),
            blake3,
        });
    }

    // Export agents.cbor (atoms)
    if let Some(agents_file) = manifest.files.iter().find(|f| f.path.ends_with("atoms.cbor")) {
        let src = checkpoint_dir.join(&agents_file.path);
        let dst = world_dir.join("agents.cbor");
        std::fs::copy(&src, &dst)?;
        let blake3 = hash_file(&dst)?;
        bundle_files.push(BundleFileRecord {
            path: "world_snapshot/agents.cbor".to_string(),
            blake3,
        });
    }

    // Export vocabulary.cbor
    if let Some(vocab_file) = manifest.files.iter().find(|f| f.path.ends_with("vocabulary.cbor")) {
        let src = checkpoint_dir.join(&vocab_file.path);
        let dst = world_dir.join("vocabulary.cbor");
        std::fs::copy(&src, &dst)?;
        let blake3 = hash_file(&dst)?;
        bundle_files.push(BundleFileRecord {
            path: "world_snapshot/vocabulary.cbor".to_string(),
            blake3,
        });
    }

    // Export registry.cbor
    if let Some(reg_file) = manifest.files.iter().find(|f| f.path.ends_with("registry.cbor")) {
        let src = checkpoint_dir.join(&reg_file.path);
        let dst = world_dir.join("registry.cbor");
        std::fs::copy(&src, &dst)?;
        let blake3 = hash_file(&dst)?;
        bundle_files.push(BundleFileRecord {
            path: "world_snapshot/registry.cbor".to_string(),
            blake3,
        });
    }

    // Export rng_state.bin
    if let Some(rng_file) = manifest.files.iter().find(|f| f.path.ends_with("rng_state.bin")) {
        let src = checkpoint_dir.join(&rng_file.path);
        let dst = world_dir.join("rng_state.bin");
        std::fs::copy(&src, &dst)?;
        let blake3 = hash_file(&dst)?;
        bundle_files.push(BundleFileRecord {
            path: "world_snapshot/rng_state.bin".to_string(),
            blake3,
        });
    }

    // Export airwave.cbor
    if let Some(airwave_file) = manifest.files.iter().find(|f| f.path.ends_with("airwave.cbor")) {
        let src = checkpoint_dir.join(&airwave_file.path);
        let dst = world_dir.join("airwave.cbor");
        std::fs::copy(&src, &dst)?;
        let blake3 = hash_file(&dst)?;
        bundle_files.push(BundleFileRecord {
            path: "world_snapshot/airwave.cbor".to_string(),
            blake3,
        });
    }

    // Export conventions.cbor (optional)
    if let Some(conv_file) = manifest.files.iter().find(|f| f.path.ends_with("conventions.cbor")) {
        let src = checkpoint_dir.join(&conv_file.path);
        let dst = world_dir.join("conventions.cbor");
        std::fs::copy(&src, &dst)?;
        let blake3 = hash_file(&dst)?;
        bundle_files.push(BundleFileRecord {
            path: "world_snapshot/conventions.cbor".to_string(),
            blake3,
        });
    }

    // Export knowledge_graph/graph.cbor (optional)
    if let Some(kg_file) = manifest.files.iter().find(|f| f.path.ends_with("graph.cbor")) {
        let src = checkpoint_dir.join(&kg_file.path);
        let kg_dir = world_dir.join("knowledge_graph");
        std::fs::create_dir_all(&kg_dir)?;
        let dst = kg_dir.join("graph.cbor");
        std::fs::copy(&src, &dst)?;
        let blake3 = hash_file(&dst)?;
        bundle_files.push(BundleFileRecord {
            path: "world_snapshot/knowledge_graph/graph.cbor".to_string(),
            blake3,
        });
    }

    Ok(())
}

fn has_lexicon(manifest: &CheckpointManifest) -> bool {
    manifest.files.iter().any(|f| f.path.contains("lexicon"))
}

fn export_lexicon(
    _manifest: &CheckpointManifest,
    _checkpoint_dir: &Path,
    temp_path: &Path,
    bundle_files: &mut Vec<BundleFileRecord>,
) -> Result<(), ExportError> {
    let io_dir = temp_path.join("io");
    std::fs::create_dir_all(&io_dir)?;

    let lexicon: HashMap<String, String> = HashMap::new(); // Placeholder
    let lex_path = io_dir.join("lexicon.cbor");
    let cbor = serde_cbor::to_vec(&lexicon)?;
    std::fs::write(&lex_path, cbor)?;

    let blake3 = hash_file(&lex_path)?;
    bundle_files.push(BundleFileRecord {
        path: "io/lexicon.cbor".to_string(),
        blake3,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_export_bundle_creates_valid_archive() {
        let temp = tempdir().unwrap();
        let checkpoint_dir = temp.path().join("checkpoint_step_1");
        fs::create_dir_all(&checkpoint_dir).unwrap();

        // Create minimal checkpoint manifest
        let manifest = omiai_checkpoint::manifest::Manifest {
            format_version: 1,
            git_commit: Some("test".to_string()),
            step: 1,
            timestamp_utc: chrono::Utc::now().to_rfc3339(),
            rng_seed: 42,
            rng_state_hex: "0".repeat(64),
            files: vec![],
        };
        omiai_checkpoint::manifest::Manifest::write(&checkpoint_dir, &manifest.files).unwrap();

        // Create a dummy file for hash
        let dummy = checkpoint_dir.join("dummy.bin");
        fs::write(&dummy, b"test").unwrap();

        let output = temp.path().join("model.omiai");
        let result = export_bundle(&checkpoint_dir, &output, ExportOptions::default());

        assert!(result.is_ok());
        assert!(output.exists());

        let manifest = result.unwrap();
        assert_eq!(manifest.format_version, 1);
        assert_eq!(manifest.schema, "omiai-bundle");
        assert!(manifest.capabilities.logic);
    }
}