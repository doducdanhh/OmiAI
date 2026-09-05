//! Minimal load-and-infer runtime for exported `model.omiai` bundles:
//! `load(path)` + `step(input) -> output`, nothing else.
//!
//! Constraint: this crate must NEVER depend on training/evolution code,
//! so it can compile to native lib, cdylib (FFI), wasm32-wasi, and
//! wasm32-unknown-unknown targets.
//!
//! Spec: docs/format-spec/bundle-v1.md

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use omiai_core::logic_engine::Formula;
use omiai_core::prover::TheoremProver;
use omiai_core::inference::ProofResult;
use omiai_knowledge::graph::KnowledgeGraph;
use omiai_probabilistic::bayesian::BayesianNetwork;
use omiai_causal::dag::CausalDag;
use omiai_neuro::reservoir::Reservoir;
use omiai_world::world_loop::World;
use omiai_io::{ChatEngine, ChatRequest, ChatResponse, DetectedLanguage, ConversationMemory, DialogueRouter, ReasoningResult};
use omiai_io::nlp_parser::QueryType;
use serde::{Deserialize, Serialize};
use tar::Archive;
use zstd::stream::Decoder as ZstdDecoder;
use blake3;
use ciborium::de::from_reader;
use tempfile::tempdir;
use thiserror::Error;
use sha2::{Sha256, Digest};

pub const SUPPORTED_FORMAT_VERSION: u32 = 1;
pub const EXPECTED_SCHEMA: &str = "omiai-bundle";

/// Errors during bundle loading and inference
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("Unsupported bundle format version: {0} (supported: {})", SUPPORTED_FORMAT_VERSION)]
    UnsupportedFormatVersion(u32),
    #[error("Manifest missing or invalid")]
    ManifestMissing,
    #[error("Invalid schema: expected '{}', got '{0}'", EXPECTED_SCHEMA)]
    InvalidSchema(String),
    #[error("Hash mismatch for file '{path}': expected '{expected}', got '{actual}'")]
    HashMismatch { path: String, expected: String, actual: String },
    #[error("Missing file in bundle: {0}")]
    MissingFile(String),
    #[error("Capability disabled: {0}")]
    CapabilityDisabled(&'static str),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("CBOR deserialization error: {0}")]
    CiboriumError(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Knowledge graph error: {0}")]
    KnowledgeGraphError(String),
    #[error("Archive error: {0}")]
    ArchiveError(String),
    #[error("Bundle verification failed: {0}")]
    VerificationError(String),
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
    pub role: String,
    pub may_assert_unverified_facts: bool,
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

/// Input to the inference step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferInput {
    pub text: String,
    pub session_id: Option<String>,
    pub language: Option<String>,
}

/// Output from the inference step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferOutput {
    pub text: String,
    pub grounded: bool,
    pub reasoning: Option<String>,
    pub intent: String,
    pub confidence: u8,
}

/// Main model handle — loads bundle and runs inference
pub struct OmiaiModel {
    manifest: BundleManifest,
    extract_dir: PathBuf,
    _temp_dir: tempfile::TempDir, // Keep alive for extracted files

    // Pillar states (loaded lazily on first use)
    prover: Option<TheoremProver>,
    seed_facts: Vec<Formula>,
    knowledge_graph: Option<KnowledgeGraph>,
    bayesian_networks: Vec<BayesianNetwork>,
    causal_dag: Option<CausalDag>,
    reservoir: Option<Reservoir>,
    world: Option<World>,
    chat_engine: Option<ChatEngine>,
}

