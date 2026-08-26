# Slice 3 — Ngôn ngữ nổi sinh (Lewis signaling) trong `omiai-world` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Trong repo này subagent dispatch KHÔNG dùng được** (alias model → 400 "Ambiguous model", agent con chết 402). Chạy inline bằng `superpowers:executing-plans`.

**Goal:** atom phát tín hiệu tuỳ tiện từ gene LTL của chính nó, atom kề nghe được và hành động được theo tín hiệu, và toàn bộ được đo bằng mutual information giữa ký hiệu và hướng tài nguyên — cơ chế + thước đo, không hứa hội tụ.

**Architecture:** thêm module `omiai-world/src/communication.rs` (bảng ký hiệu, neighbourhood valuation 16 mệnh đề, giải mã voice, `Vocabulary` + MI). `Atom` mọc thêm `voice: Vec<FormulaId>` — K arm formula, mỗi arm là một genome trong `FormulaRegistry` đã có. World loop lên 6 phase: `speak` chèn giữa `metabolism` và `agent_act`, ghi một `airwave` đóng băng mà mọi receiver cùng đọc. Checkpoint minor-bump lên `format_version = 1_001` với payload mới `communication/vocabulary.cbor`; checkpoint version 1 vẫn load được (atom câm).

**Tech Stack:** Rust 2021 workspace, `rand_chacha::ChaCha8Rng` (ADR-0006), `generational_arena` (registry), `ciborium` (CBOR), `serde_json` (manifest), `blake3` (hash), `proptest`, `criterion` (không dùng ở lát cắt này).

**Spec:** [`docs/superpowers/specs/2026-08-26-world-communication-slice3-design.md`](../specs/2026-08-26-world-communication-slice3-design.md) — đọc cả hai; plan này lập luận từ spec đó.

## Global Constraints

- **Git**: làm trực tiếp trên `main`, không nhánh feature. Commit + push sau **mỗi** task (mandate của user). Commit message kết thúc bằng `Co-Authored-By: Claude <noreply@anthropic.com>`.
- **Ngôn ngữ**: doc comment và test message trong `omiai-world` / `omiai-checkpoint` viết tiếng Việt (theo code hiện có của slice 2); README + format-spec + ADR viết tiếng Anh (theo code hiện có).
- **TDD**: test thất bại trước, rồi mới cài. Không tuyên bố "xong" trước khi `cargo test --workspace` xanh.
- **Cổng chất lượng cuối mỗi task**: `cargo test --workspace` xanh **và** `cargo clippy --workspace --all-targets` 0 cảnh báo.
- Hằng số cố định, copy nguyên văn từ spec: `N_SYMBOLS = 4`, `N_SIGNAL_VALUES = 5`, `N_STATE_CLASSES = 5`, trần MI = `log₂5 ≈ 2.322` bit, `format_version` mới = `1_001`, `MUTATION_PROB = 0.3` (không tinh chỉnh hằng số sinh thái ở lát cắt này).
- **Thứ tự tiêu thụ RNG là hợp đồng**: `World::new` rải tài nguyên → rồi mỗi atom mồi lấy voice; mỗi lần sinh sản rút gene di chuyển **trước**, voice **sau**. Đổi thứ tự là đổi mọi quỹ đạo.
- **`speak` KHÔNG được tiêu thụ RNG.** Bit-exact resume phụ thuộc điều này.
- **Voice arm KHÔNG đọc `hear*`** (spec §2.4). Vi phạm là làm ký hiệu phụ thuộc thứ tự `Vec`.
- **`airwave` là trạng thái phái sinh**: ghi một lần trong `speak`, chỉ-đọc suốt bước, KHÔNG lưu checkpoint.
- YAGNI: không knowledge promotion, không đa loài, không ô nguy hiểm, không benchmark, không chạm `export`/`runtime`/`serve`.

## File Structure

**Tạo mới**

| file | trách nhiệm |
|---|---|
| `crates/omiai-world/src/communication.rs` | Bảng ký hiệu (`Symbol`, `SignalValue`, `StateClass`), 16 tên mệnh đề có hướng, `neighbourhood_valuation`, `decode_voice`, `state_class`, `Vocabulary` + mutual information. Thuần tuý: không biết `World`, không rút RNG. |
| `crates/omiai-world/tests/communication.rs` | Integration test **cơ chế**: dân số dựng tay có quy ước đúng vs dân số câm. |
| `crates/omiai-world/examples/communication_demo.rs` | Demo master spec yêu cầu: chạy N bước, in MI + tần suất ký hiệu + dân số. Thư mục `examples/` chưa tồn tại — tạo mới. |
| `crates/omiai-world/tests/demo_smoke.rs` | Chốt cấu hình demo mà README trích số: chạy được, số đo trong biên, deterministic. |
| `crates/omiai-checkpoint/tests/version_gate.rs` | Cổng version từ ngoài crate: chấp nhận `1` và `1_001`, từ chối phần còn lại. |
| `docs/adr/0007-signal-channel-one-step.md` | ADR cho kênh 1 bước (người nói ghi ô của chính nó, tầm với 1 ô đến từ phía nhận) / im lặng là giá trị thứ năm / voice là formula chứ không phải bảng tra / voice ⊥ hear và giá của nó / giới hạn receiver không bộ nhớ. |

**Sửa**

| file | thay đổi |
|---|---|
| `crates/omiai-world/src/lib.rs` | `pub mod communication;` + cập nhật doc comment (communication không còn là "later slice"). |
| `crates/omiai-world/src/atoms.rs` | `Atom.voice: Vec<FormulaId>` với `#[serde(default)]`. |
| `crates/omiai-world/src/agents.rs` | `Observation.heard`, `observe_with`, `observe_surroundings(+heard)`, `hear_flags`, `valuation_with_hear`, `decide_with_hear`, `MOVEMENT_ATOM_NAMES` (8 tên). |
| `crates/omiai-world/src/world_loop.rs` | `World.airwave`, `World.vocabulary`, phase `speak`, `step` 6 phase, voice trong `World::new`, `mutate_formula_with`, `random_voice`, di truyền voice trong `reproduce_and_evolve`, `seed_voices`. |
| `crates/omiai-world/tests/properties.rs` | proptest MI trong biên + bất biến `total` + airwave hợp lệ. |
| `crates/omiai-checkpoint/src/manifest.rs` | `FORMAT_VERSION_CURRENT = 1_001`, `is_supported_version`, `write` phát version mới. |
| `crates/omiai-checkpoint/src/lib.rs` | `verify_dir` dùng `is_supported_version`. |
| `crates/omiai-checkpoint/src/ca_grid.rs` | dùng `is_supported_version`. |
| `crates/omiai-checkpoint/src/world_bundle.rs` | payload `communication/vocabulary.cbor`, kiểm tham chiếu voice, khởi tạo `airwave`, unit test nạp checkpoint v1. |
| `crates/omiai-world/Cargo.toml` | thêm `ciborium` vào `[dev-dependencies]` (test tương thích ngược dựng bản ghi CBOR slice 2 bằng tay). `omiai-checkpoint` đã có sẵn `ciborium` + `serde_json` ở `[dependencies]` nên **không cần sửa** Cargo.toml của nó. |
| `crates/omiai-checkpoint/tests/world_roundtrip.rs` | bit-exact resume khi signaling bật (so cả `vocabulary` và `word_pos`), world câm resume vẫn im, manifest ghi đúng version + 5 payload. |
| `docs/format-spec/checkpoint-v1.md` | §2 bảng version, §5c payload mới, §6 phạm vi của tuyên bố bit-exact. |
| `README.md` | sửa thứ tự build (runtime KHÔNG phải bước 5), chuyển communication từ "scaffolded" sang "implemented", số test mới, số MI đo thật. |

**Vì sao chia thế này:** `communication.rs` thuần tuý nên test được không cần `World` — MI là thước đo, sai thầm lặng ở đây làm hỏng mọi kết luận của lát cắt, nên nó phải kiểm được bằng bảng dựng tay có đáp số chính xác. Phần dính RNG và thứ tự phase ở lại `world_loop.rs` vì đó là nơi hợp đồng determinism sống.

---

### Task 1: `Vocabulary` + mutual information (thuần tuý, có đáp số chính xác)

**Files:**
- Create: `crates/omiai-world/src/communication.rs`
- Modify: `crates/omiai-world/src/lib.rs`

**Interfaces:**
- Consumes: không gì (module thuần tuý, chỉ `serde`).
- Produces: `pub type Symbol = u8`; `pub const N_SYMBOLS: usize = 4`; `pub const N_SIGNAL_VALUES: usize = 5`; `pub const N_STATE_CLASSES: usize = 5`; `pub enum SignalValue { Silent, Sym(Symbol) }` với `fn row(self) -> usize`; `pub enum StateClass { North, East, South, West, None }` với `fn col(self) -> usize`; `pub struct Vocabulary { pub joint: [[u64; 5]; 5], pub total: u64 }` với `record(&mut self, SignalValue, StateClass)`, `mutual_information(&self) -> f64`, `entropy_signal(&self) -> f64`, `entropy_state(&self) -> f64`, `symbol_frequency(&self, Symbol) -> f64`, `impl Default`.

- [ ] **Step 1: Viết test thất bại trước**

Tạo `crates/omiai-world/src/communication.rs` **chỉ với phần test** (chưa có code), để bước 2 thấy lỗi biên dịch đúng chỗ:

```rust
//! Ngôn ngữ nổi sinh: bảng ký hiệu, giải mã voice gene, thước đo MI.

#[cfg(test)]
mod tests {
    use super::*;

    /// log₂5 — trần MI khi 5 giá trị tín hiệu song ánh với 5 lớp trạng thái.
    const LOG2_5: f64 = 2.321928094887362;

    fn vocab_from(joint: [[u64; N_STATE_CLASSES]; N_SIGNAL_VALUES]) -> Vocabulary {
        let total = joint.iter().flatten().sum();
        Vocabulary { joint, total }
    }

    #[test]
    fn empty_vocabulary_has_zero_mi() {
        let v = Vocabulary::default();
        assert_eq!(v.total, 0);
        assert_eq!(v.mutual_information(), 0.0);
        assert_eq!(v.entropy_signal(), 0.0);
        assert_eq!(v.entropy_state(), 0.0);
    }

    #[test]
    fn record_counts_row_and_column() {
        let mut v = Vocabulary::default();
        v.record(SignalValue::Silent, StateClass::None);
        v.record(SignalValue::Sym(2), StateClass::South);
        assert_eq!(v.total, 2);
        assert_eq!(v.joint[0][4], 1, "Silent là hàng 0, None là cột 4");
        assert_eq!(v.joint[3][2], 1, "Sym(2) là hàng 3, South là cột 2");
    }

    #[test]
    fn perfect_bijection_reaches_log2_5() {
        // Mỗi giá trị tín hiệu ứng đúng một lớp trạng thái, tần suất đều.
        let mut joint = [[0u64; N_STATE_CLASSES]; N_SIGNAL_VALUES];
        for s in 0..N_SIGNAL_VALUES {
            joint[s][s] = 10;
        }
        let v = vocab_from(joint);
        assert!(
            (v.mutual_information() - LOG2_5).abs() < 1e-12,
            "MI = {}, cần log₂5",
            v.mutual_information()
        );
        assert!((v.entropy_signal() - LOG2_5).abs() < 1e-12);
        assert!((v.entropy_state() - LOG2_5).abs() < 1e-12);
    }

    #[test]
    fn exactly_independent_table_has_zero_mi() {
        // joint[s][m] = w[s] * w[m] ⇒ p(s,m) = p(s)p(m) đúng tuyệt đối.
        let w = [1u64, 2, 3, 4, 5];
        let mut joint = [[0u64; N_STATE_CLASSES]; N_SIGNAL_VALUES];
        for s in 0..N_SIGNAL_VALUES {
            for m in 0..N_STATE_CLASSES {
                joint[s][m] = w[s] * w[m];
            }
        }
        let v = vocab_from(joint);
        assert!(
            v.mutual_information().abs() < 1e-12,
            "bảng độc lập phải cho MI = 0, nhận {}",
            v.mutual_information()
        );
        // Nhưng entropy hai phía đều dương — MI = 0 không phải vì bảng trống.
        assert!(v.entropy_signal() > 2.0);
        assert!(v.entropy_state() > 2.0);
    }

    #[test]
    fn single_symbol_says_nothing() {
        // Luôn phát Sym(0) bất kể trạng thái ⇒ H(S) = 0 ⇒ MI = 0.
        let mut joint = [[0u64; N_STATE_CLASSES]; N_SIGNAL_VALUES];
        joint[1] = [7, 3, 11, 5, 2];
        let v = vocab_from(joint);
        assert!(v.mutual_information().abs() < 1e-12);
        assert_eq!(v.entropy_signal(), 0.0);
        assert!(v.entropy_state() > 0.0);
    }

    #[test]
    fn partial_convention_lies_strictly_between() {
        // Sym(0) dùng cho hai lớp, Sym(1) riêng một lớp ⇒ 0 < MI < log₂5.
        let mut joint = [[0u64; N_STATE_CLASSES]; N_SIGNAL_VALUES];
        joint[1][0] = 10;
        joint[1][1] = 10;
        joint[2][2] = 10;
        let v = vocab_from(joint);
        let mi = v.mutual_information();
        assert!(mi > 1e-9, "phải mang tin, nhận {mi}");
        assert!(mi < LOG2_5 - 1e-9, "không thể đạt trần, nhận {mi}");
        // Chặn trên lý thuyết: MI ≤ min(H(S), H(M)).
        assert!(mi <= v.entropy_signal().min(v.entropy_state()) + 1e-12);
    }

    #[test]
    fn symbol_frequency_reads_rows() {
        let mut v = Vocabulary::default();
        for _ in 0..3 {
            v.record(SignalValue::Sym(1), StateClass::East);
        }
        v.record(SignalValue::Silent, StateClass::None);
        assert!((v.symbol_frequency(1) - 0.75).abs() < 1e-12);
        assert_eq!(v.symbol_frequency(3), 0.0);
    }
}
```

Nối module vào crate — sửa `crates/omiai-world/src/lib.rs`:

```rust
pub mod agents;
pub mod atoms;
pub mod communication;
pub mod ecology;
pub mod registry;
pub mod substrate;
pub mod world_loop;
```

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world --lib communication`
Expected: FAIL — `cannot find type Vocabulary in this scope` (và tương tự cho `SignalValue`, `StateClass`, `N_SIGNAL_VALUES`, `N_STATE_CLASSES`).

- [ ] **Step 3: Cài phần đủ để test xanh**

Chèn vào **đầu** `crates/omiai-world/src/communication.rs`, ngay dưới doc comment dòng đầu:

```rust
//!
//! Module này **thuần tuý**: không biết `World`, không rút RNG, không I/O.
//! Nhờ vậy thước đo MI kiểm được bằng bảng dựng tay có đáp số chính xác —
//! điều kiện để mọi kết luận của lát cắt 3 đáng tin.

use serde::{Deserialize, Serialize};

/// Ký hiệu phát được: 0..N_SYMBOLS-1.
pub type Symbol = u8;

/// Số arm của voice gene = số ký hiệu phát được.
pub const N_SYMBOLS: usize = 4;

/// Giá trị tín hiệu quan sát được = N_SYMBOLS ký hiệu + im lặng.
pub const N_SIGNAL_VALUES: usize = N_SYMBOLS + 1;

/// Số lớp trạng thái thế giới: 4 hướng + "không có tài nguyên kề".
pub const N_STATE_CLASSES: usize = 5;

/// Giá trị tín hiệu. **Im lặng LÀ một giá trị**, không phải dữ liệu thiếu:
/// nếu im lặng bị bỏ khỏi bảng đếm thì trần MI bị chặn dưới log₂5 vì lý do
/// cấu trúc, và mọi phép đo đọc ra như "hội tụ thất bại" dù cơ chế đúng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalValue {
    Silent,
    Sym(Symbol),
}

impl SignalValue {
    /// Hàng trong bảng joint: 0 = Silent, k+1 = Sym(k).
    pub fn row(self) -> usize {
        match self {
            SignalValue::Silent => 0,
            SignalValue::Sym(k) => k as usize + 1,
        }
    }
}

/// Lớp trạng thái mà tín hiệu nói về: hướng ô tài nguyên KỀ sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateClass {
    North,
    East,
    South,
    West,
    None,
}

impl StateClass {
    /// Cột trong bảng joint, theo đúng thứ tự khai báo N,E,S,W,None.
    pub fn col(self) -> usize {
        match self {
            StateClass::North => 0,
            StateClass::East => 1,
            StateClass::South => 2,
            StateClass::West => 3,
            StateClass::None => 4,
        }
    }
}

/// Bảng đếm đồng thời (ký hiệu, lớp trạng thái) — tích luỹ toàn run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vocabulary {
    /// `joint[s][m]`: số lần giá trị tín hiệu hàng `s` xuất hiện cùng lớp
    /// trạng thái cột `m`. Hàng 0 = Silent, hàng k+1 = Sym(k).
    pub joint: [[u64; N_STATE_CLASSES]; N_SIGNAL_VALUES],
    pub total: u64,
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self {
            joint: [[0; N_STATE_CLASSES]; N_SIGNAL_VALUES],
            total: 0,
        }
    }
}

/// Entropy Shannon (bit) của một phân bố cho bằng số đếm.
fn entropy_of(counts: impl IntoIterator<Item = u64>, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let t = total as f64;
    let mut h = 0.0;
    for c in counts {
        if c == 0 {
            continue;
        }
        let p = c as f64 / t;
        h -= p * p.log2();
    }
    h
}

impl Vocabulary {
    /// Ghi một quan sát. Gọi đúng một lần cho mỗi (atom còn sống, bước) —
    /// kể cả atom câm, vốn ghi `Silent`.
    pub fn record(&mut self, signal: SignalValue, state: StateClass) {
        self.joint[signal.row()][state.col()] += 1;
        self.total += 1;
    }

    fn row_sums(&self) -> [u64; N_SIGNAL_VALUES] {
        let mut r = [0; N_SIGNAL_VALUES];
        for s in 0..N_SIGNAL_VALUES {
            r[s] = self.joint[s].iter().sum();
        }
        r
    }

    fn col_sums(&self) -> [u64; N_STATE_CLASSES] {
        let mut c = [0; N_STATE_CLASSES];
        for m in 0..N_STATE_CLASSES {
            c[m] = (0..N_SIGNAL_VALUES).map(|s| self.joint[s][m]).sum();
        }
        c
    }

    /// I(S;M) = Σ p(s,m) · log₂( p(s,m) / (p(s)·p(m)) ), bỏ qua ô đếm 0.
    ///
    /// KHÔNG kẹp về 0: số đo được báo đúng như tính ra. Sai số dấu phẩy động
    /// có thể cho ra ~-1e-16 với bảng độc lập; ai so sánh thì dùng dung sai.
    pub fn mutual_information(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let t = self.total as f64;
        let rows = self.row_sums();
        let cols = self.col_sums();
        let mut mi = 0.0;
        for s in 0..N_SIGNAL_VALUES {
            for m in 0..N_STATE_CLASSES {
                let c = self.joint[s][m];
                if c == 0 {
                    continue;
                }
                let c = c as f64;
                mi += (c / t) * ((c * t) / (rows[s] as f64 * cols[m] as f64)).log2();
            }
        }
        mi
    }

    /// H(S) — entropy của phía tín hiệu (5 giá trị).
    pub fn entropy_signal(&self) -> f64 {
        entropy_of(self.row_sums(), self.total)
    }

    /// H(M) — entropy của phía trạng thái (5 lớp).
    pub fn entropy_state(&self) -> f64 {
        entropy_of(self.col_sums(), self.total)
    }

    /// Tần suất ký hiệu `sym` trên tổng số quan sát (im lặng không tính là
    /// ký hiệu; đọc `joint[0]` nếu cần tần suất im lặng).
    pub fn symbol_frequency(&self, sym: Symbol) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let row = SignalValue::Sym(sym).row();
        let count: u64 = self.joint[row].iter().sum();
        count as f64 / self.total as f64
    }
}
```

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-world --lib communication`
Expected: PASS — 7 test.

Run: `cargo clippy -p omiai-world --all-targets`
Expected: 0 cảnh báo. (Nếu clippy đòi `needless_range_loop` ở `row_sums`/`col_sums`, dùng `for (s, row) in self.joint.iter().enumerate()` thay vì nới lỏng lint.)

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/src/communication.rs crates/omiai-world/src/lib.rs
git commit -m "$(cat <<'MSG'
feat(world): Vocabulary + mutual information — thước đo ngôn ngữ nổi sinh

Im lặng là giá trị tín hiệu thứ năm, không phải dữ liệu thiếu: nhờ vậy
trần MI = log₂5 đạt tới được và một phép đo thấp đọc đúng là "chưa hội tụ"
chứ không phải "bị chặn vì cấu trúc bảng".

MI không kẹp về 0 — số đo báo đúng như tính ra. Test dùng bảng dựng tay có
đáp số chính xác: song ánh → log₂5, bảng dạng tích → 0 tuyệt đối, một ký
hiệu duy nhất → 0, hội tụ một phần → nằm hẳn giữa.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 2: 16 mệnh đề có hướng, `state_class`, `decode_voice`

