# OmiAI Slice 2 — Atoms, Agents, World Loop + World Checkpoint Bundle — Design

- Ngày: 2026-08-26
- Trạng thái: Đã duyệt qua hội thoại (user chọn: atoms+agents+world_loop,
  world bundle checkpoint, retention N gần nhất + mốc mỗi K bước, Cách 1
  FormulaRegistry)
- Phạm vi: **Lát cắt 2**. Tiền đề: lát cắt 1 đã xong (tag `slice-1-complete`,
  workspace 15 crate xanh, `checkpoint-v1` cho `ca_grid`).
- Ràng buộc phần cứng xuyên suốt: CPU-only i7-7700K (4C/8T), 8GB RAM, không GPU.

## 0. Mục tiêu

Biến `omiai-world` từ khung sườn (chỉ `substrate`) thành trụ cột chạy được thật:
atom sống trên lưới CA, agent điều khiển bởi gene là con trỏ tới `LtlFormula`,
world loop 5 phase cố định, và toàn bộ trạng thái thế giới checkpoint/resume
**bit-exact** được.

Sau lát cắt này, tiêu chí trung thực phải đạt:

- `cargo test --workspace` xanh, gồm các test mới liệt kê ở mục 5.
- README/architecture docs ghi đúng cái nào "đã cài đặt và test", cái nào
  vẫn là khung (communication, export/runtime/serve/cli).

## 1. `omiai-world` — module mới

### 1.1 `registry.rs` — FormulaRegistry (Cách 1, ADR-0004)

```rust
pub struct Genome {
    pub formula: LtlFormula,
    pub fitness: Option<f64>,   // cache; None = chưa đánh giá
}

pub struct FormulaId(generational_arena::Index); // newtype, không pub bên trong

pub struct FormulaRegistry {
    arena: generational_arena::Arena<Genome>,
}
// API: insert(Genome) -> FormulaId, get(FormulaId) -> &Genome,
//      get_mut -> &mut Genome, remove(FormulaId) -> Option<Genome>, len()
```

- Registry **sống trong `World`**, không global static.
- Handle generational nên remove + insert lại không gây aliasing.
- Nhiều atom có thể trỏ chung một `FormulaId` (chia sẻ gene) — sinh sản
  kế thừa handle chứ không nhân bản công thức.
- Serialize: tuần tự arena thành `Vec<Genome>` theo thứ tự slot CBOR;
  load tái tạo bằng cách insert lại giữ nguyên thứ tự, atom lưu slot-index
  (`u32`) thay vì handle thô để map về id mới sau load. `FormulaId`
  serialize dưới dạng slot-index trong mọi file CBOR của world.
- Garbage: registry chỉ bơm phình khi tiến hoá thêm genome — lát cắt này
  KHÔNG làm refcount/GC genome (YAGNI); ghi nhận rõ là giới hạn đã biết.

### 1.2 `atoms.rs`

```rust
pub struct Atom {
    pub pos: (usize, usize),
    pub energy: f64,          // clamp [0, ENERGY_MAX=1.0]
    pub gene: FormulaId,
    pub age: u64,
}
```

- Cell value ngữ nghĩa: `0` = trống, `1` = vật cản/đã chiếm bởi substrate,
  `≥2` = tài nguyên (giá trị lớn hơn = giàu năng lượng hơn). Ghi vào
  doc-comment module và format-spec.
- Sinh sản: energy ≥ `REPRODUCE_THRESHOLD` → tách đôi, cha/con mỗi bên
  nửa energy, con trỏ gene kế thừa (cùng `FormulaId`), vị trí con là ô kề
  trống đầu tiên tìm thấy (thứ tự quét cố định: N,E,S,W); không có ô trống
  thì không sinh.
- Chết: energy ≤ 0 → atom bị loại khỏi `Vec<Atom>` cuối phase metabolism.

### 1.3 `agents.rs` — policy decode từ LtlFormula

Mỗi agent là một Atom hành động theo gene của chính nó:

- Quan sát cục bộ 4 ô kề (N,E,S,W) mã hoá thành chuỗi sự kiện atom-LTL,
  ví dụ `"res_n"`, `"res_e"`, `"wall_s"`, …
