# Slice 5 — Đề bạt quy ước nổi sinh thành tri thức có tên

Trạng thái: **thiết kế** (chưa cài). Lát cắt này khép vòng dưới-lên ↔
trên-xuống: quy ước giao tiếp nổi sinh trong `omiai-world` mà **chứng
minh được ích lợi ổn định** thì trở thành node có tên trong
`omiai-knowledge::graph`.

Tiền đề đã có (đọc code, không đọc README): slice 3–4 đã có `Vocabulary`
(bảng đồng xuất hiện ký hiệu × lớp trạng thái + MI), `speak` phase,
`airwave`, `hear0..3`, `team_reward` theo ngưỡng MI.

## 1. Hai lỗ hổng phải bịt trước khi nói tới "qua đủ nhiều thế hệ"

### 1.1 Voice không di truyền
`reproduce_and_evolve` đặt `voice: Vec::new()` cho mọi con → **mọi con
đều câm**. `registry::export_genomes/import_genomes` (slice 4) không
được gọi ở đâu cả. Hệ quả: không quy ước nào sống qua một thế hệ, và
`Vocabulary` tích luỹ toàn run bị dân số câm pha loãng dần. Mọi phát biểu
kiểu "quy ước ổn định qua N thế hệ" hiện nay sẽ là phát biểu suông.

**Sửa**: con kế thừa từng arm voice của cha, mỗi arm đột biến độc lập với
xác suất `VOICE_MUTATION_PROB`. Cha câm → con câm (không rút RNG), giữ
nguyên quỹ đạo của mọi test dùng atom câm.

### 1.2 Không có thước đo ích lợi
MI đo *tương quan* ký hiệu ↔ trạng thái, không đo *ích lợi*. Yêu cầu gốc
là "chứng minh được ích lợi ổn định ... theo một ngưỡng thống kê rõ
ràng". Vì vậy slice này thêm bộ đếm ích lợi thật, đo bằng kết quả sinh
thái (ăn được tài nguyên), không phải bằng tương quan.

## 2. Hợp đồng RNG (bit-exact resume)

Thứ tự rút RNG trong `reproduce_and_evolve`, mỗi atom sinh sản:

1. 1 lần `f64` cho quyết định đột biến gene di chuyển;
2. nếu đột biến: các lần rút bên trong `mutate_formula`;
3. **mới**: với cha có voice, đúng `N_SYMBOLS` lần `f64` (arm 0 → arm
   K−1), mỗi lần kèm các lần rút của `mutate_formula_with` nếu arm đó
   đột biến;
4. `split_energy` không rút RNG.

Cha câm rút 0 lần ở bước 3. Đổi thứ tự này = đổi quỹ đạo của mọi seed đã
lưu; đó là lý do nó được viết ra đây thành hợp đồng.

Hệ quả đã biết và chấp nhận: quỹ đạo của một seed **khác** trước slice 5
(dynamics đổi). Định dạng checkpoint v1 không đổi, checkpoint cũ vẫn đọc
được (§5); chỉ tương lai sau điểm resume là khác.

## 3. Thước đo ích lợi — `BenefitCounters`

Thu trong `agent_act`, một lần cho mỗi atom-step:

| trường | nghĩa |
|---|---|
| `heard_steps[s]` | số atom-step mà cờ `hear{s}` bật |
| `heard_feeds[s]` | trong số đó, số lần atom ăn được tài nguyên |
| `quiet_steps` | số atom-step không nghe ký hiệu nào |
| `quiet_feeds` | trong số đó, số lần ăn được |

Một atom-step nghe hai ký hiệu thì cộng vào cả hai hàng — bộ đếm là
*theo ký hiệu*, không phân hoạch. Ghi rõ vì nó làm `Σ heard_steps` không
bằng dân số.

Ích lợi của ký hiệu `s` trong một epoch = **tỉ lệ ăn khi nghe s ≥ tỉ lệ
ăn khi không nghe gì**, so bằng nhân chéo số nguyên (u128, không dùng
float nên không có sai số dấu phẩy động, đúng bit trên mọi máy):

```
heard_feeds[s] * quiet_steps  ≥  quiet_feeds * heard_steps[s]
```

Điều kiện phụ: `heard_steps[s] ≥ MIN_BENEFIT_SUPPORT`. Nếu
`quiet_steps == 0` (không có nền để so) thì ích lợi chỉ được coi là đạt
khi `heard_feeds[s] > 0`.

Đây là *tương quan có điều kiện*, không phải nhân quả — `do_calculus`
mới cho nhân quả và đó là slice sau. Spec này không được phát biểu quá
những gì phép đo trên chịu nổi.

## 4. Ngưỡng thống kê và tiêu chí đề bạt — `ConventionTracker`