**Files:**
- Modify: `crates/omiai-world/src/communication.rs`

**Interfaces:**
- Consumes: `crate::agents::{Direction, Observation, eval_current}` (slice 2); `crate::registry::{FormulaId, FormulaRegistry}`.
- Produces: `pub const VOICE_ATOM_NAMES: [&str; 16]`; `pub fn dir_suffix(Direction) -> &'static str`; `pub fn neighbourhood_valuation(&[(Direction, Observation)]) -> BTreeMap<String, bool>`; `pub fn state_class(&[(Direction, Observation)]) -> StateClass`; `pub fn decode_voice(&[FormulaId], &FormulaRegistry, &BTreeMap<String, bool>) -> SignalValue`; `pub fn voice_seed_formula() -> LtlFormula`.

Ghi chú thiết kế trước khi viết code: `state_class` nhận **đúng cái mảng quan sát mà sender dùng để phát**. Đó là cách rẻ nhất để bảo đảm nguyên tắc spec §4 — `M` là hàm của đúng cái sender quan sát được — và nó cho không cả 4 luật: bán kính 1 (mảng chỉ có 4 ô kề), thắng theo thứ tự (mảng theo N,E,S,W), giá trị ô không phân định (`Observation.res` là bool), tài nguyên trên ô bị chiếm vẫn tính (`observe` đặt `res` và `occupied` độc lập).

- [ ] **Step 1: Viết test thất bại trước**

Thêm vào `mod tests` của `crates/omiai-world/src/communication.rs`:

```rust
    use crate::agents::{observe, Direction};
    use crate::registry::{FormulaRegistry, Genome};
    use omiai_core::ltl::LtlFormula;

    /// Bốn quan sát theo thứ tự N,E,S,W từ giá trị ô + tình trạng chiếm.
    fn obs4(cells: [(u8, bool); 4]) -> Vec<(Direction, Observation)> {
        crate::agents::ALL_DIRECTIONS
            .iter()
            .zip(cells)
            .map(|(&d, (v, occ))| (d, observe(v, occ)))
            .collect()
    }

    #[test]
    fn valuation_has_all_sixteen_directional_names() {
        let val = neighbourhood_valuation(&obs4([(0, false), (3, false), (1, false), (0, true)]));
        assert_eq!(val.len(), 16, "phải đủ 4 mệnh đề × 4 hướng");
        for name in VOICE_ATOM_NAMES {
            assert!(val.contains_key(name), "thiếu mệnh đề {name}");
        }
        assert!(val["open_n"]);
        assert!(val["res_e"]);
        assert!(val["wall_s"]);
        assert!(val["occupied_w"]);
        assert!(!val["res_n"]);
    }

    #[test]
    fn state_class_scans_nesw_in_order() {
        // Tài nguyên ở cả E và S → E thắng vì quét trước.
        let obs = obs4([(0, false), (2, false), (3, false), (0, false)]);
        assert_eq!(state_class(&obs), StateClass::East);
        // Không có tài nguyên kề → None.
        let obs = obs4([(0, false), (0, false), (1, false), (0, true)]);
        assert_eq!(state_class(&obs), StateClass::None);
    }

    #[test]
    fn state_class_ignores_resource_value() {
        // N giá trị 2 thắng E giá trị 3: thứ tự quyết định, không phải độ giàu.
        let obs = obs4([(2, false), (3, false), (0, false), (0, false)]);
        assert_eq!(state_class(&obs), StateClass::North);
    }

    #[test]
    fn state_class_counts_resource_under_another_atom() {
        // ca_step (Margolus) đẩy được tài nguyên vào ô đang bị chiếm; ô đó
        // vẫn là tài nguyên. `res` và `occupied` là hai mệnh đề độc lập.
        let obs = obs4([(0, false), (2, true), (0, false), (0, false)]);
        assert_eq!(state_class(&obs), StateClass::East);
        let val = neighbourhood_valuation(&obs);
        assert!(val["res_e"] && val["occupied_e"]);
    }

    #[test]
    fn state_class_radius_is_exactly_one() {
        // 4 ô kề trống ⇒ None, bất kể có gì cách hai ô: mảng quan sát chỉ
        // chứa 4 ô kề nên "xa hơn" về mặt cấu trúc không vào được.
        let obs = obs4([(0, false), (0, false), (0, false), (0, false)]);
        assert_eq!(state_class(&obs), StateClass::None);
    }

    #[test]
    fn decode_voice_picks_first_satisfied_arm() {
        let mut reg = FormulaRegistry::new();
        let arms: Vec<_> = ["res_n", "res_e", "res_s", "res_w"]
            .iter()
            .map(|n| {
                reg.insert(Genome { formula: LtlFormula::atom(*n), fitness: None })
            })
            .collect();
        // Tài nguyên ở E và S → arm 1 (res_e) là arm đầu tiên thoả.
        let val = neighbourhood_valuation(&obs4([
            (0, false),
            (2, false),
            (2, false),
            (0, false),
        ]));
        assert_eq!(decode_voice(&arms, &reg, &val), SignalValue::Sym(1));
    }

    #[test]
    fn decode_voice_silent_when_no_arm_fires_or_atom_is_mute() {
        let mut reg = FormulaRegistry::new();
        let arms: Vec<_> = (0..N_SYMBOLS)
            .map(|_| {
                reg.insert(Genome { formula: LtlFormula::atom("res_n"), fitness: None })
            })
            .collect();
        let val = neighbourhood_valuation(&obs4([
            (0, false),
            (0, false),
            (0, false),
            (0, false),
        ]));
        assert_eq!(decode_voice(&arms, &reg, &val), SignalValue::Silent);
        // Atom câm: không có arm nào ⇒ luôn im lặng.
        assert_eq!(decode_voice(&[], &reg, &val), SignalValue::Silent);
    }

    #[test]
    fn voice_seed_formula_is_expressible_in_the_voice_pool() {
        // Hạt giống phải dùng tên trong pool voice, không phải pool di chuyển:
        // dùng "res" thay vì "res_n" thì mọi arm khởi tạo đánh giá false và
        // dân số im lặng vĩnh viễn mà không lỗi nào nổi lên.
        let f = voice_seed_formula();
        let printed = format!("{f:?}");
        assert!(printed.contains("res_n"), "hạt giống phải nói tên có hướng: {printed}");
        let val = neighbourhood_valuation(&obs4([
            (2, false),
            (0, false),
            (0, false),
            (0, false),
        ]));
        assert!(crate::agents::eval_current(&f, &val), "hạt giống phải bắn được");
    }
```

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world --lib communication`
Expected: FAIL — `cannot find function neighbourhood_valuation` / `state_class` / `decode_voice` / `voice_seed_formula`, `cannot find value VOICE_ATOM_NAMES`.

- [ ] **Step 3: Cài**

Thêm vào phần `use` ở đầu `communication.rs`:

```rust
use std::collections::BTreeMap;

use omiai_core::ltl::LtlFormula;
use serde::{Deserialize, Serialize};

use crate::agents::{eval_current, Direction, Observation};
use crate::registry::{FormulaId, FormulaRegistry};
```

Thêm vào cuối phần code (trước `#[cfg(test)] mod tests`):

```rust
/// Hậu tố tên mệnh đề của một hướng.
pub fn dir_suffix(dir: Direction) -> &'static str {
    match dir {
        Direction::North => "n",
        Direction::East => "e",
        Direction::South => "s",
        Direction::West => "w",
    }
}

/// Pool tên mệnh đề của voice arm: {open,wall,res,occupied} × {n,e,s,w}.
///
/// Không có mệnh đề chỉ hướng thì một arm **về mặt vật lý không thể** diễn
/// đạt "thức ăn ở phía Đông", và thước đo MI ở mục 4 spec mất nghĩa ngay từ
/// đầu. Đây cũng là pool đột biến của voice (spec §2.2).
pub const VOICE_ATOM_NAMES: [&str; 16] = [
    "open_n", "wall_n", "res_n", "occupied_n", //
    "open_e", "wall_e", "res_e", "occupied_e", //
    "open_s", "wall_s", "res_s", "occupied_s", //
    "open_w", "wall_w", "res_w", "occupied_w",
];

/// Valuation 16 mệnh đề của cả vùng lân cận — miền đánh giá của voice arm.
///
/// KHÔNG chứa `hear*`: voice không được phụ thuộc cái nghe được, xem spec
/// §2.4 (mọi atom phát cùng lúc nên đọc airwave đang-ghi-dở sẽ làm ký hiệu
/// phụ thuộc thứ tự `Vec`).
pub fn neighbourhood_valuation(
    obs_by_dir: &[(Direction, Observation)],
) -> BTreeMap<String, bool> {
    let mut val = BTreeMap::new();
    for (dir, obs) in obs_by_dir {
        let s = dir_suffix(*dir);
        val.insert(format!("open_{s}"), obs.open);
        val.insert(format!("wall_{s}"), obs.wall);
        val.insert(format!("res_{s}"), obs.res);
        val.insert(format!("occupied_{s}"), obs.occupied);
    }
    val
}

/// Lớp trạng thái: hướng ô tài nguyên kề, quét theo đúng thứ tự của
/// `obs_by_dir` (world loop luôn truyền theo `ALL_DIRECTIONS` = N,E,S,W).
///
/// Bốn luật của spec §4 đến miễn phí từ việc nhận **đúng mảng quan sát mà
/// sender dùng để phát**: bán kính đúng 1, thắng theo thứ tự chứ không theo
/// giá trị ô, và tài nguyên dưới chân atom khác vẫn là tài nguyên.
pub fn state_class(obs_by_dir: &[(Direction, Observation)]) -> StateClass {
    for (dir, obs) in obs_by_dir {
        if obs.res {
            return match dir {
                Direction::North => StateClass::North,
                Direction::East => StateClass::East,
                Direction::South => StateClass::South,
                Direction::West => StateClass::West,
            };
        }
    }
    StateClass::None
}

/// Ký hiệu phát ra = index của arm đầu tiên đánh giá `true`; không arm nào
/// thoả → `Silent`. Atom câm (`voice` rỗng) luôn `Silent`.
///
/// Arm trỏ vào slot không có trong registry bị **bỏ qua** thay vì panic:
/// `World::load` đã chặn trường hợp đó thành `Corrupt`, nên tới được đây là
/// bug ở nơi khác và giết cả simulation không giúp gì.
pub fn decode_voice(
    voice: &[FormulaId],
    registry: &FormulaRegistry,
    val: &BTreeMap<String, bool>,
) -> SignalValue {
    debug_assert!(
        voice.is_empty() || voice.len() == N_SYMBOLS,
        "voice phải rỗng hoặc đúng N_SYMBOLS arm, nhận {}",
        voice.len()
    );
    for (k, id) in voice.iter().enumerate() {
        let Some(genome) = registry.get(*id) else {
            continue;
        };
        if eval_current(&genome.formula, val) {
            return SignalValue::Sym(k as Symbol);
        }
    }
    SignalValue::Silent
}

/// Hạt giống để đột biến ra voice khởi tạo. Phải dùng tên trong
/// [`VOICE_ATOM_NAMES`] — dùng tên pool di chuyển (`res`) thì mọi arm đánh
/// giá false và dân số im lặng vĩnh viễn mà không lỗi nào nổi lên.
pub fn voice_seed_formula() -> LtlFormula {
    LtlFormula::or(LtlFormula::atom("res_n"), LtlFormula::atom("open_n"))
}
```

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-world --lib communication`
Expected: PASS — 15 test.

Run: `cargo clippy -p omiai-world --all-targets`
Expected: 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/src/communication.rs
git commit -m "$(cat <<'MSG'
feat(world): 16 mệnh đề có hướng, state_class, decode_voice

state_class nhận đúng mảng quan sát mà sender dùng để phát. Nhờ vậy cả bốn
luật spec §4 đến miễn phí: bán kính đúng 1, thắng theo thứ tự N,E,S,W chứ
không theo giá trị ô, tài nguyên dưới chân atom khác vẫn tính. Nguyên tắc:
biến trạng thái phải là hàm của đúng cái sender quan sát được, nếu không MI
bị kéo xuống vì lý do cấu trúc.

Voice arm đánh giá trên 16 tên có hướng — không có chúng thì "thức ăn ở
phía Đông" là điều không thể diễn đạt và thước đo MI mất nghĩa.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 3: `Atom.voice` — trường mới, tương thích ngược với CBOR slice 2

**Files:**
- Modify: `crates/omiai-world/src/atoms.rs` (struct + `mod tests`)
- Modify: `crates/omiai-world/src/world_loop.rs` (9 literal `Atom { … }`)
- Modify: `crates/omiai-world/src/agents.rs:310` (1 literal `Atom { … }`)

**Interfaces:**
- Consumes: `communication::N_SYMBOLS` (Task 1).
- Produces: `Atom.voice: Vec<FormulaId>` với `#[serde(default)]`; `Atom::is_mute(&self) -> bool`; `Atom::voice_is_valid(&self) -> bool`.

Bất biến: `voice.len() == 0` (câm) **hoặc** `voice.len() == N_SYMBOLS`. Không
có độ dài nào khác hợp lệ — `decode_voice` map index arm sang ký hiệu 1-1 nên
một `Vec` 3 phần tử nghĩa là ký hiệu 3 không tồn tại, và `Vocabulary` sẽ có
một hàng chết mà không ai giải thích được tại sao.

`#[serde(default)]` là toàn bộ cơ chế tương thích ngược: `atoms.cbor` của
slice 2 không có khoá `voice`, ciborium điền `Vec::new()`, atom hồi sinh
thành **câm** — đúng luật spec §6.4.

- [ ] **Step 1: Viết test thất bại trước**

Thêm vào `mod tests` của `crates/omiai-world/src/atoms.rs`:

```rust
    #[test]
    fn slice2_atom_cbor_deserializes_to_mute() {
        // Bản ghi CBOR đúng hình dạng slice 2: KHÔNG có khoá `voice`.
        // Đây là hợp đồng tương thích ngược của spec §6.4 — nếu ai đó bỏ
        // #[serde(default)] thì checkpoint slice 2 hết đọc được, và test này
        // là chỗ duy nhất phát hiện ra trước khi người dùng mất dữ liệu.
        #[derive(serde::Serialize)]
        struct Slice2Atom {
            pos: (usize, usize),
            energy: f64,
            gene: FormulaId,
            age: u64,
        }
        let old = Slice2Atom {
            pos: (3, 4),
            energy: 0.75,
            gene: FormulaId::from_slot(0),
            age: 9,
        };
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&old, &mut buf).unwrap();

        let back: Atom = ciborium::de::from_reader(&buf[..]).unwrap();
        assert_eq!(back.pos, (3, 4));
        assert_eq!(back.age, 9);
        assert!(back.voice.is_empty(), "atom slice 2 phải hồi sinh thành câm");
        assert!(back.is_mute());
        assert!(back.voice_is_valid());
    }

    #[test]
    fn voice_is_valid_only_for_empty_or_full_arity() {
        let mut a = atom_at(0, 0, 0.5);
        assert!(a.voice_is_valid()); // rỗng
        a.voice = vec![FormulaId::from_slot(0); crate::communication::N_SYMBOLS];
        assert!(a.voice_is_valid() && !a.is_mute());
        a.voice = vec![FormulaId::from_slot(0); crate::communication::N_SYMBOLS - 1];
        assert!(!a.voice_is_valid(), "arity thiếu = ký hiệu không tồn tại");
    }

    #[test]
    fn atom_with_voice_round_trips_cbor() {
        let mut a = atom_at(1, 2, 0.5);
        a.voice = (0..crate::communication::N_SYMBOLS)
            .map(|i| FormulaId::from_slot(i as u32))
            .collect();
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&a, &mut buf).unwrap();
        let back: Atom = ciborium::de::from_reader(&buf[..]).unwrap();
        assert_eq!(back, a);
    }
```

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world --lib atoms`
Expected: FAIL — `no field 'voice' on type 'Atom'`, `no method named 'is_mute'`.

- [ ] **Step 3: Cài**

`crates/omiai-world/src/atoms.rs` — thêm trường vào struct (sau `gene`, trước `age` giữ thứ tự đọc tự nhiên không quan trọng với CBOR map, nhưng đặt `voice` **cuối** để literal cũ đọc thuận mắt):

```rust
    /// Số bước đã sống.
    pub age: u64,
    /// Gene tiếng nói: 0 arm (câm) hoặc đúng `N_SYMBOLS` arm.
    ///
    /// `#[serde(default)]` là hợp đồng tương thích ngược với `atoms.cbor`
    /// của slice 2 (không có khoá này) — atom cũ hồi sinh thành câm, spec
    /// §6.4. Bỏ attribute này = checkpoint slice 2 hết đọc được.
    #[serde(default)]
    pub voice: Vec<FormulaId>,
```

Thêm hai method vào `impl Atom`:

```rust
    /// Atom không phát được ký hiệu nào.
    pub fn is_mute(&self) -> bool {
        self.voice.is_empty()
    }

    /// Bất biến arity: rỗng (câm) hoặc đủ `N_SYMBOLS` arm.
    pub fn voice_is_valid(&self) -> bool {
        self.voice.is_empty() || self.voice.len() == crate::communication::N_SYMBOLS
    }
```

Cập nhật 11 literal `Atom { … }` — thêm `voice: Vec::new()` (hoặc
`voice: vec![]`) vào mỗi cái:
- `world_loop.rs:97` (atom mồi trong `World::new` — Task 6 sẽ thay bằng voice thật)
- `world_loop.rs:213` (con trong `reproduce_and_evolve` — Task 7 thay bằng voice kế thừa)
- `world_loop.rs:334, 357, 382, 383, 404, 432, 435` (test)
- `agents.rs:310` (test)
- `atoms.rs:96` (`atom_at` helper — thêm `voice: Vec::new()`)

Thêm `ciborium` vào dev-dependencies của `crates/omiai-world/Cargo.toml` nếu
chưa có:

```toml
[dev-dependencies]
ciborium = { workspace = true }
```

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-world`
Expected: PASS — mọi test slice 2 vẫn xanh (trường mới mặc định rỗng nên
không đổi hành vi), cộng 3 test mới.

Run: `cargo test --workspace`
Expected: PASS — `omiai-checkpoint` vẫn round-trip được (voice rỗng serialize
thành mảng rỗng; đọc lại bằng nhau).

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/src/atoms.rs crates/omiai-world/src/world_loop.rs \
        crates/omiai-world/src/agents.rs crates/omiai-world/Cargo.toml
git commit -m "$(cat <<'MSG'
feat(world): Atom.voice — gene tiếng nói, tương thích ngược CBOR slice 2

#[serde(default)] là toàn bộ cơ chế tương thích: atoms.cbor của slice 2
không có khoá voice nên atom cũ hồi sinh thành câm (spec §6.4). Test dựng
đúng hình dạng bản ghi slice 2 rồi deserialize — đó là chỗ duy nhất phát
hiện regression này trước khi người dùng mất checkpoint.

Bất biến arity: rỗng hoặc đúng N_SYMBOLS arm. Độ dài lỡ cỡ nghĩa là một ký
hiệu không tồn tại và Vocabulary có hàng chết không ai giải thích được.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 4: đột biến theo pool — `mutate_formula_with` + `random_voice`

**Files:**
- Modify: `crates/omiai-world/src/agents.rs` (thêm `MOVEMENT_ATOM_NAMES`)
- Modify: `crates/omiai-world/src/world_loop.rs` (`mutate_formula` → `mutate_formula_with`, thêm `random_voice`)

**Interfaces:**
- Consumes: `communication::{VOICE_ATOM_NAMES, N_SYMBOLS, voice_seed_formula}` (Task 1–2).
- Produces: `pub const agents::MOVEMENT_ATOM_NAMES: [&str; 4]`; `pub fn world_loop::mutate_formula_with(&LtlFormula, &mut ChaCha8Rng, &[&str]) -> LtlFormula`; `pub fn world_loop::mutate_formula(&LtlFormula, &mut ChaCha8Rng) -> LtlFormula` (giữ nguyên chữ ký cũ, giờ là wrapper pool di chuyển); `pub fn world_loop::random_voice(&mut FormulaRegistry, &mut ChaCha8Rng) -> Vec<FormulaId>`.

Task này để `MOVEMENT_ATOM_NAMES` ở đúng **4 tên của slice 2**. Task 5 mới nới
lên 8 khi receiver thực sự nghe được — nới sớm hơn thì pool di chuyển sinh ra
`hear0` trong lúc valuation chưa có khoá đó, `eval_current` trả false, và ta
có một đột biến im lặng vô nghĩa không test nào bắt.

- [ ] **Step 1: Viết test thất bại trước**

Thêm vào `mod tests` của `crates/omiai-world/src/world_loop.rs`:

```rust
    /// Thu mọi tên atom xuất hiện trong công thức.
    fn atom_names(f: &LtlFormula, out: &mut Vec<String>) {
        match f {
            LtlFormula::Atom(n) => out.push(n.clone()),
            LtlFormula::True_ | LtlFormula::False_ => {}
            LtlFormula::Not(g)
            | LtlFormula::Next(g)
            | LtlFormula::Eventually(g)
            | LtlFormula::Globally(g) => atom_names(g, out),
            LtlFormula::And(a, b)
            | LtlFormula::Or(a, b)
            | LtlFormula::Until(a, b)
            | LtlFormula::Release(a, b) => {
                atom_names(a, out);
                atom_names(b, out);
            }
        }
    }

    #[test]
    fn mutate_with_pool_only_emits_pool_names() {
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let seed = voice_seed_formula();
        for _ in 0..64 {
            let m = mutate_formula_with(&seed, &mut rng, &VOICE_ATOM_NAMES);
            let mut names = Vec::new();
            atom_names(&m, &mut names);
            assert!(!names.is_empty());
            for n in names {
                assert!(
                    VOICE_ATOM_NAMES.contains(&n.as_str()),
                    "đột biến voice rò tên ngoài pool: {n}"
                );
            }
        }
    }

    #[test]
    fn mutate_formula_still_uses_movement_pool() {
        // Wrapper cũ phải giữ đúng hành vi slice 2: chỉ 4 tên không hướng.
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let base = default_genome_formula();
        for _ in 0..64 {
            let m = mutate_formula(&base, &mut rng);
            let mut names = Vec::new();
            atom_names(&m, &mut names);
            for n in names {
                assert!(
                    crate::agents::MOVEMENT_ATOM_NAMES.contains(&n.as_str()),
                    "đột biến di chuyển rò tên ngoài pool: {n}"
                );
            }
        }
    }

    #[test]
    fn random_voice_has_full_arity_bounded_depth_and_is_deterministic() {
        let mut reg_a = FormulaRegistry::new();
        let mut rng_a = ChaCha8Rng::seed_from_u64(99);
        let voice_a = random_voice(&mut reg_a, &mut rng_a);

        assert_eq!(voice_a.len(), crate::communication::N_SYMBOLS);
        let seed_depth = depth(&voice_seed_formula());
        for id in &voice_a {
            let f = &reg_a.get(*id).expect("arm phải có trong registry").formula;
            assert!(depth(f) <= seed_depth, "đột biến không được làm sâu thêm");
        }

        // Cùng seed → cùng voice (bit-exact resume phụ thuộc điều này).
        let mut reg_b = FormulaRegistry::new();
        let mut rng_b = ChaCha8Rng::seed_from_u64(99);
        let voice_b = random_voice(&mut reg_b, &mut rng_b);
        assert_eq!(voice_a, voice_b);
        assert_eq!(reg_a.genomes_in_order(), reg_b.genomes_in_order());

        // Khác seed → gần như chắc chắn khác (không khẳng định cứng vì
        // trùng ngẫu nhiên là hợp lệ; ta chỉ cần biết nó phụ thuộc seed).
        let mut reg_c = FormulaRegistry::new();
        let mut rng_c = ChaCha8Rng::seed_from_u64(100);
        let voice_c = random_voice(&mut reg_c, &mut rng_c);
        assert_ne!(reg_a.genomes_in_order(), reg_c.genomes_in_order());
    }

    #[test]
    fn random_voice_arms_are_decodable() {
        // Voice sinh ngẫu nhiên phải phát được ký hiệu trên valuation thật —
        // nếu pool sai, mọi arm false và dân số câm mà không lỗi nào nổi lên.
        let mut reg = FormulaRegistry::new();
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let mut fired = 0;
        for _ in 0..32 {
            let voice = random_voice(&mut reg, &mut rng);
            let obs = crate::agents::observe_surroundings(
                (1, 1),
                3,
                3,
                &|x, _y| if x == 2 { 2 } else { 0 },
                &|_x, _y| false,
            );
            let val = crate::communication::neighbourhood_valuation(&obs);
            if crate::communication::decode_voice(&voice, &reg, &val)
                != crate::communication::SignalValue::Silent
            {
                fired += 1;
            }
        }
        assert!(fired > 0, "32 voice ngẫu nhiên mà không ai phát được gì ⇒ pool sai");
    }
```

Thêm vào phần `use` của `mod tests` trong `world_loop.rs`:

```rust
    use crate::communication::{voice_seed_formula, VOICE_ATOM_NAMES};
    use crate::registry::FormulaRegistry;
```

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world --lib world_loop`
Expected: FAIL — `cannot find function mutate_formula_with`, `cannot find function random_voice`, `cannot find value MOVEMENT_ATOM_NAMES`.

- [ ] **Step 3: Cài**

`crates/omiai-world/src/agents.rs` — thêm sau `valuation`:

```rust
/// Pool tên mệnh đề của gene DI CHUYỂN. Đột biến chỉ được sinh tên ở đây;
/// tên ngoài pool đánh giá thành false (xem `eval_current`) nên rò rỉ tên
/// lạ = đột biến vô hiệu thầm lặng.
pub const MOVEMENT_ATOM_NAMES: [&str; 4] = ["open", "wall", "res", "occupied"];
```

`crates/omiai-world/src/world_loop.rs` — đổi `mutate_formula` thành phiên bản
nhận pool, giữ tên cũ làm wrapper:

```rust
/// Đột biến cấu trúc với pool tên cho trước. Các biến đổi: đổi atom thành
/// atom khác **trong pool**, đảo And↔Or / Until↔Release / X↔F↔G khi đi
/// xuống qua node đó. Không xoá cấu trúc — genome luôn còn đánh giá được,
/// và độ sâu không tăng (mọi biến đổi giữ arity).
pub fn mutate_formula_with(
    f: &LtlFormula,
    rng: &mut ChaCha8Rng,
    names: &[&str],
) -> LtlFormula {
    debug_assert!(!names.is_empty(), "pool tên rỗng thì không đột biến được");
    match f {
        LtlFormula::Atom(_) => {
            LtlFormula::atom(names[rng.gen_range(0..names.len())])
        }
        LtlFormula::Not(g) => {
            LtlFormula::Not(Box::new(mutate_formula_with(g, rng, names)))
        }
        LtlFormula::And(a, b) | LtlFormula::Or(a, b) => {
            let (a2, b2) = (
                mutate_formula_with(a, rng, names),
                mutate_formula_with(b, rng, names),
            );
            if rng.r#gen::<bool>() {
                LtlFormula::Or(Box::new(a2), Box::new(b2))
            } else {
                LtlFormula::And(Box::new(a2), Box::new(b2))
            }
        }
        LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
            let inner = mutate_formula_with(g, rng, names);
            match rng.gen_range(0..3) {
                0 => LtlFormula::Next(Box::new(inner)),
                1 => LtlFormula::Eventually(Box::new(inner)),
                _ => LtlFormula::Globally(Box::new(inner)),
            }
        }
        LtlFormula::Until(p, q) | LtlFormula::Release(p, q) => {
            let p2 = mutate_formula_with(p, rng, names);
            let q2 = mutate_formula_with(q, rng, names);
            if rng.r#gen::<bool>() {
                LtlFormula::Until(Box::new(p2), Box::new(q2))
            } else {
                LtlFormula::Release(Box::new(p2), Box::new(q2))
            }
        }
        LtlFormula::True_ | LtlFormula::False_ => f.clone(), // leaf giữ nguyên
    }
}

/// Đột biến gene DI CHUYỂN — wrapper giữ chữ ký slice 2.
pub fn mutate_formula(f: &LtlFormula, rng: &mut ChaCha8Rng) -> LtlFormula {
    mutate_formula_with(f, rng, &crate::agents::MOVEMENT_ATOM_NAMES)
}

/// Sinh voice ngẫu nhiên: đúng `N_SYMBOLS` arm, mỗi arm là một đột biến
/// độc lập của hạt giống trên pool voice, chèn vào registry.
///
/// Thứ tự rút RNG (arm 0 → arm K-1) là hợp đồng: đổi nó là đổi mọi quỹ đạo
/// của mọi seed đã lưu.
pub fn random_voice(
    registry: &mut FormulaRegistry,
    rng: &mut ChaCha8Rng,
) -> Vec<FormulaId> {
    let seed = crate::communication::voice_seed_formula();
    (0..crate::communication::N_SYMBOLS)
        .map(|_| {
            let f = mutate_formula_with(
                &seed,
                rng,
                &crate::communication::VOICE_ATOM_NAMES,
            );
            registry.insert(Genome { formula: f, fitness: None })
        })
        .collect()
}
```

Cập nhật `use` ở đầu `world_loop.rs`: `use crate::registry::{FormulaId, FormulaRegistry, Genome};`.
Xoá `const ATOM_NAMES` cũ trong `mutate_formula` (giờ nằm ở `agents::MOVEMENT_ATOM_NAMES`).

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-world`
Expected: PASS — gồm `mutate_formula_bounded_depth_and_valid` của slice 2 (không đổi hành vi) + 4 test mới.

Run: `cargo clippy -p omiai-world --all-targets`
Expected: 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/src/world_loop.rs crates/omiai-world/src/agents.rs
git commit -m "$(cat <<'MSG'
feat(world): đột biến theo pool + random_voice

mutate_formula_with nhận pool tên; mutate_formula thành wrapper pool di
chuyển nên hành vi slice 2 không đổi. Voice cần pool riêng 16 tên có hướng:
đột biến rò tên ngoài pool sẽ đánh giá false và cho một đột biến vô hiệu
thầm lặng — test kiểm chính xác điều đó ở cả hai pool.

random_voice rút RNG theo thứ tự arm 0 → K-1; đó là hợp đồng determinism,
đổi thứ tự là đổi mọi quỹ đạo của mọi seed đã lưu.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 5: phía nhận — `heard`, `hear_flags`, `decide_with_hear`

**Files:**
- Modify: `crates/omiai-world/src/agents.rs` (struct + 6 hàm + pool 8 tên)

**Interfaces:**
- Consumes: `communication::{Symbol, N_SYMBOLS}` (Task 1).
- Produces: `Observation.heard: Option<Symbol>`; `pub fn observe_with(u8, bool, Option<Symbol>) -> Observation`; `pub fn observe_surroundings_hearing(pos, width, height, &dyn Fn(usize,usize)->u8, &dyn Fn(usize,usize)->bool, &dyn Fn(usize,usize)->Option<Symbol>) -> Vec<(Direction, Observation)>`; `pub fn hear_flags(&[(Direction, Observation)]) -> [bool; N_SYMBOLS]`; `pub fn valuation_with_hear(&Observation, &[bool; N_SYMBOLS]) -> BTreeMap<String, bool>`; `pub fn decide_with_hear(&LtlFormula, &[(Direction, Observation)]) -> Action`; `MOVEMENT_ATOM_NAMES` nới lên `[&str; 8]`.

Hai quyết định về chữ ký, cùng một lý do — **không phá call-site slice 2**:

1. `observe` giữ 2 tham số (`heard = None`); bản 3 tham số tên khác là
   `observe_with`.
2. `observe_surroundings` giữ 5 tham số; bản có nghe tên khác là
   `observe_surroundings_hearing`.

`hear*` là mệnh đề **tổng hợp**: "có ô kề nào vừa nói ký hiệu k". Không gắn
hướng, vì gắn hướng thì pool di chuyển phình lên 4+16 tên và không gian đột
biến loãng ra mà chưa có bằng chứng nào cần độ phân giải đó (YAGNI).

Atom không bao giờ tự nghe mình: `airwave` tra theo **ô kề**, ô của chính nó
không nằm trong 4 ô kề. Tính chất này đến từ cấu trúc, nên có test chốt lại.

- [ ] **Step 1: Viết test thất bại trước**

Thêm vào `mod tests` của `crates/omiai-world/src/agents.rs`:

```rust
    use crate::communication::N_SYMBOLS;

    #[test]
    fn movement_pool_grows_with_symbol_count() {
        // Pool = 4 mệnh đề ô + 1 mệnh đề nghe cho mỗi ký hiệu. Nâng
        // N_SYMBOLS mà quên pool = ký hiệu mới không ai nói tới được.
        assert_eq!(MOVEMENT_ATOM_NAMES.len(), 4 + N_SYMBOLS);
        for k in 0..N_SYMBOLS {
            assert!(MOVEMENT_ATOM_NAMES.contains(&format!("hear{k}").as_str()));
        }
    }

    #[test]
    fn hear_flags_aggregate_over_all_directions() {
        let obs = vec![
            (Direction::North, observe_with(0, false, Some(2))),
            (Direction::East, observe_with(0, false, None)),
            (Direction::South, observe_with(0, false, Some(0))),
            (Direction::West, observe_with(0, false, Some(2))),
        ];
        let flags = hear_flags(&obs);
        assert_eq!(flags, [true, false, true, false]);
    }

    #[test]
    fn valuation_with_hear_merges_cell_and_hear_names() {
        let obs = observe_with(2, false, Some(1));
        let val = valuation_with_hear(&obs, &[false, true, false, false]);
        assert_eq!(val.len(), 4 + N_SYMBOLS);
        assert!(val["res"]);
        assert!(val["hear1"]);
        assert!(!val["hear0"]);
        // `heard` của CHÍNH ô đó không tự động thành hear1 — cờ nghe là
        // tổng hợp do caller tính, không phải thuộc tính của một ô.
        let val2 = valuation_with_hear(&obs, &[false; N_SYMBOLS]);
        assert!(!val2["hear1"]);
    }

    #[test]
    fn decide_with_hear_follows_the_signal() {
        // Genome: "đi tới ô trống NẾU có ai nói hear2". Lưới trống hoàn toàn.
        let formula =
            LtlFormula::and(LtlFormula::atom("open"), LtlFormula::atom("hear2"));
        let silent: Vec<_> = ALL_DIRECTIONS
            .iter()
            .map(|&d| (d, observe_with(0, false, None)))
            .collect();
        assert_eq!(decide_with_hear(&formula, &silent), Action::Stay);

        // Cùng lưới, cùng genome, chỉ thêm một tiếng nói ở phía Tây:
        // atom chuyển từ đứng yên sang đi. Đó là "tín hiệu ảnh hưởng hành vi".
        let mut heard = silent.clone();
        heard[3] = (Direction::West, observe_with(0, false, Some(2)));
        assert_eq!(decide_with_hear(&formula, &heard), Action::Move(Direction::North));
    }

    #[test]
    fn decide_ignores_hear_names_and_stays_slice2_behaviour() {
        // `decide` cũ đánh giá trên valuation 4 tên; genome có hear* luôn
        // false ở đó. Giữ tính chất này để gene slice 2 không đổi nghĩa.
        let formula = LtlFormula::atom("hear0");
        let obs: Vec<_> = ALL_DIRECTIONS
            .iter()
            .map(|&d| (d, observe_with(0, false, Some(0))))
            .collect();
        assert_eq!(decide(&formula, &obs), Action::Stay);
        assert_eq!(decide_with_hear(&formula, &obs), Action::Move(Direction::North));
    }

    #[test]
    fn observe_surroundings_hearing_reads_neighbour_cells_only() {
        // Tiếng nói ở ô của chính atom (1,1) không được nghe thấy.
        let obs = observe_surroundings_hearing(
            (1, 1),
            3,
            3,
            &|_x, _y| 0,
            &|_x, _y| false,
            &|x, y| if (x, y) == (1, 1) { Some(3) } else { None },
        );
        assert!(obs.iter().all(|(_, o)| o.heard.is_none()), "atom tự nghe mình");
        assert_eq!(hear_flags(&obs), [false; N_SYMBOLS]);

        // Tiếng nói ở ô kề phía Đông thì nghe được.
        let obs = observe_surroundings_hearing(
            (1, 1),
            3,
            3,
            &|_x, _y| 0,
            &|_x, _y| false,
            &|x, y| if (x, y) == (2, 1) { Some(3) } else { None },
        );
        assert_eq!(obs[1].1.heard, Some(3));
        assert_eq!(hear_flags(&obs), [false, false, false, true]);
    }

    #[test]
    fn out_of_bounds_neighbour_hears_nothing() {
        // Ngoài biên = cản, và cản không nói gì (không được tra `heard` ở
        // toạ độ âm — đó là panic chờ sẵn).
        let obs = observe_surroundings_hearing(
            (0, 0),
            3,
            3,
            &|_x, _y| 0,
            &|_x, _y| false,
            &|_x, _y| Some(1),
        );
        assert!(obs[0].1.wall && obs[0].1.heard.is_none()); // North ngoài biên
        assert_eq!(obs[1].1.heard, Some(1)); // East trong biên
    }
```

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world --lib agents`
Expected: FAIL — `cannot find function observe_with` / `hear_flags` /
`valuation_with_hear` / `decide_with_hear` / `observe_surroundings_hearing`;
`assert_eq!(MOVEMENT_ATOM_NAMES.len(), 8)` vỡ vì pool còn 4.

- [ ] **Step 3: Cài**

`crates/omiai-world/src/agents.rs`:

```rust
use crate::communication::{Symbol, N_SYMBOLS};
```

Thêm trường vào `Observation` (derive `Default` vẫn hợp lệ: `None`):

```rust
    pub occupied: bool,
    /// Ký hiệu vừa được nói TẠI ô này trong phase `speak` của bước hiện
    /// tại; `None` = không ai nói (ô trống hoặc người ở đó im lặng).
    ///
    /// `None` gộp hai tình huống đó lại, nhưng receiver phân biệt được bằng
    /// `occupied ∧ ¬hear*` (spec §5) nên không cần giá trị thứ ba.
    pub heard: Option<Symbol>,
```

Nới pool và thêm 5 hàm:

```rust
/// Pool tên mệnh đề của gene DI CHUYỂN: 4 mệnh đề ô + 1 mệnh đề nghe cho
/// mỗi ký hiệu. `hear*` là mệnh đề TỔNG HỢP ("có ô kề nào vừa nói k"),
/// không gắn hướng — gắn hướng thì pool thành 20 tên và không gian đột biến
/// loãng ra mà chưa có bằng chứng nào cần độ phân giải đó.
pub const MOVEMENT_ATOM_NAMES: [&str; 8] = [
    "open", "wall", "res", "occupied", "hear0", "hear1", "hear2", "hear3",
];

/// Như [`observe`], cộng ký hiệu nghe được tại ô đó.
pub fn observe_with(
    cell_value: u8,
    occupied: bool,
    heard: Option<Symbol>,
) -> Observation {
    Observation { heard, ..observe(cell_value, occupied) }
}

/// Cờ nghe tổng hợp: `flags[k]` = có ô kề nào vừa nói ký hiệu `k`.
pub fn hear_flags(obs_by_dir: &[(Direction, Observation)]) -> [bool; N_SYMBOLS] {
    let mut flags = [false; N_SYMBOLS];
    for (_, obs) in obs_by_dir {
        if let Some(sym) = obs.heard {
            if (sym as usize) < N_SYMBOLS {
                flags[sym as usize] = true;
            }
        }
    }
    flags
}

/// Valuation của một quan sát + cờ nghe tổng hợp (miền của gene di chuyển
/// khi signaling bật).
pub fn valuation_with_hear(
    obs: &Observation,
    hear: &[bool; N_SYMBOLS],
) -> BTreeMap<String, bool> {
    let mut val = valuation(obs);
    for (k, on) in hear.iter().enumerate() {
        val.insert(format!("hear{k}"), *on);
    }
    val
}

/// Như [`decide`], nhưng valuation có cả `hear*`. Cờ nghe tính MỘT lần cho
/// cả 4 hướng — nó là thuộc tính của vùng lân cận, không của từng ô.
pub fn decide_with_hear(
    formula: &LtlFormula,
    obs_by_dir: &[(Direction, Observation)],
) -> Action {
    let hear = hear_flags(obs_by_dir);
    for (dir, obs) in obs_by_dir {
        let passable = !obs.wall && !obs.occupied;
        if passable && eval_current(formula, &valuation_with_hear(obs, &hear)) {
            return Action::Move(*dir);
        }
    }
    Action::Stay
}

/// Như [`observe_surroundings`], cộng `heard(x, y)` tra airwave.
///
/// Ô ngoài biên là cản và KHÔNG tra `heard` — tra ở toạ độ âm là panic chờ
/// sẵn, và tường thì không nói gì.
pub fn observe_surroundings_hearing(
    pos: (usize, usize),
    width: usize,
    height: usize,
    cell: &dyn Fn(usize, usize) -> u8,
    occupied: &dyn Fn(usize, usize) -> bool,
    heard: &dyn Fn(usize, usize) -> Option<Symbol>,
) -> Vec<(Direction, Observation)> {
    ALL_DIRECTIONS
        .iter()
        .map(|&dir| {
            let (dx, dy) = dir.delta();
            let (x, y) = (pos.0 as isize + dx, pos.1 as isize + dy);
            if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
                (dir, observe(1, false)) // ngoài biên = cản, im lặng
            } else {
                let (x, y) = (x as usize, y as usize);
                (dir, observe_with(cell(x, y), occupied(x, y), heard(x, y)))
            }
        })
        .collect()
}
```

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-world`
Expected: PASS — 7 test mới; mọi test slice 2 của `agents`/`world_loop` vẫn
xanh vì `observe`/`observe_surroundings`/`decide` không đổi chữ ký lẫn hành vi.

Run: `cargo test --workspace` và `cargo clippy --workspace --all-targets`
Expected: PASS / 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/src/agents.rs
git commit -m "$(cat <<'MSG'
feat(world): phía nhận — Observation.heard, hear_flags, decide_with_hear

Chữ ký cũ (observe, observe_surroundings, decide) giữ nguyên; bản có nghe là
tên khác. Nhờ vậy gene slice 2 không đổi nghĩa và mọi call-site cũ vẫn đúng.