- Decode policy: đánh giá `formula` với sự kiện hướng tương ứng; chọn
  hướng đầu tiên mà sub-formula hướng đó satisfy; nếu không hướng nào thì
  `Stay`. Cài đặt cụ thể: với mỗi hướng d, kiểm tra
  `ltl::is_satisfiable(formula ∧ atom(d), bound nhỏ)` quá đắt — thay bằng
  **đánh giá trực tiếp trên trạng thái hiện tại**: map sự kiện quan sát
  thành valuation `HashMap<String,bool>`, dùng hàm evaluate propositional
  thuần (tái sử dụng logic của `omiai-core::logic_engine::evaluate` cho
  phần không có toán tử thời gian; toán tử thời gian bỏ qua = coi như
  satisfy). Hàm decode thuần túy, test được độc lập không cần world.
- Hành động: `{ Stay, Move(N|E|S|W) }`. Di chuyển vào ô trống → đổi `pos`;
  ô có tài nguyên → ăn (cell giảm về 0, atom cộng energy theo giá trị cell);
  ô bị chiếm/cản → đứng yên. Không pathfinding.

### 1.4 `world_loop.rs` — World + 5 phase cố định

```rust
pub struct World {
    pub ca: CellularAutomaton,
    pub registry: FormulaRegistry,
    pub atoms: Vec<Atom>,
    pub rng: OmiaiRng,        // xem mục 3
    pub step_count: u64,
}

impl World {
    pub fn step(&mut self) { ca_step(); metabolism(); agent_act();
                             reproduce_and_evolve(); snapshot(); }
}
```

Thứ tự phase **cố định**, mỗi phase một hàm riêng test độc lập:

1. `ca_step` — lưới tiến hoá một bước Margolus (substrate sẵn có).
2. `metabolism` — `energy -= METABOLIC_COST`; loại atom ≤ 0; giải phóng
   ô đang chiếm.
3. `agent_act` — duyệt `atoms` **theo thứ tự Vec** (không shuffle): mỗi
   agent quan sát → decode → act. Thứ tự tĩnh bảo đảm deterministic.
4. `reproduce_and_evolve` — sinh sản theo ngưỡng; mutation gene: xác suất
   nhỏ biến đổi cấu trúc formula (đổi atom, đảo and/or) tạo genome mới
   insert vào registry, con trỏ sang id mới. Mutation dùng `OmiaiRng`.
5. `snapshot` — `step_count += 1`.

Constructor: `World::new(width, height, seed)` — lưới random density,
vài atom mồi với genome khởi đầu ngẫu nhiên từ cùng seed. Không phụ thuộc
io/knowledge/probabilistic/neuro ở lát cắt này (deps: core, evolution
[chỉ nếu mutation tái dùng operator], checkpoint [dev-dependency cho test],
rayon, rand_chacha).

## 2. `omiai-checkpoint` mở rộng

### 2.1 Retention window (mục 2.3 spec gốc)

```rust
pub struct RetentionPolicy { pub keep_recent: usize /*mặc định 10*/,
                             pub milestone_every: u64 }
pub fn apply_retention(root: &Path, policy: &RetentionPolicy)
    -> Result<Vec<(u64, PathBuf)>, CheckpointError>; // trả về danh sách bị xoá
```

- Giữ: N step gần nhất **cộng** mọi step chia hết cho `milestone_every`.
  Không bao giờ xoá mốc vĩnh viễn.
- Xoá = remove_dir_all thư mục `step_XXXXXXXX` tương ứng.

### 2.2 `index.json`

- Ghi tmp + rename nguyên tử (dùng `write_atomic` sẵn có).
- Nội dung: danh sách `{step, dir}` tăng dần.
- Load: thiếu/hỏng → fallback quét `step_*` bằng `list_steps`, rebuild,
  trả warning qua return value (không panic, không im lặng tuyệt đối).

### 2.3 World bundle

`Checkpointable` cho từng mảnh, rồi struct tổng hợp:

```
step_XXXXXXXX/
├── manifest.json            # thêm record cho 3 file mới
├── world/ca_grid.bin        # có từ lát cắt 1
├── world/atoms.cbor         # Vec<{pos,energy,gene_slot,age}> + step_count
├── world/registry.cbor      # Vec<Genome> theo thứ tự slot
└── world/rng_state.bin      # raw state của OmiaiRng (mục 3)
```

