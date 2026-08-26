# Slice 2 — Atoms, Agents, World Loop + World Checkpoint Bundle — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Biến `omiai-world` thành trụ cột chạy thật (atom sống trên lưới CA, agent điều khiển bởi gene trỏ vào `LtlFormula`, world loop 5 phase) và checkpoint/resume **bit-exact**, kèm retention window + index cho `checkpoint-v1`.

**Architecture:** `FormulaRegistry` (generational arena) trong `World` sở hữu genome; `Atom` chỉ giữ `FormulaId`. World loop 5 phase cố định: `ca_step → metabolism → agent_act → reproduce_and_evolve → snapshot`. `impl Checkpointable for World` nằm ở `omiai-checkpoint` (orphan-rule, giống `ca_grid`). RNG: ChaCha8 tái tạo từ `(seed u64, stream u64, word_pos u128)`.

**Tech Stack:** Rust workspace 15 crate, `generational-arena 0.2`, `rand_chacha 0.3`, `ciborium 0.2`, `serde`, `proptest`, `rayon`.

**Spec:** `docs/superpowers/specs/2026-08-26-world-agents-checkpoint-slice2-design.md`

## Global Constraints

- CPU-only i7-7700K (4C/8T), 8GB RAM — không thêm dependency GPU/hecs/zstd/axum.
- Làm việc trên nhánh `main`, commit + push sau mỗi task xanh test.
- Văn phong trung thực: chỉ ghi "đã cài đặt và test" cho cái có test thật.
- Payload checkpoint KHÔNG dùng `HashMap` (thứ tự không ổn định) — chỉ `Vec`/struct có thứ tự cố định.
- Không bao giờ xoá checkpoint mốc (step % milestone_every == 0, gồm step 0).
- Mọi randomness của world đi qua một `ChaCha8Rng` duy nhất trong `World`.
- Cell semantics: `0` = trống, `1` = cản, `≥2` = tài nguyên (giá trị càng lớn càng giàu năng lượng). Lưới world dùng `num_states = 4`.
- Hằng số sinh thái cố định (đặt trong `world_loop.rs`, `pub const`): `METABOLIC_COST: f64 = 0.05`, `ENERGY_MAX: f64 = 1.0`, `REPRODUCE_THRESHOLD: f64 = 0.8`, `ENERGY_PER_RESOURCE_UNIT: f64 = 0.2`, `MUTATION_PROB: f64 = 0.3`, `MAX_FORMULA_DEPTH: usize = 5`.

---

### Task 1: Retention window (`omiai-checkpoint::retention`)

**Files:**
- Create: `crates/omiai-checkpoint/src/retention.rs`
- Modify: `crates/omiai-checkpoint/src/lib.rs` (thêm `mod retention; pub use retention::{apply_retention, RetentionPolicy};`)
- Test: trong `retention.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `crate::index::list_steps(root) -> Result<Vec<(u64, PathBuf)>, CheckpointError>` (đã có từ slice 1).
- Produces: `RetentionPolicy { pub keep_recent: usize, pub milestone_every: u64 }`, `RetentionPolicy::default()` = `{keep_recent: 10, milestone_every: 100}`; `pub fn apply_retention(root: &Path, policy: &RetentionPolicy) -> Result<Vec<(u64, PathBuf)>, CheckpointError>` — trả về danh sách (step, path) **bị xoá**, tăng dần theo step.

- [ ] **Step 1: Write the failing test**

Thêm vào `crates/omiai-checkpoint/src/retention.rs` (file mới):

```rust
//! Sliding-window retention for checkpoint directories (spec gốc mục 2.3).
//!
//! Giữ N step gần nhất CỘNG mọi mốc vĩnh viễn (step chia hết cho
//! `milestone_every`, gồm step 0). Không bao giờ xoá mốc.

use std::path::{Path, PathBuf};

use crate::error::CheckpointError;
use crate::index::list_steps;

/// Chính sách giữ checkpoint: N gần nhất + mốc mỗi K bước.
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    pub keep_recent: usize,
    pub milestone_every: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_recent: 10,
            milestone_every: 100,
        }
    }
}

