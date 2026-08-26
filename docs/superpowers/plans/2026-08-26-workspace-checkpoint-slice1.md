# OmiAI Workspace Restructuring + Checkpoint v1 (Slice 1) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Chuyển OmiAI từ single-crate thành Cargo workspace 15 crate, sửa toàn bộ test về xanh, dựng `omiai-checkpoint` với trait `Checkpointable` + round-trip đầu tiên cho CA grid, cùng docs/ADR/CI/README trung thực.

**Architecture:** Virtual manifest root, mọi crate trong `crates/`, version chung qua `[workspace.dependencies]`. Tách theo pillar; `cellular` chuyển từ neuro sang omiai-world; `meta` tách thành crate riêng `omiai-meta` (phát hiện khi lập kế hoạch — spec cần cập nhật 14→15). Checkpoint ghi nguyên tử tmp→fsync→rename, manifest BLAKE3, index.json, cửa sổ trượt.

**Tech Stack:** Rust 2024 edition, serde, ciborium, blake3, rayon, proptest, criterion (đã có sẵn trong dev-deps).

**Spec:** `docs/superpowers/specs/2026-08-26-workspace-checkpoint-design.md`

## Global Constraints

- CPU-only mục tiêu: i7-7700K 4C/8T, 8GB RAM, không GPU dời.
- Root `Cargo.toml` chỉ chứa `[workspace]`, `resolver = "2"`, members `crates/*`.
- Mọi dependency khai báo một lần ở `[workspace.dependencies]`; crate tham chiếu `workspace = true`. Thêm mới lát này: `blake3 = "1.8"`, `ciborium = "0.2"`. KHÔNG thêm hecs/zstd/safetensors/axum ở lát cắt này.
- Edition 2024 giữ nguyên cho mọi crate.
- Profile release đặt ở root workspace, bỏ `panic = "abort"` (test/bench cần unwind).
- Không dùng `todo!()` mới; không khẳng định hiệu năng không có benchmark.
- Commit sau mỗi task; message tiếng Anh, cuối có `Co-Authored-By: Claude <noreply@anthropic.com>`. Nếu git identity chưa được user cấu hình thì các bước commit sẽ fail — ghi nhận và tiếp tục phần còn lại của task, commit bù khi identity sẵn sàng.
- Chiến lược sửa test: API trôi → sửa test; bug implementation thật → sửa code, giữ test làm regression.

## Phát hiện so với spec (cần xác nhận khi bắt đầu)