- `impl Checkpointable for World` đặt ở **omiai-checkpoint**
  (pattern orphan-rule giống `ca_grid`: impl nằm ở crate checkpoint,
  world chỉ expose dữ liệu public).
- RNG state lưu trong `world/rng_state.bin` (thêm file thứ 4 vào sơ đồ
  trên; manifest thêm record tương ứng) — xem mục 3. Manifest v1 giữ
  nguyên schema, KHÔNG nhét RNG vào manifest.
- Round-trip bắt buộc: save ở step N → load → chạy tiếp M bước → so sánh
  bit-exact với world chạy liền N+M không qua checkpoint (so `cells`,
  `atoms`, `step_count`, toàn bộ registry formulas).

## 3. RNG deterministic — quyết định chốt trước khi viết plan

Yêu cầu: serialize state RNG sau N bước để resume đúng quỹ đạo.

- Kế hoạch A: `ChaCha8Rng` — probe API `rand_chacha` thực tế; nếu expose
  được core state (qua `rand_core::block::BlockRng` / `TryRngCore`) thì
  serialize raw words.
- **Fallback B (chốt nếu A rắc rối hơn nửa ngày): tự viết
  `Xorshift64Star`** (~20 dòng, state 8 byte, serialize tầm thường,
  đã biết fixed-point seed=0 nên seed strategy tránh 0). Toàn bộ randomness
  của world đi qua nó. Đổi: chất lượng thống kê thấp hơn ChaCha8; lấy được:
  đơn giản, đúng đắn, resume chắc chắn bit-exact.
- Quyết định cuối ghi vào ADR-0006 kèm lý do đo đạc được.

## 4. Testing

- Unit từng phase: metabolism giết atom cạn năng lượng; act di chuyển đúng
  hướng có tài nguyên / đứng yên trước cản; reproduce tách đôi tại ngưỡng,
  không sinh khi hết ô trống.
- Decode policy test độc lập (không cần world): bảng sự kiện → hành động.
- Round-trip World checkpoint: bit-exact resume (test then chốt của slice).
- Proptest: tổng energy hệ thống chỉ giảm (metabolism) hoặc chuyển giao
  (ăn/sinh sản) — không tự phát ra từ hư không; population CA vẫn bảo toàn
  qua `ca_step` (bất biến điều chỉnh đúng ngữ nghĩa: atom ĂN cell thì
  population lưới giảm có chủ đích, kiểm tra tại phase riêng).
- Retention test: tạo 25 checkpoint, áp policy → còn đúng N gần nhất +
  các mốc; mốc không bao giờ bị xoá dù ngoài window.
- Index fallback: xoá index.json → rebuild từ quét thư mục ra cùng danh sách.

## 5. Những gì lát cắt 2 KHÔNG làm (YAGNI)

- Communication / Lewis signaling-game (slice 3).
- hecs ECS, pathfinding, đa loài, thị giác đa ô.
- Benchmark criterion mới cho world_loop (chỉ thêm khi cần chứng minh
  claim hiệu năng cụ thể).
- Export bundle `model.omiai`, runtime, serve, cli.
- GC genome registry, safetensors, WASM.

## 6. ADR viết ở lát cắt này

- ADR-0006: lựa chọn RNG cho world (ChaCha8-state hay Xorshift64Star)
  kèm kết quả probe API thực tế.
- ADR-0007 (nếu cần): quyết định impl `Checkpointable for World` nằm ở
  omiai-checkpoint vs omiai-world — chỉ viết nếu gặp trở ngại orphan-rule
  khác với dự kiến.

## 7. Rủi ro

- Round-trip bit-exact dễ vỡ vì một chi tiết nhỏ (thứ tự HashMap, float
  energy qua CBOR) → mọi collection đa hình phải có thứ tự cố định
  (Vec/BTreeMap, cấm HashMap trong payload checkpoint); f64 serialize CBOR
  giữ nguyên bit (kiểm chứng bằng round-trip proptest).
- Policy-decode LTL có thể rẻ hoặc đắt tuỳ độ sâu formula → giới hạn depth
  genome lúc mutate, đo lại nếu cần.
- Slice dài (world + checkpoint + retention) → thứ tự làm: retention/index
  trước (độc lập), rồi atoms/registry, agents, world_loop, bundle cuối.