hear* là mệnh đề tổng hợp, tính một lần cho cả vùng lân cận vì nó là thuộc
tính của vùng chứ không của từng ô. Atom không bao giờ tự nghe mình: airwave
tra theo ô kề, ô của chính nó không nằm trong đó — có test chốt lại vì đó là
tính chất đến từ cấu trúc, dễ mất khi ai đó "tối ưu" vòng quan sát.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 6: phase `speak`, `airwave`, `vocabulary`, world loop 6 phase

**Files:**
- Modify: `crates/omiai-world/src/world_loop.rs` (2 trường mới, `speak`, `step`, `agent_act`, `World::new`)
- Modify: `crates/omiai-world/src/lib.rs` (`pub mod communication;`)

**Interfaces:**
- Consumes: Task 1–5 trọn bộ.
- Produces: `World.airwave: Vec<Option<Symbol>>`; `World.vocabulary: Vocabulary`; `pub fn World::speak(&mut self)`; `World::step` 6 phase; `World::new` cấp voice cho atom mồi.

Thứ tự phase, cố định: `ca_step → metabolism → speak → agent_act →
reproduce_and_evolve → snapshot`.

`speak` **sau** `metabolism` để atom chết trong bước này không nói được, và
**trước** `agent_act` để tín hiệu ảnh hưởng ngay hành động của cùng bước.

Hai bất biến phải giữ trong code, không chỉ trong tài liệu:
- `speak` **không rút RNG** (nếu rút, `word_pos` lưu checkpoint lệch và
  resume mất bit-exact).
- `speak` ghi `self.airwave` **một lần ở cuối** từ một buffer cục bộ. Ghi
  trực tiếp vào `self.airwave` trong vòng lặp sẽ tạo ra thứ tự-phụ-thuộc:
  atom sau đọc được tiếng của atom trước, và ký hiệu hoá ra phụ thuộc thứ tự
  `Vec` — đúng cái spec §2.4 cấm.

Mọi atom **còn sống lúc `speak`** đều được ghi vào `Vocabulary`, kể cả atom
câm (đóng góp vào hàng `Silent`). Bỏ qua atom câm sẽ làm `total` không còn
bằng tổng dân số và MI của thế giới câm thành `NaN` thay vì `0`.

- [ ] **Step 1: Viết test thất bại trước**

Thêm vào `mod tests` của `crates/omiai-world/src/world_loop.rs`:

```rust
    /// Voice quy ước hoàn hảo: arm k bắn đúng khi tài nguyên ở hướng k.
    fn convention_voice(reg: &mut FormulaRegistry) -> Vec<FormulaId> {
        ["res_n", "res_e", "res_s", "res_w"]
            .iter()
            .map(|n| {
                reg.insert(Genome { formula: LtlFormula::atom(*n), fitness: None })
            })
            .collect()
    }

    fn empty_world(w: usize, h: usize, seed: u64) -> World {
        World::new(
            WorldConfig {
                width: w,
                height: h,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            seed,
        )
    }

    #[test]
    fn new_world_gives_every_seed_atom_a_full_voice() {
        let w = small_world(7);
        assert_eq!(w.atoms.len(), 2);
        for a in &w.atoms {
            assert!(a.voice_is_valid() && !a.is_mute());
            for id in &a.voice {
                assert!(w.registry.get(*id).is_some(), "arm phải nằm trong registry");
            }
        }
        // airwave đúng kích thước lưới và trống khi chưa ai nói.
        assert_eq!(w.airwave.len(), 8 * 8);
        assert!(w.airwave.iter().all(|c| c.is_none()));
        assert_eq!(w.vocabulary, Vocabulary::default());
    }

    #[test]
    fn speak_writes_airwave_at_speaker_cell_only() {
        let mut w = empty_world(4, 4, 5);
        let voice = convention_voice(&mut w.registry);
        w.atoms.push(Atom {
            pos: (1, 1),
            energy: 0.5,
            gene: FormulaId::from_slot(0),
            age: 0,
            voice,
        });
        w.ca.cells[1 * 4 + 2] = 2; // tài nguyên phía Đông của (1,1)

        w.speak();

        assert_eq!(w.airwave[1 * 4 + 1], Some(1), "phải nói ký hiệu 1 = Đông");
        assert_eq!(
            w.airwave.iter().filter(|c| c.is_some()).count(),
            1,
            "chỉ ô của người nói mới có tiếng"
        );
        assert_eq!(w.vocabulary.total, 1);
        assert_eq!(
            w.vocabulary.joint[SignalValue::Sym(1).row()][StateClass::East.col()],
            1
        );
    }

    #[test]
    fn speak_records_mute_atoms_as_silence() {
        let mut w = empty_world(4, 4, 5);
        w.atoms.push(Atom {
            pos: (1, 1),
            energy: 0.5,
            gene: FormulaId::from_slot(0),
            age: 0,
            voice: Vec::new(),
        });
        w.ca.cells[1 * 4 + 2] = 2;

        w.speak();

        assert!(w.airwave.iter().all(|c| c.is_none()));
        assert_eq!(w.vocabulary.total, 1, "atom câm vẫn được đếm");
        assert_eq!(
            w.vocabulary.joint[SignalValue::Silent.row()][StateClass::East.col()],
            1
        );
        assert_eq!(w.vocabulary.mutual_information(), 0.0);
    }

    #[test]
    fn speak_consumes_no_rng() {
        // Hợp đồng bit-exact resume: word_pos không được nhúc nhích.
        let mut w = small_world(13);
        let before = w.rng.get_word_pos();
        w.speak();
        assert_eq!(w.rng.get_word_pos(), before, "speak rút RNG là mất bit-exact");
    }

    #[test]
    fn speak_is_order_independent_within_a_step() {
        // Đảo thứ tự Vec atom không được đổi airwave lẫn vocabulary. Nếu
        // speak ghi trực tiếp vào self.airwave, atom sau sẽ nghe atom trước
        // và test này vỡ.
        let mut a = empty_world(5, 5, 17);
        let voice_a = convention_voice(&mut a.registry);
        let mut b = empty_world(5, 5, 17);
        let voice_b = convention_voice(&mut b.registry);
        let mk = |pos, voice: &Vec<FormulaId>| Atom {
            pos,
            energy: 0.5,
            gene: FormulaId::from_slot(0),
            age: 0,
            voice: voice.clone(),
        };
        a.atoms = vec![mk((1, 1), &voice_a), mk((2, 1), &voice_a), mk((3, 1), &voice_a)];
        b.atoms = vec![mk((3, 1), &voice_b), mk((1, 1), &voice_b), mk((2, 1), &voice_b)];
        for w in [&mut a, &mut b] {
            w.ca.cells[1 * 5 + 4] = 3;
        }

        a.speak();
        b.speak();

        assert_eq!(a.airwave, b.airwave);
        assert_eq!(a.vocabulary, b.vocabulary);
    }

    #[test]
    fn step_runs_six_phases_and_speaks() {
        let mut w = small_world(3);
        w.step();
        assert_eq!(w.step_count, 1);
        assert!(w.atoms.iter().all(|a| a.age == 1));
        // speak đã chạy: mọi atom sống lúc speak được ghi.
        assert!(w.vocabulary.total >= 1);
    }

    #[test]
    fn vocabulary_total_accumulates_across_steps() {
        let mut w = small_world(23);
        let mut expected = 0u64;
        for _ in 0..10 {
            w.ca_step();
            w.metabolism();
            expected += w.atoms.len() as u64; // dân số ĐÚNG lúc speak
            w.speak();
            w.agent_act();
            w.reproduce_and_evolve();
            w.snapshot();
        }
        assert_eq!(w.vocabulary.total, expected);
    }

    #[test]
    fn agent_act_reacts_to_heard_signal() {
        // Atom nghe được ký hiệu 2 thì đi; không nghe thì đứng yên. Lưới
        // trống nên khác biệt duy nhất là airwave.
        let mut w = empty_world(5, 5, 31);
        let gene = w.registry.insert(Genome {
            formula: LtlFormula::and(
                LtlFormula::atom("open"),
                LtlFormula::atom("hear2"),
            ),
            fitness: None,
        });
        w.atoms.push(Atom {
            pos: (2, 2),
            energy: 0.5,
            gene,
            age: 0,
            voice: Vec::new(),
        });

        w.airwave = vec![None; 25];
        w.agent_act();
        assert_eq!(w.atoms[0].pos, (2, 2), "không nghe gì thì đứng yên");

        w.airwave = vec![None; 25];
        w.airwave[2 * 5 + 1] = Some(2); // ô kề phía Tây có tiếng
        w.agent_act();
        assert_ne!(w.atoms[0].pos, (2, 2), "nghe được thì phải hành động");
    }
```

Thêm vào `use` của `mod tests`: `use crate::communication::{SignalValue, StateClass, Vocabulary};`
Cập nhật `same_seed_same_trajectory` để so thêm `vocabulary` và `airwave`:

```rust
        assert_eq!(a.vocabulary, b.vocabulary);
        assert_eq!(a.airwave, b.airwave);
```

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world --lib world_loop`
Expected: FAIL — `no field 'airwave' on type 'World'`, `no field 'vocabulary'`,
`no method named 'speak'`, `missing field 'voice' in initializer of 'Atom'`
(literal test nào chưa thêm), `step_runs_six_phases_and_speaks` vỡ ở
`vocabulary.total`.

- [ ] **Step 3: Cài**

`crates/omiai-world/src/lib.rs` — thêm `pub mod communication;` (giữ thứ tự
alphabet với các `mod` hiện có) và sửa doc comment: communication không còn
là "khung sườn cho lát cắt sau".

`crates/omiai-world/src/world_loop.rs` — hai trường mới trên `World`:

```rust
    /// Kênh tín hiệu của bước hiện tại, một ô lưới một phần tử: ký hiệu vừa
    /// được nói TẠI ô đó, `None` nếu không ai nói.
    ///
    /// Trạng thái PHÁI SINH: `speak` ghi một lần rồi đóng băng, mọi receiver
    /// đọc cùng một ảnh. KHÔNG lưu checkpoint — `load` khởi tạo toàn `None`
    /// và bước tiếp theo ghi lại đầy đủ trước khi ai đó đọc.
    pub airwave: Vec<Option<Symbol>>,
    /// Bảng đồng xuất hiện (ký hiệu × lớp trạng thái), tích luỹ toàn bộ
    /// vòng đời world. Lưu checkpoint (`communication/vocabulary.cbor`).
    pub vocabulary: Vocabulary,
```

Trong `World::new`, sau khi khởi tạo `World { … }` thêm hai trường
(`airwave: vec![None; n_cells]`, `vocabulary: Vocabulary::default()`), và
trong vòng đặt atom mồi thay `voice: Vec::new()` bằng voice thật:

```rust
            if world.ca.cells[i] == 0 && !occupied.contains(&(x, y)) {
                // Thứ tự rút RNG là hợp đồng: rải tài nguyên xong mới tới
                // voice, một atom một lượt, theo thứ tự đặt.
                let voice = random_voice(&mut world.registry, &mut world.rng);
                world.atoms.push(Atom {
                    pos: (x, y),
                    energy: 0.5,
                    gene: default_genome,
                    age: 0,
                    voice,
                });
                placed += 1;
            }
```

`step` lên 6 phase:

```rust
    /// Một bước world: 6 phase theo thứ tự cố định.
    ///
    /// `speak` nằm SAU `metabolism` (atom chết trong bước này không nói) và
    /// TRƯỚC `agent_act` (tín hiệu ảnh hưởng ngay hành động cùng bước).
    pub fn step(&mut self) {
        self.ca_step();
        self.metabolism();
        self.speak();
        self.agent_act();
        self.reproduce_and_evolve();
        self.snapshot();
    }
```

Phase mới:

```rust
    /// Phase 3: mỗi atom còn sống quan sát vùng lân cận, giải mã voice gene
    /// thành một ký hiệu (hoặc im lặng), và ghi vào `airwave` tại ô của
    /// chính nó. Đồng thời ghi mẫu (ký hiệu, lớp trạng thái) vào
    /// `vocabulary` — **cùng một ảnh quan sát**, nên MI đo đúng cái sender
    /// thấy lúc nói (spec §5).
    ///
    /// KHÔNG rút RNG. Buffer cục bộ rồi gán một lần: ghi trực tiếp vào
    /// `self.airwave` sẽ cho atom sau nghe atom trước và làm ký hiệu phụ
    /// thuộc thứ tự `Vec`.
    pub fn speak(&mut self) {
        let (width, height) = (self.ca.width, self.ca.height);
        let mut airwave: Vec<Option<Symbol>> = vec![None; width * height];
        let occupied = occupied_set(&self.atoms);
        let cells = self.ca.cells.clone();

        for atom in &self.atoms {
            let cell = |x: usize, y: usize| cells[y * width + x];
            let occ =
                |x: usize, y: usize| occupied.contains(&(x, y)) && (x, y) != atom.pos;
            let obs =
                agents::observe_surroundings(atom.pos, width, height, &cell, &occ);
            let val = communication::neighbourhood_valuation(&obs);
            let signal = communication::decode_voice(&atom.voice, &self.registry, &val);
            let state = communication::state_class(&obs);
            // Ghi MỌI atom sống, kể cả atom câm (hàng Silent) — nếu không,
            // `total` không còn là dân số và MI của thế giới câm thành NaN.
            self.vocabulary.record(signal, state);
            if let communication::SignalValue::Sym(sym) = signal {
                let idx = atom.pos.1 * width + atom.pos.0;
                debug_assert!(airwave[idx].is_none(), "hai atom cùng ô");
                airwave[idx] = Some(sym);
            }
        }

        self.airwave = airwave;
    }
```

`agent_act` — dùng bản quan sát có nghe và `decide_with_hear`. Chụp một ảnh
`airwave` trước vòng lặp: nó đã đóng băng, và ảnh cục bộ giữ closure không
vướng borrow `self`:

```rust
    pub fn agent_act(&mut self) {
        let width = self.ca.width;
        let height = self.ca.height;
        // airwave đã đóng băng ở phase speak; ảnh cục bộ để closure không
        // vay `self` trong lúc ta sửa `self.atoms`.
        let airwave = self.airwave.clone();
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
            let occ =
                |x: usize, y: usize| occupied.contains(&(x, y)) && (x, y) != pos;
            let heard = |x: usize, y: usize| airwave[y * width + x];
            let obs = agents::observe_surroundings_hearing(
                pos, width, height, &cell, &occ, &heard,
            );

            let action = agents::decide_with_hear(&formula, &obs);
            // … phần áp dụng action giữ nguyên như slice 2 …
        }
    }
```

Thêm `use` ở đầu file: `use crate::communication::{self, Symbol, Vocabulary};`
(module `communication` dùng qua đường dẫn đủ để đọc code thấy rõ nó từ đâu).

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-world`
Expected: PASS — 8 test mới; `same_seed_same_trajectory` vẫn xanh **với** so
sánh `vocabulary`/`airwave`.

Run: `cargo test --workspace`
Expected: `omiai-checkpoint` FAIL biên dịch — `World { … }` trong
`world_bundle.rs::load` thiếu `airwave`/`vocabulary`. Đó là lỗi đúng và Task 9
sửa. Để không để workspace đỏ qua một commit, thêm ngay trong task này bản
tạm tối thiểu vào `world_bundle.rs::load`:

```rust
            airwave: vec![None; ca.width * ca.height],
            vocabulary: Default::default(),
```

Rồi chạy lại `cargo test --workspace` — PASS.

Run: `cargo clippy --workspace --all-targets`
Expected: 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/src/world_loop.rs crates/omiai-world/src/lib.rs \
        crates/omiai-checkpoint/src/world_bundle.rs
git commit -m "$(cat <<'MSG'
feat(world): phase speak + airwave đóng băng, world loop 6 phase

Thứ tự: ca_step → metabolism → speak → agent_act → reproduce → snapshot.
speak sau metabolism (atom chết không nói được) và trước agent_act (tín hiệu
ảnh hưởng ngay hành động cùng bước).

speak ghi buffer cục bộ rồi gán một lần. Ghi trực tiếp vào self.airwave sẽ
cho atom sau nghe atom trước và ký hiệu hoá ra phụ thuộc thứ tự Vec — test
speak_is_order_independent_within_a_step chốt đúng chuyện đó bằng cách đảo
thứ tự atom và đòi kết quả giống hệt.

speak không rút RNG (có test so word_pos): nếu rút, word_pos lưu checkpoint
lệch và resume mất bit-exact. Atom câm vẫn được ghi vào hàng Silent, nếu
không thì total không còn là dân số và MI của thế giới câm thành NaN.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 6b: ba test ngữ nghĩa mà spec §7 đòi — đóng băng, thời điểm, ⊥ hear

**Files:**
- Modify: `crates/omiai-world/src/world_loop.rs` (chỉ `mod tests`)

**Interfaces:**
- Consumes: Task 1–6 (không API mới).
- Produces: không có API mới. Task riêng vì đây là ba **tính chất ngữ
  nghĩa** của thiết kế, không phải test đơn vị của một hàm — reviewer có
  thể nhận Task 6 (code chạy) mà vẫn từ chối task này (thiết kế sai chỗ).

Ba tính chất này là lý do tồn tại của thứ tự phase. Không có test, chúng chỉ
là câu trong ADR:

1. **Airwave đóng băng** — người nói phát rồi đi trong `agent_act`; láng
   giềng của ô **cũ** vẫn nghe được trong cùng bước đó.
2. **Thời điểm lấy mẫu** — người nói kề tài nguyên phát ký hiệu rồi ăn mất
   tài nguyên; cặp đã ghi vẫn là (ký hiệu, hướng lúc phát), không phải
   `None`. Nếu lấy mẫu sau `agent_act` thì MI đo một thế giới không ai từng
   thấy.
3. **Voice ⊥ hear** — hai world giống nhau hoàn toàn trừ airwave có sẵn từ
   trước ⇒ `speak` cho ký hiệu y hệt.

- [ ] **Step 1: Viết test thất bại trước**

Thêm vào `mod tests` của `crates/omiai-world/src/world_loop.rs`:

```rust
    /// World rỗng, không tài nguyên — mọi thứ đặt tay.
    fn bare_world(seed: u64) -> World {
        World::new(
            WorldConfig {
                width: 9,
                height: 9,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            seed,
        )
    }

    /// 4 arm gọi đúng hướng tài nguyên: quy ước hoàn hảo.
    fn convention_voice(w: &mut World) -> Vec<FormulaId> {
        ["res_n", "res_e", "res_s", "res_w"]
            .iter()
            .map(|n| {
                w.registry.insert(Genome {
                    formula: LtlFormula::atom(*n),
                    fitness: None,
                })
            })
            .collect()
    }

    fn gene_of(w: &mut World, f: LtlFormula) -> FormulaId {
        w.registry.insert(Genome { formula: f, fitness: None })
    }

    #[test]
    fn airwave_stays_frozen_after_speaker_moves() {
        let mut w = bare_world(31);
        let voice = convention_voice(&mut w);
        // Tài nguyên phía Bắc ⇒ ký hiệu 0. Gene `open` ⇒ đi Đông (Bắc có
        // tài nguyên nên không `open`).
        w.ca.cells[3 * 9 + 4] = 3;
        let gene = gene_of(&mut w, LtlFormula::atom("open"));
        w.atoms.push(Atom { pos: (4, 4), energy: 1.0, gene, age: 0, voice });

        w.speak();
        assert_eq!(w.airwave[4 * 9 + 4], Some(0), "phải nói ký hiệu 0 ở ô của mình");

        w.agent_act();
        assert_eq!(w.atoms[0].pos, (5, 4), "người nói phải rời ô cũ");
        assert_eq!(
            w.airwave[4 * 9 + 4],
            Some(0),
            "airwave đóng băng: tiếng ở lại ô CŨ trong cùng bước"
        );
        assert_eq!(w.airwave[5 * 9 + 4], None, "ô mới không tự mọc ra tiếng");

        // Và láng giềng của ô cũ THỰC SỰ nghe được: (3,4) có ô kề phía Đông
        // là (4,4) — ô người nói vừa rời.
        let cells = w.ca.cells.clone();
        let airwave = w.airwave.clone();
        let obs = agents::observe_surroundings_hearing(
            (3, 4),
            9,
            9,
            &|x, y| cells[y * 9 + x],
            &|_, _| false,
            &|x, y| airwave[y * 9 + x],
        );
        assert!(
            agents::hear_flags(&obs)[0],
            "láng giềng của ô cũ phải nghe được ký hiệu 0 trong cùng bước"
        );
    }

    #[test]
    fn sample_is_taken_at_speak_time_not_after_eating() {
        let mut w = bare_world(32);
        let voice = convention_voice(&mut w);
        // Tài nguyên phía Đông ⇒ lớp East, ký hiệu 1. Gene `res` ⇒ đi ăn.
        w.ca.cells[4 * 9 + 5] = 3;
        let gene = gene_of(&mut w, LtlFormula::atom("res"));
        w.atoms.push(Atom { pos: (4, 4), energy: 0.5, gene, age: 0, voice });

        w.speak();
        w.agent_act();

        assert_eq!(w.atoms[0].pos, (5, 4), "phải ăn xong và đứng ở ô tài nguyên");
        assert_eq!(w.ca.cells[4 * 9 + 5], 0, "tài nguyên đã bị ăn");

        let v = &w.vocabulary;
        assert_eq!(v.total, 1);
        // row 2 = Sym(1), col 1 = East.
        assert_eq!(v.joint[2][1], 1, "cặp phải là (ký hiệu 1, hướng East) lúc phát");
        // row 0 = Silent, col 4 = None: nếu lấy mẫu SAU khi ăn thì ô này = 1.
        assert_eq!(v.joint[0][4], 0, "không được lấy mẫu sau agent_act");
    }

    #[test]
    fn speak_ignores_pre_existing_airwave() {
        // Nếu một arm voice nào đó đọc được `hear*`, hai world này sẽ cho
        // airwave khác nhau và ký hiệu thành phụ thuộc bước trước.
        let build = |preloaded: bool| {
            let mut w = bare_world(33);
            let voice = convention_voice(&mut w);
            w.ca.cells[3 * 9 + 4] = 2; // tài nguyên Bắc của (4,4)
            let gene = gene_of(&mut w, LtlFormula::False_);
            // Hai atom kề nhau: nếu voice đọc hear* thì nhau ảnh hưởng nhau.
            w.atoms.push(Atom {
                pos: (4, 4),
                energy: 1.0,
                gene,
                age: 0,
                voice: voice.clone(),
            });
            w.atoms.push(Atom { pos: (5, 4), energy: 1.0, gene, age: 0, voice });
            if preloaded {
                w.airwave = vec![Some(2); 81];
            }
            w.speak();
            w
        };
        let clean = build(false);
        let noisy = build(true);
        assert_eq!(clean.airwave, noisy.airwave, "voice không được đọc airwave cũ");
        assert_eq!(clean.vocabulary, noisy.vocabulary);
        // Và airwave cũ phải bị GHI ĐÈ hoàn toàn, không hoà trộn.
        assert_eq!(
            noisy.airwave.iter().filter(|c| c.is_some()).count(),
            1,
            "chỉ atom (4,4) có tài nguyên kề nên chỉ một ô có tiếng"
        );
    }
```