impl OmiaiModel {
    /// Load a bundle from disk, verify hashes, initialize pillars
    pub fn load(bundle_path: &Path) -> Result<Self, RuntimeError> {
        // 1. Extract tar.zst to temp directory
        let temp_dir = tempdir().map_err(|e| RuntimeError::IoError(e))?;
        let extract_path = temp_dir.path().to_path_buf();

        Self::extract_bundle(bundle_path, &extract_path)?;

        // 2. Read and parse manifest.json
        let manifest_path = extract_path.join("manifest.json");
        if !manifest_path.exists() {
            return Err(RuntimeError::ManifestMissing);
        }

        let manifest_data = std::fs::read_to_string(&manifest_path)
            .map_err(|e| RuntimeError::IoError(e))?;
        let manifest: BundleManifest = serde_json::from_str(&manifest_data)
            .map_err(|e| RuntimeError::JsonError(e))?;

        // 3. Verify format version
        if manifest.format_version != SUPPORTED_FORMAT_VERSION {
            return Err(RuntimeError::UnsupportedFormatVersion(manifest.format_version));
        }

        // 4. Verify schema
        if manifest.schema != EXPECTED_SCHEMA {
            return Err(RuntimeError::InvalidSchema(manifest.schema));
        }

        // 5. Verify all files exist and hashes match
        Self::verify_bundle_files(&extract_path, &manifest.files)?;

        // 6. Verify language model if present
        if manifest.capabilities.language_model {
            Self::verify_language_model(&extract_path, &manifest)?;
        }

        // 7. Initialize pillars that are always needed (logic + chat engine)
        let prover = TheoremProver::new();

        // Load seed facts
        let seed_facts_path = extract_path.join("logic/seed_facts.cbor");
        let seed_facts: Vec<Formula> = if seed_facts_path.exists() {
            let data = std::fs::read(&seed_facts_path)
                .map_err(|e| RuntimeError::IoError(e))?;
            from_reader(&data[..])
                .map_err(|e| RuntimeError::CiboriumError(e.to_string()))?
        } else {
            Vec::new()
        };

        // Load knowledge graph if capability enabled
        let knowledge_graph = if manifest.capabilities.knowledge_graph {
            let kg_path = extract_path.join("knowledge_graph/graph.cbor");
            if kg_path.exists() {
                let data = std::fs::read(&kg_path)
                    .map_err(|e| RuntimeError::IoError(e))?;
                let graph_file: GraphFile = from_reader(&data[..])
                    .map_err(|e| RuntimeError::CiboriumError(e.to_string()))?;

                let mut kg = KnowledgeGraph::new();
                for concept in graph_file.concepts {
                    kg.add_concept(concept);
                }
                for (from, to, kind) in graph_file.relations {
                    kg.add_relation(&from, &to, kind)
                        .map_err(|e| RuntimeError::KnowledgeGraphError(e.to_string()))?;
                }
                Some(kg)
            } else {
                return Err(RuntimeError::MissingFile("knowledge_graph/graph.cbor".to_string()));
            }
        } else {
            None
        };

        // Load Bayesian networks if capability enabled
        let bayesian_networks: Vec<BayesianNetwork> = if manifest.capabilities.probabilistic {
            let bn_path = extract_path.join("probabilistic/networks.cbor");
            if bn_path.exists() {
                let data = std::fs::read(&bn_path)
                    .map_err(|e| RuntimeError::IoError(e))?;
                from_reader(&data[..])
                    .map_err(|e| RuntimeError::CiboriumError(e.to_string()))?
            } else {
                return Err(RuntimeError::MissingFile("probabilistic/networks.cbor".to_string()));
            }
        } else {
            Vec::new()
        };

        // Load causal DAG if capability enabled
        let causal_dag: Option<CausalDag> = if manifest.capabilities.causal {
            let dag_path = extract_path.join("causal/dag.cbor");
            if dag_path.exists() {
                let data = std::fs::read(&dag_path)
                    .map_err(|e| RuntimeError::IoError(e))?;
                from_reader(&data[..])
                    .map_err(|e| RuntimeError::CiboriumError(e.to_string()))?
            } else {
                return Err(RuntimeError::MissingFile("causal/dag.cbor".to_string()));
            }
        } else {
            None
        };

        // Load reservoir if capability enabled
        let reservoir = if manifest.capabilities.reservoir {
            let res_path = extract_path.join("reservoir/weights.cbor");
            if res_path.exists() {
                let data = std::fs::read(&res_path)
                    .map_err(|e| RuntimeError::IoError(e))?;
                let res_data: ReservoirExportData = from_reader(&data[..])
                    .map_err(|e| RuntimeError::CiboriumError(e.to_string()))?;

                let mut res = Reservoir::new(
                    res_data.size,
                    res_data.input_dim,
                    res_data.output_dim,
                    res_data.spectral_radius,
                    res_data.seed
                );
                Some(res)
            } else {
                return Err(RuntimeError::MissingFile("reservoir/weights.cbor".to_string()));
            }
        } else {
            None
        };

        // Load world snapshot if capability enabled
        let world = if manifest.capabilities.world_query {
            let world_dir = extract_path.join("world_snapshot");
            if world_dir.exists() {
                Some(World::load_snapshot(&world_dir)
                    .map_err(|e| RuntimeError::VerificationError(e.to_string()))?)
            } else {
                return Err(RuntimeError::MissingFile("world_snapshot/".to_string()));
            }
        } else {
            None
        };

        // Initialize chat engine with loaded pillars
        let mut chat_engine = ChatEngine::new();

        // Configure dialogue router with loaded capabilities
        let mut router = DialogueRouter::new();
        if manifest.capabilities.knowledge_graph {
            if let Some(kg) = &knowledge_graph {
                router.set_knowledge_graph(kg.clone());
            }
        }
        if manifest.capabilities.probabilistic {
            for bn in &bayesian_networks {
                router.add_bayesian_network(bn.clone());
            }
        }
        if manifest.capabilities.causal {
            if let Some(dag) = &causal_dag {
                router.set_causal_dag(dag.clone());
            }
        }
        if manifest.capabilities.reservoir {
            if let Some(res) = &reservoir {
                router.set_reservoir(res.clone());
            }
        }
        if manifest.capabilities.world_query {
            if let Some(w) = &world {
                router.set_world((*w).clone());
            }
        }

        chat_engine.set_router(router);

        Ok(Self {
            manifest,
            extract_dir: extract_path,
            _temp_dir: temp_dir,
            prover: Some(prover),
            seed_facts,
            knowledge_graph,
            bayesian_networks,
            causal_dag,
            reservoir,
            world,
            chat_engine: Some(chat_engine),
        })
    }

