# OmiAI Slice 3 — Ngôn ngữ nổi sinh (Lewis signaling) trong `omiai-world` — Design

- Ngày: 2026-08-26
- Trạng thái: Đã duyệt qua hội thoại (user chọn: communication layer đúng
  master spec; broadcast 1 bước tới 4 ô kề; voice gene = K formula arm;
  MI đo với hướng tài nguyên gần nhất; điều kiện xong = cơ chế + đo lường
  + trung thực về kết quả)
- Phạm vi: **Lát cắt 3**. Tiền đề: lát cắt 2 đã xong (tag `slice-2-complete`,
  276 test xanh, world loop 5 phase + bit-exact resume).
- Ràng buộc phần cứng xuyên suốt: CPU-only i7-7700K (4C/8T), 8GB RAM, không GPU.

## 0. Mục tiêu

`huongdan.txt` (master spec) đặt lớp giao tiếp ngay sau agents: agent buộc
phải phối hợp cho một nhiệm vụ chung (chỉ điểm tài nguyên), **chỉ được
thưởng khi tín hiệu nội bộ tuỳ tiện ban đầu hội tụ về một quy ước chung**,
và toàn bộ được đo định lượng qua `communication::vocabulary` lưu tần suất
ký hiệu + mutual information giữa ký hiệu và trạng thái thế giới — để
"tiếng nói riêng" là **đại lượng đo được**, không phải tuyên bố mơ hồ.

Lát cắt này cài **cơ chế** và **thước đo** đó, không hứa kết quả hội tụ.

Tiêu chí trung thực sau lát cắt:

- `cargo test --workspace` xanh + `cargo clippy --workspace --all-targets`
  0 cảnh báo.
- Bit-exact resume vẫn đúng khi signaling đang hoạt động.
- README ghi rõ: cơ chế + thước đo **đã cài đặt và test**; **hội tụ quy ước
  chưa được chứng minh**. Con số MI của một lần chạy thật được báo đúng như
  đo được, kể cả khi nó gần 0.
- README sửa lại thứ tự build: `omiai-runtime` KHÔNG phải bước 5. Master
  spec đặt bundle+runtime là bước cuối cùng, và giao `resume` cho
  `omiai-cli`, không phải cho runtime.

## 1. Bảng ký hiệu và im lặng

```rust
pub type Symbol = u8;
/// Số arm của voice gene = số ký hiệu phát được.
pub const N_SYMBOLS: usize = 4;
```

Giá trị tín hiệu quan sát được có **5** khả năng: `Silent` + `Sym(0..=3)`.
Im lặng LÀ một giá trị tín hiệu, không phải dữ liệu thiếu.

Lý do chọn 4 arm chứ không phải 5: biến trạng thái (mục 4) có đúng 5 lớp
(N/E/S/W/none), nên với im lặng làm giá trị thứ năm, trần MI = log₂5 ≈
2.322 bit **đạt tới được**. Nếu chỉ có 4 giá trị tín hiệu, trần bị chặn
dưới log₂5 vì lý do cấu trúc, và mọi phép đo sẽ đọc ra như "hội tụ thất
bại" dù cơ chế đúng.

## 2. Voice gene — K formula arm

```rust
// atoms.rs
pub struct Atom {
    pub pos: (usize, usize),
    pub energy: f64,
    pub gene: FormulaId,          // policy di chuyển (đã có từ slice 2)
    pub age: u64,
    /// Gene phát tín hiệu. Bất biến: len == 0 (câm) HOẶC len == N_SYMBOLS.
    #[serde(default)]
    pub voice: Vec<FormulaId>,
}
```

- Ký hiệu phát ra = **index của arm đầu tiên** đánh giá `true`; không arm
  nào thoả → `Silent`. Atom câm (`voice.len() == 0`) luôn `Silent`.
- `Vec<FormulaId>` thay vì `[FormulaId; N_SYMBOLS]`: `#[serde(default)]`
  cho ra `vec![]` = câm, nên checkpoint slice-2 (không có field `voice`)
  đọc lại được mà không cần giá trị mặc định giả nào trỏ vào slot 0 —
  slot 0 là genome di chuyển mặc định, dùng nó làm "arm mặc định" sẽ biến
  mọi atom cũ thành máy phát ký hiệu 0 liên tục. Bất biến độ dài được
  kiểm ở `World::load` và `debug_assert` khi phát.

