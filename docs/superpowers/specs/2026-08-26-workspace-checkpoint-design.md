# OmiAI Workspace Restructuring + Checkpoint Format — Design

- Ngày: 2026-08-26
- Trạng thái: Đã duyệt qua hội thoại (phần 1 và phần 2), chờ review bản viết
- Phạm vi: **Lát cắt 1** (workspace + sửa test + `Checkpointable` đầu tiên).
  Các lát sau (atoms/agents, tiến hoá Formula, communication, knowledge
  promotion, causal/Active Inference, bundle/runtime/WASM) có spec riêng khi đến lượt.

## 0. Bối cảnh đã kiểm chứng (khác với README cũ)

Đọc và chạy thực tế repo tại thời điểm 2026-08-26:

- README cũ công bố chỉ 3 module core hoàn thiện, phần còn lại là khung sườn
  `todo!()`. **Thực tế:** toàn bộ 73 file `.rs` trong `src/` không còn một
  `todo!()` nào; `inference` (Resolution/DPLL/CDCL), `csp_solver` (AC-3 +
  backtracking), `knowledge::*`, `probabilistic::*`, `causal::*`, `neuro::*`,
  `evolution::*`, `meta::*` đều có cài đặt thật.
- Nhưng repo **không xanh**: `cargo test --lib` = 134 passed / **15 failed**
  (hmc, junction_tree, gibbs, mean_field, puct_mcts, abduction, discocat,
  icp, ltl, weights); test tích hợp `integration.rs`, `properties.rs` và
  example `interact.rs` **không compile được** (API `persistence` đã trôi so
  với test/example).
- Repo chưa phải git repository (đã `git init` trong phiên này), chưa có CI,
  chưa có workspace. Mạng hoạt động; các crate cần thiết (`blake3`,
  `ciborium`, `zstd`, `axum`, `hecs`, `proptest`) đều có trên crates.io.

Quyết định của user: **tách workspace trước, sửa test sau** — tức lát cắt này
làm cả hai nhưng theo thứ tự tách → sửa từng crate theo thứ tự dependency.

## 1. Cấu trúc workspace

Root `Cargo.toml` là **virtual manifest** (quyết định A): chỉ `[workspace]`,
`members = ["crates/*"]`, `resolver = "2"`. Không crate nào ở root.

14 crate trong `crates/` (11 theo đề xuất ban đầu + `omiai-io`,
`omiai-memory` theo quyết định user + `omiai-causal` tách riêng khỏi
probabilistic):

| Crate | Module chuyển vào | Ghi chú |
|---|---|---|
| `omiai-core` | `logic_engine`, `substitution`, `unification`, `inference`, `csp_solver`, `prover`, `asp_solver`, `ltl`, `higher_order_unification` + `utils/{arena,stats,serialization}` | nền symbolic |
| `omiai-knowledge` | `graph`, `reasoning`, `abduction`, `ontology`, `triple`, `discocat`, `sparql_like` | phụ thuộc core |
| `omiai-probabilistic` | `bayesian`, `gibbs`, `hmc`, `junction_tree`, `markov`, `mcts`, `puct_mcts`, `mean_field`, `kolmogorov`, `solomonoff` | |
| `omiai-causal` | `dag`, `do_calculus`, `scm`, `intervention`, `confounding`, `icp` | tách riêng khỏi probabilistic (trụ cột độc lập theo README gốc; world sẽ phụ thuộc nó) |
| `omiai-neuro` | `reservoir`, `liquid_state`, `weights` | **cellular chuyển sang omiai-world** (quyết định B) |
| `omiai-evolution` | `genetic_programming`, `genetic`, `crossover`, `mutation`, `selection`, `fitness` | |
| `omiai-io` | `nlp_parser`, `tokenizer`, `perception`, `action`, `chat` | quyết định user |
| `omiai-memory` | `episodic`, `semantic`, `working`, `procedural` | quyết định user |
| `omiai-world` | `substrate` (từ `neuro/cellular`), `atoms`, `agents/`, `communication`, `world_loop` | logic mới — **không thuộc lát cắt 1** ngoài substrate rỗng + ca_grid checkpoint |
| `omiai-checkpoint` | trait `Checkpointable`, helpers ghi/fsync/hash/rename, manifest, index, cửa sổ trượt; `legacy.rs` = `src/persistence.rs` cũ đánh dấu `#[deprecated]` | |
| `omiai-export` | bundle `model.omiai` (tar+zstd) | crate rỗng khung, lát sau |
| `omiai-runtime` | `load(bundle)` + `step(in) -> out`; KHÔNG depend evolution/training | crate rỗng khung, lát sau |
| `omiai-serve` | axum HTTP `/infer` | crate rỗng khung, lát sau |
| `omiai-cli` | bins: train/resume/export/bench/demo; hấp thụ `src/main.rs` cũ | |