1. **15 crate chứ không phải 14**: module `meta/` chưa được gán crate nào trong spec, nhưng nó tồn tại (5 file, ~660 dòng, có test) và phụ thuộc core+evolution+knowledge+memory+io → tạo thêm `omiai-meta`. Cập nhật spec bảng 14→15 trước khi thực thi.
2. **io phụ thuộc meta** (`nlp_parser` dùng `DetectedLanguage` từ... kiểm tra lại: io dùng `crate::meta` — nếu đúng thì cạnh phụ thuộc là io→meta; meta lại dùng io::chat → **chu trình meta↔io**. Xử lý: chuyển kiểu dữ liệu trùng lặp hoặc đảo hướng bằng cách đưa phần chung xuống core/io. Quyết định chi tiết tại Task 3 khi thấy code thật.

---

### Task 0: Git baseline commit

**Files:** none (chỉ git)

**Interfaces:**
- Produces: commit gốc trên branch `main` để rollback an toàn; mọi task sau đều commit lên đó.

- [ ] **Step 1: Kiểm tra git identity đã cấu hình**

Run: `git config user.name && git config user.email`
Expected: in ra tên + email. Nếu rỗng/lỗi → DỪNG, báo user "hãy chạy `! git config --global user.name '...'` và `user.email ...`", không tự chế identity.

- [ ] **Step 2: Commit baseline**

```bash
printf 'target/\n*.tmp\n' > .gitignore
git add -A
git commit -m "$(cat <<'EOF'
Baseline: original single-crate state before workspace restructuring

Snapshot as received: 134 passing / 15 failing lib tests, broken
integration/example compiles. Rollback point for the workspace split.

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

Expected: commit created on main.

---

### Task 1: Skeleton workspace — root manifest + 15 crate rỗng compile được

**Files:**
- Modify (thay toàn bộ): `Cargo.toml` (root)
- Create: `crates/<mỗi-crate>/Cargo.toml` × 15
- Create: `crates/<mỗi-crate>/src/lib.rs` × 15 (doc comment định hướng + re-export)

**Interfaces:**
- Consumes: source tree `src/*` hiện có (chưa di chuyển trong task này).
- Produces: `cargo build -p omiai-core` … hoạt động cho từng crate skeleton; tên lib `omiai_core`, `omiai_knowledge`, `omiai_probabilistic`, `omiai_causal`, `omiai_neuro`, `omiai_evolution`, `omiai_io`, `omiai_memory`, `omiai_meta`, `omiai_world`, `omiai_checkpoint`, `omiai_export`, `omiai_runtime`, `omiai_serve`, `omiai_cli`.

- [ ] **Step 1: Ghi root `Cargo.toml` mới (virtual manifest)**

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.dependencies]
# serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
ciborium = "0.2"
# errors & logging
thiserror = "1.0"
anyhow = "1.0"
log = "0.4"
tracing = "0.1"
# math & data
nalgebra = "0.32"
ndarray = { version = "0.15", features = ["rayon", "serde"] }
num = "0.4"
ordered-float = "4.0"
rand = "0.8"
rand_distr = "0.4"
rand_chacha = "0.3"
rayon = "1.8"
crossbeam = "0.8"
# graphs & collections
petgraph = "0.6"
indexmap = { version = "2.0", features = ["serde"] }
lru = "0.12"
generational-arena = "0.2"
typed-arena = "2.0"
bumpalo = { version = "3.14", features = ["collections"] }
# hashing & time & misc
blake3 = "1.8"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
nom = "7.1"
regex = "1.10"

[workspace.package]
edition = "2024"
rust-version = "1.97"
license = "MIT OR Apache-2.0"

[profile.release]
lto = "fat"
codegen-units = 1
opt-level = 3

[profile.bench]
debug = true
```

Lưu ý: tokio/miette/crossbeam-epoch/clap/tracing-subscriber **không** đưa vào bảng chung ở lát này — crate nào thật sự cần (serve/cli ở lát sau) sẽ thêm lúc đó. Giữ danh sách tối thiểu để build nhanh.

- [ ] **Step 2: Tạo 15 crate skeleton**

Mỗi `crates/omiai-X/Cargo.toml` theo mẫu (ví dụ omiai-core):

```toml
[package]
name = "omiai-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Symbolic core: logic engine, unification, DPLL/CDCL, CSP, prover."

[dependencies]
serde.workspace = true
```

(`[dependencies]` từng crate liệt kê đúng những gì module của nó dùng — bảng ánh xạ ở Task 2; skeleton chỉ cần đủ compile `lib.rs` rỗng.)

`crates/omiai-core/src/lib.rs`:

```rust
//! Symbolic reasoning core: first-order logic AST + CNF pipeline,
//! substitution, Robinson unification, Resolution/DPLL/CDCL, CSP solver,
//! prover, ASP, LTL, modal logic.
//!
//! Build order note (README gốc): đây là nền — mọi pillar khác phụ thuộc
//! vào đây, nhưng core không phụ thuộc pillar nào.

pub mod logic_engine;
pub mod substitution;
pub mod unification;
// ... các mod còn lại được bật dần trong Task 2
```

Các crate khác (`lib.rs` tương tự): doc comment nêu trách nhiệm + `pub mod` placeholder comment out. `omiai-world/lib.rs` ghi rõ substrate sẽ host cellular_automata (ADR-0002). `omiai-runtime` ghi constraint: KHÔNG depend evolution/training crates. `omiai-export`, `omiai-serve`, `omiai-cli` skeleton rỗng (cli có `[[bin]]` placeholder comment).

- [ ] **Step 3: Di chuyển source vào crate (giữ `src/` cũ làm nguồn tham chiếu cho đến khi Task 1 xanh)**

Thực tế đơn giản hơn: làm luôn việc di chuyển file trong Task này thay vì hai lần (spec cho phép gộp). Với mỗi nhóm module:

```bash
mkdir -p crates/omiai-core/src/core crates/omiai-core/src/utils
cp src/core/*.rs crates/omiai-core/src/
cp src/utils/*.rs crates/omiai-core/src/
mkdir -p crates/omiai-knowledge/src && cp src/knowledge/*.rs crates/omiai-knowledge/src/
mkdir -p crates/omiai-probabilistic/src && cp src/probabilistic/*.rs crates/omiai-probabilistic/src/
mkdir -p crates/omiai-causal/src/utils && cp src/causal/*.rs crates/omiai-causal/src/
cp src/utils/stats.rs crates/omiai-causal/src/utils_stats.rs   # causal dùng crate::utils::stats
mkdir -p crates/omiai-neuro/src && cp src/neuro/{reservoir,liquid_state,weights}.rs crates/omiai-neuro/src/
mkdir -p crates/omiai-evolution/src && cp src/evolution/*.rs crates/omiai-evolution/src/
mkdir -p crates/omiai-io/src && cp src/io/*.rs crates/omiai-io/src/
mkdir -p crates/omiai-memory/src && cp src/memory/*.rs crates/omiai-memory/src/
mkdir -p crates/omiai-meta/src && cp src/meta/*.rs crates/omiai-meta/src/
mkdir -p crates/omiai-world/src && cp src/neuro/cellular.rs crates/omiai-world/src/substrate.rs
cp src/persistence.rs crates/omiai-checkpoint/src/legacy.rs
```

Sau đó sửa `use crate::...` trong từng crate:
- Trong `omiai-knowledge`: `crate::core::X` → vẫn hợp lệ vì lib.rs khai báo `pub mod core;` chứa các module cũ? **Không** — quyết định: bỏ lớp `core::` thừa, `lib.rs` của omiai-core khai thẳng `pub mod logic_engine;` v.v., và các crate ngoài gọi `omiai_core::logic_engine::Formula`. Trong nội bộ omiai-core, `super::` giữ nguyên; `crate::core::X` đổi thành `crate::X`; `crate::utils::stats` đổi thành `crate::utils` module riêng.
- `omiai-causal`: `crate::utils::stats` → `crate::utils_stats` (hoặc copy utils/stats.rs vào causal như trên).
- Các crate knowledge/probabilistic/…: `crate::<pillar>::<mod>` → `crate::<mod>`; `crate::core::*` → `omiai_core::*` (thêm `use omiai_core as core;` đầu lib.rs để giảm churn — **quyết định: dùng alias `use omiai_core as core;`** trong mỗi crate phụ thuộc, giúp hầu hết `crate::core` đổi thành `core::` một cách máy móc).
- `omiai-neuro`: bỏ `pub mod cellular;`.
- `omiai-world/src/substrate.rs`: giữ nguyên nội dung cellular.rs (không đổi API).
- `omiai-checkpoint/src/legacy.rs`: thêm `#![allow(deprecated)]`… không cần; chỉ đổi `crate::` refs nếu persistence.rs có (đã kiểm tra: không có).
- `omiai-io`: xử lý phụ thuộc `crate::meta` — xem phát hiện #2; phương án mặc định: duplicate nhỏ phần cần thiết hoặc đảo hướng; chốt khi thấy code (bước này ghi lại quyết định vào ADR-0005 nếu phải đảo hướng).
- `omiai-meta`: `crate::evolution::*` → `omiai_evolution::*`, v.v. qua alias.

Mỗi crate `lib.rs` cuối cùng dạng:

```rust
#![allow(dead_code)]
pub mod logic_engine; // ... đúng danh sách module thực tế
```

và các crate phụ thuộc mở đầu bằng:

```rust
use omiai_core as core; // alias giảm churn khi tách
```

hoặc bên trong function-level imports đổi `crate::core` → `omiai_core`.

- [ ] **Step 4: Build từng crate theo thứ tự dependency, sửa import đến khi mỗi crate compile**

```bash
cargo build -p omiai-core && cargo build -p omiai-probabilistic && \
cargo build -p omiai-knowledge && cargo build -p omiai-causal && \
cargo build -p omiai-neuro && cargo build -p omiai-evolution && \
cargo build -p omiai-io && cargo build -p omiai-memory && \
cargo build -p omiai-meta && cargo build -p omiai-world && \
cargo build -p omiai-checkpoint && cargo build --workspace
```

Expected: toàn bộ OK. Lỗi import là bình thường — sửa từng cái.

- [ ] **Step 5: Xoá `src/` cũ + examples/tests/benches cũ khỏi root (sẽ phân bổ lại ở Task 2)**

```bash
rm -rf src benches tests fuzz/examples 2>/dev/null
# fuzz/ giữ nguyên target definitions nhưng sẽ fix ở Task 5; nếu fuzz break build:
mv fuzz fuzz.disabled || true
```

Chú ý: `examples/` giữ thư mục (Task 2 tái lập), xoá file cũ bên trong.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "wip(workspace): move modules into 15-crate layout, all crates compile"
```

---

### Task 2: Phân bổ tests/examples/benches về đúng crate + workspace xanh

**Files:**
- Create: `crates/*/tests/*.rs`, `crates/*/examples/*.rs`, `crates/*/benches/*.rs` (từ bản cũ)
- Modify: nội dung test/example/bench đổi đường dẫn `omiai::` → `omiai_<crate>::`
- Create: root `tests/` cho integration chéo (nếu cần)

**Interfaces:**
- Consumes: 15 crate compile từ Task 1.
- Produces: `cargo test --workspace` chạy được (có thể FAIL — sửa ở Task 3); `cargo bench --workspace` ít nhất biên dịch được các bench pillar đã hoàn thiện.

- [ ] **Step 1: Phân bổ theo bảng**

| File cũ | Đích |
|---|---|
| `tests/integration.rs`, `integration_v2.rs`, `integration_v3.rs`, `properties.rs` | root `tests/` (dev-dependencies trên nhiều crate) — root cần `[package]`? Virtual manifest không cho test ở root → **quyết định: đặt vào crate sở hữu chủ đề chính**, cụ thể integration_v2/v3 + properties → `crates/omiai-core/tests/`, integration.rs (dùng NlpParser/chat) → `crates/omiai-io/tests/` |
| `examples/logic_demo.rs` | `crates/omiai-core/examples/logic_demo.rs` |
| `examples/learning_demo.rs`, `interact.rs` | `crates/omiai-meta/examples/` (learning_demo dùng GP+meta; interact dùng chat+persistence→legacy) |
| `benches/sat.rs`, `cnf.rs` | `crates/omiai-core/benches/` |
| `benches/knowledge.rs` | `crates/omiai-knowledge/benches/` |
| `benches/bayesian.rs` | `crates/omiai-probabilistic/benches/` |
| `benches/reservoir.rs` | `crates/omiai-neuro/benches/` |
| `benches/cellular.rs` | `crates/omiai-world/benches/` |
| `benches/cgp.rs` | `crates/omiai-evolution/benches/` |

- [ ] **Step 2: Đổi `omiai::` → crate path đúng trong mọi test/example/bench**

Sed máy móc + sửa tay chỗ phức tạp. Ví dụ `use omiai::core::logic_engine::Formula;` → `use omiai_core::logic_engine::Forma;` (cẩn thận: sed phải đúng — kiểm tra bằng build).

- [ ] **Step 3: Thêm `[[test]]`/`[[example]]`/`[[bench]]` entries vào Cargo.toml của crate tương ứng**

Criterion bench mẫu (`crates/omiai-core/Cargo.toml`):

```toml
[dev-dependencies]
criterion.workspace = true
proptest.workspace = true

[[bench]]
name = "cnf"
harness = false
```

(`criterion`, `proptest` thêm vào `[workspace.dependencies]` dưới dev-deps section.)

- [ ] **Step 4: `cargo test --workspace --no-run` compile sạch**

Run: `cargo test --workspace --no-run`
Expected: mọi target compile (fail nào sửa import tới khi sạch).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test(workspace): distribute tests/examples/benches to owning crates"
```

---

### Task 3: Sửa 15 test fail + lỗi compile — workspace xanh hoàn toàn

**Files:**
- Modify: các file impl/test liên quan trong `crates/` (xem bảng)

**Interfaces:**
- Consumes: `cargo test --workspace` chạy được từ Task 2.
- Produces: `cargo test --workspace` 100% pass — điều kiện tiên quyết cho mọi task sau.

- [ ] **Step 1: Phân loại từng fail (API-trôi vs bug-thật), sửa theo thứ tự dependency**

Bảng fail đã chẩn đoán trước (từ phiên brainstorm):

| Test | Triệu chứng | Kế hoạch xử lý |
|---|---|---|
| `probabilistic::hmc::std_dev_positive`, `samples_isotropic_normal_recovers_mean` | mean/std lệch mạnh | Bug sampler thật khả năng cao — đọc leapfrog step-size/negate-gradient; sửa impl, test giữ nguyên làm regression |
| `junction_tree` (3 test) | JT diverged brute force; potential sum ≠ 1 | Bug chuẩn hoá potential — đọc multiply/marginalize/calibrate |
| `gibbs::empty_bn_returns_empty_marginals` | samples không rỗng | Guard early-return khi BN rỗng |
| `mean_field::mf_increases_rain_with_wet_evidence` | P=0.2 thay vì >0.5 | Có thể sign lỗi update equation |
| `puct_mcts::puct_with_prior_picks_highest_prior` | picked 3 thay vì 1 | Công thức PUCT sai hạng (exploration term dấu/normalize) |
| `causal::icp` (2 test) | beta lệch; parent không recovered | Regression OLS/ICP invariant set selection |
| `ltl::f_p_implies_eventually_p` | satisfiable=false kỳ vọng ngược | Đọc semantics F operator |
| `knowledge::abduction` (2 test) | không tìm explanation | Ranking/subsumption trong explanation search |
| `discocat::pregroup_transitive_verb_reduces_via_subj_obj` | reduce không ra s | Grammar reduction rule |
| `neuro::weights::spectral_normalize` | rho=0.72 ≠ 1.0 | Power iteration chưa hội tụ/sai số iterations |

- [ ] **Step 2: Với mỗi bug-thật: viết/kiểm tra test phản ánh hành vi đúng, sửa impl, chạy lại**

Quy trình mỗi bug: đọc code + test → xác định đúng sai ở đâu (không sửa test cho khớp code sai) → sửa → `cargo test -p <crate> --lib` → xanh.

- [ ] **Step 3: `cargo test --workspace` xanh 100%**

Run: `cargo test --workspace`
Expected: tất cả pass. Ghi số test pass vào commit message.

- [ ] **Formalize: chạy `cargo clippy --workspace --all-targets` chỉ ghi nhận, không bắt buộc zero-warning**

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix: repair 15 failing tests across probabilistic/causal/knowledge/neuro/ltl"
```

---

### Task 4: omiai-checkpoint — trait + helpers + ca_grid round-trip (TDD)

**Files:**
- Create: `crates/omiai-checkpoint/src/lib.rs`, `error.rs`, `fsutil.rs`, `manifest.rs`, `index.rs`, `traits.rs`
- Create: `crates/omiai-checkpoint/src/ca_grid.rs` (impl Checkpointable cho CellularAutomaton)
- Test: `crates/omiai-checkpoint/tests/roundtrip_ca_grid.rs`, `crates/omiai-checkpoint/tests/atomic_write.rs`

**Interfaces:**
- Consumes: `omiai_world::substrate::CellularAutomaton` (fields public: width/height/num_states/cells — đã xác nhận).
- Produces (later slices rely on these exact signatures):
  ```rust
  pub trait Checkpointable: Sized {
      type Error;
      fn save(&self, dir: &Path) -> Result<(), Self::Error>;
      fn load(dir: &Path) -> Result<Self, Self::Error>;
  }
  pub fn hash_file(path: &Path) -> Result<String, CheckpointError>;       // BLAKE3 hex
  pub fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), CheckpointError>; // fsync + rename
  pub struct Manifest { pub format_version: u32, pub git_commit: Option<String>, pub step: u64,
                        pub timestamp_utc: String, pub rng_seed: u64, pub rng_state_hex: String,
                        pub files: Vec<FileRecord> }   // FileRecord { path, blake3 }
  pub fn verify_dir(dir: &Path) -> Result<(), CheckpointError>;
  ```

- [ ] **Step 1: deps cho omiai-checkpoint Cargo.toml**

```toml
[dependencies]
omiai-world = { path = "../omiai-world" }
serde.workspace = true
serde_json.workspace = true
ciborium = { workspace = true }
blake3.workspace = true
thiserror.workspace = true

[dev-dependencies]
proptest.workspace = true
tempfile = "3"
```

(`tempfile` thêm vào workspace.dependencies.)

- [ ] **Step 2: Write failing test — round-trip CA grid**

`crates/omiai-checkpoint/tests/roundtrip_ca_grid.rs`:

```rust
use omiai_checkpoint::{Checkpointable};
use omiai_world::substrate::CellularAutomaton;

#[test]
fn ca_grid_roundtrip_is_identical() {
    let dir = tempfile::tempdir().unwrap();
    let mut ca = CellularAutomaton::random(17, 9, 0.4, 12345);
    ca.steps(3);
    let snap = ca.clone();

    ca.save(dir.path()).unwrap();
    let back = CellularAutomaton::load(dir.path()).unwrap();
    assert_eq!(back.width, snap.width);
    assert_eq!(back.height, snap.height);
    verify_dir(dir.path()).unwrap();
    assert_eq!(back.cells, snap.cells);
    assert_eq!(back.num_states, snap.num_states);
}
```

(verify_dir import từ omiai_checkpoint; test cũng xác nhận manifest + hash được ghi.)

- [ ] **Step 3: Chạy test → FAIL (trait chưa tồn tại)**

Run: `cargo test -p omiai-checkpoint --test roundtrip_ca_grid`
Expected: compile error "Checkpointable not found".

- [ ] **Step 4: Implement trait + helpers**

`traits.rs`:

```rust
use std::path::Path;

pub trait Checkpointable: Sized {
    type Error;
    fn save(&self, dir: &Path) -> Result<(), Self::Error>;
    fn load(dir: &path::Path) -> Result<Self, Self::Error>;
}
```

`error.rs`:

```rust
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("I/O error at {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("corrupt checkpoint at {path}: expected blake3 {expected}, got {actual}")]
    Corrupt { path: PathBuf, expected: String, actual: String },
    #[error("bad header magic in {path}")]
    BadMagic { path: PathBuf },
    #[error("missing manifest field `{0}`")]
    MissingField(String),
    #[error("cbor: {0}")]
    Cbor(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
```

`fsutil.rs` — `hash_file` (BLAKE3 stream), `write_atomic(dir, name, bytes)`:
ghi `.<name>.tmp` → sync_all → rename → fsync dir (open dir + sync_all, unix).

`ca_grid.rs` — format nhị phân (ADR/spec phần 3):
- Header 16 byte: `OMICAGRID\0` (10 byte magic) + u16 LE width + u16 LE height + u8 num_states + u8 flags(=0) + u32 LE reserved(=0)
  (width/height u16 đủ cho lưới CA thực tế ≤65535; nếu vượt → error `GridTooLarge`)
- Body: bit-packed row-major LSB-first, ceil(w*h/8) byte.

save: serialize → `write_atomic(dir, "grid.bin", &bytes)`; load: read, check magic, check body length == ceil(w*h/8), reconstruct. Impl `Checkpointable for CellularAutomaton` trong omiai-checkpoint (không đụng omiai-world) — pattern orphan-rule-friendly: impl nằm ở checkpoint crate, thế nên `CellularAutomaton` clone-able đã có.

Lưu ý phase/block_cache là private trong omiai-world — checkpoint chỉ persist cells/width/height/num_states; phase khởi tạo về 0 khi load (documented behavior, ghi vào format-spec: "phase không thuộc persistent state").

- [ ] **Step 5: Chạy test roundtrip → PASS**

Run: `cargo test -p omiai-checkpoint --test roundtrip_ca_grid`
Expected: PASS.

- [ ] **Step 6: Write failing test — atomic write + corrupt detection**

`crates/omiai-checkpoint/tests/atomic_write.rs`:

```rust
#[test]
fn atomic_write_leaves_no_tmp_on_success() {
    let dir = tempfile::tempdir().unwrap();
    write_atomic(dir.path(), "f.bin", b"data").unwrap();
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(entries, vec!["f.bin".to_string()]);
}

#[test]
fn verify_detects_tampered_file() {
    let dir = tempfile::tempdir().unwrap();
    write_atomic(dir.path(), "f.bin", b"data").unwrap();
    // tamper
    std::fs::write(dir.path().join("f.bin"), b"datX").unwrap();
    let err = verify_dir(dir.path()).unwrap_err();
    assert!(matches!(err, CheckpointError::Corrupt { .. }));
}
```

- [ ] **Step 7: Implement verify_dir (đọc manifest.json, hash lại từng file, so khớp)**

- [ ] **Step 8: Proptest bất biến bảo toàn năng lượng qua save/load + step**

`crates/omiai-checkpoint/tests/proptest_grid.rs`:

```rust
proptest! {
    #[test]
    fn roundtrip_preserves_cells(w in 1usize..64, h in 1usize..64, seed in any::<u64>()) {
        let dir = tempfile::tempdir()?;
        let mut ca = CellularAutomaton::random(w, h, 0.3, seed);
        let snap = ca.clone();
        ca.save(dir.path())?;
        let back = CellularAutomaton::load(dir.path())?;
        prop_assert_eq!(back.cells, snap.cells);
    }

    #[test]
    fn population_preserved_by_step_and_roundtrip(w in 2usize..32, h in 2usize..32, seed in any::<u64>()) {
        let dir = tempfile::tempdir()?;
        let mut ca = CellularAutomaton::random(w, h, 0.3, seed);
        let p0 = ca.population();
        ca.step();
        prop_assert_eq!(ca.population(), p0); // Margolus rotation bảo toàn population
        let snap = ca.clone();
        ca.save(dir.path())?;
        let back = CellularAutomaton::load(dir.path())?;
        prop_assert_eq!(back.population(), p0);
    }
}
```

- [ ] **Step 9: Chạy proptest → PASS**

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(checkpoint): Checkpointable trait + atomic writes + CA grid round-trip (TDD)"
```

---

### Task 5: docs — format-spec checkpoint-v1, ADR-0001..0005, architecture khung, CI workflow

**Files:**
- Create: `docs/format-spec/checkpoint-v1.md`
- Create: `docs/adr/0001-workspace-layout.md` … `0005-io-meta-cycle.md`
- Create: `docs/architecture/README.md` (+ stub per pillar)
- Create: `.github/workflows/ci.yml`
- Modify: `README.md` (root)

**Interfaces:**
- Consumes: code thật từ Task 4 (format ca_grid.bin phải khớp từng byte với doc).
- Produces: docs khớp code; CI green trên push.

- [ ] **Step 1: `docs/format-spec/checkpoint-v1.md`**

Nội dung bắt buộc: bố cục thư mục (như spec §2.2), schema manifest.json đầy đủ từng trường + ví dụ JSON thật, thuật toán ghi nguyên tử 5 bước, cửa sổ trượt + milestone, format `world/ca_grid.bin` từng byte (magic/LE/bit-packing LSB-first/phase-not-persisted), policy tương thích ngược: "reader v2 PHẢI đọc được v1; writer v1 không bao giờ emit trường chưa định nghĩa; thêm trường mới = bump minor trong format_version encoding".

- [ ] **Step 2: ADR-0001..0005** (mỗi file ngắn: Context/Decision/Consequences)

0001 virtual workspace 15 crate + bỏ panic=abort; 0002 cellular→world; 0003 checkpoint directory + BLAKE3 + atomic rename (không file đơn); 0004 gen-as-Formula-pointer định hướng world; 0005 giải quyết chu trình io↔meta (theo quyết định thật ở Task 1 Step 3).

- [ ] **Step 3: `docs/architecture/README.md` + stub per pillar** — mỗi pillar một file ngắn mô tả trạng thái thật (tested/stubbed) + benchmark nào đo nó.

- [ ] **Step 4: CI workflow**

`.github/workflows/ci.yml`:

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --workspace
      - run: cargo test --workspace
      - run: cargo clippy --workspace --all-targets -- -D warnings || true  # advisory ở slice này
```

- [ ] **Step 5: README.md rewrite** theo văn phong dự án ("What's actually implemented (and tested)" / "What's scaffolded" / "Suggested build order") phản ánh: workspace 15 crate, 149 test xanh (số thật từ Task 3), checkpoint trait + ca_grid round-trip done, phần còn lại của omiai-world là khung sườn, thứ tự xây dựng tăng dần giữ nguyên tinh thần README gốc, hardware constraints (CPU-only 8GB).

- [ ] **Step 6: Update spec 14→15 crate + ghi chú phát hiện chu trình io↔meta**

Edit `docs/superpowers/specs/2026-08-26-workspace-checkpoint-design.md`: bảng crate 14→15 thêm omiai-meta; ghi chú phát hiện phụ thuộc chéo.

- [ ] **Step 7: Commit + tag slice**

```bash
git add -A
git commit -m "docs: format-spec v1, ADR-0001..0005, architecture stubs, CI, honest README"
git tag slice-1-complete
```

---

## Verification cuối lát cắt

1. `cargo build --workspace` — OK
2. `cargo test --workspace` — 100% pass (số test ghi trong README khớp output thật)
3. Round-trip + proptest ca_grid pass
4. `docs/format-spec/checkpoint-v1.md` khớp code từng byte (so magic/header bằng tay)
5. README không tuyên bố gì vượt quá test/benchmark thật đang có