Cửa sổ đo là **epoch** = `EPOCH_STEPS` bước world. Tracker giữ một
`Vocabulary` riêng của epoch hiện tại (khác `World::vocabulary` tích luỹ
toàn run) + `BenefitCounters` của epoch hiện tại.

Cuối mỗi epoch, với từng ký hiệu `s ∈ 0..N_SYMBOLS`:

1. `n_s` = tổng hàng `Sym(s)` trong `Vocabulary` của epoch.
   **Độ đỡ**: `n_s ≥ MIN_EPOCH_SUPPORT`.
2. `m*` = cột có số đếm lớn nhất; hoà thì cột nhỏ nhất thắng
   (deterministic, không phụ thuộc thứ tự duyệt).
3. **Độ chính xác**: `count[s][m*] * PRECISION_DEN ≥ n_s * PRECISION_NUM`
   với `PRECISION_NUM/PRECISION_DEN = 7/8` — số nguyên, không float.
4. **Ích lợi**: theo §3.
5. Cả 1+3+4 đạt ⇒ epoch này "chấp nhận" nghĩa `(s → m*)`.

Ổn định qua thế hệ: `streak[s]` = số epoch **liên tiếp** chấp nhận cùng
một `(s → m*)`. Nghĩa đổi ⇒ `streak = 1`; epoch không chấp nhận ⇒
`streak = 0`. Khi `streak[s] ≥ PROMOTION_EPOCHS` ⇒ **đề bạt** (một lần,
idempotent theo cặp `(s, m*)`).

Hằng số ở `ecology.rs`, một chỗ duy nhất, để test bơm giá trị nhỏ được:

```
EPOCH_STEPS = 64      MIN_EPOCH_SUPPORT = 16   MIN_BENEFIT_SUPPORT = 8
PRECISION_NUM = 7     PRECISION_DEN = 8        PROMOTION_EPOCHS = 3
VOICE_MUTATION_PROB = 0.1
```

Không rút RNG ở bất kỳ bước nào của §4 — đề bạt là hàm của trạng thái
đếm, nên resume không đổi kết quả.

## 5. Đề bạt vào `knowledge::graph`

`World` mang thêm `knowledge: KnowledgeGraph`. Mỗi lần đề bạt tạo:

- concept ký hiệu: id `symbol_1`, label `sym1`
- concept nghĩa: id `state_res_east` (hoặc `state_no_resource`,
  `state_on_resource`), label đọc được
- concept quy ước: id `convention_sym1_state_res_east`, label chứa
  **bằng chứng đã đo**: epoch, độ chính xác dạng phân số, tỉ lệ ăn
  nghe/không nghe
- quan hệ: `convention --signals--> symbol`, `convention --means-->
  state`, `symbol --denotes--> state`

Label chứa số đo là cố ý: node tự mang bằng chứng của chính nó, đọc
graph là đọc được vì sao nó ở đó. `PromotedConvention` lưu các số đó
dạng số nguyên (tử/mẫu), không lưu float.

## 6. Checkpoint

Thêm hai payload, hash vào `manifest.json` như mọi file khác:

| file | nội dung |
|---|---|
| `communication/conventions.cbor` | `ConventionTracker` (epoch vocab, benefit, streaks, promoted) |
| `knowledge_graph/graph.cbor` | `{concepts: [Concept], relations: [(from,to,kind)]}` |

**Tương thích ngược (bắt buộc, §6 checkpoint-v1)**: hai file này
**optional lúc load** — checkpoint slice 2/3/4 không có chúng vẫn đọc
được, cho tracker rỗng + graph rỗng. Không bump `format_version`: schema
cũ là tập con hợp lệ.

`KnowledgeGraph` không có `PartialEq`, nên round-trip test so bằng
`concept_ids()` đã sắp + `relations()` đã sắp.

## 7. Tiêu chí "xong" của lát cắt

- [ ] voice di truyền + test: con của cha có voice không câm; cha câm →
      con câm; cùng seed → cùng quỹ đạo
- [ ] `BenefitCounters` + test bảng dựng tay có đáp số chính xác
- [ ] `ConventionTracker` + test: đạt ngưỡng thì đề bạt đúng epoch thứ
      `PROMOTION_EPOCHS`, đổi nghĩa thì reset streak, dưới ngưỡng độ đỡ
      thì không đề bạt, đề bạt idempotent
- [ ] graph có node + quan hệ đúng tên, label chứa bằng chứng
- [ ] round-trip checkpoint bit-exact gồm tracker + graph; đọc được
      checkpoint không có hai file mới
- [ ] `cargo test --workspace` xanh, clippy sạch
- [ ] docs: ADR-0007, architecture/world.md, format-spec, README

Không có phát biểu hiệu năng nào trong lát cắt này ⇒ không thêm bench.