### 2.1 Valuation của arm — 16 mệnh đề có hướng

Arm KHÔNG đánh giá trên valuation một-hướng của `decide`. Nó đánh giá trên
**neighbourhood valuation** gồm 16 mệnh đề:

```
{open, wall, res, occupied} × {_n, _e, _s, _w}
```

Ví dụ `res_e ∧ ¬occupied_e`. Không có mệnh đề chỉ hướng thì một arm **về
mặt vật lý không thể** diễn đạt "thức ăn ở phía Đông", và ngôn ngữ không
bao giờ có thể nói về hướng tài nguyên — tức thước đo MI ở mục 4 mất ý
nghĩa ngay từ đầu.

### 2.2 Đột biến voice

`mutate_formula` hiện hard-code pool 4 tên (`open/wall/res/occupied`).
Tách thành:

```rust
pub fn mutate_formula_with(f: &LtlFormula, rng: &mut ChaCha8Rng, names: &[&str]) -> LtlFormula;
pub fn mutate_formula(f: &LtlFormula, rng: &mut ChaCha8Rng) -> LtlFormula; // wrapper pool di chuyển
```

- Voice dùng pool 16 tên có hướng.
- Tính bảo toàn arity (không tăng độ sâu) của slice 2 giữ nguyên.
- Đột biến voice giữ đúng `N_SYMBOLS` arm: đột biến **từng arm**, không
  thêm/bớt arm.

### 2.3 Khởi tạo và di truyền voice