Thêm vào `use` của `mod tests` (nếu chưa có): `use crate::agents;`,
`use crate::registry::{FormulaId, Genome};`, `use omiai_core::ltl::LtlFormula;`
— `LtlFormula` đã được `use` ở đầu file nên `super::*` đã mang vào.

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world world_loop::tests::airwave_stays_frozen -- --exact`
Expected: FAIL nếu Task 6 chưa xong. Nếu Task 6 đã xong, cả ba phải xanh —
khi đó **cố tình phá để chứng minh test có răng**, mỗi lần một thay đổi rồi
hoàn nguyên ngay:
- đổi thứ tự trong `step` thành `agent_act` trước `speak` ⇒
  `sample_is_taken_at_speak_time_not_after_eating` phải đỏ.
- trong `speak`, khởi tạo buffer bằng `self.airwave.clone()` thay vì
  `vec![None; …]` ⇒ `speak_ignores_pre_existing_airwave` phải đỏ.
- trong `agent_act`, thêm `self.airwave[idx] = None` khi atom rời ô ⇒
  `airwave_stays_frozen_after_speaker_moves` phải đỏ.

Ghi kết quả ba lần phá này vào commit message. Nếu một trong ba KHÔNG đỏ,
test đó chưa kiểm cái nó tuyên bố — sửa test, đừng sửa kết luận.

- [ ] **Step 3: Sửa nếu cần**

Không có code sản phẩm mới. Nếu test đỏ ở trạng thái sạch thì Task 6 sai
thiết kế — sửa `speak`/`step` cho khớp spec §5, KHÔNG nới test.

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-world` và `cargo clippy --workspace --all-targets`
Expected: PASS / 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/src/world_loop.rs
git commit -m "$(cat <<'MSG'
test(world): airwave đóng băng, thời điểm lấy mẫu, voice ⊥ hear

Ba tính chất ngữ nghĩa của thứ tự phase, trước đây chỉ là câu trong ADR:
người nói phát rồi đi thì láng giềng ô CŨ vẫn nghe được trong cùng bước;
cặp (ký hiệu, hướng) ghi lúc phát chứ không phải sau khi ăn mất tài nguyên;
airwave có sẵn từ trước không ảnh hưởng ký hiệu phát ra.

Đã kiểm chứng cả ba có răng bằng cách lần lượt: đảo agent_act lên trước
speak, khởi tạo buffer từ airwave cũ, xoá airwave khi atom rời ô — mỗi lần
đúng một test đỏ.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 7: di truyền voice + `seed_voices`

**Files:**
- Modify: `crates/omiai-world/src/world_loop.rs` (`reproduce_and_evolve`, thêm `seed_voices`)

**Interfaces:**
- Consumes: `random_voice`, `mutate_formula_with` (Task 4); `Atom.voice` (Task 3).
- Produces: `World::reproduce_and_evolve` cấp voice cho con; `pub fn World::seed_voices(&mut self) -> usize` (trả về số atom vừa được cấp tiếng).

Ba luật, cả ba đều là hợp đồng determinism nên viết ra thành test:

1. **Cha câm → con câm, và KHÔNG rút RNG cho voice.** Rút rồi bỏ đi cũng là
   tiêu thụ: `word_pos` sẽ khác giữa một world câm và một world có tiếng ở
   cùng seed, và checkpoint version 1 hết resume đúng.
2. **Cha có tiếng → con kế thừa cả K arm**, rồi với xác suất `MUTATION_PROB`
   **đúng một arm** (chọn đều) bị thay bằng đột biến của nó. Đột biến cả K
   arm một lúc sẽ xoá quy ước vừa hình thành nhanh hơn chọn lọc dựng được nó.
3. **Thứ tự rút: gene di chuyển trước, voice sau.** Đảo là đổi mọi quỹ đạo.

`seed_voices` là cách DUY NHẤT để một thế giới câm có tiếng lại (spec §6.4).
`load` không được tự làm việc đó: nó sẽ phải rút RNG và phá `word_pos` đã lưu.

- [ ] **Step 1: Viết test thất bại trước**

Thêm vào `mod tests` của `crates/omiai-world/src/world_loop.rs`:

```rust
    #[test]
    fn mute_parent_yields_mute_child_and_draws_no_voice_rng() {
        let mut w = empty_world(4, 4, 41);
        w.atoms.push(Atom {
            pos: (1, 1),
            energy: REPRODUCE_THRESHOLD,
            gene: FormulaId::from_slot(0),
            age: 0,
            voice: Vec::new(),
        });
        let genomes_before = w.registry.len();

        w.reproduce_and_evolve();

        assert_eq!(w.atoms.len(), 2);
        assert!(w.atoms[1].is_mute(), "cha câm phải sinh con câm");
        // Không arm nào được chèn vào registry cho voice. (Đột biến gene di
        // chuyển có thể chèn 1 genome — nên biên trên là +1.)
        assert!(
            w.registry.len() <= genomes_before + 1,
            "cha câm không được sinh arm voice nào"
        );
    }

    #[test]
    fn voiced_parent_yields_child_with_valid_voice() {
        let mut w = empty_world(6, 6, 43);
        let voice = convention_voice(&mut w.registry);
        let mut differed = false;
        for round in 0..24 {
            w.atoms.clear();
            w.atoms.push(Atom {
                pos: (2, 2),
                energy: REPRODUCE_THRESHOLD,
                gene: FormulaId::from_slot(0),
                age: 0,
                voice: voice.clone(),
            });
            w.reproduce_and_evolve();
            assert_eq!(w.atoms.len(), 2, "vòng {round} phải sinh được");
            let child = &w.atoms[1];
            assert!(child.voice_is_valid() && !child.is_mute());
            for id in &child.voice {
                assert!(w.registry.get(*id).is_some());
            }
            // Đột biến chỉ đổi ĐÚNG MỘT arm khi nó xảy ra.
            let diffs = child
                .voice
                .iter()
                .zip(&voice)
                .filter(|(a, b)| a != b)
                .count();
            assert!(diffs <= 1, "đột biến voice chỉ được đổi một arm, thấy {diffs}");
            differed |= diffs == 1;
        }
        assert!(differed, "24 lần sinh mà không đột biến voice lần nào ⇒ code chết");
    }

    #[test]
    fn reproduction_draw_order_is_gene_then_voice() {
        // Hợp đồng: cùng seed, cùng dân số ⇒ cùng registry và cùng voice con.
        // Đảo thứ tự rút sẽ làm test này vỡ ở `genomes_in_order`.
        let mk = |seed: u64| {
            let mut w = empty_world(6, 6, seed);
            let voice = convention_voice(&mut w.registry);
            w.atoms.push(Atom {
                pos: (2, 2),
                energy: REPRODUCE_THRESHOLD,
                gene: FormulaId::from_slot(0),
                age: 0,
                voice,
            });
            w.reproduce_and_evolve();
            w
        };
        let a = mk(47);
        let b = mk(47);
        assert_eq!(a.atoms, b.atoms);
        assert_eq!(a.registry.genomes_in_order(), b.registry.genomes_in_order());
        assert_eq!(a.rng.get_word_pos(), b.rng.get_word_pos());
    }

    #[test]
    fn seed_voices_revives_only_mute_atoms_and_is_deterministic() {
        let mk = || {
            let mut w = empty_world(6, 6, 53);
            let voice = convention_voice(&mut w.registry);
            w.atoms.push(Atom {
                pos: (1, 1),
                energy: 0.5,
                gene: FormulaId::from_slot(0),
                age: 0,
                voice: voice.clone(),
            });
            w.atoms.push(Atom {
                pos: (2, 2),
                energy: 0.5,
                gene: FormulaId::from_slot(0),
                age: 0,
                voice: Vec::new(),
            });
            (w, voice)
        };

        let (mut a, voice) = mk();
        let revived = a.seed_voices();
        assert_eq!(revived, 1, "chỉ atom câm được cấp tiếng");
        assert_eq!(a.atoms[0].voice, voice, "atom đã có tiếng không bị đổi");
        assert!(a.atoms[1].voice_is_valid() && !a.atoms[1].is_mute());

        let (mut b, _) = mk();
        b.seed_voices();
        assert_eq!(a.atoms, b.atoms);
        assert_eq!(a.registry.genomes_in_order(), b.registry.genomes_in_order());

        // Gọi lần hai không cấp thêm cho ai (không còn atom câm).
        assert_eq!(a.seed_voices(), 0);
    }
```

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world --lib world_loop`
Expected: FAIL — `no method named 'seed_voices'`; `voiced_parent_yields_child_with_valid_voice`
vỡ ở `!child.is_mute()` vì `reproduce_and_evolve` còn đặt `voice: Vec::new()`.

- [ ] **Step 3: Cài**

Trong `reproduce_and_evolve`, ngay **sau** khối `child_gene` (giữ thứ tự rút:
gene trước, voice sau) và **trước** `atom.split_energy()`:

```rust
            // Voice của con: cha câm → con câm, và KHÔNG rút RNG (rút rồi bỏ
            // cũng là tiêu thụ, sẽ làm word_pos khác giữa world câm và world
            // có tiếng ở cùng seed).
            let child_voice = if atom.voice.is_empty() {
                Vec::new()
            } else {
                let mut voice = atom.voice.clone();
                if self.rng.r#gen::<f64>() < MUTATION_PROB {
                    // Đúng MỘT arm đổi. Đột biến cả K arm một lúc sẽ xoá quy
                    // ước vừa hình thành nhanh hơn chọn lọc dựng được nó.
                    let k = self.rng.gen_range(0..voice.len());
                    if let Some(g) = self.registry.get(voice[k]) {
                        let mutated = mutate_formula_with(
                            &g.formula,
                            &mut self.rng,
                            &crate::communication::VOICE_ATOM_NAMES,
                        );
                        voice[k] = self
                            .registry
                            .insert(Genome { formula: mutated, fitness: None });
                    }
                }
                voice
            };
```

rồi truyền vào literal `Atom`:

```rust
            children.push(Atom {
                pos: (sx, sy),
                energy: child_energy,
                gene: child_gene,
                age: 0,
                voice: child_voice,
            });
```

Thêm method mới trên `World`:

```rust
    /// Cấp tiếng cho mọi atom đang câm; trả về số atom được cấp.
    ///
    /// Đây là cách DUY NHẤT để một thế giới câm (checkpoint version 1, spec
    /// §6.4) có tiếng lại. `load` không được tự làm: nó sẽ phải rút RNG và
    /// phá `word_pos` đã lưu, tức là phá chính hợp đồng resume bit-exact.
    ///
    /// Rút RNG theo thứ tự `Vec` atom — deterministic, nhưng có tiêu thụ:
    /// gọi hàm này rồi thì quỹ đạo tiếp theo khác với world không gọi.
    pub fn seed_voices(&mut self) -> usize {
        let mut revived = 0;
        for i in 0..self.atoms.len() {
            if self.atoms[i].voice.is_empty() {
                let voice = random_voice(&mut self.registry, &mut self.rng);
                self.atoms[i].voice = voice;
                revived += 1;
            }
        }
        revived
    }
```

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-world`
Expected: PASS — 4 test mới.

Run: `cargo test --workspace` và `cargo clippy --workspace --all-targets`
Expected: PASS / 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/src/world_loop.rs
git commit -m "$(cat <<'MSG'
feat(world): di truyền voice + seed_voices

Cha câm → con câm, và không rút RNG cho voice: rút rồi bỏ cũng là tiêu thụ,
sẽ làm word_pos khác nhau giữa world câm và world có tiếng ở cùng seed, và
checkpoint version 1 hết resume đúng.

Đột biến đổi đúng một arm. Đổi cả K arm một lúc sẽ xoá quy ước vừa hình
thành nhanh hơn chọn lọc dựng được nó — có test đòi diffs <= 1.

seed_voices là cách duy nhất để thế giới câm có tiếng lại; load giữ nguyên
tính thuần khiết (không rút RNG) vì đó là hợp đồng resume bit-exact.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 8: `format_version = 1_001` — minor bump đọc được cả v1

**Files:**
- Modify: `crates/omiai-checkpoint/src/manifest.rs`
- Modify: `crates/omiai-checkpoint/src/lib.rs:36` (`verify_dir`)
- Modify: `crates/omiai-checkpoint/src/ca_grid.rs:61` (`load`)
- Modify: `crates/omiai-checkpoint/src/world_bundle.rs:118` (`load`)

**Interfaces:**
- Produces: `pub const manifest::FORMAT_VERSION_CURRENT: u32 = 1_001`; `pub fn manifest::is_supported_version(u32) -> bool`. `FORMAT_VERSION_V1` giữ nguyên tên và giá trị 1.

Ba chỗ kiểm version hiện đang copy-paste cùng một câu `!= FORMAT_VERSION_V1`.
Đó chính là lý do phải gom lại: bỏ sót một chỗ nghĩa là một đường đọc chấp
nhận version mà đường khác từ chối, và người dùng gặp lỗi phụ thuộc vào việc
họ gọi `verify_dir` hay `World::load` trước.

Bump là **minor**: `1_001` đọc được checkpoint `1`. Nâng lên major (2) chỉ khi
có thay đổi khiến v1 không còn đọc nổi.

- [ ] **Step 1: Viết test thất bại trước**

Thêm `mod tests` vào cuối `crates/omiai-checkpoint/src/manifest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_is_a_minor_bump_over_v1() {
        assert_eq!(FORMAT_VERSION_V1, 1);
        assert_eq!(FORMAT_VERSION_CURRENT, 1_001);
        assert!(FORMAT_VERSION_CURRENT > FORMAT_VERSION_V1);
    }

    #[test]
    fn supported_versions_are_exactly_v1_and_current() {
        assert!(is_supported_version(FORMAT_VERSION_V1));
        assert!(is_supported_version(FORMAT_VERSION_CURRENT));
        for bad in [0, 2, 1_000, 1_002, u32::MAX] {
            assert!(!is_supported_version(bad), "version {bad} không được chấp nhận");
        }
    }

    #[test]
    fn write_emits_current_version() {
        let dir = std::env::temp_dir()
            .join(format!("omiai-manifest-ver-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Manifest::write(&dir, &[]).unwrap();
        let m = Manifest::read(&dir).unwrap();
        assert_eq!(m.format_version, FORMAT_VERSION_CURRENT);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

Thêm vào `crates/omiai-checkpoint/tests/` — hoặc vào `mod tests` sẵn có của
`lib.rs` nếu có; nếu chưa có, dùng integration test mới
`crates/omiai-checkpoint/tests/version_gate.rs`:

```rust
//! Cổng version phải nhất quán giữa mọi đường đọc.

use std::path::PathBuf;