    /// Single inference step — the ONLY entry point for omiai-serve / omiai-cli / FFI / WASM
    pub fn step(&mut self, input: &InferInput) -> Result<InferOutput, RuntimeError> {
        let chat_engine = self.chat_engine.as_mut()
            .ok_or_else(|| RuntimeError::VerificationError("chat engine not initialized".to_string()))?;

        let lang = match input.language.as_deref() {
            Some("vi") | Some("vietnamese") => DetectedLanguage::Vietnamese,
            _ => DetectedLanguage::English,
        };

        let request = ChatRequest {
            text: input.text.clone(),
            preferred_language: Some(lang),
        };

        // Create or retrieve conversation memory for session
        let mut memory = ConversationMemory::default();

        let response: ChatResponse = chat_engine.handle(&request, &mut memory);

        let grounded = response.proven;
        let reasoning = Some(format!("{:?}", response.intent));

        Ok(InferOutput {
            text: response.text,
            grounded,
            reasoning,
            intent: format!("{:?}", response.intent),
            confidence: response.confidence,
        })
    }

    /// Get manifest for introspection
    pub fn manifest(&self) -> &BundleManifest {
        &self.manifest
    }

    /// Check if a capability is available
    pub fn has_capability(&self, name: &str) -> bool {
        match name {
            "logic" => self.manifest.capabilities.logic,
            "knowledge_graph" => self.manifest.capabilities.knowledge_graph,
            "probabilistic" => self.manifest.capabilities.probabilistic,
            "causal" => self.manifest.capabilities.causal,
            "reservoir" => self.manifest.capabilities.reservoir,
            "world_query" => self.manifest.capabilities.world_query,
            "language_model" => self.manifest.capabilities.language_model,
            _ => false,
        }
    }

    fn extract_bundle(bundle_path: &Path, extract_to: &Path) -> Result<(), RuntimeError> {
        let file = std::fs::File::open(bundle_path)
            .map_err(|e| RuntimeError::IoError(e))?;

        let mut zstd_decoder = ZstdDecoder::new(file)
            .map_err(|e| RuntimeError::ArchiveError(e.to_string()))?;

        let mut archive = Archive::new(&mut zstd_decoder);
        archive.unpack(extract_to)
            .map_err(|e| RuntimeError::ArchiveError(e.to_string()))?;

        Ok(())
    }