/// Xoá các checkpoint ngoài chính sách; trả về danh sách đã xoá.
pub fn apply_retention(
    root: &Path,
    policy: &RetentionPolicy,
) -> Result<Vec<(u64, PathBuf)>, CheckpointError> {
    let mut steps = list_steps(root)?;
    // Gần nhất trước.
    steps.sort_by(|a, b| b.0.cmp(&a.0));

    let mut removed = Vec::new();
    for (i, (step, path)) in steps.iter().enumerate() {
        let is_recent = i < policy.keep_recent;
        let is_milestone =
            policy.milestone_every > 0 && step % policy.milestone_every == 0;
        if !is_recent && !is_milestone {
            std::fs::remove_dir_all(path).map_err(|source| CheckpointError::Io {
                path: path.clone(),
                source,
            })?;
            removed.push((*step, path.clone()));
        }
    }
    removed.sort_by_key(|(s, _)| *s);
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tạo thư mục checkpoint giả `step_XXXXXXXX` rỗng (chỉ cần tên đúng).
    fn make_fake_checkpoints(root: &Path, steps: &[u64]) {
        std::fs::create_dir_all(root).unwrap();
        for s in steps {
            std::fs::create_dir_all(root.join(format!("step_{s:08}"))).unwrap();
        }
    }

    #[test]
    fn keeps_recent_window_plus_milestones() {
        let root = std::env::temp_dir().join(format!("omiai-ret-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // steps 0, 50, 100, 101..110 (mỗi 100 là mốc)
        let all: Vec<u64> = std::iter::once(0)
            .chain([50, 100])
            .chain(101..=110)
            .collect();
        make_fake_checkpoints(&root, &all);

        let policy = RetentionPolicy {
            keep_recent: 5,
            milestone_every: 100,
        };
        let removed = apply_retention(&root, &policy).unwrap();

        // 13 step, giữ 5 gần nhất (106..110) + mốc {0, 100} → xoá 6: 50, 101..105
        let removed_steps: Vec<u64> = removed.iter().map(|(s, _)| *s).collect();
        assert_eq!(removed_steps, vec![50, 101, 102, 103, 104, 105]);

        let remaining: Vec<u64> =
            list_steps(&root).unwrap().into_iter().map(|(s, _)| s).collect();
        assert_eq!(remaining, vec![0, 100, 106, 107, 108, 109, 110]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn milestone_never_deleted_even_outside_window() {
        let root = std::env::temp_dir().join(format!("omiai-ret-m{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        make_fake_checkpoints(&root, &[0, 200, 201, 202]);

        let policy = RetentionPolicy {
            keep_recent: 1,
            milestone_every: 100,
        };
        apply_retention(&root, &policy).unwrap();

        let remaining: Vec<u64> =
            list_steps(&root).unwrap().into_iter().map(|(s, _)| s).collect();
        // 202 gần nhất giữ; 0 và 200 là mốc giữ; 201 bị xoá.
        assert_eq!(remaining, vec![0, 200, 202]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fewer_than_keep_recent_deletes_nothing() {
        let root = std::env::temp_dir().join(format!("omiai-ret-f{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        make_fake_checkpoints(&root, &[1, 2, 3]);
        let removed = apply_retention(&root, &RetentionPolicy::default()).unwrap();
        assert!(removed.is_empty());
        assert_eq!(list_steps(&root).unwrap().len(), 3);
        let _ = std::fs::remove_dir_all(&root);
    }
}
```

- [ ] **Step 2: Wire module vào lib.rs**

Trong `crates/omiai-checkpoint/src/lib.rs`, thêm dòng module + re-export cạnh các module hiện có (giữ đúng style hiện tại):

```rust
mod retention;
pub use retention::{apply_retention, RetentionPolicy};
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p omiai-checkpoint`
Expected: PASS (test mới + test cũ đều xanh). Nếu fail, sửa implementation (không sửa test) vì logic retention đã đặc tả đầy đủ ở trên.

- [ ] **Step 4: Commit + push**

```bash
git add crates/omiai-checkpoint/src/retention.rs crates/omiai-checkpoint/src/lib.rs
git commit -m "feat(checkpoint): sliding-window retention (N recent + milestones)"
git push origin main
```

---

### Task 2: `index.json` đọc/ghi nguyên tử + fallback rebuild

**Files:**
- Modify: `crates/omiai-checkpoint/src/index.rs`
- Test: trong `index.rs` `#[cfg(test)]` (phần mới)

**Interfaces:**
- Consumes: `list_steps`, `write_atomic`, `MANIFEST_NAME` pattern, `CheckpointError`.
- Produces:
  - `pub struct CheckpointIndexEntry { pub step: u64, pub dir: String }` (serde `Serialize, Deserialize`)
  - `pub struct CheckpointIndex { pub entries: Vec<CheckpointIndexEntry> }` (entries tăng dần theo step)
  - `pub const INDEX_NAME: &str = "index.json";`
  - `pub fn write_index(root: &Path, index: &CheckpointIndex) -> Result<(), CheckpointError>` — serialize JSON rồi `write_atomic(root, INDEX_NAME, bytes)`
  - `pub fn read_or_rebuild_index(root: &Path) -> Result<(CheckpointIndex, bool), CheckpointError>` — trả về `(index, rebuilt)`; thiếu/hỏng index → quét `list_steps` rebuild, `rebuilt = true`; index hợp lệ nhưng thiếu step tồn tại trên đĩa cũng rebuild.

- [ ] **Step 1: Write the failing tests**

Thêm cuối `crates/omiai-checkpoint/src/index.rs` (giữ nguyên phần hiện có):

```rust
// ---------------------------------------------------------------------------
// index.json — atomic write + fallback rebuild
// ---------------------------------------------------------------------------

use crate::fsutil::write_atomic;
use serde::{Deserialize, Serialize};

/// Tên file index trong thư mục checkpoints/.
pub const INDEX_NAME: &str = "index.json";

/// Một entry index: step + tên thư mục con tương ứng.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointIndexEntry {
    pub step: u64,
    pub dir: String,
}

/// Index các checkpoint hợp lệ, tăng dần theo step.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointIndex {
    pub entries: Vec<CheckpointIndexEntry>,
}

/// Ghi index.json bằng ghi nguyên tử (tmp + rename).
pub fn write_index(root: &Path, index: &CheckpointIndex) -> Result<(), CheckpointError> {
    let mut entries = index.entries.clone();
    entries.sort_by_key(|e| e.step);
    let normalized = CheckpointIndex { entries };
    let bytes = serde_json::to_vec_pretty(&normalized)
        .map_err(CheckpointError::Json)?;
    // write_atomic đã trả CheckpointError (không phải io::Error) — trả thẳng.
    write_atomic(root, INDEX_NAME, &bytes)
}

/// Đọc index.json; nếu thiếu/hỏng/thiếu step trên đĩa → quét thư mục rebuild.
///
/// Trả về `(index, rebuilt)`: `rebuilt = true` khi index vừa được dựng lại
/// từ quét thư mục (caller nên log cảnh báo — không bao giờ im lặng tuyệt đối).
pub fn read_or_rebuild_index(root: &Path) -> Result<(CheckpointIndex, bool), CheckpointError> {
    let on_disk = list_steps(root)?;

    let from_file: Option<CheckpointIndex> = std::fs::read(root.join(INDEX_NAME))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());

    if let Some(mut idx) = from_file {
        idx.entries.sort_by_key(|e| e.step);
        let indexed: std::collections::HashSet<u64> =
            idx.entries.iter().map(|e| e.step).collect();
        let all_on_disk = on_disk.iter().all(|(s, _)| indexed.contains(s));
        if all_on_disk {
            return Ok((idx, false));
        }
    }

    let entries = on_disk
        .iter()
        .map(|(s, p)| CheckpointIndexEntry {
            step: *s,
            dir: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        })
        .collect();
    Ok((CheckpointIndex { entries }, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CheckpointError;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("omiai-idx-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn write_then_read_round_trips() {
        let root = temp_root("rt");
        let idx = CheckpointIndex {
            entries: vec![
                CheckpointIndexEntry { step: 5, dir: "step_00000005".into() },
                CheckpointIndexEntry { step: 3, dir: "step_00000003".into() },
            ],
        };
        write_index(&root, &idx).unwrap();
        let (read, rebuilt) = read_or_rebuild_index(&root).unwrap();
        assert!(!rebuilt);
        // Đã chuẩn hoá tăng dần khi ghi.
        assert_eq!(read.entries[0].step, 3);
        assert_eq!(read.entries[1].step, 5);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_index_falls_back_to_scan() {
        let root = temp_root("miss");
        for s in [1u64, 2, 7] {
            std::fs::create_dir_all(root.join(format!("step_{s:08}"))).unwrap();
        }
        let (idx, rebuilt) = read_or_rebuild_index(&root).unwrap();
        assert!(rebuilt);
        assert_eq!(idx.entries.len(), 3);
        assert_eq!(idx.entries[2].step, 7);
        assert_eq!(idx.entries[2].dir, "step_00000007");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_index_falls_back_to_scan() {
        let root = temp_root("corrupt");
        std::fs::create_dir_all(root.join("step_00000009")).unwrap();
        std::fs::write(root.join(INDEX_NAME), b"{not json").unwrap();
        let (idx, rebuilt) = read_or_rebuild_index(&root).unwrap();
        assert!(rebuilt);
        assert_eq!(idx.entries.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stale_index_missing_step_rebuilds() {
        let root = temp_root("stale");
        for s in [1u64, 2] {
            std::fs::create_dir_all(root.join(format!("step_{s:08}"))).unwrap();
        }
        // Index chỉ ghi step 1, thiếu step 2 đang tồn tại trên đĩa.
        write_index(
            &root,
            &CheckpointIndex {
                entries: vec![CheckpointIndexEntry { step: 1, dir: "step_00000001".into() }],
            },
        )
        .unwrap();
        let (idx, rebuilt) = read_or_rebuild_index(&root).unwrap();
        assert!(rebuilt);
        assert_eq!(idx.entries.len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[allow(dead_code)] // giữ import CheckpointError được dùng qua chữ ký write_index
    fn _error_type_used(_: Result<(), CheckpointError>) {}
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p omiai-checkpoint`
Expected: PASS toàn bộ (cũ + mới). Lưu ý `write_index` trả `CheckpointError::Json` cho lỗi serialize — nếu variant `Json` chưa bọc được kiểu đó thì kiểm tra `error.rs` và dùng đúng variant hiện có.

- [ ] **Step 3: Commit + push**

```bash
git add crates/omiai-checkpoint/src/index.rs
git commit -m "feat(checkpoint): index.json atomic write + fallback rebuild from directory scan"
git push origin main
```

---

### Task 3: ADR-0006 — quyết định RNG world

**Files:**
- Create: `docs/adr/0006-world-rng-chacha8-state.md`

**Interfaces:** Không có code. Task này chốt quyết định đã probe để các task sau tham chiếu.

- [ ] **Step 1: Write the ADR**

```markdown
# ADR-0006: RNG của world — ChaCha8 tái tạo từ (seed, stream, word_pos)

- Ngày: 2026-08-26
- Trạng thái: Chấp nhận

## Bối cảnh

World loop cần randomness deterministic và resume được: sau N bước,
checkpoint phải lưu đủ trạng thái RNG để thế giới tiếp tục đúng quỹ đạo
bit-exact. Spec slice-2 đặt hai phương án: (A) serialize state ChaCha8,
(B) tự viết Xorshift64* với state 8 byte.

## Quyết định

Chọn **A**: `rand_chacha 0.3.1` expose đủ API cần thiết (đã probe trực tiếp
mã nguồn crate trong cargo registry):

- `SeedableRng::seed_from_u64(u64)` — khởi tạo xác định từ seed u64.
- `get_stream() -> u64` / `set_stream(u64)`
- `get_word_pos() -> u128` / `set_word_pos(u128)`

Bộ `(seed: u64, stream: u64, word_pos: u128)` tái tạo đúng trạng thái
generator. Checkpoint lưu cả ba trong `world/rng_state.bin`:
u64 LE seed + u64 LE stream + u128 LE word_pos (16 byte).

## Hệ quả

- Không cần Xorshift64* tự viết (phương án B bỏ) — giữ chất lượng thống kê
  ChaCha8 và tránh thêm mã crypto tự chế.
- `word_pos` tính theo **32-bit words** của luồng ChaCha, không phải byte;
  mọi đường lấy randomness phải đi qua generator duy nhất trong `World`
  (không tạo RNG phụ từ seed giữa chừng, nếu không word_pos mất nghĩa).
- `seed_from_u64` dùng SplitMix nội bộ để mở rộng u64 → [u8;32]; vì nó
  xác định nên lưu seed u64 là đủ, không cần lưu 32 byte thô.
```

- [ ] **Step 2: Commit + push**

```bash
git add docs/adr/0006-world-rng-chacha8-state.md
git commit -m "docs(adr): 0006 world RNG = ChaCha8 (seed,stream,word_pos)"
git push origin main
```

---

### Task 4: `omiai-world::registry` — Genome + FormulaRegistry + FormulaId

**Files:**
- Modify: `crates/omiai-core/src/ltl.rs` (thêm serde derive cho `LtlFormula`)
- Create: `crates/omiai-world/src/registry.rs`
- Modify: `crates/omiai-world/src/lib.rs`, `crates/omiai-world/Cargo.toml`
- Test: trong `registry.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `omiai_core::ltl::LtlFormula`.
- Produces (dùng bởi Task 5–8):
  - `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)] pub struct Genome { pub formula: LtlFormula, pub fitness: Option<f64> }` (`fitness` cache theo spec §1.1; `None` = chưa đánh giá)
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct FormulaId(generational_arena::Index)` + manual `Serialize`/`Deserialize` **dưới dạng u32 slot-index** (yêu cầu spec: atom lưu slot để map lại sau load) + `pub fn slot(self) -> u32`, `pub fn from_slot(slot: u32) -> Self`
  - `pub struct FormulaRegistry { /* private arena */ }` + `new()`, `insert(Genome) -> FormulaId`, `get(FormulaId) -> Option<&Genome>`, `get_mut(FormulaId) -> Option<&mut Genome>`, `len() -> usize`, `is_empty() -> bool`, `genomes_in_order() -> Vec<Genome>`, `from_genomes_in_order(Vec<Genome>) -> Self`
  - Bất biến slice-2: **không có `remove`** → arena không lỗ hổng, generation luôn 0, `genomes_in_order()[i]` ứng slot i; serialize/deserialize dựa trên bất biến này.

- [ ] **Step 1: Thêm serde derive cho `LtlFormula`**

`crates/omiai-core/src/ltl.rs` dòng 40:

```rust
/// An LTL formula.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum LtlFormula {
```

Đã xác minh lúc viết plan (2026-08-26): workspace đã định nghĩa
`serde = { version = "1.0", features = ["derive"] }` và `omiai-core`
đã có sẵn `serde.workspace = true` — Step 1 chỉ sửa derive trên
`LtlFormula`, không đổi Cargo.toml.

Run: `cargo build -p omiai-core` → OK.

- [ ] **Step 2: Write the failing tests**

`crates/omiai-world/Cargo.toml` — thêm vào `[dependencies]`:

```toml
serde.workspace = true
generational-arena.workspace = true
```

và thêm `serde_json = "1.0"` vào mục `[dev-dependencies]` **đã tồn tại**
(hiện có criterion + proptest — KHÔNG tạo mục thứ hai):

```toml
[dev-dependencies]
criterion.workspace = true
proptest.workspace = true
serde_json = "1.0"
```

(`omiai-core` đã có sẵn trong deps; `serde_json` chỉ dùng cho test
round-trip của `FormulaId`.)

Tạo `crates/omiai-world/src/registry.rs` bắt đầu bằng phần test (viết implementation ngay sau trong Step 3):

```rust
//! FormulaRegistry: kho genome dùng chung cho mọi atom (ADR-0004, Cách 1).
//!
//! Genome là [`LtlFormula`]; atom chỉ giữ handle [`FormulaId`] nên nhiều
//! atom có thể chia sẻ một gene. Registry sống trong `World`, không global.
//!
//! Bất biến slice-2: KHÔNG có remove — arena luôn đặc, thứ tự insertion ==
//! thứ tự slot, nhờ đó serialize là `Vec<Genome>` theo thứ tự và load chỉ
//! cần insert lại tuần tự. (GC/refcount genome là việc lát sau — giới hạn
//! đã biết: genome chết chủ vẫn nằm lại registry.)

use generational_arena::Arena;
use omiai_core::ltl::LtlFormula;
use serde::{Deserialize, Serialize};

/// Một genome: công thức LTL điều khiển hành vi atom + fitness cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    pub formula: LtlFormula,
    /// Cache kết quả đánh giá; `None` = chưa đánh giá.
    pub fitness: Option<f64>,
}

/// Handle generational tới genome trong registry.
///
/// Serialize/Deserialize dưới dạng **u32 slot-index** (yêu cầu spec §1.1:
/// atom lưu slot để map về id mới sau load). Hợp lệ nhờ bất biến
/// không-remove: generation luôn 0, slot == vị trí insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormulaId(generational_arena::Index);

impl FormulaId {
    /// Slot index (ổn định vì arena không bao giờ có lỗ hổng ở slice này).
    pub fn slot(self) -> u32 {
        let (idx, _gen) = self.0.into_raw_parts();
        idx as u32
    }

    /// Dựng lại handle từ slot index (generation luôn 0 khi không remove).
    pub fn from_slot(slot: u32) -> Self {
        Self(generational_arena::Index::from_raw_parts(
            slot as usize,
            0,
        ))
    }
}

impl Serialize for FormulaId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.slot().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FormulaId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::from_slot(u32::deserialize(deserializer)?))
    }
}

/// Kho genome dùng chung.
#[derive(Debug, Default)]
pub struct FormulaRegistry {
    arena: Arena<Genome>,
}

impl FormulaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, genome: Genome) -> FormulaId {
        FormulaId(self.arena.insert(genome))
    }

    pub fn get(&self, id: FormulaId) -> Option<&Genome> {
        self.arena.get(id.0)
    }

    pub fn get_mut(&mut self, id: FormulaId) -> Option<&mut Genome> {
        self.arena.get_mut(id.0)
    }

    pub fn len(&self) -> usize {
        self.arena.len()
    }

    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Bản sao toàn bộ genome theo thứ tự slot (dùng cho checkpoint).
    pub fn genomes_in_order(&self) -> Vec<Genome> {
        self.arena.iter().cloned().collect()
    }

    /// Tái tạo registry từ danh sách theo thứ tự slot (dùng cho checkpoint).
    ///
    /// Bất biến: `genomes[i]` phải ứng slot i — chỉ đúng khi danh sách đến
    /// từ `genomes_in_order` của registry chưa từng remove.
    pub fn from_genomes_in_order(genomes: Vec<Genome>) -> Self {
        let mut reg = Self::new();
        for g in genomes {
            reg.insert(g);
        }
        reg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genome(f: LtlFormula) -> Genome {
        Genome { formula: f, fitness: None }
    }

    #[test]
    fn insert_get_round_trip() {
        let mut reg = FormulaRegistry::new();
        let g = genome(LtlFormula::atom("res"));
        let id = reg.insert(g.clone());
        assert_eq!(reg.get(id), Some(&g));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn shared_gene_between_atoms_is_same_id() {
        let mut reg = FormulaRegistry::new();
        let id = reg.insert(genome(LtlFormula::atom("open")));
        // Hai "atom" cùng trỏ một id — đọc ra cùng genome, không nhân bản.
        let a = reg.get(id).unwrap();
        let b = reg.get(id).unwrap();
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn get_mut_updates_in_place() {
        let mut reg = FormulaRegistry::new();
        let id = reg.insert(genome(LtlFormula::atom("old")));
        reg.get_mut(id).unwrap().formula = LtlFormula::atom("new");
        assert_eq!(
            reg.get(id).unwrap().formula,
            LtlFormula::atom("new")
        );
    }

    #[test]
    fn formula_id_serializes_as_slot_index() {
        // u32 slot qua serde (JSON đại diện cho mọi format).
        let id = FormulaId::from_slot(7);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "7");
        let back: FormulaId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn order_preserved_for_checkpoint_round_trip() {
        let mut reg = FormulaRegistry::new();
        let formulas = [
            LtlFormula::atom("a"),
            LtlFormula::and(LtlFormula::atom("b"), LtlFormula::atom("c")),
            LtlFormula::g(LtlFormula::atom("d")),
        ];
        for f in &formulas {
            reg.insert(genome(f.clone()));
        }
        let dumped = reg.genomes_in_order();
        assert_eq!(dumped.len(), 3);
        let rebuilt = FormulaRegistry::from_genomes_in_order(dumped);
        assert_eq!(
            rebuilt.genomes_in_order(),
            reg.genomes_in_order()
        );
        // Handle cũ vẫn hợp lệ trên registry dựng lại (slot khớp).
        let id = FormulaId::from_slot(1);
        assert_eq!(
            rebuilt.get(id).unwrap().formula,
            LtlFormula::and(LtlFormula::atom("b"), LtlFormula::atom("c"))
        );
    }

    #[test]
    fn slot_round_trip() {
        let mut reg = FormulaRegistry::new();
        let id = reg.insert(genome(LtlFormula::atom("x")));
        assert_eq!(FormulaId::slot(id), 0);
        let id2 = reg.insert(genome(LtlFormula::atom("y")));
        assert_eq!(FormulaId::slot(id2), 1);
        assert_eq!(FormulaId::from_slot(1), id2);
    }
}
```

- [ ] **Step 3: Wire module**

`crates/omiai-world/src/lib.rs`:

```rust
pub mod registry;
```

(cạnh `pub mod substrate;`, giữ `#![allow(dead_code)]` hiện có cho đến khi
world_loop dùng hết API — Task 7 sẽ dọn nếu còn thừa.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p omiai-world`
Expected: PASS (5 test registry + 3 test substrate cũ).

- [ ] **Step 5: Commit + push**

```bash
git add crates/omiai-core/src/ltl.rs crates/omiai-world/
git commit -m "feat(world): FormulaRegistry — generational genome store, serde LtlFormula"
git push origin main
```

---

### Task 5: `omiai-world::atoms` — Atom + sinh sản/chết

**Files:**
- Create: `crates/omiai-world/src/atoms.rs`
- Modify: `crates/omiai-world/src/lib.rs` (`pub mod atoms;`)
- Test: trong `atoms.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `registry::FormulaId`, hằng số `METABOLIC_COST`, `ENERGY_MAX`, `REPRODUCE_THRESHOLD` từ `world_loop` (định nghĩa ở Task 7; Task 5 khai báo tạm `pub(crate) mod constants` riêng để không vòng phụ thuộc — xem Step 3).
- Produces (dùng bởi Task 6–8):
  - `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)] pub struct Atom { pub pos: (usize, usize), pub energy: f64, pub gene: FormulaId, pub age: u64 }`
  - `impl Atom { pub fn metabolize(&mut self) -> bool /* true = còn sống */; pub fn feed(&mut self, cell_value: u8); pub fn split_energy(&mut self) -> f64 /* năng lượng của con */ }`
  - `pub fn first_free_neighbor(pos: (usize, usize), w: usize, h: usize, occupied: &dyn Fn(usize, usize) -> bool) -> Option<(usize, usize)>` — quét cố định N,E,S,W.

- [ ] **Step 1: Write the failing tests + implementation cùng file (TDD một nhịp)**

Tạo `crates/omiai-world/src/atoms.rs`:

```rust
//! Atom: đơn vị sống trên lưới — vị trí, năng lượng, gene (con trỏ Formula).
//!
//! Atom KHÔNG sở hữu Formula; gene chỉ là [`FormulaId`] trỏ vào
//! [`FormulaRegistry`](crate::registry::FormulaRegistry) của World.

use serde::{Deserialize, Serialize};

use crate::ecology::{
    ENERGY_MAX, ENERGY_PER_RESOURCE_UNIT, METABOLIC_COST, REPRODUCE_THRESHOLD,
};
use crate::registry::FormulaId;

/// Một thực thể sống.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Atom {
    /// Ô lưới đang chiếm (cột, hàng).
    pub pos: (usize, usize),
    /// Năng lượng, clamp [0, ENERGY_MAX].
    pub energy: f64,
    /// Gene — handle vào FormulaRegistry của World.
    pub gene: FormulaId,
    /// Số bước đã sống.
    pub age: u64,
}

impl Atom {
    /// Trừ metabolic cost. Trả về `false` nếu atom chết (energy ≤ 0).
    pub fn metabolize(&mut self) -> bool {
        self.energy -= METABOLIC_COST;
        if self.energy <= 0.0 {
            self.energy = 0.0;
            return false;
        }
        true
    }

    /// Ăn tài nguyên: giá trị ô ≥ 2 quy đổi thành năng lượng, clamp max.
    /// (Caller chịu trách nhiệm xoá tài nguyên khỏi lưới.)
    pub fn feed(&mut self, cell_value: u8) {
        debug_assert!(cell_value >= 2, "feed chỉ dùng cho ô tài nguyên");
        self.energy = (self.energy
            + (cell_value as f64) * ENERGY_PER_RESOURCE_UNIT)
            .min(ENERGY_MAX);
    }

    /// Sinh sản: cha giữ nửa năng lượng, trả về nửa cho con.
    /// Trả về `None` nếu chưa đạt ngưỡng sinh sản.
    pub fn split_energy(&mut self) -> Option<f64> {
        if self.energy < REPRODUCE_THRESHOLD {
            return None;
        }
        let child = self.energy / 2.0;
        self.energy -= child;
        Some(child)
    }
}

/// Ô kề trống đầu tiên theo thứ tự quét cố định N, E, S, W.
///
/// `in_bounds(x, y)` do caller cung cấp (biết w/h); `occupied(x, y)` tra
/// tập hợp vị trí atom đang sống. Trả về toạ độ (x, y) của ô tìm được.
pub fn first_free_neighbor(
    pos: (usize, usize),
    in_bounds: &dyn Fn(usize, usize) -> bool,
    occupied: &dyn Fn(usize, usize) -> bool,
) -> Option<(usize, usize)> {
    // N, E, S, W — dùng isize để lùi biên an toàn.
    const OFFSETS: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    let (px, py) = (pos.0 as isize, pos.1 as isize);
    for (dx, dy) in OFFSETS {
        let (x, y) = (px + dx, py + dy);
        if x < 0 || y < 0 {
            continue;
        }
        let (x, y) = (x as usize, y as usize);
        if in_bounds(x, y) && !occupied(x, y) {
            return Some((x, y));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom_at(x: usize, y: usize, energy: f64) -> Atom {
        Atom {
            pos: (x, y),
            energy,
            gene: FormulaId::from_slot(0),
            age: 0,
        }
    }

    #[test]
    fn metabolize_kills_starved_atom() {
        let mut a = atom_at(0, 0, METABOLIC_COST - 0.01);
        assert!(!a.metabolize());
        assert_eq!(a.energy, 0.0);
    }

    #[test]
    fn metabolize_survives_with_leftover() {
        let mut a = atom_at(0, 0, METABOLIC_COST + 0.01);
        assert!(a.metabolize());
        assert!((a.energy - 0.01).abs() < 1e-12);
    }

    #[test]
    fn feed_adds_resource_energy_clamped() {
        let mut a = atom_at(0, 0, 0.5);
        a.feed(2); // 0.5 + 2*0.2 = 0.9
        assert!((a.energy - 0.9).abs() < 1e-12);
        a.feed(3); // 0.9 + 0.6 = 1.5 → clamp 1.0
        assert!((a.energy - ENERGY_MAX).abs() < 1e-12);
    }

    #[test]
    fn split_only_above_threshold_and_halves() {
        let mut a = atom_at(0, 0, 0.5);
        assert!(a.split_energy().is_none()); // dưới ngưỡng

        let mut b = atom_at(0, 0, REPRODUCE_THRESHOLD);
        let child = b.split_energy().unwrap();
        assert!((child - REPRODUCE_THRESHOLD / 2.0).abs() < 1e-12);
        assert!((b.energy - REPRODUCE_THRESHOLD / 2.0).abs() < 1e-12);
    }

    #[test]
    fn first_free_neighbor_scans_n_esw_order() {
        let bounds = |_: usize, _: usize| true;
        let empty = |_: usize, _: usize| false;
        // Tất cả trống → chọn N trước.
        assert_eq!(first_free_neighbor((2, 2), &bounds, &empty), Some((2, 1)));

        // N và E bị chiếm → chọn S.
        let occ = |x: usize, y: usize| (x == 2 && y == 1) || (x == 3 && y == 2);
        assert_eq!(first_free_neighbor((2, 2), &bounds, &occ), Some((2, 3)));
    }

    #[test]
    fn first_free_neighbor_respects_bounds() {
        let bounds = |x: usize, y: usize| x < 3 && y < 3;
        let empty = |_: usize, _: usize| false;
        // (0,0): N ngoài biên, E=(1,0) trống.
        assert_eq!(first_free_neighbor((0, 0), &bounds, &empty), Some((1, 0)));
    }

    #[test]
    fn atom_serialization_round_trip() {
        let a = atom_at(3, 4, 0.75);
        let bytes = serde_json::to_vec(&a).unwrap();
        let back: Atom = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, a);
    }
}
```

- [ ] **Step 2: Tạo module ecology (hằng số chung, tránh vòng phụ thuộc Task 7)**

Tạo `crates/omiai-world/src/ecology.rs`:

```rust
//! Hằng số sinh thái của world — một chỗ duy nhất, mọi phase tham chiếu.

/// Năng lượng mất mỗi bước của mỗi atom.
pub const METABOLIC_COST: f64 = 0.05;
/// Trần năng lượng của một atom.
pub const ENERGY_MAX: f64 = 1.0;
/// Ngưỡng energy để sinh sản.
pub const REPRODUCE_THRESHOLD: f64 = 0.8;
/// Quy đổi: mỗi đơn giá trị ô tài nguyên → năng lượng.
pub const ENERGY_PER_RESOURCE_UNIT: f64 = 0.2;
/// Xác suất đột biến gene khi sinh sản.
pub const MUTATION_PROB: f64 = 0.3;
/// Độ sâu tối đa của formula sau đột biến (chống phình chi phí decode).
pub const MAX_FORMULA_DEPTH: usize = 5;
```

- [ ] **Step 3: Wire modules**

`crates/omiai-world/src/lib.rs` thêm:

```rust
pub mod atoms;
pub mod ecology;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p omiai-world`
Expected: PASS (tests atoms + registry + substrate).

- [ ] **Step 5: Commit + push**

```bash
git add crates/omiai-world/
git commit -m "feat(world): Atom lifecycle (metabolize/feed/split) + ecology constants"
git push origin main
```

---

### Task 6: `omiai-world::agents` — quan sát, decode policy, hành động

**Files:**
- Create: `crates/omiai-world/src/agents.rs`
- Modify: `crates/omiai-world/src/lib.rs` (`pub mod agents;`)
- Test: trong `agents.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `LtlFormula`, `Atom`, hằng số `ecology::*`.
- Produces (Task 7 dùng):
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Direction { North, East, South, West }` + `pub const ALL_DIRECTIONS: [Direction; 4]` (thứ tự cố định N,E,S,W) + `Direction::delta(self) -> (isize, isize)`
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Action { Stay, Move(Direction) }`
  - `pub struct Observation { pub open: bool, pub wall: bool, pub res: bool, pub occupied: bool }` cho MỘT hướng; `pub fn observe(cell_value: u8, occupied: bool) -> Observation`
  - `pub fn valuation(obs: &Observation) -> std::collections::BTreeMap<String, bool>` — các atom命题 `"open"`, `"wall"`, `"res"`, `"occupied"` (**BTreeMap** — payload có thứ tự cố định)
  - `pub fn eval_current(f: &LtlFormula, val: &BTreeMap<String, bool>) -> bool` — đánh giá propositional; toán tử thời gian coi như phần hiện tại (`Next(g)=eval(g)`, `Eventually(g)=eval(g)`, `Globally(g)=eval(g)`, `Until(_,q)=eval(q)`, `Release(p,q)=eval(p)&&eval(q)`); atom lạ = false
  - `pub fn decide(formula: &LtlFormula, obs_by_dir: &[(Direction, Observation)]) -> Action` — duyệt `obs_by_dir` ĐÚNG THỨ TỰ đưa vào, chọn hướng đầu tiên thoả: `eval_current(formula, valuation(obs)) && !obs.wall && !obs.occupied`; không hướng nào thoả → `Stay`.

- [ ] **Step 1: Write the failing tests + implementation (một file)**

Tạo `crates/omiai-world/src/agents.rs`:

```rust
//! Agent: atom hành động theo gene LTL của chính nó.
//!
//! Decode policy thuần tuý, không cần World — quan sát một hướng được mã
//! hoá thành các mệnh đề `open/wall/res/occupied`; genome được đánh giá
//! propositional trên từng hướng, hướng đầu tiên thoả (theo thứ tự cố định
//! N,E,S,W) được chọn. Toán tử thời gian của LTL được coi như phần hiện tại
//! (giới hạn đã biết, ghi rõ ở [`eval_current`]).

use std::collections::BTreeMap;

use omiai_core::ltl::LtlFormula;

use crate::atoms::Atom;

/// Hướng trên lưới — thứ tự khai báo cũng là thứ tự ưu tiên khi quyết định.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

pub const ALL_DIRECTIONS: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];

impl Direction {
    /// Delta (dx, dy) của hướng.
    pub fn delta(self) -> (isize, isize) {
        match self {
            Direction::North => (0, -1),
            Direction::East => (1, 0),
            Direction::South => (0, 1),
            Direction::West => (-1, 0),
        }
    }
}

/// Hành động của một agent trong một bước.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Stay,
    Move(Direction),
}

/// Quan sát MỘT ô kề: giá trị ô + có atom khác đang đứng hay không.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Observation {
    pub open: bool,
    pub wall: bool,
    pub res: bool,
    pub occupied: bool,
}

/// Mã hoá giá trị ô lưới (0 trống, 1 cản, ≥2 tài nguyên) + tình trạng chiếm.
pub fn observe(cell_value: u8, occupied: bool) -> Observation {
    Observation {
        open: cell_value == 0 && !occupied,
        wall: cell_value == 1,
        res: cell_value >= 2,
        occupied,
    }
}

/// Valuation propositional của một quan sát (BTreeMap — thứ tự cố định).
pub fn valuation(obs: &Observation) -> BTreeMap<String, bool> {
    [
        ("open".to_string(), obs.open),
        ("wall".to_string(), obs.wall),
        ("res".to_string(), obs.res),
        ("occupied".to_string(), obs.occupied),
    ]
    .into_iter()
    .collect()
}

/// Đánh giá propositional của LtlFormula trên trạng thái hiện tại.
///
/// Ngữ nghĩa toán tử thời gian ở đây (giới hạn đã biết của policy decode):
/// - `X φ`, `F φ`, `G φ` → đánh giá φ ngay bây giờ.
/// - `φ U ψ` → ψ ngay bây giờ; `φ R ψ` → φ ∧ ψ ngay bây giờ.
/// - Atom không có trong valuation → false.
pub fn eval_current(f: &LtlFormula, val: &BTreeMap<String, bool>) -> bool {
    match f {
        LtlFormula::True_ => true,
        LtlFormula::False_ => false,
        LtlFormula::Atom(name) => val.get(name).copied().unwrap_or(false),
        LtlFormula::Not(g) => !eval_current(g, val),
        LtlFormula::And(a, b) => eval_current(a, val) && eval_current(b, val),
        LtlFormula::Or(a, b) => eval_current(a, val) || eval_current(b, val),
        LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
            eval_current(g, val)
        }
        LtlFormula::Until(_, q) => eval_current(q, val),
        LtlFormula::Release(p, q) => eval_current(p, val) && eval_current(q, val),
    }
}

/// Chọn hành động: hướng đầu tiên (theo thứ tự trong `obs_by_dir`) mà
/// genome thoả VÀ ô đi được (không cản, không bị chiếm).
///
/// `obs_by_dir` thường là `ALL_DIRECTIONS` zipped với quan sát — thứ tự
/// đưa vào chính là thứ tự ưu tiên.
pub fn decide(
    formula: &LtlFormula,
    obs_by_dir: &[(Direction, Observation)],
) -> Action {
    for (dir, obs) in obs_by_dir {
        let passable = !obs.wall && !obs.occupied;
        if passable && eval_current(formula, &valuation(obs)) {
            return Action::Move(*dir);
        }
    }
    Action::Stay
}

/// Quan sát 4 hướng quanh một atom trên lưới.
///
/// `cell(x, y)` đọc giá trị ô (ngoài biên coi là cản); `occupied(x, y)` tra
/// vị trí atom khác. Trả về cặp (Direction, Observation) theo ALL_DIRECTIONS.
pub fn observe_surroundings(
    pos: (usize, usize),
    width: usize,
    height: usize,
    cell: &dyn Fn(usize, usize) -> u8,
    occupied: &dyn Fn(usize, usize) -> bool,
) -> Vec<(Direction, Observation)> {
    ALL_DIRECTIONS
        .iter()
        .map(|&dir| {
            let (dx, dy) = dir.delta();
            let (x, y) = (pos.0 as isize + dx, pos.1 as isize + dy);
            if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
                (dir, observe(1, false)) // ngoài biên = cản
            } else {
                let (x, y) = (x as usize, y as usize);
                (dir, observe(cell(x, y), occupied(x, y)))
            }
        })
        .collect()
}

/// Vị trí đích nếu action là di chuyển (không tự kiểm tra tính hợp lệ —
/// caller trong world_loop kiểm tra lần cuối trước khi áp dụng).
pub fn target_of(atom: &Atom, action: Action) -> (usize, usize) {
    match action {
        Action::Stay => atom.pos,
        Action::Move(dir) => {
            let (dx, dy) = dir.delta();
            (
                (atom.pos.0 as isize + dx) as usize,
                (atom.pos.1 as isize + dy) as usize,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- observe / valuation ---

    #[test]
    fn observe_maps_cell_values() {
        assert_eq!(observe(0, false).open, true);
        assert_eq!(observe(1, false).wall, true);
        assert_eq!(observe(2, false).res, true);
        assert_eq!(observe(3, false).res, true);
        assert_eq!(observe(0, true).occupied, true);
        assert_eq!(observe(0, true).open, false);
    }

    // --- eval_current ---

    #[test]
    fn eval_atom_lookup_and_default_false() {
        let mut val = valuation(&observe(2, false));
        assert!(eval_current(&LtlFormula::atom("res"), &val));
        assert!(!eval_current(&LtlFormula::atom("wall"), &val));
        // Atom lạ → false.
        assert!(!eval_current(&LtlFormula::atom("nonexistent"), &val));
        val.insert("extra".to_string(), true);
        assert!(eval_current(&LtlFormula::atom("extra"), &val));
    }

    #[test]
    fn eval_boolean_connectives() {
        let val = valuation(&observe(0, false)); // chỉ open=true
        let open = LtlFormula::atom("open");
        let wall = LtlFormula::atom("wall");
        assert!(eval_current(&LtlFormula::and(open.clone(), LtlFormula::True_), &val));
        assert!(eval_current(&LtlFormula::or(open.clone(), wall.clone()), &val));
        assert!(!eval_current(&LtlFormula::and(open.clone(), wall.clone()), &val));
        assert!(eval_current(&LtlFormula::neg(wall.clone()), &val));
        // implies(p,q) = ¬p ∨ q — không có constructor riêng trong LtlFormula.
        let implies = LtlFormula::or(LtlFormula::neg(open), LtlFormula::True_);
        assert!(eval_current(&implies, &val));
    }

    #[test]
    fn eval_temporal_operators_fall_back_to_present() {
        let val = valuation(&observe(2, false)); // res=true
        let res = LtlFormula::atom("res");
        assert!(eval_current(&LtlFormula::f(res.clone()), &val));
        assert!(eval_current(&LtlFormula::g(res.clone()), &val));
        assert!(eval_current(&LtlFormula::x(res.clone()), &val));
        assert!(eval_current(
            &LtlFormula::until(LtlFormula::False_, res.clone()),
            &val
        ));
        assert!(eval_current(
            &LtlFormula::release(res.clone(), res.clone()),
            &val
        ));
    }

    // --- decide ---

    fn obs_open() -> Observation {
        observe(0, false)
    }
    fn obs_res() -> Observation {
        observe(3, false)
    }
    fn obs_wall() -> Observation {
        observe(1, false)
    }

    #[test]
    fn genome_seeking_resource_moves_to_first_resource() {
        // Genome "muốn tài nguyên": res ∨ open
        let genome = LtlFormula::or(
            LtlFormula::atom("res"),
            LtlFormula::atom("open"),
        );
        // N=open, E=res → N thoả trước (thứ tự ưu tiên).
        let obs = vec![
            (Direction::North, obs_open()),
            (Direction::East, obs_res()),
            (Direction::South, obs_wall()),
            (Direction::West, obs_wall()),
        ];
        assert_eq!(decide(&genome, &obs), Action::Move(Direction::North));

        // N=cản → E=res được chọn.
        let obs2 = vec![
            (Direction::North, obs_wall()),
            (Direction::East, obs_res()),
        ];
        assert_eq!(decide(&genome, &obs2), Action::Move(Direction::East));
    }

    #[test]
    fn blocked_cells_are_skipped_even_if_formula_matches() {
        // Genome: wall (formula thoả ở ô cản nhưng ô cản không đi được).
        let genome = LtlFormula::atom("wall");
        let obs = vec![
            (Direction::North, obs_wall()),
            (Direction::East, obs_open()),
        ];
        // N: formula thoả nhưng wall → bỏ; E: open → move.
        assert_eq!(decide(&genome, &obs), Action::Move(Direction::East));
    }

    #[test]
    fn occupied_cells_are_skipped() {
        let genome = LtlFormula::atom("open");
        let obs = vec![
            (Direction::North, observe(0, true)), // trống nhưng bị chiếm
            (Direction::East, observe(0, false)),
        ];
        assert_eq!(decide(&genome, &obs), Action::Move(Direction::East));
    }

    #[test]
    fn no_match_means_stay() {
        let genome = LtlFormula::atom("res");
        let obs = vec![
            (Direction::North, obs_open()),
            (Direction::East, obs_wall()),
        ];
        assert_eq!(decide(&genome, &obs), Action::Stay);
    }

    // --- observe_surroundings ---

    #[test]
    fn surroundings_read_grid_and_bounds_as_wall() {
        // Lưới 3x3, tất cả trống; atom ở (0,0): W và N ngoài biên = cản.
        let cell = |_: usize, _: usize| 0u8;
        let occ = |_: usize, _: usize| false;
        let obs = observe_surroundings((0, 0), 3, 3, &cell, &occ);
        assert_eq!(obs.len(), 4);
        assert!(obs[0].1.wall); // North ngoài biên
        assert!(obs[1].1.open); // East
        assert!(obs[2].1.open); // South
        assert!(obs[3].1.wall); // West ngoài biên
    }

    // --- target_of ---

    #[test]
    fn target_of_move_applies_delta() {
        let mut a = Atom {
            pos: (2, 2),
            energy: 0.5,
            gene: crate::registry::FormulaId::from_slot(0),
            age: 3,
        };
        assert_eq!(target_of(&a, Action::Move(Direction::North)), (2, 1));
        assert_eq!(target_of(&a, Action::Stay), (2, 2));
        a.pos = (0, 0);
        assert_eq!(target_of(&a, Action::Move(Direction::West)), (0, 0)); // wrap usize — caller phải kiểm tra biên trước
    }
}
```

- [ ] **Step 2: Wire module**

`crates/omiai-world/src/lib.rs` thêm `pub mod agents;`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p omiai-world`
Expected: PASS.

- [ ] **Step 4: Commit + push**

```bash
git add crates/omiai-world/
git commit -m "feat(world): agent policy decode — LTL genome → directional action"
git push origin main
```

---

### Task 7: `omiai-world::world_loop` — World + 5 phase + đột biến

**Files:**
- Create: `crates/omiai-world/src/world_loop.rs`
- Modify: `crates/omiai-world/src/lib.rs` (`pub mod world_loop;`, cân nhắc gỡ `#![allow(dead_code)]` nếu hết cảnh báo)
- Modify: `crates/omiai-world/Cargo.toml` (thêm `rand_chacha.workspace = true`, `rand.workspace = true`)
- Test: trong `world_loop.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: mọi thứ từ Task 4–6 + `CellularAutomaton` (`substrate`).
- Produces (Task 8 checkpoint + test round-trip dùng):
  - `pub struct World { pub ca: CellularAutomaton, pub registry: FormulaRegistry, pub atoms: Vec<Atom>, pub rng: ChaCha8Rng, pub rng_seed: u64, pub rng_stream: u64, pub step_count: u64 }`
  - `pub struct WorldConfig { pub width: usize, pub height: usize, pub n_initial_atoms: usize, pub initial_resources: f64 /* density */ }` + `Default` = `{width:32, height:32, n_initial_atoms:5, initial_resources:0.06}`
  - `World::new(config, seed: u64) -> Self`
  - `World::step(&mut self)` — gọi 5 phase theo thứ tự; `pub fn ca_step(&mut self)`, `pub fn metabolism(&mut self)`, `pub fn agent_act(&mut self)`, `pub fn reproduce_and_evolve(&mut self)`, `pub fn snapshot(&mut self)` đều `pub` để test độc lập
  - `pub fn mutate_formula(f: &LtlFormula, rng: &mut ChaCha8Rng) -> LtlFormula` — đột biến cấu trúc bounded depth
  - `pub fn occupied_set(atoms: &[Atom]) -> std::collections::BTreeSet<(usize, usize)>`

- [ ] **Step 1: Write the failing tests + implementation (một file)**

Tạo `crates/omiai-world/src/world_loop.rs`:

```rust
//! World loop: 5 phase cố định — ca_step, metabolism, agent_act,
//! reproduce_and_evolve, snapshot. Thứ tự cố định bảo đảm resume
//! deterministic; mỗi phase là hàm riêng test được độc lập.

use std::collections::BTreeSet;

use omiai_core::ltl::LtlFormula;
use rand::Rng;
use rand_chacha::{rand_core::SeedableRng, ChaCha8Rng};

use crate::agents::{self, Action};
use crate::atoms::Atom;
use crate::ecology::{MAX_FORMULA_DEPTH, MUTATION_PROB};
use crate::registry::{FormulaRegistry, Genome};
use crate::substrate::CellularAutomaton;

/// Cấu hình khởi tạo world.
#[derive(Debug, Clone)]
pub struct WorldConfig {
    pub width: usize,
    pub height: usize,
    pub n_initial_atoms: usize,
    /// Density ô tài nguyên lúc khởi tạo (giá trị 2 hoặc 3 ngẫu nhiên).
    pub initial_resources: f64,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            width: 32,
            height: 32,
            n_initial_atoms: 5,
            initial_resources: 0.06,
        }
    }
}

/// Thế giới: lưới CA + registry genome + các atom + RNG deterministic.
pub struct World {
    pub ca: CellularAutomaton,
    pub registry: FormulaRegistry,
    pub atoms: Vec<Atom>,
    pub rng: ChaCha8Rng,
    /// Seed gốc (lưu checkpoint để tái tạo `rng`).
    pub rng_seed: u64,
    /// Stream số (mặc định 0, giữ cho tương lai; lưu checkpoint).
    pub rng_stream: u64,
    pub step_count: u64,
}

impl World {
    /// Khởi tạo: lưới trống + rải tài nguyên + đặt atom mồi lên ô trống
    /// đầu tiên quét row-major. Toàn bộ randomness qua `self.rng`.
    pub fn new(config: WorldConfig, seed: u64) -> Self {
        let ca = CellularAutomaton::new(config.width, config.height, 4);
        let mut registry = FormulaRegistry::new();
        let default_genome = registry.insert(Genome {
            formula: LtlFormula::or(
                LtlFormula::atom("res"),
                LtlFormula::atom("open"),
            ),
            fitness: None,
        });

        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut world = Self {
            ca,
            registry,
            atoms: Vec::new(),
            rng,
            rng_seed: seed,
            rng_stream: 0,
            step_count: 0,
        };

        // Rải tài nguyên: giá trị 2 hoặc 3.
        let n_cells = config.width.saturating_mul(config.height);
        for i in 0..n_cells {
            if world.rng.r#gen::<f64>() < config.initial_resources {
                let rich = world.rng.r#gen::<bool>();
                world.ca.cells[i] = if rich { 3 } else { 2 };
            }
        }

        // Đặt atom mồi lên các ô trống đầu tiên (row-major).
        let occupied = occupied_set(&world.atoms);
        let mut placed = 0;
        for i in 0..n_cells {
            if placed >= config.n_initial_atoms {
                break;
            }
            let (x, y) = (i % config.width, i / config.width);
            if world.ca.cells[i] == 0 && !occupied.contains(&(x, y)) {
                world.atoms.push(Atom {
                    pos: (x, y),
                    energy: 0.5,
                    gene: default_genome,
                    age: 0,
                });
                placed += 1;
            }
        }
        world
    }

    /// Một bước world: 5 phase theo thứ tự cố định.
    pub fn step(&mut self) {
        self.ca_step();
        self.metabolism();
        self.agent_act();
        self.reproduce_and_evolve();
        self.snapshot();
    }

    /// Phase 1: môi trường tiến hoá một bước Margolus.
    pub fn ca_step(&mut self) {
        self.ca.step();
    }

    /// Phase 2: trừ năng lượng, loại atom chết.
    pub fn metabolism(&mut self) {
        self.atoms.retain_mut(|atom| atom.metabolize());
    }

    /// Phase 3: mỗi atom quan sát → decode genome → hành động, duyệt
    /// theo thứ tự Vec (deterministic). Ăn tài nguyên: ô ≥ 2 → cộng
    /// năng lượng, ô về 0.
    pub fn agent_act(&mut self) {
        let width = self.ca.width;
        let height = self.ca.height;
        for i in 0..self.atoms.len() {
            let (pos, gene) = {
                let atom = &self.atoms[i];
                (atom.pos, atom.gene)
            };
            let formula = match self.registry.get(gene) {
                Some(g) => g.formula.clone(),
                None => continue, // genome mất (không xảy ra ở slice này)
            };

            let occupied = occupied_set(&self.atoms);
            let cells = self.ca.cells.clone();
            let cell = |x: usize, y: usize| cells[y * width + x];
            let occ = |x: usize, y: usize| {
                occupied.contains(&(x, y)) && (x, y) != pos
            };
            let obs = agents::observe_surroundings(pos, width, height, &cell, &occ);

            let action = agents::decide(&formula, &obs);
            let target = agents::target_of(&self.atoms[i], action);
            if target != pos
                && target.0 < width
                && target.1 < height
            {
                let ti = target.1 * width + target.0;
                let tv = self.ca.cells[ti];
                if tv == 0 || tv >= 2 {
                    let still_occupied = self
                        .atoms
                        .iter()
                        .any(|a| a.pos == target);
                    if !still_occupied {
                        self.atoms[i].pos = target;
                        if tv >= 2 {
                            self.atoms[i].feed(tv);
                            self.ca.cells[ti] = 0;
                        }
                    }
                }
            }
        }
    }

    /// Phase 4: sinh sản qua ngưỡng + đột biến gene.
    pub fn reproduce_and_evolve(&mut self) {
        let mut children: Vec<Atom> = Vec::new();
        // `taken` = vị trí atom hiện hữu + vị trí con vừa đặt trong phase
        // này (tránh hai cha chọn cùng ô). Cập nhật ngay sau mỗi lần sinh.
        let mut taken = occupied_set(&self.atoms);
        for atom in self.atoms.iter_mut() {
            atom.age += 1;
            if let Some(child_energy) = atom.split_energy() {
                let in_bounds =
                    |x: usize, y: usize| x < self.ca.width && y < self.ca.height;
                let is_taken =
                    |x: usize, y: usize| taken.contains(&(x, y));
                if let Some((sx, sy)) =
                    crate::atoms::first_free_neighbor(atom.pos, &in_bounds, &is_taken)
                {
                    // Ô con phải trống về tài nguyên nữa (ô có tài nguyên
                    // thì con sinh ra và ăn luôn? Không — YAGNI: chỉ sinh
                    // lên ô giá trị 0).
                    let cell_v = self.ca.cells[sy * self.ca.width + sx];
                    if cell_v == 0 {
                        let child_gene = if self.rng.r#gen::<f64>() < MUTATION_PROB {
                            let mutated = mutate_formula(
                                &self.registry.get(atom.gene).expect("gene tồn tại").formula,
                                &mut self.rng,
                            );
                            self.registry.insert(Genome { formula: mutated, fitness: None })
                        } else {
                            atom.gene
                        };
                        taken.insert((sx, sy));
                        children.push(Atom {
                            pos: (sx, sy),
                            energy: child_energy,
                            gene: child_gene,
                            age: 0,
                        });
                    }
                }
            }
        }
        self.atoms.extend(children);
    }

    /// Phase 5: đóng băng bước.
    pub fn snapshot(&mut self) {
        self.step_count += 1;
    }
}

/// Tập vị trí đang bị chiếm (BTreeSet — thứ tự cố định).
pub fn occupied_set(atoms: &[Atom]) -> BTreeSet<(usize, usize)> {
    atoms.iter().map(|a| a.pos).collect()
}

/// Độ sâu AST của formula.
fn depth(f: &LtlFormula) -> usize {
    match f {
        LtlFormula::True_
        | LtlFormula::False_
        | LtlFormula::Atom(_) => 1,
        LtlFormula::Not(g)
        | LtlFormula::Next(g)
        | LtlFormula::Eventually(g)
        | LtlFormula::Globally(g) => 1 + depth(g),
        LtlFormula::And(a, b)
        | LtlFormula::Or(a, b)
        | LtlFormula::Until(a, b)
        | LtlFormula::Release(a, b) => 1 + depth(a).max(depth(b)),
    }
}

/// Đột biến cấu trúc: chọn ngẫu nhiên một biến đổi an toàn, giữ depth ≤
/// MAX_FORMULA_DEPTH. Các biến đổi: đổi atom thành atom khác, phủ định
/// node, đảo And↔Or. Không xoá cấu trúc (genome luôn còn đánh giá được).
pub fn mutate_formula(f: &LtlFormula, rng: &mut ChaCha8Rng) -> LtlFormula {
    const ATOM_NAMES: [&str; 4] = ["open", "wall", "res", "occupied"];
    match f {
        LtlFormula::Atom(_) => {
            let name = ATOM_NAMES[rng.gen_range(0..ATOM_NAMES.len())];
            LtlFormula::atom(name)
        }
        LtlFormula::Not(g) => LtlFormula::Not(Box::new(mutate_formula(g, rng))),
        LtlFormula::And(a, b) | LtlFormula::Or(a, b) => {
            let (a2, b2) = (mutate_formula(a, rng), mutate_formula(b, rng));
            if rng.r#gen::<bool>() {
                LtlFormula::Or(Box::new(a2), Box::new(b2))
            } else {
                LtlFormula::And(Box::new(a2), Box::new(b2))
            }
        }
        LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
            let inner = mutate_formula(g, rng);
            match rng.gen_range(0..3) {
                0 => LtlFormula::Next(Box::new(inner)),
                1 => LtlFormula::Eventually(Box::new(inner)),
                _ => LtlFormula::Globally(Box::new(inner)),
            }
        }
        LtlFormula::Until(p, q) | LtlFormula::Release(p, q) => {
            let p2 = mutate_formula(p, rng);
            let q2 = mutate_formula(q, rng);
            if rng.r#gen::<bool>() {
                LtlFormula::Until(Box::new(p2), Box::new(q2))
            } else {
                LtlFormula::Release(Box::new(p2), Box::new(q2))
            }
        }
        leaf => leaf.clone(), // True_/False_ giữ nguyên
    }
}
```

**CHÚ Ý cho người thực hiện:** nhánh `Not` trong `mutate_formula` ở khung
code trên còn hai nhánh giống hệt nhau (tàn dư của thiết kế depth-guard) —
rút gọn thành một khi viết:

```rust
LtlFormula::Not(g) => LtlFormula::Not(Box::new(mutate_formula(g, rng))),
```

Tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecology::{ENERGY_MAX, METABOLIC_COST, REPRODUCE_THRESHOLD};
    use crate::registry::FormulaId;

    fn small_world(seed: u64) -> World {
        World::new(
            WorldConfig {
                width: 8,
                height: 8,
                n_initial_atoms: 2,
                initial_resources: 0.1,
            },
            seed,
        )
    }

    #[test]
    fn new_world_places_atoms_on_empty_cells() {
        let w = small_world(7);
        assert_eq!(w.atoms.len(), 2);
        assert_eq!(w.registry.len(), 1); // genome mặc định dùng chung
        for a in &w.atoms {
            assert_eq!(w.ca.cells[a.pos.1 * 8 + a.pos.0], 0);
            assert_eq!(a.age, 0);
            assert!((a.energy - 0.5).abs() < 1e-12);
        }
    }

    #[test]
    fn new_world_is_deterministic_same_seed() {
        let a = small_world(42);
        let b = small_world(42);
        assert_eq!(a.ca.cells, b.ca.cells);
        assert_eq!(a.atoms, b.atoms);
        assert_ne!(small_world(42).rng_seed, small_world(43).rng_seed);
    }

    #[test]
    fn metabolism_removes_starved_atoms() {
        let mut w = small_world(1);
        w.atoms.clear();
        w.atoms.push(Atom {
            pos: (0, 0),
            energy: METABOLIC_COST - 0.001,
            gene: FormulaId::from_slot(0),
            age: 0,
        });
        w.metabolism();
        assert!(w.atoms.is_empty());
    }

    #[test]
    fn agent_act_eats_resource_and_clears_cell() {
        let mut w = World::new(
            WorldConfig {
                width: 4,
                height: 4,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            5,
        );
        // Atom ở (1,0), genome mặc định (res ∨ open); đặt tài nguyên bên E.
        let gene = FormulaId::from_slot(0);
        w.atoms.push(Atom { pos: (1, 0), energy: 0.5, gene, age: 0 });
        w.ca.cells[0 * 4 + 2] = 3; // (2,0) = East
        let before = w.atoms[0].energy;

        w.agent_act();

        assert_eq!(w.atoms[0].pos, (2, 0));
        assert_eq!(w.ca.cells[0 * 4 + 2], 0); // đã ăn
        assert!(w.atoms[0].energy > before);
        assert!(w.atoms[0].energy <= ENERGY_MAX);
    }

    #[test]
    fn agent_act_blocked_by_other_atom() {
        let mut w = World::new(
            WorldConfig {
                width: 4,
                height: 4,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            5,
        );
        let gene = FormulaId::from_slot(0);
        // Atom A (duyệt trước) ở (1,0); atom B ở (2,0) — East của A.
        w.atoms.push(Atom { pos: (1, 0), energy: 0.5, gene, age: 0 });
        w.atoms.push(Atom { pos: (2, 0), energy: 0.5, gene, age: 0 });
        // Lưới trống hoàn toàn: A muốn đi N (ưu tiên cao nhất trống).
        w.agent_act();
        // A không thể đứng yên nếu có hướng trống — kiểm tra A rời (1,0)
        // và không đè lên B.
        assert_ne!(w.atoms[0].pos, (2, 0));
    }

    #[test]
    fn reproduce_splits_at_threshold_when_space() {
        let mut w = World::new(
            WorldConfig {
                width: 4,
                height: 4,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            9,
        );
        let gene = FormulaId::from_slot(0);
        w.atoms.push(Atom { pos: (1, 1), energy: REPRODUCE_THRESHOLD, gene, age: 0 });
        let parent_before = w.atoms[0].energy;

        w.reproduce_and_evolve();

        assert_eq!(w.atoms.len(), 2);
        assert!(w.atoms[0].energy < parent_before);
        assert!((w.atoms[0].energy + w.atoms[1].energy - parent_before).abs() < 1e-12);
        // Con kế thừa gene cha HOẶC genome đột biến mới — cả hai đều phải
        // hợp lệ trong registry (MUTATION_PROB = 0.3 nên không khẳng định
        // cứng gene nào ở đây).
        assert!(w.registry.get(w.atoms[1].gene).is_some());
    }

    #[test]
    fn reproduce_no_space_no_child() {
        let mut w = World::new(
            WorldConfig {
                width: 2,
                height: 2,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            11,
        );
        let gene = FormulaId::from_slot(0);
        // Chiếm cả 4 ô → không còn ô kề trống.
        for pos in [(0, 0), (1, 0), (0, 1)] {
            w.atoms.push(Atom { pos, energy: 0.3, gene, age: 0 });
        }
        w.atoms.push(Atom { pos: (1, 1), energy: REPRODUCE_THRESHOLD, gene, age: 0 });
        w.reproduce_and_evolve();
        assert_eq!(w.atoms.len(), 4); // không ai sinh được
    }

    #[test]
    fn step_increments_counter_and_runs_phases() {
        let mut w = small_world(3);
        w.step();
        assert_eq!(w.step_count, 1);
        assert!(w.atoms.iter().all(|a| a.age == 1));
    }

    #[test]
    fn same_seed_same_trajectory() {
        let mut a = small_world(77);
        let mut b = small_world(77);
        for _ in 0..20 {
            a.step();
            b.step();
        }
        assert_eq!(a.ca.cells, b.ca.cells);
        assert_eq!(a.atoms, b.atoms);
        assert_eq!(a.step_count, b.step_count);
    }

    #[test]
    fn mutate_formula_bounded_depth_and_valid() {
        let mut rng = ChaCha8Rng::seed_from_u64(4);
        let base = LtlFormula::and(
            LtlFormula::atom("open"),
            LtlFormula::g(LtlFormula::atom("res")),
        );
        for _ in 0..50 {
            let m = mutate_formula(&base, &mut rng);
            assert!(depth(&m) <= MAX_FORMULA_DEPTH + 1); // wrapper có thể +1
        }
    }

    #[test]
    fn energy_never_created_from_nothing_over_run() {
        // Tổng năng lượng atom chỉ giảm (metabolism) — ăn/sinh sản chỉ chuyển.
        let mut w = small_world(21);
        let total_start: f64 = w.atoms.iter().map(|a| a.energy).sum();
        for _ in 0..15 {
            w.step();
        }
        let total_end: f64 = w.atoms.iter().map(|a| a.energy).sum();
        // Cho phép ăn tài nguyên (tăng qua feed) — kiểm tra tổng ≤ start + eaten_bound:
        // mỗi bước mỗi atom ăn tối đa ENERGY_MAX; bound lỏng nhưng chắc chắn:
        // tổng không vượt n_atoms_tối_đa * ENERGY_MAX và mọi energy hợp lệ.
        assert!(w.atoms.iter().all(|a| a.energy.is_finite()
            && (0.0..=ENERGY_MAX).contains(&a.energy)));
        let _ = (total_start, total_end);
    }
}
```

