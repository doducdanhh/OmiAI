# Slice 5 — kế hoạch triển khai (TDD, 9 task)

Spec: [`2026-08-30-world-knowledge-promotion-slice5-design.md`](../specs/2026-08-30-world-knowledge-promotion-slice5-design.md).
Baseline trước lát cắt: `cargo test --workspace` = **317 test xanh**.
Mỗi task chỉ xong khi cả workspace còn xanh.

| # | Task | File | Test bắt buộc |
|---|---|---|---|
| 1 | Hằng số epoch/ngưỡng/voice-mutation | `world/ecology.rs` | dùng ở task sau |
| 2 | `StateClass::from_col/id/label` | `world/communication.rs` | round-trip col ↔ StateClass cho cả 6 lớp |
| 3 | `BenefitCounters` + tiêu chí ích lợi | `world/communication.rs` | bảng dựng tay: đạt/không đạt, `quiet_steps = 0`, dưới support |
| 4 | `ConventionTracker` (epoch, streak, promote) | `world/communication.rs` | đề bạt đúng epoch thứ K; đổi nghĩa reset; dưới support không đề bạt; idempotent |
| 5 | `inherit_voice` + gọi trong `reproduce_and_evolve` | `world/world_loop.rs` | cha có voice → con có voice hợp lệ; cha câm → con câm; cùng seed cùng quỹ đạo |
| 6 | Thu `BenefitCounters` trong `agent_act` | `world/world_loop.rs` | ăn khi nghe → `heard_feeds` tăng; không nghe → `quiet_*` |
| 7 | Phase `promote_knowledge` + `World::knowledge` | `world/world_loop.rs` | world chạy nhiều epoch: node xuất hiện, `step()` 8 phase, không rút RNG |
| 8 | Checkpoint 2 payload mới + optional lúc load | `checkpoint/world_bundle.rs` | round-trip bit-exact; đọc checkpoint thiếu 2 file mới |
| 9 | Docs: ADR-0007, architecture, format-spec, README | `docs/`, `README.md` | — |

Thứ tự cố định vì task 5–7 phụ thuộc 1–4, task 8 phụ thuộc 4+7.

## Rủi ro đã biết

- **Task 5 đổi quỹ đạo mọi seed.** Không tránh được (thêm lần rút RNG là
  đổi dãy). Ghi thành hợp đồng ở spec §2 + ADR-0007; test bit-exact vẫn
  giữ giá trị vì nó so save↔load trong cùng phiên bản.
- **Task 7 thêm dependency `omiai-world → omiai-knowledge`.** Không tạo
  vòng: `omiai-knowledge` chỉ phụ thuộc `omiai-core`.
- **Đề bạt là tương quan, không phải nhân quả.** Ghi rõ trong doc node và
  ADR; `do_calculus` là slice sau.