    fn verify_bundle_files(extract_path: &Path, files: &[BundleFileRecord]) -> Result<(), RuntimeError> {
        for record in files {
            let file_path = extract_path.join(&record.path);
            if !file_path.exists() {
                return Err(RuntimeError::MissingFile(record.path.clone()));
            }

            let data = std::fs::read(&file_path)
                .map_err(|e| RuntimeError::IoError(e))?;
            let hash = blake3::hash(&data);
            let actual_hash = hex::encode(hash.as_bytes());

            if actual_hash != record.blake3 {
                return Err(RuntimeError::HashMismatch {
                    path: record.path.clone(),
                    expected: record.blake3.clone(),
                    actual: actual_hash,
                });
            }
        }
        Ok(())
    }

    fn verify_language_model(extract_path: &Path, manifest: &BundleManifest) -> Result<(), RuntimeError> {
        let info = manifest.language_model_info.as_ref()
            .ok_or_else(|| RuntimeError::VerificationError("language_model_info missing but capability enabled".to_string()))?;

        // Verify role constraint
        if info.role != "surface_realization_only" {
            return Err(RuntimeError::VerificationError(
                format!("language_model_info.role must be 'surface_realization_only', got '{}'", info.role)
            ));
        }

        // Verify may_assert_unverified_facts constraint
        if info.may_assert_unverified_facts {
            return Err(RuntimeError::VerificationError(
                "language_model_info.may_assert_unverified_facts must be false".to_string()
            ));
        }

        // Verify SHA256 of model file
        let model_path = extract_path.join("language_model/model.gguf");
        if model_path.exists() {
            let data = std::fs::read(&model_path)
                .map_err(|e| RuntimeError::IoError(e))?;
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let actual = hex::encode(hasher.finalize());

            if actual != info.sha256 {
                return Err(RuntimeError::HashMismatch {
                    path: "language_model/model.gguf".to_string(),
                    expected: info.sha256.clone(),
                    actual,
                });
            }
        }

        Ok(())
    }
}

/// Graph file format for knowledge graph (matches checkpoint)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphFile {
    concepts: Vec<omiai_knowledge::graph::Concept>,
    relations: Vec<(String, String, String)>,
}

/// Serializable reservoir data for export/import
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ReservoirExportData {
    size: usize,
    input_dim: usize,
    output_dim: usize,
    spectral_radius: f64,
    leak_rate: f64,
    seed: u64,
}

/// FFI-compatible C interface for cdylib target
#[cfg(feature = "ffi")]
pub mod ffi {
    use super::*;
    use std::os::raw::c_char;
    use std::ffi::{CStr, CString};

    #[no_mangle]
    pub extern "C" fn omiai_load(path: *const c_char) -> *mut OmiaiModel {
        let path = unsafe { CStr::from_ptr(path) }.to_str().unwrap();
        match OmiaiModel::load(Path::new(path)) {
            Ok(model) => Box::into_raw(Box::new(model)),
            Err(_) => std::ptr::null_mut(),
        }
    }

    #[no_mangle]
    pub extern "C" fn omiai_step(model: *mut OmiaiModel, input_json: *const c_char) -> *mut c_char {
        if model.is_null() || input_json.is_null() {
            return std::ptr::null_mut();
        }

        let model = unsafe { &mut *model };
        let input_str = unsafe { CStr::from_ptr(input_json) }.to_str().unwrap();

        let input: InferInput = match serde_json::from_str(input_str) {
            Ok(i) => i,
            Err(_) => return std::ptr::null_mut(),
        };

        let output = match model.step(&input) {
            Ok(o) => o,
            Err(_) => return std::ptr::null_mut(),
        };

        let json = serde_json::to_string(&output).unwrap();
        CString::new(json).unwrap().into_raw()
    }

    #[no_mangle]
    pub extern "C" fn omiai_free_string(s: *mut c_char) {
        if !s.is_null() {
            unsafe { drop(CString::from_raw(s)) };
        }
    }

    #[no_mangle]
    pub extern "C" fn omiai_free_model(model: *mut OmiaiModel) {
        if !model.is_null() {
            unsafe { drop(Box::from_raw(model)) };
        }
    }
}