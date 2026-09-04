# Pillar: world (`omiai-world`)

Status: **implemented and tested** (slice 5) — the world pillar now has
agents, a deterministic 8-phase loop, emergent communication with
heritable voice, convention promotion to knowledge graph, and bit-exact
checkpoint resume including the new payloads.

## Đã cài đặt và test (slice 1–5)

- **Substrate** — reversible Margolus block CA (population-preserving,
  rayon sweeps, HashLife-style block cache), unit + integration tests,
  criterion bench, checkpoint round-trip + conservation proptests.
- **`FormulaRegistry`** (`registry.rs`) — generational arena của
  `Genome { formula: LtlFormula, fitness }`; `FormulaId` serialize như
  u32 slot-index. Không GC: genome không bao giờ bị xoá, registry chỉ
  phình theo số lần sinh sản.
- **Atom lifecycle** (`atoms.rs`) — `metabolize()` (trừ năng lượng, chết
  khi ≤0), `feed()` (ăn resource cell, +0.2/đơn vị, clamp ENERGY_MAX),
  `split_energy()` (chia đôi tại ngưỡng REPRODUCE_THRESHOLD).
- **Agent policy decode** (`agents.rs`) — atom quan sát 4 hướng
  (open/wall/res/occupied), decode policy **propositional projection**
  của LTL genome: toán tử thời gian (X/F/G) được coi như hiện tại,
  Until→toán hạng phải, Release→p∧q. Giới hạn ghi rõ ở dưới.
- **Voice & Emergent communication** (`communication.rs`, slice 3–5) —
  `N_SYMBOLS = 6` ký hiệu, `StateClass` 6 nghĩa (4 hướng tài nguyên +
  không tài nguyên + đang đứng trên tài nguyên). `Vocabulary` tích luỹ
  bảng đồng xuất hiện ký hiệu × nghĩa + MI. `hear` cờ 4-bit gửi từ
  `speak` phase → `agent_act`. Lewis signaling game: atom phát ký hiệu
  tùy ý, *team reward* (bonus năng lượng) kích hoạt khi MI vượt ngưỡng
  (slice 4). Slice 5 thêm **bộ đếm ích lợi** (`BenefitCounters`) và
  `ConventionTracker` (epoch + streak), đề bạt quy ước ổn định vào
  `knowledge::graph`.
- **Voice inheritance** (`world_loop.rs::inherit_voice`, slice 5) — con
  kế thừa từng arm voice của cha, mỗi arm đột biến độc lập với xác suất
  `VOICE_MUTATION_PROB = 0.1`. Cha câm → con câm (không rút RNG),
  giữ nguyên quỹ đạo test. Hợp đồng RNG ghi trong spec §2 + ADR-0007.
- **World loop** (`world_loop.rs`) — **8 phase cố định** mỗi step:
  `ca_step → metabolism → speak → agent_act → reproduce_and_evolve →
  team_reward → promote_knowledge → snapshot`. `speak` ghi `airwave` và
  `Vocabulary` + `ConventionTracker`. `agent_act` thu `BenefitCounters`
  (nghe ký hiệu s → ăn/không ăn). `reproduce_and_evolve` gọi
  `inherit_voice`. `promote_knowledge` đóng epoch, thử đề bạt, nạp
  `knowledge::graph` (node + 3 quan hệ, label chứa bằng chứng đo được).
  Deterministic hoàn toàn qua ChaCha8Rng (seed/stream/word_pos,
  ADR-0006): cùng seed → cùng quỹ đạo bit-exact. Mutation arity-preserving
  nên depth genome không tăng.
- **Checkpoint bundle** — `impl Checkpointable for World` trong
  omiai-checkpoint: save/load **8 file** (`world/*` + `communication/conventions.cbor`
  + `knowledge_graph/graph.cbor`), manifest hash toàn bộ; round-trip test
  chứng minh save step N → load → chạy M bước ra cùng trạng thái với
  world chạy liền N+M. **Backward compatible**: checkpoint slice 2/3/4
  (thiếu 2 file mới) vẫn load được → tracker rỗng + graph rỗng.

## Hằng số ecology (`ecology.rs`)

| Hằng | Giá trị | Ý nghĩa |
|---|---|---|
| `METABOLIC_COST` | 0.05 | năng lượng trừ mỗi step |
| `ENERGY_MAX` | 1.0 | trần năng lượng mỗi atom |
| `REPRODUCE_THRESHOLD` | 0.8 | ngưỡng chia đôi năng lượng sinh con |
| `ENERGY_PER_RESOURCE_UNIT` | 0.2 | năng lượng nhận khi ăn resource cell |
| `MUTATION_PROB` | 0.3 | xác suất đột biến genome lúc sinh |
| `MAX_FORMULA_DEPTH` | 5 | (đặt trước cho giới hạn depth tương lai; hiện mutation giữ nguyên depth gốc) |
| `VOICE_MUTATION_PROB` | 0.1 | xác suất đột biến từng arm voice khi con kế thừa |
| `EPOCH_STEPS` | 64 | độ dài epoch (bước world) để đo quy ước |
| `MIN_EPOCH_SUPPORT` | 16 | độ đỡ tối thiểu một ký hiệu trong epoch |
| `MIN_BENEFIT_SUPPORT` | 8 | nghe tối thiểu để tỉ lệ ăn có nghĩa |
| `PRECISION_NUM` | 7 | tử số ngưỡng độ chính xác (7/8) |
| `PRECISION_DEN` | 8 | mẫu số ngưỡng độ chính xác |
| `PROMOTION_EPOCHS` | 3 | số epoch liên tiếp để đề bạt |
| `TEAM_MI_THRESHOLD` | 0.5 | ngưỡng MI cho team reward (slice 4) |
| `TEAM_REWARD_ENERGY` | 0.1 | bonus năng lượng team reward |

## Giới hạn đã biết

- Registry không GC — genome tích tụ vô hạn qua các thế hệ.
- Policy decode là phép chiếu propositional: ngữ nghĩa thời gian của LTL
  (X/F/G/U/R) bị bỏ, chỉ còn logic mệnh đề tại bước hiện tại.
- Grid checkpoint lưu 1 byte/cell (đủ mọi state); Margolus phase và block
  cache không persist — reset khi load (xem format-spec §5).
- **Quy ước được đề bạt là tương quan có điều kiện, KHÔNG phải nhân quả**.
  Tiêu chí đo `P(feed|hear s) ≥ P(feed|hear nothing)`; `do_calculus`
  (slice sau) mới cho nhân quả. Ghi trong ADR-0007 và label node graph.
- Checkpoint slice 4 resume trên slice 5 **đổi quỹ đạo** từ lần sinh
  sản đầu tiên (voice inheritance thêm lần rút RNG). Không bump
  `format_version` (optional payloads), ghi hợp đồng trong ADR-0007.

Proptest bất biến: atom luôn trong biên lưới, energy ∈ [0, ENERGY_MAX],
gene hợp lệ trong registry, hai atom không dùng chung ô; ca_step bảo toàn
tổng giá trị ô (`tests/properties.rs`).