Phân bổ tài nguyên hiện có:

- `benches/*.rs` → bench tương ứng trong crate sở hữu pillar (`cellular.rs`
  → omiai-world, `reservoir.rs` → omiai-neuro, `cnf.rs`/`sat.rs` → omiai-core,
  `knowledge.rs` → omiai-knowledge, `bayesian.rs` → omiai-probabilistic,
  `cgp.rs` → omiai-evolution). Bench chỉ build lại được nếu pillar đã compile;
  bench nào lỗi do API trôi thì sửa cùng lúc với sửa test pillar đó.
- `tests/integration*.rs`, `tests/properties.rs`, `examples/*` → phân bổ về
  crate tương ứng; test chéo nhiều crate đặt ở root `tests/`.
- `fuzz/` giữ nguyên ở root (cần `cargo-fuzz`, ngoài phạm vi lát cắt 1).

### Đồ thị phụ thuộc

```
core ← knowledge ← world
core ← probabilistic ← causal ← world
core ← evolution ← world
neuro → core; memory → core; io → core (+ knowledge cho parser)
checkpoint → core (trait thuần I/O, không biết pillar cụ thể)
world → checkpoint, core, evolution, causal, knowledge, neuro(readout)
export/runtime/serve/cli → các crate trên; runtime KHÔNG depend evolution/training
```

### Dependency version chung

Một bảng `[workspace.dependencies]` duy nhất; mọi crate tham chiếu qua
`workspace = true`. Thêm mới ở lát cắt này: `blake3`, `ciborium`. Các crate
`hecs`, `zstd`, `safetensors` (hoặc crate safetensors chính thức), `axum`
**chỉ thêm khi đến lát cắt tương ứng**, không thêm trước.

Mục tiêu phần cứng xuyên suốt (ràng buộc mọi lựa chọn): CPU-only,
Intel i7-7700K (4C/8T), 8GB RAM, không GPU dời — ưu tiên rayon đa lõi,
mmap/streaming thay vì nạp hết RAM, không giả định VRAM.

## 2. Checkpoint format v1

### 2.1 Trait

```rust
pub trait Checkpointable: Sized {
    type Error;
    /// Ghi trạng thái vào thư mục con do caller tạo sẵn.
    fn save(&self, dir: &Path) -> Result<(), Self::Error>;
    /// Đọc lại; kết quả phải tương đương bit-level với lúc save.
    fn load(dir: &Path) -> Result<Self, Self::Error>;
}
```

- Thuần I/O: crate `omiai-checkpoint` chỉ phụ thuộc `serde`, `serde_json`,
  `ciborium`, `blake3`, `thiserror` (+ `std::fs`). Không biết gì về pillar.
- Round-trip test (`save → load → assert_eq`) là điều kiện bắt buộc để một
  pillar được coi là xong.

### 2.2 Bố cục thư mục

```
checkpoints/
├── index.json                    # danh sách checkpoint hợp lệ tăng dần theo step
└── step_00001234/                # số bước pad-zero 8 chữ số
    ├── manifest.json
    ├── logic/clauses.cbor
    ├── knowledge_graph/graph.cbor
    ├── evolution/population.cbor # chỉ top-N + thống kê phần còn lại
    ├── reservoir/weights.safetensors   (lát sau; v1 tạm .cbor)
    ├── active_inference/beliefs.safetensors (lát sau; v1 tạm .cbor)
    ├── causal/dag.cbor
    ├── world/ca_grid.bin         # bit-packed + header
    ├── world/agents.cbor         (lát sau)
    └── communication/vocabulary.cbor   (lát sau)
```

`manifest.json`: `format_version: 1`, `git_commit`, `step`, `timestamp_utc`,
RNG đầy đủ (`rng_seed`, `rng_state_hex`) để resume đúng quỹ đạo xác định,
và bảng `files: [{path, blake3}]`. Resume đọc `index.json`, verify hash từng
file trước khi dùng.

### 2.3 Ghi nguyên tử & cửa sổ trượt