use omiai_checkpoint::manifest::{
    is_supported_version, FORMAT_VERSION_CURRENT, FORMAT_VERSION_V1,
};
use omiai_checkpoint::{verify_dir, Manifest};

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("omiai-vergate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Ghi lại manifest.json với format_version khác, giữ nguyên phần còn lại.
fn patch_version(dir: &std::path::Path, version: u32) {
    let path = dir.join("manifest.json");
    let mut v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    v["format_version"] = serde_json::json!(version);
    std::fs::write(&path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}

#[test]
fn verify_dir_accepts_v1_and_current_but_rejects_others() {
    let dir = temp_dir("verify");
    std::fs::write(dir.join("payload.bin"), b"hello").unwrap();
    let rec = omiai_checkpoint::FileRecord {
        path: "payload.bin".to_string(),
        blake3: omiai_checkpoint::hash_file(&dir.join("payload.bin")).unwrap(),
    };
    Manifest::write(&dir, &[rec]).unwrap();

    verify_dir(&dir).expect("version hiện tại phải chấp nhận");
    patch_version(&dir, FORMAT_VERSION_V1);
    verify_dir(&dir).expect("v1 vẫn đọc được — đây là minor bump");
    patch_version(&dir, 2);
    assert!(verify_dir(&dir).is_err(), "version lạ phải từ chối");
    assert!(!is_supported_version(2));
    assert!(is_supported_version(FORMAT_VERSION_CURRENT));

    let _ = std::fs::remove_dir_all(&dir);
}
```

Thêm `serde_json` vào `[dev-dependencies]` của `crates/omiai-checkpoint/Cargo.toml`
không cần — nó đã là `[dependencies]`, integration test dùng được luôn.

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-checkpoint`
Expected: FAIL — `cannot find value FORMAT_VERSION_CURRENT`, `cannot find
function is_supported_version`.

- [ ] **Step 3: Cài**

`crates/omiai-checkpoint/src/manifest.rs`:

```rust
pub const MANIFEST_NAME: &str = "manifest.json";
/// Version gốc của format (slice 1–2): world không có tiếng nói.
pub const FORMAT_VERSION_V1: u32 = 1;
/// Version crate này PHÁT RA. Minor bump: thêm payload
/// `communication/vocabulary.cbor` và khoá `voice` trong `atoms.cbor`.
pub const FORMAT_VERSION_CURRENT: u32 = 1_001;

/// Version nào đọc được. Gom một chỗ vì có ba đường đọc (`verify_dir`,
/// `CellularAutomaton::load`, `World::load`) — bỏ sót một chỗ nghĩa là lỗi
/// phụ thuộc vào việc người dùng gọi hàm nào trước.
pub fn is_supported_version(version: u32) -> bool {
    version == FORMAT_VERSION_V1 || version == FORMAT_VERSION_CURRENT
}
```

Sửa `Manifest::write`: `format_version: FORMAT_VERSION_CURRENT`, và doc
comment của trường: "Bumped on format changes; see `is_supported_version`."

Sửa cả ba chỗ kiểm, mỗi chỗ thành:

```rust
        if !crate::manifest::is_supported_version(manifest.format_version) {
            return Err(CheckpointError::MissingField(format!(
                "unsupported format_version {}",
                manifest.format_version
            )));
        }
```

(`world_bundle.rs:118` hiện trả `CheckpointError::Corrupt` — giữ kiểu lỗi
`Corrupt` ở đó, chỉ thay điều kiện thành `!is_supported_version(...)` và
`expected` thành `format!("format_version 1 hoặc {}", FORMAT_VERSION_CURRENT)`.)

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-checkpoint`
Expected: PASS — 3 unit test + 1 integration test mới; mọi test slice 2 vẫn
xanh (checkpoint ghi mới đọc bằng chính version mới).

Run: `cargo test --workspace` và `cargo clippy --workspace --all-targets`
Expected: PASS / 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-checkpoint/src/manifest.rs crates/omiai-checkpoint/src/lib.rs \
        crates/omiai-checkpoint/src/ca_grid.rs crates/omiai-checkpoint/src/world_bundle.rs \
        crates/omiai-checkpoint/tests/version_gate.rs
git commit -m "$(cat <<'MSG'
feat(checkpoint): format_version 1_001 — minor bump, v1 vẫn đọc được

Gom ba chỗ kiểm version copy-paste thành is_supported_version. Bỏ sót một
chỗ nghĩa là một đường đọc chấp nhận version mà đường khác từ chối, và lỗi
phụ thuộc vào việc người dùng gọi verify_dir hay World::load trước.

Minor: 1_001 đọc checkpoint 1 (world hồi sinh câm). Major chỉ khi v1 không
còn đọc nổi.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 9: payload `communication/vocabulary.cbor` + nạp được checkpoint v1

**Files:**
- Modify: `crates/omiai-checkpoint/src/world_bundle.rs`

**Interfaces:**
- Consumes: `manifest::{FORMAT_VERSION_V1, FORMAT_VERSION_CURRENT, is_supported_version}` (Task 8); `omiai_world::communication::Vocabulary` (Task 1); `Atom.voice` (Task 3).
- Produces: layout checkpoint 5 file; `World::load` khôi phục `vocabulary`, khởi tạo `airwave`, và kiểm 3 bất biến liên-payload mới.

Layout sau task này:

```text
world/grid.bin                    — lưới CA
world/atoms.cbor                  — {step_count, atoms[]} (atom có khoá voice)
world/registry.cbor               — {genomes[]} theo thứ tự slot
world/rng_state.bin               — seed + stream + word_pos
communication/vocabulary.cbor     — bảng đồng xuất hiện (chỉ ở ≥ 1_001)
```

`airwave` KHÔNG có payload: nó phái sinh, `speak` của bước kế ghi lại toàn bộ
trước khi ai đọc. Lưu nó vào checkpoint là mời một bất biến thứ hai vào cửa
(airwave phải khớp với vị trí atom và voice của chúng) mà chẳng đổi lấy gì.

Ba kiểm liên-payload mới, cùng một lý do như kiểm tham chiếu gene ở slice 2 —
**hỏng âm thầm là loại hỏng tệ nhất**:
1. `voice_is_valid()`: arity lỡ cỡ ⇒ có ký hiệu không tồn tại.
2. mỗi arm slot `< n_genomes`: arm mồ côi ⇒ atom im lặng vô cớ.
3. payload vocabulary **phải có** ở `≥ 1_001` và **phải không có** ở `1`:
   version nói một chuyện mà thư mục nói chuyện khác thì có kẻ đã sửa tay,
   và đoán bừa ở đây làm mất số đo tích luỹ của cả lần chạy.

- [ ] **Step 1: Viết test thất bại trước**

Thêm vào cuối `crates/omiai-checkpoint/src/world_bundle.rs` (unit test, KHÔNG
phải integration test: `mod world_bundle` là private và fixture cần
`AtomsFile`/`encode_ca` ở phạm vi crate):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use omiai_world::communication::{SignalValue, StateClass, Vocabulary};
    use omiai_world::registry::FormulaId;
    use omiai_world::world_loop::WorldConfig;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir()
            .join(format!("omiai-wb-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn world_with_voice(seed: u64) -> World {
        let mut w = World::new(
            WorldConfig {
                width: 6,
                height: 6,
                n_initial_atoms: 3,
                initial_resources: 0.1,
            },
            seed,
        );
        for _ in 0..5 {
            w.step();
        }
        w
    }

    /// Hạ cấp một thư mục checkpoint 1_001 xuống đúng hình dạng v1.
    fn downgrade_to_v1(dir: &std::path::Path) {
        #[derive(Serialize)]
        struct V1Atom {
            pos: (usize, usize),
            energy: f64,
            gene: FormulaId,
            age: u64,
        }
        #[derive(Serialize)]
        struct V1AtomsFile {
            step_count: u64,
            atoms: Vec<V1Atom>,
        }

        // 1. atoms.cbor không có khoá `voice`.
        let atoms_path = dir.join(WORLD_DIR).join(ATOMS_FILE);
        let cur: AtomsFile =
            ciborium::de::from_reader(&std::fs::read(&atoms_path).unwrap()[..])
                .unwrap();
        let old = V1AtomsFile {
            step_count: cur.step_count,
            atoms: cur
                .atoms
                .iter()
                .map(|a| V1Atom {
                    pos: a.pos,
                    energy: a.energy,
                    gene: a.gene,
                    age: a.age,
                })
                .collect(),
        };
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&old, &mut buf).unwrap();
        std::fs::write(&atoms_path, &buf).unwrap();

        // 2. v1 không có payload vocabulary.
        std::fs::remove_file(dir.join(COMM_DIR).join(VOCAB_FILE)).unwrap();

        // 3. manifest: 4 record, hash lại, format_version = 1.
        let mut records = Vec::new();
        for name in [GRID_FILE, ATOMS_FILE, REGISTRY_FILE, RNG_FILE] {
            records.push(FileRecord {
                path: format!("{WORLD_DIR}/{name}"),
                blake3: hash_file(&dir.join(WORLD_DIR).join(name)).unwrap(),
            });
        }
        Manifest::write(dir, &records).unwrap();
        let mpath = dir.join(crate::manifest::MANIFEST_NAME);
        let mut v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&mpath).unwrap()).unwrap();
        v["format_version"] =
            serde_json::json!(crate::manifest::FORMAT_VERSION_V1);
        std::fs::write(&mpath, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    }

    #[test]
    fn vocabulary_round_trips_with_the_world() {
        let dir = temp_dir("vocab");
        let w = world_with_voice(1234);
        assert!(w.vocabulary.total > 0, "5 bước phải ghi được mẫu nào đó");
        w.save(&dir).unwrap();

        let back = World::load(&dir).unwrap();
        assert_eq!(back.vocabulary, w.vocabulary);
        assert_eq!(back.atoms, w.atoms);
        assert_eq!(back.step_count, w.step_count);
        assert_eq!(back.rng.get_word_pos(), w.rng.get_word_pos());
        // airwave phái sinh: khởi tạo trống, đúng kích thước.
        assert_eq!(back.airwave.len(), 6 * 6);
        assert!(back.airwave.iter().all(|c| c.is_none()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resume_after_load_is_bit_exact_with_signaling_on() {
        let dir = temp_dir("bitexact");
        let mut a = world_with_voice(555);
        a.save(&dir).unwrap();
        let mut b = World::load(&dir).unwrap();
        for _ in 0..10 {
            a.step();
            b.step();
        }
        assert_eq!(a.ca.cells, b.ca.cells);
        assert_eq!(a.atoms, b.atoms);
        assert_eq!(a.vocabulary, b.vocabulary);
        assert_eq!(a.airwave, b.airwave);
        assert_eq!(a.rng.get_word_pos(), b.rng.get_word_pos());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_1_checkpoint_loads_permanently_silent() {
        let dir = temp_dir("v1");
        let w = world_with_voice(777);
        let word_pos = w.rng.get_word_pos();
        let step_count = w.step_count;
        w.save(&dir).unwrap();
        downgrade_to_v1(&dir);

        let mut back = World::load(&dir).expect("checkpoint v1 phải nạp được");
        assert!(back.atoms.iter().all(|a| a.is_mute()), "v1 ⇒ mọi atom câm");
        assert_eq!(back.vocabulary, Vocabulary::default());
        assert_eq!(back.step_count, step_count);
        assert_eq!(back.rng.get_word_pos(), word_pos, "load không được rút RNG");

        // Chạy tiếp: thế giới đúng, và im lặng vĩnh viễn — MI = 0 là đáp án
        // ĐÚNG, không phải cơ chế hỏng.
        for _ in 0..5 {
            back.step();
        }
        assert!(back.atoms.iter().all(|a| a.is_mute()), "câm phải di truyền");
        assert!(back.airwave.iter().all(|c| c.is_none()));
        assert_eq!(back.vocabulary.mutual_information(), 0.0);
        assert_eq!(
            back.vocabulary.joint[SignalValue::Silent.row()]
                .iter()
                .sum::<u64>(),
            back.vocabulary.total,
            "toàn bộ mẫu phải nằm ở hàng Silent"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn seed_voices_then_resave_upgrades_to_current_version() {
        let dir = temp_dir("revive");
        let w = world_with_voice(888);
        w.save(&dir).unwrap();
        downgrade_to_v1(&dir);

        let mut back = World::load(&dir).unwrap();
        let revived = back.seed_voices();
        assert!(revived > 0, "phải có atom câm để hồi sinh");
        back.save(&dir).unwrap();

        let m = Manifest::read(&dir).unwrap();
        assert_eq!(m.format_version, crate::manifest::FORMAT_VERSION_CURRENT);
        let again = World::load(&dir).unwrap();
        assert!(again.atoms.iter().all(|a| !a.is_mute() && a.voice_is_valid()));
        assert_eq!(again.atoms, back.atoms);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_1_with_vocabulary_payload_is_corrupt() {
        // Version nói v1 mà thư mục có payload 1_001 ⇒ có kẻ sửa tay. Đoán
        // bừa ở đây làm mất số đo tích luỹ của cả lần chạy.
        let dir = temp_dir("v1-extra");
        let w = world_with_voice(999);
        w.save(&dir).unwrap();
        let mpath = dir.join(crate::manifest::MANIFEST_NAME);
        let mut v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&mpath).unwrap()).unwrap();
        v["format_version"] = serde_json::json!(1);
        std::fs::write(&mpath, serde_json::to_vec_pretty(&v).unwrap()).unwrap();

        assert!(matches!(
            World::load(&dir),
            Err(CheckpointError::Corrupt { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn orphan_or_misarity_voice_is_corrupt() {
        // (a) arm trỏ vào slot không tồn tại.
        let dir = temp_dir("orphan");
        let mut w = world_with_voice(1111);
        let bad = FormulaId::from_slot(w.registry.len() as u32 + 7);
        w.atoms[0].voice[0] = bad;
        w.save(&dir).unwrap();
        assert!(matches!(
            World::load(&dir),
            Err(CheckpointError::Corrupt { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);

        // (b) arity lỡ cỡ.
        let dir = temp_dir("misarity");
        let mut w = world_with_voice(2222);
        w.atoms[0].voice.truncate(1);
        w.save(&dir).unwrap();
        assert!(matches!(
            World::load(&dir),
            Err(CheckpointError::Corrupt { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-checkpoint --lib world_bundle`
Expected: FAIL — `cannot find value COMM_DIR` / `VOCAB_FILE`;
`vocabulary_round_trips_with_the_world` vỡ ở `back.vocabulary == w.vocabulary`
(Task 6 để tạm `Default::default()`).

- [ ] **Step 3: Cài**

`crates/omiai-checkpoint/src/world_bundle.rs`:

```rust
const COMM_DIR: &str = "communication";
const VOCAB_FILE: &str = "vocabulary.cbor";
```

Thêm `use omiai_world::communication::Vocabulary;`.

Trong `save`, sau khi ghi 4 payload `world/*`:

```rust
        // 5. vocabulary (payload của version ≥ 1_001)
        let comm_dir = dir.join(COMM_DIR);
        std::fs::create_dir_all(&comm_dir).map_err(|source| {
            CheckpointError::Io { path: comm_dir.clone(), source }
        })?;
        let mut vocab_buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(&self.vocabulary, &mut vocab_buf)
            .map_err(cbor_error)?;
        write_atomic(&comm_dir, VOCAB_FILE, vocab_buf.get_ref())?;
```

và mở rộng phần manifest thành 5 record:

```rust
        let mut records = Vec::with_capacity(5);
        for name in [GRID_FILE, ATOMS_FILE, REGISTRY_FILE, RNG_FILE] {
            let blake3 = hash_file(&world_dir.join(name))?;
            records.push(FileRecord {
                path: format!("{WORLD_DIR}/{name}"),
                blake3,
            });
        }
        records.push(FileRecord {
            path: format!("{COMM_DIR}/{VOCAB_FILE}"),
            blake3: hash_file(&comm_dir.join(VOCAB_FILE))?,
        });
        Manifest::write(dir, &records)
```

Trong `load`, sau khi verify manifest + hash (điều kiện version đã sửa ở
Task 8), đọc vocabulary theo version:

```rust
        // Vocabulary: bắt buộc có ở ≥ 1_001, bắt buộc KHÔNG có ở 1. Version
        // nói một chuyện mà thư mục nói chuyện khác ⇒ có kẻ sửa tay, và đoán
        // bừa ở đây làm mất số đo tích luỹ của cả lần chạy.
        let vocab_rel = format!("{COMM_DIR}/{VOCAB_FILE}");
        let has_vocab = manifest.files.iter().any(|f| f.path == vocab_rel);
        let vocabulary = if manifest.format_version
            == crate::manifest::FORMAT_VERSION_V1
        {
            if has_vocab {
                return Err(CheckpointError::Corrupt {
                    path: dir.join(&vocab_rel),
                    expected: "không có payload vocabulary ở format_version 1"
                        .to_string(),
                    actual: "payload có mặt".to_string(),
                });
            }
            Vocabulary::default()
        } else {
            if !has_vocab {
                return Err(CheckpointError::MissingField(vocab_rel));
            }
            let path = dir.join(&vocab_rel);
            let bytes = std::fs::read(&path).map_err(|source| {
                CheckpointError::Io { path: path.clone(), source }
            })?;
            ciborium::de::from_reader(&bytes[..]).map_err(de_cbor_error)?
        };
```

Bổ sung vào vòng kiểm atom sẵn có (cạnh kiểm `pos` và `gene`):

```rust
            if !atom.voice_is_valid() {
                return Err(CheckpointError::Corrupt {
                    path: world_dir.join(ATOMS_FILE),
                    expected: format!(
                        "voice rỗng hoặc {} arm",
                        omiai_world::communication::N_SYMBOLS
                    ),
                    actual: format!("{} arm", atom.voice.len()),
                });
            }
            for arm in &atom.voice {
                if (arm.slot() as usize) >= n_genomes {
                    return Err(CheckpointError::Corrupt {
                        path: world_dir.join(ATOMS_FILE),
                        expected: format!("voice arm slot < {n_genomes}"),
                        actual: format!("slot {}", arm.slot()),
                    });
                }
            }
```

Và thay hai trường tạm của Task 6 bằng bản thật:

```rust
        Ok(World {
            ca,
            registry: FormulaRegistry::from_genomes_in_order(registry_file.genomes),
            atoms: atoms_file.atoms,
            rng: restore_rng(seed, stream, word_pos),
            rng_seed: seed,
            rng_stream: stream,
            step_count: atoms_file.step_count,
            // Phái sinh: `speak` của bước kế ghi lại toàn bộ trước khi ai đọc.
            airwave: vec![None; ca_len],
            vocabulary,
        })
```

với `let ca_len = ca.width * ca.height;` lấy **trước** khi `ca` bị move vào
struct (thứ tự trường trong literal không giúp gì — borrow checker xét cả
biểu thức).

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-checkpoint`
Expected: PASS — 6 unit test mới + toàn bộ test slice 2.

Run: `cargo test --workspace` và `cargo clippy --workspace --all-targets`
Expected: PASS / 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-checkpoint/src/world_bundle.rs
git commit -m "$(cat <<'MSG'
feat(checkpoint): payload vocabulary + nạp được checkpoint v1 (câm)

Checkpoint version 1 nạp thành một thế giới đúng và im lặng vĩnh viễn: MI = 0
là đáp án ĐÚNG, không phải cơ chế hỏng. load giữ nguyên tính thuần khiết —
không rút RNG để cấp tiếng, vì làm vậy là phá word_pos đã lưu; hồi sinh là
lệnh tường minh World::seed_voices.

Test dựng fixture v1 thật (viết lại atoms.cbor không khoá voice, xoá payload
vocabulary, hash lại, hạ format_version) rồi nạp — đó là cách duy nhất chứng
minh tương thích ngược thay vì tuyên bố nó.

airwave không có payload: nó phái sinh, và lưu nó là mời thêm một bất biến
(airwave phải khớp vị trí atom + voice) mà chẳng đổi lấy gì.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 9b: `world_roundtrip.rs` — resume bit-exact KHI signaling bật

**Files:**
- Modify: `crates/omiai-checkpoint/tests/world_roundtrip.rs`

**Interfaces:**
- Consumes: Task 1–9 (không API mới).
- Produces: không có API mới.

Test `world_save_load_resume_is_bit_exact` của slice 2 vẫn xanh sau Task 9,
nhưng nó **không so `vocabulary`** — nghĩa là một bug ở payload mới đi qua
được nó mà không ai thấy. Task này mở rộng test then chốt để nó chốt cả tầng
giao tiếp, và thêm hai test đòi hỏi API công khai (`seed_voices`, cổng
version) mà unit test trong `world_bundle.rs` không chạm tới được từ ngoài.

- [ ] **Step 1: Viết test thất bại trước**

Thêm 3 assertion vào `world_save_load_resume_is_bit_exact` trong
`crates/omiai-checkpoint/tests/world_roundtrip.rs`.

Sau khối "Trạng thái ngay sau load khớp trạng thái trước save":

```rust
    assert_eq!(
        loaded.vocabulary, resumed.vocabulary,
        "vocabulary phải sống sót qua checkpoint — nó là số đo tích luỹ"
    );
    assert!(
        loaded.airwave.iter().all(|c| c.is_none()),
        "airwave là trạng thái phái sinh, load phải dựng lại rỗng"
    );
    assert_eq!(loaded.airwave.len(), loaded.ca.cells.len());
    assert_eq!(
        loaded.atoms.iter().filter(|a| a.is_mute()).count(),
        resumed.atoms.iter().filter(|a| a.is_mute()).count(),
        "voice phải qua được CBOR"
    );
```

Và sau khối bit-exact cuối cùng:

```rust
    assert_eq!(
        loaded.vocabulary, continuous.vocabulary,
        "số đo sau resume phải khớp thế giới chạy liền — nếu lệch thì speak \
         đã lấy mẫu khác nhau ở hai đường"
    );
    assert_eq!(
        loaded.rng.get_word_pos(),
        continuous.rng.get_word_pos(),
        "word_pos lệch ⇒ resume đã rút thêm/thiếu RNG"
    );
```

Thêm hai test mới vào cuối file:

```rust
#[test]
fn resumed_v1_world_is_silent_until_seed_voices() {
    // Không dựng file v1 bằng tay ở đây (unit test trong world_bundle.rs làm
    // việc đó). Ở đây mô phỏng đúng TÌNH HUỐNG: world câm hoàn toàn.
    let root = std::env::temp_dir()
        .join(format!("omiai-wrt-mute-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000000");
    std::fs::create_dir_all(&cp).unwrap();

    let mut w = World::new(config(), 55);
    for atom in w.atoms.iter_mut() {
        atom.voice.clear();
    }
    w.vocabulary = Default::default();
    w.save(&cp).expect("save");

    let mut loaded = World::load(&cp).expect("load");
    assert!(loaded.atoms.iter().all(|a| a.is_mute()));
    for _ in 0..10 {
        loaded.step();
    }
    assert!(
        loaded.airwave.iter().all(|c| c.is_none()),
        "world câm chạy tiếp vẫn phải im — con của cha câm cũng câm"
    );
    assert_eq!(
        loaded.vocabulary.mutual_information(),
        0.0,
        "MI của thế giới câm là ĐÚNG 0, không phải NaN"
    );
    assert!(loaded.vocabulary.total > 0, "atom câm vẫn được lấy mẫu");

    let revived = loaded.seed_voices();
    assert_eq!(revived, loaded.atoms.len(), "seed_voices cấp voice cho mọi atom câm");
    loaded.step();
    assert!(
        loaded.airwave.iter().any(|c| c.is_some()) || loaded.atoms.is_empty(),
        "sau seed_voices phải có ai nói (trừ khi đã tuyệt chủng)"
    );
    assert_eq!(loaded.seed_voices(), 0, "gọi lại không đổi gì — idempotent");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn saved_checkpoint_declares_current_version_and_vocabulary_payload() {
    let root = std::env::temp_dir()
        .join(format!("omiai-wrt-version-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000002");
    std::fs::create_dir_all(&cp).unwrap();

    let mut w = World::new(config(), 8);
    w.step();
    w.step();
    w.save(&cp).expect("save");
    verify_dir(&cp).expect("manifest + hash ok");

    let manifest = omiai_checkpoint::Manifest::read(&cp).expect("read manifest");
    assert_eq!(
        manifest.format_version,
        omiai_checkpoint::manifest::FORMAT_VERSION_CURRENT
    );
    assert!(
        cp.join("communication").join("vocabulary.cbor").is_file(),
        "payload vocabulary phải được ghi ở version hiện tại"
    );
    let paths: Vec<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        paths.contains(&"communication/vocabulary.cbor"),
        "payload phải được hash vào manifest, nếu không tamper không bắt được: {paths:?}"
    );
    assert_eq!(paths.len(), 5, "v1.1 có 5 payload: {paths:?}");

    let _ = std::fs::remove_dir_all(&root);
}
```

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-checkpoint --test world_roundtrip`
Expected: FAIL trước Task 9. Sau Task 9 phải xanh; khi đó phá để kiểm răng:
tạm bỏ dòng ghi `vocabulary` trong `World::save` ⇒
`saved_checkpoint_declares_current_version_and_vocabulary_payload` phải đỏ ở
assertion `paths.len() == 5`. Hoàn nguyên.

- [ ] **Step 3: Sửa nếu cần**

Nếu `loaded.rng.get_word_pos() != continuous.rng.get_word_pos()`: hợp đồng
thứ tự rút RNG bị vi phạm — kiểm `speak` có rút RNG không (không được), và
kiểm nhánh cha-câm trong `reproduce_and_evolve` có rút RNG không (không
được). Đây là bug thật, không phải sai số.

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test --workspace` và `cargo clippy --workspace --all-targets`
Expected: PASS / 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-checkpoint/tests/world_roundtrip.rs
git commit -m "$(cat <<'MSG'
test(checkpoint): resume bit-exact khi signaling bật + cổng v1 từ ngoài

Test then chốt slice 2 trước đây không so vocabulary, nên bug ở payload mới
đi qua được nó. Giờ so cả vocabulary, cả word_pos (lệch word_pos ⇒ resume đã
rút thêm/thiếu RNG), và chốt airwave được dựng lại RỖNG vì là trạng thái phái
sinh.

Thêm: world câm resume vẫn im mãi, MI đúng 0 chứ không NaN, seed_voices hồi
sinh rồi idempotent; và manifest ghi FORMAT_VERSION_CURRENT với đúng 5 payload
có hash — payload không vào manifest thì tamper không bắt được.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 10: proptest bất biến MI + integration test cơ chế

**Files:**
- Modify: `crates/omiai-world/tests/properties.rs`
- Create: `crates/omiai-world/tests/communication.rs`

**Interfaces:**
- Consumes: toàn bộ Task 1–7 qua API công khai của `omiai-world`.
- Produces: 3 proptest mới; 1 integration test cơ chế.

Integration test này là **lập luận trung tâm của lát cắt**: nó chứng minh cơ
chế *có khả năng* mang thông tin, mà không hứa tiến hoá sẽ tìm ra. Dựng tay
một dân số có quy ước hoàn hảo (arm k ⇔ tài nguyên hướng k) rồi chỉ chạy
`ca_step` + `speak` — không `agent_act`, không sinh sản, không đột biến — thì
ký hiệu là **hàm xác định** của lớp trạng thái, nên MI phải bằng **đúng**
`entropy_state()`. Đó là đẳng thức, mạnh hơn hẳn một bất đẳng thức "MI > 0".

Vì sao bỏ `agent_act`: atom di chuyển rồi thì hai bước khác nhau lấy mẫu ở
hai vị trí khác nhau, đẳng thức vẫn đúng nhưng test hết đọc được lý do khi nó
vỡ. Giữ nó cố định để nguyên nhân duy nhất còn lại là đường dẫn ký hiệu.

- [ ] **Step 1: Viết test thất bại trước**

Thêm vào `proptest!` block của `crates/omiai-world/tests/properties.rs`:

```rust
    #[test]
    fn mutual_information_stays_within_theoretical_bounds(
        w in 6u16..20,
        h in 6u16..20,
        seed in 1u64..1000,
        steps in 1u8..12,
    ) {
        let mut world = World::new(config_for(w, h), seed);
        for _ in 0..steps {
            world.step();
        }
        let mi = world.vocabulary.mutual_information();
        let hs = world.vocabulary.entropy_signal();
        let hm = world.vocabulary.entropy_state();
        prop_assert!(mi.is_finite(), "MI phải hữu hạn, thấy {mi}");
        // MI không kẹp về 0 nên biên dưới là -epsilon (sai số dấu phẩy động
        // của bảng độc lập), KHÔNG phải 0 tuyệt đối.
        prop_assert!(mi >= -1e-9, "MI âm quá dung sai: {mi}");
        prop_assert!(mi <= hs.min(hm) + 1e-9,
            "MI vượt min(H(S),H(M)): {mi} > min({hs},{hm})");
        prop_assert!(hs <= LOG2_5 + 1e-9 && hm <= LOG2_5 + 1e-9,
            "entropy vượt trần log2(5): H(S)={hs}, H(M)={hm}");
    }

    #[test]
    fn vocabulary_total_equals_live_population_at_speak_time(
        w in 6u16..20,
        h in 6u16..20,
        seed in 1u64..1000,
        steps in 1u8..12,
    ) {
        // Chạy phase bằng tay để đếm được dân số ĐÚNG lúc speak. Dùng
        // world.step() thì không quan sát được thời điểm đó, và bất biến này
        // chính là định nghĩa của "thời điểm lấy mẫu MI" trong spec §5.
        let mut world = World::new(config_for(w, h), seed);
        let mut expected = 0u64;
        for _ in 0..steps {
            world.ca_step();
            world.metabolism();
            expected += world.atoms.len() as u64;
            world.speak();
            world.agent_act();
            world.reproduce_and_evolve();
            world.snapshot();
        }
        prop_assert_eq!(world.vocabulary.total, expected);
    }

    #[test]
    fn airwave_is_consistent_with_speakers(
        w in 6u16..20,
        h in 6u16..20,
        seed in 1u64..1000,
        steps in 1u8..10,
    ) {
        let mut world = World::new(config_for(w, h), seed);
        for _ in 0..steps {
            world.step();
        }
        prop_assert_eq!(world.airwave.len(), (w as usize) * (h as usize));
        for sym in world.airwave.iter().flatten() {
            prop_assert!((*sym as usize) < N_SYMBOLS,
                "ký hiệu ngoài bảng chữ: {sym}");
        }
        // Mỗi ô có tiếng ứng với đúng một atom sống lúc speak; sau speak không
        // ai chết nữa trong bước đó (metabolism đứng TRƯỚC speak) và sinh sản
        // chỉ thêm atom ⇒ số ô có tiếng không vượt dân số cuối bước. Nhưng
        // KHÔNG khẳng định ô có tiếng là ô có atom: agent_act có thể đã đưa
        // người nói đi, tiếng vẫn ở lại — airwave đóng băng theo thiết kế.
        let speakers = world.airwave.iter().filter(|c| c.is_some()).count();
        prop_assert!(speakers <= world.atoms.len(),
            "số ô có tiếng ({speakers}) vượt dân số cuối bước ({})",
            world.atoms.len());
    }
```

Thêm vào phần `use` của `properties.rs`:

```rust
use omiai_world::communication::N_SYMBOLS;

/// Trần entropy khi 5 giá trị phân bố đều.
const LOG2_5: f64 = 2.321928094887362;
```

Tạo `crates/omiai-world/tests/communication.rs`:

```rust
//! Integration test CƠ CHẾ: kênh tín hiệu có mang được thông tin không?
//!
//! Không kiểm "tiến hoá tìm ra quy ước" — lát cắt này không hứa điều đó.
//! Kiểm rằng khi quy ước ĐÃ có, thước đo nhìn thấy nó; và khi không ai nói
//! gì, thước đo báo đúng 0.

use omiai_world::atoms::Atom;
use omiai_world::communication::{N_SYMBOLS, VOICE_ATOM_NAMES};
use omiai_world::registry::{FormulaId, Genome};
use omiai_world::world_loop::{World, WorldConfig};
use omiai_core::ltl::LtlFormula;

fn empty_world(w: usize, h: usize, seed: u64) -> World {
    World::new(
        WorldConfig {
            width: w,
            height: h,
            n_initial_atoms: 0,
            initial_resources: 0.0,
        },
        seed,
    )
}

/// Dân số dựng tay: mọi atom dùng cùng một quy ước hoàn hảo
/// (arm k bắn ⇔ tài nguyên ở hướng k), rải đều trên lưới.
fn seed_convention_population(world: &mut World, positions: &[(usize, usize)]) {
    let voice: Vec<FormulaId> = ["res_n", "res_e", "res_s", "res_w"]
        .iter()
        .map(|n| {
            assert!(VOICE_ATOM_NAMES.contains(n), "quy ước phải nói tên trong pool");
            world
                .registry
                .insert(Genome { formula: LtlFormula::atom(*n), fitness: None })
        })
        .collect();
    assert_eq!(voice.len(), N_SYMBOLS);
    let gene = world
        .registry
        .insert(Genome { formula: LtlFormula::atom("open"), fitness: None });
    for &pos in positions {
        world.atoms.push(Atom {
            pos,
            energy: 1.0,
            gene,
            age: 0,
            voice: voice.clone(),
        });
    }
}

/// Rải tài nguyên thưa để có đủ cả 5 lớp trạng thái xuất hiện.
fn scatter_resources(world: &mut World) {
    for (i, cell) in world.ca.cells.iter_mut().enumerate() {
        if i % 7 == 3 {
            *cell = 2;
        } else if i % 11 == 5 {
            *cell = 3;
        }
    }
}

#[test]
fn perfect_convention_makes_signal_a_function_of_state() {
    let mut world = empty_world(12, 12, 4242);
    scatter_resources(&mut world);
    let positions: Vec<(usize, usize)> =
        (1..11).step_by(3).flat_map(|x| (1..11).step_by(3).map(move |y| (x, y))).collect();
    seed_convention_population(&mut world, &positions);

    // CHỈ ca_step + speak: atom không di chuyển, không sinh sản, không đột
    // biến. Nguyên nhân duy nhất còn lại nếu test vỡ là đường dẫn ký hiệu.
    for _ in 0..40 {
        world.ca_step();
        world.speak();
    }

    let v = &world.vocabulary;
    assert_eq!(v.total, (positions.len() * 40) as u64);
    assert!(v.entropy_state() > 0.0, "lưới phải sinh nhiều hơn một lớp trạng thái");

    // Ký hiệu là HÀM XÁC ĐỊNH của lớp trạng thái ⇒ H(S|M) = 0 ⇒ I = H(S).
    // Và quy ước là song ánh trên các lớp xuất hiện ⇒ I = H(M) luôn.
    let mi = v.mutual_information();
    assert!(
        (mi - v.entropy_state()).abs() < 1e-9,
        "quy ước hoàn hảo phải cho MI = H(M): MI={mi}, H(M)={}",
        v.entropy_state()
    );
    assert!(
        (mi - v.entropy_signal()).abs() < 1e-9,
        "và MI = H(S): MI={mi}, H(S)={}",
        v.entropy_signal()
    );
    assert!(mi <= 2.321928094887363, "không được vượt trần log2(5)");

    // Mỗi ký hiệu chỉ đi cùng đúng một lớp trạng thái.
    for row in 1..=N_SYMBOLS {
        let nonzero = v.joint[row].iter().filter(|&&c| c > 0).count();
        assert!(nonzero <= 1, "ký hiệu {} đi với {} lớp trạng thái", row - 1, nonzero);
    }
}

#[test]
fn mute_population_measures_exactly_zero() {
    let mut world = empty_world(12, 12, 4242);
    scatter_resources(&mut world);
    let gene = world
        .registry
        .insert(Genome { formula: LtlFormula::atom("open"), fitness: None });
    for x in (1..11).step_by(3) {
        for y in (1..11).step_by(3) {
            world.atoms.push(Atom {
                pos: (x, y),
                energy: 1.0,
                gene,
                age: 0,
                voice: Vec::new(),
            });
        }
    }
    let n = world.atoms.len() as u64;

    for _ in 0..40 {
        world.ca_step();
        world.speak();
    }

    let v = &world.vocabulary;
    assert_eq!(v.total, n * 40, "atom câm vẫn được lấy mẫu");
    assert_eq!(v.mutual_information(), 0.0);
    assert_eq!(v.entropy_signal(), 0.0, "một giá trị tín hiệu duy nhất ⇒ H(S) = 0");
    assert!(v.entropy_state() > 0.0, "phía trạng thái vẫn có entropy");
    assert!(world.airwave.iter().all(|c| c.is_none()));
    for k in 0..N_SYMBOLS {
        assert_eq!(v.symbol_frequency(k as u8), 0.0);
    }
}

#[test]
fn signal_changes_receiver_behaviour_in_a_full_step() {
    // Cùng thế giới, cùng seed, khác duy nhất một điều: người nghe có genome
    // đọc `hear*` hay không. Nếu tín hiệu không tới được phía nhận thì hai
    // quỹ đạo giống nhau và test này vỡ.
    let build = |listen: bool| {
        let mut world = empty_world(9, 9, 606);
        // Tài nguyên phía Đông của người nói.
        world.ca.cells[4 * 9 + 6] = 3;
        let voice: Vec<FormulaId> = ["res_n", "res_e", "res_s", "res_w"]
            .iter()
            .map(|n| {
                world
                    .registry
                    .insert(Genome { formula: LtlFormula::atom(*n), fitness: None })
            })
            .collect();
        let quiet_gene = world
            .registry
            .insert(Genome { formula: LtlFormula::False_, fitness: None });
        let listen_gene = world.registry.insert(Genome {
            formula: LtlFormula::and(
                LtlFormula::atom("open"),
                LtlFormula::atom("hear1"),
            ),
            fitness: None,
        });
        // Người nói ở (5,4) — tài nguyên ở (6,4) là phía Đông ⇒ ký hiệu 1.
        world.atoms.push(Atom {
            pos: (5, 4),
            energy: 1.0,
            gene: quiet_gene,
            age: 0,
            voice,
        });
        // Người nghe ở (4,4), kề phía Tây của người nói.
        world.atoms.push(Atom {
            pos: (4, 4),
            energy: 1.0,
            gene: if listen { listen_gene } else { quiet_gene },
            age: 0,
            voice: Vec::new(),
        });
        world.metabolism();
        world.speak();
        world.agent_act();
        world
    };

    let deaf = build(false);
    let hearing = build(true);
    // Người nói ghi ký hiệu vào Ô CỦA CHÍNH NÓ (5,4); tầm với 1 ô đến từ phía
    // nhận — `heard` của một hướng là airwave ở ô kề theo hướng đó (spec §3).
    // Nhờ vậy atom không bao giờ tự nghe mình, do cấu trúc.
    assert_eq!(deaf.airwave[4 * 9 + 5], Some(1), "người nói phải nói ký hiệu 1");
    assert_eq!(hearing.airwave[4 * 9 + 5], Some(1));
    assert_eq!(deaf.airwave[4 * 9 + 4], None, "ô người nghe không có ai nói");
    assert_eq!(deaf.atoms[1].pos, (4, 4), "genome False_ thì đứng yên");
    assert_ne!(
        hearing.atoms[1].pos,
        (4, 4),
        "nghe được ký hiệu 1 thì phải hành động — tín hiệu chưa tới phía nhận"
    );
}
```

Thêm `omiai-core` vào `[dev-dependencies]`? Không cần — nó đã là
`[dependencies]` của `omiai-world`, integration test dùng được.

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world --test communication --test properties`
Expected: FAIL. Trước Task 1–7 thì lỗi biên dịch; **sau** Task 1–7 các test
này phải xanh ngay — nếu vậy, cố tình phá để chứng minh test có răng:
đổi `speak` thành `if let SignalValue::Sym(_) = signal { }` (không ghi airwave)
rồi chạy lại và xác nhận `perfect_convention_makes_signal_a_function_of_state`
vẫn xanh nhưng `signal_changes_receiver_behaviour_in_a_full_step` vỡ; rồi
hoàn nguyên. Ghi lại kết quả kiểm chứng này trong commit message.

- [ ] **Step 3: Sửa nếu cần**

Nếu `perfect_convention_makes_signal_a_function_of_state` vỡ ở
`entropy_state() > 0.0`, chỉ mật độ tài nguyên của `scatter_resources` là sai
(cần nhiều hơn một lớp trạng thái xuất hiện) — điều chỉnh `i % 7` / `i % 11`,
KHÔNG nới lỏng assertion. Nếu vỡ ở đẳng thức `mi == entropy_state()`, đó là
bug thật ở `state_class` hoặc `decode_voice`: quy ước là song ánh nên đẳng
thức phải đúng chính xác.

- [ ] **Step 4: Chạy lại, phải xanh**

Run: `cargo test -p omiai-world`
Expected: PASS — 3 proptest (64 case mỗi cái) + 3 integration test.

Run: `cargo test --workspace` và `cargo clippy --workspace --all-targets`
Expected: PASS / 0 cảnh báo.

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/tests/properties.rs crates/omiai-world/tests/communication.rs
git commit -m "$(cat <<'MSG'
test(world): bất biến MI + test cơ chế kênh tín hiệu

Test trung tâm của lát cắt: dân số dựng tay có quy ước hoàn hảo, chỉ chạy
ca_step + speak, thì ký hiệu là hàm xác định của lớp trạng thái nên MI phải
bằng ĐÚNG entropy_state() — đẳng thức, không phải "MI > 0". Dân số câm cho
đúng 0. Đây là bằng chứng kênh CÓ KHẢ NĂNG mang thông tin; không có tuyên bố
nào về việc tiến hoá sẽ tìm ra quy ước.

Proptest chốt biên: 0 ≤ MI ≤ min(H(S),H(M)) ≤ log2(5), và total đúng bằng
tổng dân số tại thời điểm speak (chạy phase bằng tay để quan sát được đúng
thời điểm đó).

Đã kiểm chứng test có răng bằng cách tạm bỏ ghi airwave trong speak.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 11: example `communication_demo` — số đo thật để dán vào README

**Files:**
- Create: `crates/omiai-world/examples/communication_demo.rs` (thư mục
  `examples/` CHƯA tồn tại — tạo mới; Cargo tự nhận, không cần khai báo
  `[[example]]` trong `Cargo.toml`)
- Create: `crates/omiai-world/tests/demo_smoke.rs`

**Interfaces:**
- Consumes: `World`, `WorldConfig`, `World::seed_voices`, `Vocabulary`
  (`mutual_information`, `entropy_signal`, `entropy_state`,
  `symbol_frequency`, `total`).
- Produces: số MI thật, dùng làm nguồn duy nhất cho con số trong README
  ở Task 12. **Không được đoán con số này.**

Example là cách duy nhất trong lát cắt này để lấy ra một con số trung thực:
"chạy 500 bước, lưới 48×48, seed 1 → MI = X bit". Test `demo_smoke.rs` bọc
đúng cấu hình đó lại thành một test để con số trong README không bao giờ trôi
đi mà không ai biết — nhưng chỉ chốt biên (hữu hạn, ≤ log2 5), KHÔNG chốt giá
trị chính xác, vì như thế thì mọi thay đổi hợp lệ ở ecology cũng làm test đỏ.

- [ ] **Step 1: Viết test thất bại trước**

Tạo `crates/omiai-world/tests/demo_smoke.rs`:

```rust
//! Chốt rằng cấu hình demo trong README chạy được và số đo hợp lệ.
//! KHÔNG chốt giá trị MI chính xác — nó là kết quả quan sát, không phải
//! hợp đồng; chốt cứng sẽ khoá mọi tinh chỉnh ecology về sau.

use omiai_world::world_loop::{World, WorldConfig};

/// Trần entropy khi 5 giá trị phân bố đều.
const LOG2_5: f64 = 2.321928094887362;

fn demo_config() -> WorldConfig {
    WorldConfig {
        width: 48,
        height: 48,
        n_initial_atoms: 24,
        initial_resources: 0.10,
    }
}

#[test]
fn readme_demo_configuration_runs_and_measures() {
    let mut world = World::new(demo_config(), 1);
    let revived = world.seed_voices();
    assert_eq!(revived, 0, "World::new đã cấp voice, không còn ai câm để hồi sinh");
    for _ in 0..500 {
        world.step();
    }
    assert_eq!(world.step_count, 500);

    let v = &world.vocabulary;
    assert!(v.total > 0, "phải có mẫu — nếu 0 thì dân số tuyệt chủng ngay bước đầu");
    let mi = v.mutual_information();
    assert!(mi.is_finite(), "MI phải hữu hạn, thấy {mi}");
    assert!(mi >= -1e-9 && mi <= LOG2_5 + 1e-9, "MI ngoài biên lý thuyết: {mi}");
    assert!(v.entropy_signal() <= LOG2_5 + 1e-9);
    assert!(v.entropy_state() <= LOG2_5 + 1e-9);

    // Tần suất là phân bố xác suất trên 4 ký hiệu + im lặng ⇒ tổng ≤ 1.
    let sum: f64 = (0..4).map(|k| v.symbol_frequency(k as u8)).sum();
    assert!(sum <= 1.0 + 1e-9, "tổng tần suất ký hiệu > 1: {sum}");
}

#[test]
fn demo_configuration_is_deterministic() {
    let run = || {
        let mut w = World::new(demo_config(), 1);
        for _ in 0..120 {
            w.step();
        }
        (w.vocabulary.clone(), w.atoms.len(), w.rng.get_word_pos())
    };
    assert_eq!(run(), run(), "cùng seed phải cho cùng số đo — bit-exact");
}
```

`get_word_pos` cần `use rand_chacha::rand_core::...`? Không: nó là method
inherent của `ChaCha8Rng`, nhưng `rand_chacha` phải có trong
`[dev-dependencies]` của `omiai-world` để integration test gọi được. Nó đã
là `[dependencies]` — dùng được, không thêm gì.

- [ ] **Step 2: Chạy để thấy nó vỡ**

Run: `cargo test -p omiai-world --test demo_smoke`
Expected: FAIL — `error[E0599]: no method named 'seed_voices'` nếu Task 7 chưa
xong; nếu Task 1–7 đã xong thì test này phải xanh và chuyển sang Step 3.

- [ ] **Step 3: Viết example**

Tạo `crates/omiai-world/examples/communication_demo.rs`:

```rust
//! Demo tầng giao tiếp: chạy world N bước rồi in số đo thông tin tương hỗ
//! giữa ký hiệu phát ra và hướng tài nguyên gần nhất quanh người nói.
//!
//! Chạy: `cargo run -p omiai-world --example communication_demo --release`
//! Tuỳ chọn: `--example communication_demo -- <steps> <seed>`
//!
//! Con số MI in ra là KẾT QUẢ QUAN SÁT, không phải mục tiêu đã đạt. Lát cắt
//! này cài cơ chế và thước đo; nó không hứa tiến hoá sẽ tìm ra quy ước.

use omiai_world::communication::{N_SYMBOLS, N_SIGNAL_VALUES, N_STATE_CLASSES};
use omiai_world::world_loop::{World, WorldConfig};

const CLASS_LABELS: [&str; N_STATE_CLASSES] = ["N", "E", "S", "W", "none"];

fn main() {
    let mut args = std::env::args().skip(1);
    let steps: u64 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);

    let config = WorldConfig {
        width: 48,
        height: 48,
        n_initial_atoms: 24,
        initial_resources: 0.10,
    };
    println!(
        "world {}x{}, {} atom mồi, resource density {:.2}, seed {}, {} bước",
        config.width,
        config.height,
        config.n_initial_atoms,
        config.initial_resources,
        seed,
        steps
    );

    let mut world = World::new(config, seed);
    let mut extinct_at: Option<u64> = None;
    for i in 0..steps {
        world.step();
        if world.atoms.is_empty() && extinct_at.is_none() {
            extinct_at = Some(i + 1);
        }
    }

    let v = &world.vocabulary;
    println!("\n-- dân số --");
    println!("atom còn sống      : {}", world.atoms.len());
    println!("genome trong registry: {}", world.registry.len());
    println!("atom câm           : {}", world.atoms.iter().filter(|a| a.is_mute()).count());
    if let Some(step) = extinct_at {
        println!("CẢNH BÁO: tuyệt chủng lần đầu tại bước {step}");
    }

    println!("\n-- thước đo thông tin ({} mẫu) --", v.total);
    println!("H(signal)          : {:.6} bit", v.entropy_signal());
    println!("H(state)           : {:.6} bit", v.entropy_state());
    println!("I(signal; state)   : {:.6} bit", v.mutual_information());
    println!("trần lý thuyết      : {:.6} bit (log2 {N_SIGNAL_VALUES})", (N_SIGNAL_VALUES as f64).log2());

    println!("\n-- tần suất ký hiệu --");
    for k in 0..N_SYMBOLS {
        println!("  sym {k}            : {:.4}", v.symbol_frequency(k as u8));
    }
    let silence: f64 = 1.0 - (0..N_SYMBOLS).map(|k| v.symbol_frequency(k as u8)).sum::<f64>();
    println!("  im lặng          : {silence:.4}");

    println!("\n-- bảng joint (hàng = tín hiệu, cột = lớp trạng thái) --");
    print!("{:>8}", "");
    for label in CLASS_LABELS {
        print!("{label:>8}");
    }
    println!();
    for (row, counts) in v.joint.iter().enumerate() {
        let name = if row == 0 { "silent".to_string() } else { format!("sym {}", row - 1) };
        print!("{name:>8}");
        for c in counts {
            print!("{c:>8}");
        }
        println!();
    }

    if v.mutual_information() < 1e-6 {
        println!(
            "\nMI ≈ 0: ký hiệu và hướng tài nguyên độc lập trong lần chạy này.\n\
             Đó là kết quả hợp lệ — cơ chế có sẵn, áp lực chọn lọc chưa buộc\n\
             ai phải dùng nó (xem spec §7, phần 'không hứa hội tụ')."
        );
    }
}
```

- [ ] **Step 4: Chạy và ghi lại con số**

Run: `cargo run -p omiai-world --example communication_demo --release`
Expected: in ra bảng đầy đủ, không panic. **Chép nguyên số `I(signal; state)`
và số atom còn sống vào ghi chú để Task 12 dán vào README.** Nếu dân số tuyệt
chủng sớm (`atom còn sống: 0`), thử `--release -- 500 7` vài seed và ghi lại
seed đã dùng — README phải nói đúng seed nào cho ra con số nào.

Run: `cargo test -p omiai-world --test demo_smoke`
Expected: PASS (2 test).

Run: `cargo test --workspace` và `cargo clippy --workspace --all-targets`
Expected: PASS / 0 cảnh báo. Ghi chú: clippy kiểm cả `--examples` qua
`--all-targets`, nên `println!` với inline arg phải viết `{k}` chứ không
`{}` , k` (rule `uninlined_format_args`).

- [ ] **Step 5: Commit**

```bash
git add crates/omiai-world/examples/communication_demo.rs crates/omiai-world/tests/demo_smoke.rs
git commit -m "$(cat <<'MSG'
feat(world): example communication_demo + smoke test cấu hình README

Example in ra H(signal), H(state), I(signal;state), tần suất ký hiệu và bảng
joint đầy đủ, kèm giải thích khi MI ≈ 0 rằng đó là kết quả hợp lệ — cơ chế có
sẵn nhưng áp lực chọn lọc chưa buộc ai dùng nó.

demo_smoke chốt biên (hữu hạn, ≤ log2 5, tổng tần suất ≤ 1) và tính
deterministic của cấu hình README; KHÔNG chốt giá trị MI chính xác vì đó là
kết quả quan sát, không phải hợp đồng.

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

### Task 12: tài liệu — ADR-0007, format-spec, README

**Files:**
- Create: `docs/adr/0007-signal-channel-one-step.md`
- Modify: `docs/format-spec/checkpoint-v1.md` (§2 bảng field, §5b, §5c mới, §6)
- Modify: `README.md` (mô tả `omiai-world`, mục "What's scaffolded", build
  order item 4–5, số test)

**Interfaces:**
- Consumes: mọi thứ Task 1–11 đã cài; con số MI đo được ở Task 11.
- Produces: không có API mới — đây là task đóng sổ, biến "đã làm" thành "đã
  ghi đúng những gì đã làm".

Ngôn ngữ: ADR / format-spec / README viết **tiếng Anh** (nhất quán với
0001–0006 và toàn bộ docs hiện có), doc comment trong code viết tiếng Việt.

- [ ] **Step 1: Viết ADR-0007**

Tạo `docs/adr/0007-signal-channel-one-step.md`:

```markdown
# ADR-0007: Signal channel — one-step broadcast, frozen airwave

## Context

Slice 3 adds inter-agent communication to `omiai-world`. The design space
had two axes that could not both be deferred:

1. **Reach and lifetime of a signal.** Options: a persistent pheromone
   field written into the CA grid; a one-step broadcast to the 4 adjacent
   cells; point-to-point addressed messages.
2. **How a symbol is chosen.** Options: a single formula whose truth
   value is one bit; K formula arms where the first true arm names the
   symbol; a learned mapping outside the genome.

The constraint that decides both: the world loop must stay bit-exactly
resumable from a checkpoint (ADR-0006), and the project's ethos is that
behaviour comes from evaluating LTL genomes, never from an opaque
learned table.

## Decision

**One-step broadcast into a frozen airwave, symbols from K formula arms.**

- `Symbol = u8`, `N_SYMBOLS = 4`. Silence is a fifth *signal value*, not
  a symbol: `N_SIGNAL_VALUES = 5`.
- `World.airwave: Vec<Option<Symbol>>`, one slot per grid cell. The
  `speak` phase computes every symbol into a local buffer, then writes
  the airwave once. For the rest of the step the airwave is read-only.
- A speaker writes its symbol into **its own cell**. Reach comes from the
  receiving side: `Observation.heard` for a direction is the airwave of the
  adjacent cell in that direction, so a symbol is audible in exactly the 4
  cells around the speaker and an atom can never hear itself. Making
  non-hearing-of-self structural beats enforcing it with an index check.
- `Atom.voice: Vec<FormulaId>` holds either 0 arms (mute) or exactly
  `N_SYMBOLS` arms. The emitted symbol is the index of the first arm
  that evaluates true under the speaker's neighbourhood valuation;
  if none is true, the speaker is silent.
- Voice arms are evaluated over 16 directional propositions
  (`{open,wall,res,occupied} × {_n,_e,_s,_w}`); movement genes gain 4
  aggregate `hear0..hear3` propositions.
- **Voice arms never read `hear*`.**
- The airwave is derived state and is **not** checkpointed.
- `speak` consumes no RNG.

## Consequences

- **Order-independence is structural, not tested-into-existence.** Because
  every symbol is computed before any is published, shuffling the atom
  `Vec` cannot change the resulting airwave. A single-pass
  compute-and-publish loop would have made symbols depend on `Vec` order,
  which is exactly the kind of hidden ordering dependency that breaks
  bit-exact resume when a later slice reorders atoms for cache locality.
- **Voice ⊥ heard is what buys that.** If a voice arm could read `hear*`,
  either it reads this step's airwave — order-dependent again — or it
  reads the previous step's, which makes the airwave persistent state
  needing its own checkpoint payload and its own version bump.
- **Echo and relay are therefore not expressible in slice 3.** An atom
  cannot repeat what it heard. This is a real capability cost, accepted
  deliberately: it is the price of an airwave that needs no checkpoint
  payload. A future slice that wants relay must checkpoint the airwave
  and bump the format version.
- **No RNG in `speak`** means adding communication does not shift the
  RNG draw sequence of an existing run at the point of speaking; only
  `World::new` and reproduction draw for voice, both at defined points
  (see the draw-order contract in the plan's Global Constraints).
- The 16 directional propositions make a voice arm able to say something
  about *where* a resource is, which is what makes a Lewis-style
  signaling convention possible at all. The cost is a larger valuation
  map built per speaker per step.
- Silence being a signal *value* rather than an absence means a fully
  mute world measures mutual information exactly `0`, not `NaN`.

## Known limits — what this channel cannot be read as claiming

- **The receiver has no memory.** Movement policies are propositional
  formulas evaluated on the current step, so no lineage can navigate
  toward a referent two cells away. What the channel can support is a
  convention plus clustering toward food-rich regions — not full
  referential naming.
- **"Move toward whoever said k" is structurally impossible.** The
  speaker's cell is always `occupied`, and `decide` only moves into
  passable cells. A gene can react to hearing `k`, but not by walking at
  the speaker.
- **The movement mutation pool grows from 4 names to 8.** That dilutes
  selection pressure on feeding behaviour: a mutation now has a 50%
  chance of landing on a hearing proposition. Accepted without
  compensating tweaks, because retuning ecology constants in the same
  slice that introduces the channel would make any measured change
  unattributable. If a later slice finds feeding behaviour degraded, this
  is the first place to look.
- **`hear*` is aggregate, not directional.** `heark` is true in every
  direction when any adjacent atom said `k`. Directional hearing would
  push the movement pool to 20 names; there is no evidence yet that the
  extra resolution is needed.
```

- [ ] **Step 2: Cập nhật format-spec**

Trong `docs/format-spec/checkpoint-v1.md`:

Sửa hàng `format_version` của bảng §2 thành:

```markdown
| `format_version` | u32 | encoded `major * 1000 + minor`; writers emit `1_001` (v1.1). A reader accepts `1` and `1_001` and rejects anything else with an error — never silently adapts. See §6. |
```

Sửa hàng `atoms.cbor` của bảng §5b thành:

```markdown
| `atoms.cbor` | CBOR `{step_count: u64, atoms: [{pos, energy, gene, age, voice}]}` — `voice` is absent in v1 files and deserializes to the empty vector (mute) |
```

Thêm vào cuối §5b, ngay trước dòng "Bit-exact resume is test-enforced":

```markdown
- Load also checks the **voice invariant**: `voice.len()` is `0` or
  `N_SYMBOLS`, and every voice arm slot exists in `registry.cbor`.
  Anything else is `Corrupt`.
- `airwave` and `vocabulary` are **not** in the bundle. The airwave is
  derived state rebuilt empty on load (ADR-0007); the vocabulary lives in
  §5c because it is accumulated measurement, not world state.
```

Thêm §5c mới ngay sau §5b:

```markdown
## 5c. `communication/vocabulary.cbor` (slice 3, implemented)

| file | content |
|---|---|
| `communication/vocabulary.cbor` | CBOR `{joint: [[u64; 5]; 5], total: u64}` |

- `joint[row][col]`: `row` 0 is silence, rows 1..=4 are symbols 0..=3;
  `col` 0..=3 are resource directions N/E/S/W and col 4 is "no adjacent
  resource". `total` is the number of samples, one per atom alive at
  `speak` time per step — mute atoms included, as a silence row.
- **Required at `format_version >= 1_001`**; its absence there is
  `CheckpointError::MissingField`.
- **Required absent at `format_version == 1`**; its presence there is
  `Corrupt` — a v1 directory carrying a v1.1 payload is a writer bug, and
  guessing which one to believe is exactly the silent adaptation §2 bans.
- Hashed into `manifest.json` as `communication/vocabulary.cbor` like any
  other payload. A v1 bundle has 4 file records; v1.1 has 5.
- Loading a v1 directory yields a world whose atoms are all mute and
  whose vocabulary is empty. That is the correct answer, not a
  degradation: the run being resumed genuinely had no communication.
  `load` never draws RNG to grant voice — that would desynchronize the
  saved `word_pos` and break bit-exact resume. Granting voice to a
  resumed v1 world is an explicit, separate call:
  `World::seed_voices(&mut self) -> usize`.
```

Sửa §6 thành:

```markdown
## 6. Compatibility policy

`format_version` encodes `major * 1000 + minor`.

- A **v1.1 reader MUST read v1** directories. Enforced by
  `is_supported_version` plus a downgrade-fixture test that rewrites a
  real checkpoint to v1 (drops `voice` from `atoms.cbor`, deletes the
  vocabulary payload, re-hashes) and loads it.
- Adding a new payload or optional field = **minor** bump (`1` → `1_001`).
  A reader must know, per version, which payloads are required and which
  must be absent — see §5c.
- Changing or removing a field, or altering any byte layout here = major
  bump to `2`, with a migration note.
- **Scope of the bit-exactness claim.** "Bit-exact resume" means: load a
  directory, continue stepping, and the trajectory matches an
  uninterrupted run from the same seed — for the state the format
  persists. Derived state (`airwave`) is rebuilt, and it is empty for the
  step boundary at which checkpoints are taken, so nothing is lost. A v1
  directory resumes bit-exactly as the mute world it was; calling
  `seed_voices` afterwards is a new trajectory, not a resume.
```

- [ ] **Step 3: Cập nhật README**

Thêm vào cuối bullet `omiai-world` (sau "and world-invariant proptests."):

```markdown
  Slice 3 adds the communication layer: a `speak` phase between
  metabolism and action broadcasts a symbol from each atom's K-arm voice
  gene into its 4 adjacent cells (frozen airwave, ADR-0007), movement
  genes gain `hear0..hear3` propositions, and a `Vocabulary` measures
  mutual information between emitted symbol and the direction of the
  nearest adjacent resource. The mechanism is implemented and measured;
  **no convergence to a shared convention is claimed** — see the
  measured numbers below.
```

Trong "What's scaffolded", thay bullet `omiai-world` bằng:

```markdown
- `omiai-world`: multi-species ecology — the substrate, agents, world
  loop, communication layer and checkpoint resume are real; multiple
  species with distinct ecological roles are not.
```

Sửa build order item 4–5 thành:

```markdown
4. ~~Checkpoint payloads for every pillar + retention policy~~ — world
   bundle ✅ + vocabulary ✅ + retention ✅ · other pillars' payloads remain
5. ~~`omiai-world` communication layer~~ ✅ done and measured ·
   `omiai-cli resume` next (the `resume` entry point belongs to
   `omiai-cli`, not `omiai-runtime` — `omiai-runtime` loads an exported
   bundle and must not depend on training/evolution code)
```

Thêm một mục mới ngay trước `## Building`:

```markdown
## Measured: does the signal channel carry information?

```sh
cargo run -p omiai-world --example communication_demo --release
```

48×48 grid, 24 seed atoms, resource density 0.10, seed 1, 500 steps:

| quantity | value |
|---|---|
| samples | `<TOTAL>` |
| H(signal) | `<HS>` bits |
| H(state) | `<HM>` bits |
| **I(signal; state)** | **`<MI>` bits** |
| theoretical ceiling | 2.321928 bits (log₂ 5) |

`<INTERPRETATION>`

What is proven and what is not: `tests/communication.rs` builds a
population with a perfect convention by hand and shows the measure sees
it exactly — mutual information equals `H(state)` to floating-point
tolerance — and shows a mute population measures exactly `0`. That is a
proof the **channel can carry information**. Whether mutation and
selection *find* a convention in a given run is an empirical question
this slice measures and does not promise.
```

Cập nhật số test trong khối `## Building`:

```markdown
cargo test --workspace          # <N> tests, all passing
```

- [ ] **Step 4: Điền số thật và kiểm chứng**

Đây là bước dễ gian lận nhất trong cả kế hoạch, nên nó có kiểm tra riêng.

1. Chạy `cargo run -p omiai-world --example communication_demo --release`
   và thay `<TOTAL>`, `<HS>`, `<HM>`, `<MI>` bằng con số **in ra thật**.
2. Viết `<INTERPRETATION>` theo đúng cái đã thấy — nếu MI ≈ 0 thì viết
   "Mutual information is ≈ 0 in this run: symbol and resource direction
   are independent. The channel exists and is measured; nothing in the
   current ecology rewards using it." **Không** viết một câu lạc quan hơn
   dữ liệu.
3. Chạy `cargo test --workspace 2>&1 | tail -40`, cộng đúng tổng số test
   qua mọi binary, thay `<N>`. Không ước lượng.
4. `grep -rn "TODO\|TBD\|<MI>\|<HS>\|<HM>\|<TOTAL>\|<N>\|<INTERPRETATION>" README.md docs/format-spec/checkpoint-v1.md docs/adr/0007-signal-channel-one-step.md`
   Expected: không có kết quả nào.
5. `grep -n "omiai-runtime" README.md` — xác nhận không còn dòng nào nói
   `resume` thuộc `omiai-runtime`.

Run: `cargo test --workspace` và `cargo clippy --workspace --all-targets`
Expected: PASS / 0 cảnh báo. Commit này chỉ sửa tài liệu nên không thể làm
vỡ biên dịch, nhưng chạy lại là rẻ và nó chốt trạng thái cuối cùng của lát
cắt — con số test trong README phải là con số của chính lần chạy này.

- [ ] **Step 5: Commit**

```bash
git add docs/adr/0007-signal-channel-one-step.md docs/format-spec/checkpoint-v1.md README.md
git commit -m "$(cat <<'MSG'
docs: ADR-0007 kênh tín hiệu, format-spec v1.1, README số đo thật

ADR-0007 ghi lại quyết định broadcast 1 bước + airwave đóng băng + voice ⊥
hear, kèm chi phí đã chấp nhận: echo/relay không biểu diễn được trong lát cắt
này, và lý do vì sao (airwave persistent sẽ cần payload checkpoint riêng).

format-spec: format_version thành major*1000+minor, §5c payload vocabulary
(bắt buộc có ở >= 1_001, bắt buộc vắng ở == 1), và khoanh lại phạm vi tuyên
bố bit-exact resume.

README: sửa build order — resume thuộc omiai-cli, KHÔNG phải omiai-runtime
(runtime không được phụ thuộc code huấn luyện/tiến hoá); chuyển communication
ra khỏi mục "scaffolded"; thêm bảng số đo MI thật từ example, kèm phân biệt rõ
"kênh mang được thông tin" (đã chứng minh) và "tiến hoá tìm ra quy ước" (không
hứa).

Co-Authored-By: Claude <noreply@anthropic.com>
MSG
)"
git push origin main
```

---

## Definition of Done cho toàn lát cắt

Lát cắt 3 xong khi **tất cả** đúng, kiểm được bằng lệnh:

1. `cargo test --workspace` xanh, `cargo clippy --workspace --all-targets`
   0 cảnh báo.
2. `cargo test -p omiai-world --test communication` xanh — kênh mang được
   thông tin (đẳng thức MI = H(state) cho quy ước dựng tay) và im lặng cho
   đúng 0.
3. `cargo test -p omiai-checkpoint` xanh, bao gồm fixture hạ cấp v1 thật
   (không phải mock) và cổng version.
4. `cargo run -p omiai-world --example communication_demo --release` in ra
   số đo, và **cùng con số đó** nằm trong README.
5. README không còn nói `resume` thuộc `omiai-runtime`, và không còn liệt
   communication trong mục "scaffolded".
6. Ba tính chất ngữ nghĩa của Task 6b đều có test và mỗi test đã được chứng
   minh là có răng (phá đúng một chỗ ⇒ đúng test đó đỏ).
7. `cargo test -p omiai-checkpoint --test world_roundtrip` so cả `vocabulary`
   và `word_pos` — không chỉ grid + atoms như slice 2.
8. Mọi commit đã push lên `origin/main`.

**14 task, chạy tuần tự.** Task 1 → 12 theo số; `6b` chạy ngay sau `6`,
`9b` ngay sau `9`. Không task nào được bỏ qua: `6b` và `9b` là hai chỗ duy
nhất trong kế hoạch kiểm các tính chất mà spec §7 đòi và không hàm nào sở
hữu, nên bỏ chúng là bỏ đúng phần lập luận.

Điều lát cắt này **không** tuyên bố, và tài liệu phải nói rõ: rằng tiến hoá
hội tụ về một quy ước dùng chung. Nếu MI đo được ≈ 0 thì lát cắt vẫn xong —
cơ chế và thước đo là sản phẩm, con số là quan sát.
