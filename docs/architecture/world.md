# Pillar: world (`omiai-world`)

Status: **implemented and tested** (slice 2) — the world pillar now has
agents, a deterministic loop, and bit-exact checkpoint resume on top of
the slice-1 substrate.

## Đã cài đặt và test

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
- **World loop** (`world_loop.rs`) — 5 phase cố định mỗi step:
  `ca_step → metabolism → agent_act → reproduce_and_evolve → snapshot`.
  Deterministic hoàn toàn qua ChaCha8Rng (seed/stream/word_pos,
  ADR-0006): cùng seed → cùng quỹ đạo bit-exact (test chạy 20 bước so
  từng byte). Mutation arity-preserving nên depth genome không tăng.
- **Checkpoint bundle** — `impl Checkpointable for World` trong
  omiai-checkpoint: save/load 4 file `world/*`, manifest hash toàn bộ;
  round-trip test chứng minh save step N → load → chạy M bước ra cùng
  trạng thái với world chạy liền N+M.

## Hằng số ecology (`ecology.rs`)

| Hằng | Giá trị | Ý nghĩa |
|---|---|---|
| `METABOLIC_COST` | 0.05 | năng lượng trừ mỗi step |
| `ENERGY_MAX` | 1.0 | trần năng lượng mỗi atom |
| `REPRODUCE_THRESHOLD` | 0.8 | ngưỡng chia đôi năng lượng sinh con |
| `ENERGY_PER_RESOURCE_UNIT` | 0.2 | năng lượng nhận khi ăn resource cell |
| `MUTATION_PROB` | 0.3 | xác suất đột biến genome lúc sinh |
| `MAX_FORMULA_DEPTH` | 5 | (đặt trước cho giới hạn depth tương lai; hiện mutation giữ nguyên depth gốc) |

## Giới hạn đã biết

- Registry không GC — genome tích tụ vô hạn qua các thế hệ.
- Policy decode là phép chiếu propositional: ngữ nghĩa thời gian của LTL
  (X/F/G/U/R) bị bỏ, chỉ còn logic mệnh đề tại bước hiện tại.
- Không communication/signaling giữa các atom; một loài duy nhất.
- Grid checkpoint lưu 1 byte/cell (đủ mọi state); Margolus phase và block
  cache không persist — reset khi load (xem format-spec §5).

Proptest bất biến: atom luôn trong biên lưới, energy ∈ [0, ENERGY_MAX],
gene hợp lệ trong registry, hai atom không dùng chung ô; ca_step bảo toàn
tổng giá trị ô (`tests/properties.rs`).