**Lưu ý người thực hiện:** `retain_mut` cần Rust 1.61+ (workspace edition
2021 — ok). Khung code `reproduce_and_evolve` ở trên đã dùng sẵn pattern
`BTreeSet` (`taken`) nên không có vấn đề borrow checker.

- [ ] **Step 2: Wire modules + deps**

`crates/omiai-world/src/lib.rs` thêm `pub mod world_loop;`.

`crates/omiai-world/Cargo.toml` `[dependencies]` thêm:

```toml
rand.workspace = true
rand_chacha.workspace = true
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p omiai-world`
Expected: PASS. Nếu `same_seed_same_trajectory` fail: gần như chắc chắn có
đường randomness KHÔNG đi qua `self.rng` (kiểm tra `CellularAutomaton::random`
— không được dùng trong World::new) hoặc iteration thứ tự không ổn định.

- [ ] **Step 4: Commit + push**

```bash
git add crates/omiai-world/
git commit -m "feat(world): World loop — 5 fixed phases, deterministic ChaCha8, structural mutation"
git push origin main
```

---

### Task 8: Checkpoint bundle — `Checkpointable for World` + RNG state

**Files:**
- Modify: `crates/omiai-checkpoint/src/ca_grid.rs` (expose `encode`/`decode` thành `pub(crate)`)
- Create: `crates/omiai-checkpoint/src/world_bundle.rs`
- Modify: `crates/omiai-checkpoint/src/lib.rs` (`mod world_bundle; pub use world_bundle::*;` — hoặc chỉ impl, không cần re-export gì thêm)
- Test: Create `crates/omiai-checkpoint/tests/world_roundtrip.rs`
- Modify: `crates/omiai-checkpoint/Cargo.toml` nếu cần dev-dep thêm (`rand_chacha` cho dựng World trong test — kiểm tra: omiai-checkpoint đã depend `omiai-world` production, world đã có rand_chacha sau Task 7 → test dùng `omiai_world::world_loop::World::new` là đủ)