- `World::new` cấp cho mỗi atom mồi một voice **ngẫu nhiên**: `N_SYMBOLS`
  arm, mỗi arm sinh từ `mutate_formula_with` áp lên một formula hạt giống
  (`res_n ∨ open_n`) với pool 16 tên. Tuỳ tiện là **yêu cầu**, không phải
  tiện tay: nếu voice khởi tạo đã đúng nghĩa (arm k ≡ "tài nguyên ở hướng
  k") thì quy ước được cài sẵn, MI cao ngay từ bước 0, và trò chơi Lewis
  không còn gì để hội tụ. Mỗi atom mồi lấy voice riêng.
- `reproduce_and_evolve`: với xác suất `MUTATION_PROB`, đột biến **đúng
  một arm** chọn ngẫu nhiên (1 entry registry mới), phần còn lại kế thừa
  nguyên. Không đột biến toàn bộ K arm cùng lúc — vừa đỡ phình registry
  (registry chưa bao giờ GC, giới hạn đã biết từ slice 2) vừa cho quy ước
  cơ hội ổn định thay vì bị đổi trắng mỗi lần sinh.
- **Thứ tự tiêu thụ RNG cố định** (điều kiện của bit-exact resume): trong
  mỗi lần sinh, rút cho gene di chuyển trước, rồi mới rút cho voice. Ghi
  rõ trong doc comment vì đây là thứ tự mà checkpoint phụ thuộc vào.
- **Cha câm → con câm.** `voice.len() == 0` không có arm nào để đột biến,
  nên câm là tính trạng di truyền và dòng dõi đó im lặng mãi. Không âm
  thầm cấp voice cho con của atom câm: `World::new` là chỗ duy nhất tạo
  voice từ không khí, và mục 6.4 là chỗ duy nhất còn lại.

### 2.4 Voice KHÔNG phụ thuộc `heard` — và giá của lựa chọn khác

Voice arm chỉ đánh giá trên 16 mệnh đề không gian ở mục 2.1. Nó **không**
đọc `hear*`.

Lý do là bắt buộc, không phải sở thích: mọi atom phát cùng lúc trong
`speak`, nên nếu arm đọc airwave đang-ghi-dở thì ký hiệu phát ra phụ
thuộc thứ tự Vec — đúng cái mà việc tách phase sinh ra để tránh. Muốn arm
nghe được thì phải đọc airwave của **bước trước**, tức airwave trở thành
trạng thái bền và bit-exact resume phải checkpoint nó
(`communication/airwave.bin`, một payload nữa + một bất biến hai buffer).

Hệ quả phải nói thẳng: **vọng lại (echo) và lan truyền nhiều chặng
(relay) KHÔNG diễn đạt được ở lát cắt 3.** Đây là hoãn có chủ ý kèm giá
đã biết, không phải khả năng để ngỏ cho người đọc tự suy diễn. Lát cắt
sau thêm payload airwave là có relay.

## 3. Phía nhận

- `Observation` thêm `heard: Option<Symbol>` — ký hiệu do atom đứng ở ô
  theo hướng đó phát ra (`None` nếu ô không có atom, hoặc atom đó im lặng).
  Chỉ dùng cho phía **nhận** (mệnh đề tổng hợp dưới đây), KHÔNG vào
  valuation của voice arm — xem mục 2.4.
- Valuation di chuyển thêm mệnh đề tổng hợp `hear0..hear3`: `heark` đúng
  ở **mọi hướng** nếu CÓ BẤT KỲ atom kề nào phát ký hiệu k. Đây là mệnh
  đề receiver hành động được.
- Pool đột biến của genome **di chuyển** mở rộng để gồm `hear0..hear3`.
  Không có bước này thì không dòng dõi nào phát hiện được là tín hiệu tồn
  tại, và lớp giao tiếp thành đồ trang trí.

### 3.1 Giới hạn đã biết — ghi vào ADR-0007, không được ngụ ý nhiều hơn

Ô của sender luôn `occupied`, nên **không bao giờ passable**: "đi về phía
ai vừa nói k" là bất khả thi do cấu trúc. Thêm nữa, receiver
propositional không có bộ nhớ nên không thể điều hướng tới referent cách
hai ô. Cái kênh này chống đỡ được là **quy ước + tụ tập về vùng nhiều
thức ăn**, KHÔNG phải đặt tên referential đầy đủ.

## 4. Vocabulary + mutual information

Biến trạng thái `M` = **hướng ô tài nguyên KỀ sender**, 5 lớp:
`North / East / South / West / None`.

Luật tính `state_class`, deterministic hoàn toàn:

1. Quét đúng thứ tự `N, E, S, W`; ô đầu tiên có giá trị ≥ 2 thắng. Hoà là
   chuyện thường (hai ô kề đều có tài nguyên) và luật thứ tự này giải
   quyết hết. Cùng thứ tự ưu tiên mà `decide` và `first_free_neighbor`
   đang dùng.
2. **Bán kính đúng bằng 1** — chỉ 4 ô kề, không xa hơn. Không có khái
   niệm "gần nhất" cần tìm kiếm.
3. **Giá trị tài nguyên không phân định hoà**: 3 không hơn 2.
4. `res` bỏ qua tình trạng chiếm: giá trị ≥ 2 là tài nguyên dù có atom
   đứng trên đó hay không — đúng như `observe()`. Trường hợp này có thật,
   không phải giả thiết: `ca_step` (Margolus) dịch chuyển giá trị ô nên
   có thể đẩy tài nguyên vào ô đang bị chiếm.

Nguyên tắc chung phía sau cả bốn luật: **`M` phải là hàm của đúng cái
sender quan sát được.** Một biến trạng thái phụ thuộc ô hoặc phân biệt mà
16 mệnh đề của sender không có sẽ kéo MI xuống vì lý do cấu trúc — cùng
một sai lầm với việc chặn bảng ký hiệu dưới 5 giá trị.

```rust
// communication.rs
/// Giá trị tín hiệu quan sát được. Im lặng là một giá trị, không phải thiếu dữ liệu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalValue { Silent, Sym(Symbol) }

/// Lớp trạng thái thế giới mà tín hiệu nói về: hướng tài nguyên gần nhất.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateClass { North, East, South, West, None }

pub const N_SIGNAL_VALUES: usize = N_SYMBOLS + 1; // + Silent
pub const N_STATE_CLASSES: usize = 5;

pub struct Vocabulary {
    /// joint[s][m]: đếm số lần giá trị tín hiệu s xuất hiện cùng lớp trạng thái m.
    /// Hàng 0 = Silent, hàng k+1 = Sym(k).
    pub joint: [[u64; N_STATE_CLASSES]; N_SIGNAL_VALUES],
    pub total: u64,
}

impl Vocabulary {
    pub fn record(&mut self, signal: SignalValue, state: StateClass);
    /// I(S;M) = Σ p(s,m) log₂( p(s,m) / (p(s)p(m)) ), bỏ qua ô đếm 0.
    pub fn mutual_information(&self) -> f64;
    pub fn symbol_frequency(&self, sym: Symbol) -> f64;
    pub fn entropy_signal(&self) -> f64;
    pub fn entropy_state(&self) -> f64;
}
```

Đếm **tích luỹ toàn run**, một record cho mỗi (atom còn sống, bước).

## 5. World loop — 6 phase

```
ca_step → metabolism → speak → agent_act → reproduce_and_evolve → snapshot
```

`speak`:

1. Dựng `airwave: Vec<Option<Symbol>>` dài `w*h`, dựng lại mỗi bước.
2. Với mỗi atom còn sống (thứ tự Vec, deterministic): tính neighbourhood
   valuation, decode arm → `SignalValue`, ghi vào `airwave[y*w+x]`.
3. `vocabulary.record(signal, state_class(atom))` — cho **mọi** atom còn
   sống, kể cả atom câm (record `Silent`).

**Thời điểm lấy mẫu MI.** Ký hiệu và lớp trạng thái lấy từ **cùng một ảnh
chụp valuation** đã dùng để decode arm, ngay trong `speak`. Tuyệt đối
không tính lại `state_class` sau `agent_act`: lúc đó atom có thể đã bước
lên ô tài nguyên và ăn mất nó, và ta sẽ ghép ký hiệu với một trạng thái
chưa từng gây ra nó.

- Atom bị `metabolism` của chính bước này giết đã bị loại khỏi Vec ⇒ không
  ghi.
- Atom sinh ở `reproduce_and_evolve` xuất hiện lần đầu ở `speak` bước sau.
- Vì vậy bất biến chính xác là
  `total == Σ_bước |atom sống tại thời điểm speak|`.
- Đếm tích luỹ toàn run và được lưu checkpoint, nên sau resume tally tiếp
  tục liền mạch.

**Ngữ nghĩa im lặng và `heard`.**

- `airwave[cell] == None` gộp cả "không có atom" và "atom ở đó im lặng".
  Receiver vẫn phân biệt được vì `occupied_d` là mệnh đề riêng:
  `occupied_d ∧ ¬hear*` nghĩa là "láng giềng im lặng".
- Airwave ghi **một lần** trong `speak`, rồi **đóng băng chỉ-đọc** suốt
  phần còn lại của bước: ký hiệu vẫn nghe được ở đúng ô đã phát ra nó kể
  cả khi người nói đã đi khỏi trong `agent_act`. Làm khác đi là mời thứ
  tự Vec quay lại quyết định semantics.
- Atom không bao giờ tự nghe mình: ô của chính nó không nằm trong 4 ô kề.

- `airwave` là **trạng thái phái sinh**, KHÔNG lưu checkpoint (giống
  Margolus phase và block cache — xem ADR-0003 / format-spec §5).
- `speak` KHÔNG tiêu thụ RNG → bit-exact resume không bị ảnh hưởng.
- Phát **trước** hành động, không gộp vào `agent_act`: nếu gộp, receiver
  chỉ nghe được atom nào tình cờ đứng trước trong Vec, semantics phụ
  thuộc thứ tự lưu trữ. Tách phase ⇒ mọi receiver đọc cùng một airwave
  đóng băng.

## 6. Checkpoint

### 6.1 `atoms.cbor` — thêm field optional

`voice` là field optional (`#[serde(default)]`), nên theo đúng chính sách
tương thích §6 của `checkpoint-v1`: **minor bump** — writer ghi
`format_version = 1_001`, reader nhận cả `1` và `1_001` qua
`manifest::is_supported_version(v)`. `ca_grid.rs` và `world_bundle.rs`
dùng chung hàm này thay vì so sánh `!= FORMAT_VERSION_V1`.

### 6.2 `communication/vocabulary.cbor` — payload mới

- CBOR của `Vocabulary`, hash vào manifest dưới path
  `communication/vocabulary.cbor`.
- `format_version >= 1_001` → file **bắt buộc** phải có; thiếu là lỗi.
- `format_version == 1` (checkpoint slice-2) → vắng mặt là **đúng mong
  đợi**, vocabulary khởi tạo rỗng. Nhờ phân biệt theo version, tương thích
  ngược không bao giờ biến thành mặc định âm thầm.

### 6.3 Kiểm tra tham chiếu khi load

Mở rộng phần kiểm nhất quán liên-payload đã có (commit b149ca6): mọi slot
trong `voice` phải tồn tại trong registry; `voice.len()` phải là 0 hoặc
`N_SYMBOLS`. Sai → `Corrupt`, không phải atom câm âm thầm.

### 6.4 Nạp checkpoint slice-2 (`format_version == 1`)

Luật, theo đúng thứ tự:

1. `voice` vắng mặt trong CBOR → `#[serde(default)]` → `vec![]` → atom
   **câm**. Hợp lệ với bất biến "0 hoặc `N_SYMBOLS`" ở mục 6.3.
2. `communication/vocabulary.cbor` vắng mặt → `Vocabulary::default()`
   (toàn 0, `total == 0`) — mong đợi ở version 1, xem mục 6.2.
3. Kết quả: world resume **đúng và im lặng vĩnh viễn**. Mọi atom record
   `Silent`, `total` vẫn cộng đủ, `MI == 0` — và đó là câu trả lời đúng
   cho một thế giới không ai nói, không phải mechanism hỏng. Cha câm →
   con câm (mục 2.3) nên nó không tự sống lại.
4. **`load` không được cấp voice.** Cấp voice ở `load` sẽ rút RNG lúc nạp,
   làm `load` không còn tái tạo đúng cái đã lưu và làm lệch quỹ đạo so với
   một run liên tục — vi phạm hợp đồng resume của ADR-0006.
5. Hồi sinh là **thao tác riêng, opt-in, gọi tường minh**:
   `World::seed_voices(&mut self)` cấp voice ngẫu nhiên cho mọi atom đang
   câm, rút RNG theo đúng thứ tự cố định của mục 2.3. Ai gọi thì biết mình
   vừa đổi quỹ đạo; ghi rõ trong doc comment là hàm này **không** bảo toàn
   bit-exactness so với run gốc.
6. Save lại một world đã nạp từ version 1 sẽ ghi `format_version = 1_001`
   (voice rỗng + vocabulary rỗng). Nâng cấp một chiều, không có đường quay
   về đọc bằng reader slice-2 — đúng chính sách minor bump.

## 7. Testing

Unit (`communication.rs`):

- decode arm: arm đầu tiên thoả thắng; không arm nào thoả → `Silent`;
  `voice` rỗng → `Silent`.
- `hear0..hear3` tổng hợp đúng: một atom kề phát k ⇒ `heark` đúng ở mọi
  hướng; không ai phát ⇒ tất cả sai.
- MI trên bảng dựng tay, đáp số chính xác:
  - song ánh hoàn hảo (5 giá trị ↔ 5 lớp) → `log₂5`, sai số < 1e-12.
  - bảng độc lập chính xác → 0, sai số < 1e-12.
  - luôn một ký hiệu → 0.
  - hội tụ một phần → nằm hẳn giữa 0 và log₂5.
- `state_class` (mục 4, cả 4 luật đều có test):
  - quét N,E,S,W đúng thứ tự; không có tài nguyên → `None`.
  - hai ô kề đều có tài nguyên → thắng theo thứ tự, không theo giá trị
    (ô N giá trị 2 thắng ô E giá trị 3).
  - tài nguyên trên ô đang bị atom khác chiếm vẫn tính là tài nguyên.
  - ô cách 2 bước có tài nguyên, 4 ô kề trống → `None` (bán kính đúng 1).

Unit (`atoms.rs` / `world_loop.rs`):

- deserialize atom slice-2 (không có `voice`) → câm, không panic.
- đột biến voice giữ đúng `N_SYMBOLS` arm và không tăng độ sâu.
- **cha câm → con câm**: atom câm đủ năng lượng sinh sản ⇒ con cũng
  `voice.len() == 0` (mục 2.3).
- **airwave đóng băng**: atom phát ký hiệu rồi di chuyển trong
  `agent_act` ⇒ láng giềng của ô **cũ** vẫn nghe được ký hiệu đó trong
  cùng bước (mục 5).
- **thời điểm lấy mẫu**: atom kề tài nguyên phát ký hiệu rồi ăn mất tài
  nguyên trong `agent_act` ⇒ cặp đã ghi vẫn là (ký hiệu, hướng tài nguyên
  lúc phát), không phải `None` (mục 5).
- **voice không đọc `hear*`**: hai world giống nhau hoàn toàn trừ airwave
  bước trước ⇒ ký hiệu phát ra y hệt (mục 2.4).

Proptest (`properties.rs`):

- bất biến world của slice 2 vẫn đúng khi signaling bật.
- `0 ≤ MI ≤ min(H(S), H(M))` với **mọi** bảng đếm sinh ngẫu nhiên.
- `vocabulary.total == Σ_bước (số atom sống tại thời điểm `speak`)`.

Integration (`crates/omiai-world/tests/communication.rs`):

- Dân số dựng tay dùng chung một voice genome gọi đúng hướng tài nguyên
  so với dân số câm: `MI(quy ước) > MI(câm)` và `MI(câm) == 0`.
  Đây là test **cơ chế**, không phải tuyên bố hội tụ.

Integration (`crates/omiai-checkpoint/tests/world_roundtrip.rs`):

- bit-exact resume với signaling bật; so cả `vocabulary.joint`.
- checkpoint `format_version == 1` (không có vocabulary) vẫn load được:
  mọi atom câm, `MI == 0`, chạy thêm bước vẫn im lặng — rồi
  `seed_voices()` làm nó nói (mục 6.4).
- save lại world nạp từ version 1 ⇒ `format_version == 1_001` và có
  `communication/vocabulary.cbor`.

Example: `examples/communication_demo.rs` — chạy N bước, in MI, tần suất
từng ký hiệu, dân số. Master spec yêu cầu `communication_demo`.

## 8. ADR viết ở lát cắt này

**ADR-0007 — kênh tín hiệu 1 bước, 4 ô kề, im lặng là giá trị thứ năm**:
vì sao broadcast tạm thời thay vì pheromone có phân rã (không có hằng số
decay phải biện minh, không thêm lớp bền vào bit-exact resume); vì sao
voice là K formula arm thay vì bảng tra (`gen là Formula, in ra đọc được,
suy luận lại được` — nguyên tắc gốc của dự án); vì sao MI đo với hướng
tài nguyên; và giới hạn receiver không bộ nhớ ở mục 3.1.

## 9. Những gì lát cắt 3 KHÔNG làm (YAGNI)

- **Đề bạt tri thức** vào `knowledge::graph` — lát cắt riêng, cần
  vocabulary làm đầu vào.
- **Sinh thái đa loài**.
- **"Né nguy hiểm"** — ecology chưa có ô nguy hiểm nào; thêm state mới
  chỉ để có nhiệm vụ thứ hai là phình phạm vi.
- **Chứng minh MI tăng** — user đã chốt: cơ chế + đo lường + trung thực.
- Bất cứ gì trong `export` / `runtime` / `serve`.
- **Không benchmark**: lát cắt này không đưa ra tuyên bố hiệu năng nào.

## 10. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| MI của run thật ≈ 0, trông như thất bại | Đã chốt trước: báo đúng số đo, README nói rõ hội tụ chưa chứng minh. Test cơ chế dùng dân số dựng tay nên không phụ thuộc tiến hoá. |
| Pool đột biến di chuyển phình từ 4 lên 8 tên làm loãng áp lực chọn lọc lên hành vi ăn | Ghi vào ADR-0007 như giới hạn đã biết; không tinh chỉnh hằng số sinh thái trong lát cắt này. |
| `format_version` 1_001 làm reader cũ vỡ | Chính sách §6 đã định: minor bump, reader nhận cả 1 và 1_001; test load checkpoint version 1. |
| 16 mệnh đề có hướng làm formula phình | Đột biến bảo toàn arity từ slice 2 giữ độ sâu không tăng; pool chỉ đổi **tên** atom. |
| `World::new` giờ tiêu thụ RNG cho voice ⇒ cùng seed cho quỹ đạo khác slice 2 | Không có gì để giữ: slice 2 chưa công bố quỹ đạo nào là hợp đồng. Bit-exact chỉ được khẳng định **trong cùng một phiên bản code**; ghi rõ điều đó vào format-spec thay vì để người đọc suy diễn. |
| Checkpoint slice-2 resume dưới code slice-3 sẽ đi quỹ đạo khác (atom câm, RNG lệch) | Load được là yêu cầu; **cùng quỹ đạo thì không**. Nói thẳng trong §6.2 và trong docs. |