1. Ghi vào `checkpoints/.tmp_step_XXXXXXXX/` (xoá sẵn nếu tồn tại từ crash trước).
2. fsync từng file → fsync thư mục tmp.
3. `rename()` nguyên tử sang `step_XXXXXXXX/`.
4. Cập nhật `index.json` bằng cách ghi tmp + rename.
5. Xoá checkpoint ngoài cửa sổ trượt (giữ N gần nhất, cấu hình được) cộng các
   mốc vĩnh viễn mỗi K bước (cấu hình được). Không bao giờ ghi đè checkpoint cũ.

### 2.4 Xử lý lỗi

- Hash mismatch khi load → `CheckpointError::Corrupt { path, expected, actual }`;
  resume dừng rõ ràng, không im lặng bỏ qua.
- `index.json` thiếu/hỏng → fallback quét `step_*`, rebuild index, log cảnh báo.
- Manifest thiếu trường bắt buộc → error, không default âm thầm.

## 3. `world/ca_grid.bin` — định dạng lưới CA (chi tiết duy nhất của lát cắt 1)

Header 16 byte: magic `"OMICAGRID"` (10 byte) + u32 LE width + u16 LE height…
— bố cục header cuối cùng cố định khi viết code và ghi rõ trong
`docs/format-spec/checkpoint-v1.md`; nguyên tắc: little-endian, magic check,
kích thước lưới nằm trong header, cell value là enum nhỏ (trống/tài nguyên/
nguy hiểm…) đóng gói bit-packed row-major LSB-first. Round-trip test + proptest
bất biến bảo toàn năng lượng là tiêu chí xong.

## 4. Phạm vi lát cắt 1 (tiêu chí xong từng việc)

| # | Việc | Tiêu chí xong |
|---|---|---|
| 1 | `git init` + commit baseline (đã init; commit chờ user set identity) | `git log` có commit gốc |
| 2 | Chuyển workspace 14 crate + `[workspace.dependencies]` | `cargo build` toàn workspace OK |
| 3 | Sửa 15 test lib fail + lỗi compile integration/example | `cargo test` xanh toàn workspace |
| 4 | `omiai-checkpoint`: trait + helpers + manifest/hash + `Checkpointable` cho `ca_grid` | round-trip pass; proptest energy-conservation pass |
| 5 | `docs/format-spec/checkpoint-v1.md` + ADR-0001..0004 + `docs/architecture/` khung | docs khớp code thật |
| 6 | `.github/workflows/ci.yml` build+test | workflow hợp lệ |
| 7 | README cập nhật đúng trạng thái mới (văn phong "đã cài đặt và test" / "khung sườn") | README trung thực |

Chiến lược sửa test: test sai do API trôi → sửa test; implementation sai thật
(ví dụ hmc `std_dev_positive` sinh giá trị ngoài khoảng) → sửa code, giữ test
làm regression. Mỗi crate phải xanh trước khi sang crate kế tiếp theo thứ tự
dependency: core → knowledge/probabilistic/causal/neuro/memory/io → evolution.

## 5. Những gì lát cắt 1 KHÔNG làm (YAGNI)

- atoms/agents/communication/world_loop logic (chỉ substrate khung + ca_grid checkpoint)
- bundle/export/runtime/WASM/HTTP (crate khung rỗng với doc comment định hướng)
- benchmark criterion mới cho world; safetensors (weights vẫn CBOR tạm trong v1)
- hecs/zstd/axum dependencies (thêm khi đến lát cắt)

## 6. ADR cần viết ở lát cắt này

- ADR-0001: virtual workspace manifest, 13 crate, đồ thị phụ thuộc
- ADR-0002: cellular_automata về omiai-world thay vì neuro
- ADR-0003: checkpoint dạng thư mục + BLAKE3 + ghi nguyên tử (không file đơn)
- ADR-0004: gen là con trỏ tới Formula trong core::logic_engine (định hướng
  cho lát cắt world; ghi quyết định trước để atoms/agents bám vào)

## 7. Rủi ro & mở

- 15 test fail có thể lộ bug sâu hơn dự kiến (ví dụ HMC sampler sai thống kê)
  → mỗi bug là một fix riêng có regression test, không gộp "sửa đại cho xanh".
- `panic = "abort"` trong profile release cũ sẽ không chuyển thẻ profile sang
  workspace được cho test/bench — quyết định: profile đặt ở root workspace
  `[profile.release]`, bỏ `panic = "abort"` vì gây khó chịu với `cargo test --release`
  (test harness cần unwind); ghi vào ADR-0001.
- Commit git hoãn cho tới khi user cấu hình identity xong; mọi lát cắt sẽ
  commit bù theo thứ tự.