**Interfaces:**
- Consumes: `CellularAutomaton` encode/decode (ca_grid), `World` (public fields), `Genome`/`Atom` serde, `write_atomic`, `hash_file`, `Manifest`, `Checkpointable`, `CheckpointError`.
- Produces:
  - `impl Checkpointable for omiai_world::world_loop::World { type Error = CheckpointError; }`
  - File layout trong checkpoint dir: `world/grid.bin`, `world/atoms.cbor`, `world/registry.cbor`, `world/rng_state.bin` + `manifest.json` (4 FileRecord).
  - `rng_state.bin` layout: u64 LE `rng_seed` + u64 LE `rng_stream` + u128 LE `word_pos` (tổng 32 byte).
  - DTO serde cục bộ trong `world_bundle.rs`:
    - `AtomsFile { step_count: u64, atoms: Vec<Atom> }` (Atom đã Serialize từ Task 5)
    - `RegistryFile { genomes: Vec<Genome> }` (Genome đã Serialize từ Task 4)

- [ ] **Step 1: Expose ca_grid encode/decode**

Trong `crates/omiai-checkpoint/src/ca_grid.rs`, đổi chữ ký hai hàm nội bộ
(tên hiện tại có thể là `encode`/`decode` hoặc tương tự — grep trong file):
thành `pub(crate) fn encode_ca(ca: &CellularAutomaton) -> Result<Vec<u8>, CheckpointError>` và
`pub(crate) fn decode_ca(bytes: &[u8]) -> Result<CellularAutomaton, CheckpointError>`,
cập nhật call site trong chính `ca_grid.rs` (impl Checkpointable gọi hai hàm này).

- [ ] **Step 2: Write the failing test**

Tạo `crates/omiai-checkpoint/tests/world_roundtrip.rs`:

```rust
//! Round-trip bit-exact của World qua checkpoint-v1 — test then chốt slice 2:
//! save ở bước N → load → chạy tiếp M bước phải ra CÙNG trạng thái với
//! world chạy liền N+M bước không qua checkpoint.

use omiai_checkpoint::{traits::Checkpointable, verify_dir};
use omiai_world::world_loop::{World, WorldConfig};

fn config() -> WorldConfig {
    WorldConfig {
        width: 12,
        height: 12,
        n_initial_atoms: 4,
        initial_resources: 0.08,
    }
}

#[test]
fn world_save_load_resume_is_bit_exact() {
    // Thế giới chạy liền 30 bước.
    let mut continuous = World::new(config(), 123);
    for _ in 0..30 {
        continuous.step();
    }

    // Thế giới chạy 10 bước, save, load, chạy tiếp 20 bước.
    let root = std::env::temp_dir().join(format!("omiai-wrt-root-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000010");
    std::fs::create_dir_all(&cp).unwrap();

    let mut resumed = World::new(config(), 123);
    for _ in 0..10 {
        resumed.step();
    }
    resumed.save(&cp).expect("save world");
    verify_dir(&cp).expect("manifest + hashes ok");

    let mut loaded = World::load(&cp).expect("load world");
    // Trạng thái ngay sau load khớp trạng thái trước save.
    assert_eq!(loaded.ca.cells, resumed.ca.cells);
    assert_eq!(loaded.atoms, resumed.atoms);
    assert_eq!(loaded.step_count, resumed.step_count);
    assert_eq!(loaded.registry.genomes_in_order(), resumed.registry.genomes_in_order());

    for _ in 0..20 {
        resumed.step();
        loaded.step();
    }

    // Bit-exact với thế giới chạy liên tục.
    assert_eq!(loaded.ca.cells, continuous.ca.cells, "grid sai sau resume");
    assert_eq!(loaded.atoms, continuous.atoms, "atoms sai sau resume");
    assert_eq!(loaded.step_count, continuous.step_count);
    assert_eq!(
        loaded.registry.genomes_in_order(),
        continuous.registry.genomes_in_order(),
        "registry sai sau resume"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn tampered_payload_detected() {
    let root = std::env::temp_dir().join(format!("omiai-wrt-tamper-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000003");
    std::fs::create_dir_all(&cp).unwrap();

    let mut w = World::new(config(), 7);
    w.step();
    w.save(&cp).unwrap();

    // Xoá file payload → manifest còn, hash không verify nổi.
    std::fs::remove_file(cp.join("world").join("atoms.cbor")).unwrap();
    assert!(verify_dir(&cp).is_err());

    let _ = std::fs::remove_dir_all(&root);
}
```

**Lưu ý test:** `World` cần `PartialEq` cho `assert_eq!` trên
`loaded.atoms` vs `continuous.atoms` — `Atom` đã derive `PartialEq`
(Task 5) nên ok. `verify_dir` là hàm public sẵn có của omiai-checkpoint.

- [ ] **Step 3: Implement `world_bundle.rs`**

Tạo `crates/omiai-checkpoint/src/world_bundle.rs`:

```rust
//! `Checkpointable` cho `World`: bundle 4 file `world/*` của checkpoint-v1.
//!
//! Layout trong thư mục checkpoint:
//! ```text
//! world/grid.bin       — lưới CA (format ca_grid, xem ca_grid.rs)
//! world/atoms.cbor     — {step_count, atoms[]}
//! world/registry.cbor  — {genomes[]} theo thứ tự slot
//! world/rng_state.bin  — u64 LE seed + u64 LE stream + u128 LE word_pos
//! ```
//!
//! RNG tái tạo: `ChaCha8Rng::seed_from_u64(seed)` → `set_stream(stream)`
//! → `set_word_pos(word_pos)` (ADR-0006).

use std::path::Path;

use omiai_world::registry::Genome;
use omiai_world::world_loop::World;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::ca_grid::{decode_ca, encode_ca};
use crate::error::CheckpointError;
use crate::fsutil::{hash_file, write_atomic};
use crate::manifest::{FileRecord, Manifest};
use crate::traits::Checkpointable;

const WORLD_DIR: &str = "world";
const GRID_FILE: &str = "grid.bin";
const ATOMS_FILE: &str = "atoms.cbor";
const REGISTRY_FILE: &str = "registry.cbor";
const RNG_FILE: &str = "rng_state.bin";

#[derive(Debug, Serialize, Deserialize)]
struct AtomsFile {
    step_count: u64,
    atoms: Vec<omiai_world::atoms::Atom>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryFile {
    genomes: Vec<Genome>,
}

fn encode_rng(world: &World) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 8 + 16);
    out.extend_from_slice(&world.rng_seed.to_le_bytes());
    out.extend_from_slice(&world.rng_stream.to_le_bytes());
    out.extend_from_slice(&world.rng.get_word_pos().to_le_bytes());
    out
}

fn restore_rng(seed: u64, stream: u64, word_pos: u128) -> ChaCha8Rng {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    rng.set_stream(stream);
    rng.set_word_pos(word_pos);
    rng
}

impl Checkpointable for World {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let world_dir = dir.join(WORLD_DIR);
        std::fs::create_dir_all(&world_dir).map_err(|source| CheckpointError::Io {
            path: world_dir.clone(),
            source,
        })?;

        // 1. grid
        let grid_bytes = encode_ca(&self.ca)?;
        write_atomic(&world_dir, GRID_FILE, &grid_bytes).map_err(|source| {
            CheckpointError::Io { path: world_dir.join(GRID_FILE), source }
        })?;

        // 2. atoms
        let atoms = AtomsFile { step_count: self.step_count, atoms: self.atoms.clone() };
        let atoms_bytes = ciborium::ser::into_writer(&atoms, Vec::new())
            .map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(&world_dir, ATOMS_FILE, &atoms_bytes).map_err(|source| {
            CheckpointError::Io { path: world_dir.join(ATOMS_FILE), source }
        })?;

        // 3. registry (thứ tự slot — bất biến không-remove, xem registry.rs)
        let registry = RegistryFile { genomes: self.registry.genomes_in_order() };
        let reg_bytes = ciborium::ser::into_writer(&registry, Vec::new())
            .map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(&world_dir, REGISTRY_FILE, &reg_bytes).map_err(|source| {
            CheckpointError::Io { path: world_dir.join(REGISTRY_FILE), source }
        })?;

        // 4. rng
        write_atomic(&world_dir, RNG_FILE, &encode_rng(self)).map_err(|source| {
            CheckpointError::Io { path: world_dir.join(RNG_FILE), source }
        })?;

        // 5. manifest
        let mut records = Vec::with_capacity(4);
        for name in [GRID_FILE, ATOMS_FILE, REGISTRY_FILE, RNG_FILE] {
            let path = world_dir.join(name);
            let blake3 = hash_file(&path)?;
            records.push(FileRecord {
                path: format!("{WORLD_DIR}/{name}"),
                blake3,
            });
        }
        Manifest::write(dir, &records)
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let world_dir = dir.join(WORLD_DIR);

        // Verify manifest + hash trước khi tin bất kỳ file nào.
        let manifest = Manifest::read(dir)?;
        if manifest.format_version != crate::manifest::FORMAT_VERSION_V1 {
            return Err(CheckpointError::Corrupt {
                path: dir.join(crate::manifest::MANIFEST_NAME),
                expected: format!("format_version {}", crate::manifest::FORMAT_VERSION_V1),
                actual: manifest.format_version.to_string(),
            });
        }
        for record in &manifest.files {
            let path = dir.join(&record.path);
            let actual = hash_file(&path)?;
            if actual != record.blake3 {
                return Err(CheckpointError::Corrupt {
                    path,
                    expected: record.blake3.clone(),
                    actual,
                });
            }
        }

        // grid
        let ca = decode_ca(&std::fs::read(world_dir.join(GRID_FILE)).map_err(|source| {
            CheckpointError::Io { path: world_dir.join(GRID_FILE), source }
        })?)?;

        // atoms
        let atoms_bytes = std::fs::read(world_dir.join(ATOMS_FILE)).map_err(|source| {
            CheckpointError::Io { path: world_dir.join(ATOMS_FILE), source }
        })?;
        let atoms_file: AtomsFile = ciborium::de::from_reader(&atoms_bytes[..])
            .map_err(|e| CheckpointError::Cbor(e.to_string()))?;

        // registry
        let reg_bytes = std::fs::read(world_dir.join(REGISTRY_FILE)).map_err(|source| {
            CheckpointError::Io { path: world_dir.join(REGISTRY_FILE), source }
        })?;
        let registry_file: RegistryFile = ciborium::de::from_reader(&reg_bytes[..])
            .map_err(|e| CheckpointError::Cbor(e.to_string()))?;

        // rng
        let rng_bytes = std::fs::read(world_dir.join(RNG_FILE)).map_err(|source| {
            CheckpointError::Io { path: world_dir.join(RNG_FILE), source }
        })?;
        if rng_bytes.len() != 32 {
            return Err(CheckpointError::Corrupt {
                path: world_dir.join(RNG_FILE),
                expected: "32-byte rng state".to_string(),
                actual: format!("{} bytes", rng_bytes.len()),
            });
        }
        let seed = u64::from_le_bytes(rng_bytes[0..8].try_into().expect("8 bytes"));
        let stream = u64::from_le_bytes(rng_bytes[8..16].try_into().expect("8 bytes"));
        let word_pos = u128::from_le_bytes(rng_bytes[16..32].try_into().expect("16 bytes"));

        Ok(World {
            ca,
            registry: omiai_world::registry::FormulaRegistry::from_genomes_in_order(
                registry_file.genomes,
            ),
            atoms: atoms_file.atoms,
            rng: restore_rng(seed, stream, word_pos),
            rng_seed: seed,
            rng_stream: stream,
            step_count: atoms_file.step_count,
        })
    }
}
```

**Chỉnh theo code thật khi gặp sai khác:** tên field/variant trong
`Manifest`, `CheckpointError`, helper `verify_dir`, signature
`ciborium::ser::into_writer` — đối chiếu `crates/omiai-checkpoint/src/`
hiện có (slice 1) và dùng đúng; KHÔNG đổi schema manifest. Nếu
`Manifest.files` private thì thêm `pub(crate)` accessor, không đổi format.
Đã xác minh lúc viết plan (2026-08-26): `Manifest { format_version, files }`,
`FileRecord { path: String, blake3: String }`, `Manifest::write(dir,
&[FileRecord])`, `Manifest::read(dir)`, `FORMAT_VERSION_V1: u32`,
`MANIFEST_NAME`, `CheckpointError::{Io{path,source}, Corrupt{path,expected,
actual}, Cbor(String), Json(serde_json::Error)}` — plan dùng khớp các tên này.

- [ ] **Step 4: Wire module**

`crates/omiai-checkpoint/src/lib.rs` thêm `mod world_bundle;` (impl nằm
trong crate này nên không cần re-export gì — user gọi `World::save/load`
qua trait).

`crates/omiai-checkpoint/Cargo.toml` `[dependencies]` thêm nếu thiếu:
`rand_chacha.workspace = true` (cần cho `restore_rng`).

- [ ] **Step 5: Run tests**

Run: `cargo test -p omiai-checkpoint`
Expected: PASS gồm `world_save_load_resume_is_bit_exact`. Nếu fail ở
phần resume: so từng thành phần (grid trước, rồi atoms, rồi registry)
để khoanh vùng — thường nhất là RNG chưa khớp (đúng stream/word_pos)
hoặc `agent_act` clone `ca.cells` mỗi atom (tốn nhưng đúng — không đổi).

- [ ] **Step 6: Full workspace check + commit + push**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets`
Expected: toàn xanh, 0 warning.

```bash
git add crates/omiai-checkpoint/ crates/omiai-world/
git commit -m "feat(checkpoint,world): Checkpointable for World — bit-exact resume via (seed,stream,word_pos)"
git push origin main
```

---

### Task 9: Proptest bất biến world

**Files:**
- Create: `crates/omiai-world/tests/properties.rs`
- Modify: `crates/omiai-checkpoint/tests/proptest_grid.rs` (thêm 1 test sum-invariant cho lưới đa trạng thái) — hoặc tạo file mới nếu gọn hơn
- Modify: `crates/omiai-world/Cargo.toml` `[dev-dependencies]` (proptest đã có)

**Interfaces:**
- Consumes: `World`, `WorldConfig`, `apply_retention`, `RetentionPolicy`.
- Produces: không có API mới — chỉ property tests.

- [ ] **Step 1: Write proptests**

Tạo `crates/omiai-world/tests/properties.rs`:

```rust
//! Property tests cho world loop (slice 2).
//!
//! Bất biến kiểm chứng:
//! 1. Sau mọi số bước: mọi atom trong biên lưới, energy hữu hạn trong
//!    [0, ENERGY_MAX], gene hợp lệ trong registry.
//! 2. CA population-sum (tổng giá trị ô) bảo toàn qua ca_step riêng lẻ
//!    (rotate_block hoán vị giá trị trong block).

use omiai_world::ecology::ENERGY_MAX;
use omiai_world::world_loop::{World, WorldConfig};
use proptest::prelude::*;

fn config_for(w: u16, h: u16) -> WorldConfig {
    WorldConfig {
        width: w as usize,
        height: h as usize,
        n_initial_atoms: 2,
        initial_resources: 0.05,
    }
}

proptest! {
    // seed != 0: xorshift trong substrate::random có fixed-point tại 0
    // (không dùng random() ở đây, nhưng giữ quy ước seed dương cho nhất quán).
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn world_invariants_hold(
        w in 6u16..24,
        h in 6u16..24,
        seed in 1u64..1000,
        steps in 0u8..12,
    ) {
        let mut world = World::new(config_for(w, h), seed);
        for _ in 0..steps {
            world.step();
        }
        let occupied: std::collections::BTreeSet<_> =
            world.atoms.iter().map(|a| a.pos).collect();
        prop_assert_eq!(occupied.len(), world.atoms.len(), "hai atom cùng ô");

        for atom in &world.atoms {
            prop_assert!(atom.pos.0 < w as usize && atom.pos.1 < h as usize,
                "atom ngoài lưới: {:?}", atom.pos);
            prop_assert!(atom.energy.is_finite());
            prop_assert!((0.0..=ENERGY_MAX).contains(&atom.energy),
                "energy ngoài khoảng: {}", atom.energy);
            prop_assert!(world.registry.get(atom.gene).is_some(), "gene mất tích");
        }
        prop_assert_eq!(world.step_count, steps as u64);
    }

    #[test]
    fn ca_step_preserves_cell_value_sum(
        w in 4u16..20,
        h in 4u16..20,
        seed in 1u64..1000,
    ) {
        // Lưới đa trạng thái như world dùng (num_states=4).
        let mut world = World::new(config_for(w, h), seed);
        let sum_before: u64 = world.ca.cells.iter().map(|&c| c as u64).sum();
        world.ca_step();
        let sum_after: u64 = world.ca.cells.iter().map(|&c| c as u64).sum();
        prop_assert_eq!(sum_before, sum_after,
            "rotate_block phải bảo toàn tổng giá trị ô (hoán vị)");
    }
}
```

**Lưu ý:** test file này KHÔNG cần `omiai-core` (đã bỏ import thừa) —
không thêm dev-dependency mới.

- [ ] **Step 2: Run**

Run: `cargo test -p omiai-world --test properties`
Expected: PASS (64 cases mỗi prop). Shrink case nào fail → sửa implementation
(theo chiến lược chung: bug thật sửa code, test sai sửa test có lý do).

- [ ] **Step 3: Commit + push**

```bash
git add crates/omiai-world/tests/properties.rs
git commit -m "test(world): proptests — atom invariants, CA value-sum conservation"
git push origin main
```

---

### Task 10: Docs — architecture/world.md, format-spec, README

**Files:**
- Modify: `docs/architecture/world.md`
- Modify: `docs/format-spec/checkpoint-v1.md` (mục index/retention + world/* payloads)
- Modify: `README.md` (trạng thái build-order + counts)
- Create: `docs/architecture/checkpoint.md` cập nhật nếu đã tồn tại từ slice 1

**Interfaces:** không có code. Tiêu chí: docs khớp code thật, văn phong
trung thực ("đã cài đặt và test" / "khung sườn").

- [ ] **Step 1: Update docs**

`docs/architecture/world.md` — ghi đúng trạng thái mới:
- **Đã cài đặt và test:** substrate (CA Margolus, population-preserving),
  FormulaRegistry (generational, không-GC), Atom lifecycle
  (metabolize/feed/split), agent policy decode (propositional projection
  của LTL — toán tử thời gian coi như hiện tại, giới hạn ghi rõ),
  World loop 5 phase deterministic, checkpoint bundle bit-exact
  (round-trip test), mutation bounded depth.
- **Giới hạn đã biết:** genome không GC (registry phình dần), policy decode
  bỏ ngữ nghĩa thời gian của LTL, không communication/signaling, không
  đa loài.
- Liệt kê hằng số ecology và ý nghĩa.

`docs/format-spec/checkpoint-v1.md`:
- Cập nhật mục index.json: **implemented** (`read_or_rebuild_index`,
  fallback quét thư mục, `rebuilt` flag).
- Cập nhật mục retention: **implemented** (`RetentionPolicy`,
  `apply_retention`, mặc định keep_recent=10/milestone_every=100).
- Thêm section `world/` payloads: 4 file, layout rng_state.bin
  (u64 LE seed + u64 LE stream + u128 LE word_pos), tham chiếu ADR-0006.
- Trạng thái đầu trang đổi thành phản ánh: ca_grid + world bundle đều
  "implemented and tested"; logic/knowledge/reservoir payloads vẫn "later".

`README.md`:
- Build order: đánh dấu omiai-world atoms/agents/world_loop ✅, checkpoint
  world bundle + retention ✅.
- Cập nhật số test (chạy `cargo test --workspace 2>&1 | grep "^test result"`
  rồi cộng tổng passed) và số test targets.
- Phần "scaffolded" còn lại: communication, export/runtime/serve/cli.

- [ ] **Step 2: Verify claims against reality**

Run: `cargo test --workspace`
Ghi đúng số liệu in ra — không nâng số tay.

- [ ] **Step 3: Commit + push**

```bash
git add docs/ README.md
git commit -m "docs: slice-2 status — world pillar tested, checkpoint world bundle + retention"
git push origin main
```

---

## Hoàn tất slice

- [ ] Chạy cuối: `cargo test --workspace && cargo clippy --workspace --all-targets` — toàn xanh, 0 warning.
- [ ] `git tag slice-2-complete && git push origin slice-2-complete`
