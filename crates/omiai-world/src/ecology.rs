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
/// Ngưỡng MI để kích hoạt team reward (shared energy bonus).
/// log₂6 ≈ 2.585 là trần, dùng 0.5 làm ngưỡng hội tụ ý nghĩa.
pub const TEAM_MI_THRESHOLD: f64 = 0.5;
/// Phần thưởng năng lượng mỗi atom khi team reward kích hoạt.
pub const TEAM_REWARD_ENERGY: f64 = 0.1;

// ── Truyền văn hoá (slice 5) ──────────────────────────────────────────

/// Xác suất đột biến **từng arm** voice khi con kế thừa từ cha.
///
/// Thấp hơn `MUTATION_PROB` của gene di chuyển: quy ước chỉ có ích khi
/// đủ nhiều cá thể dùng CÙNG nghĩa, nên tiếng nói phải bảo thủ hơn hành
/// vi. Cao quá thì không quy ước nào sống nổi tới `PROMOTION_EPOCHS`.
pub const VOICE_MUTATION_PROB: f64 = 0.1;

// ── Ngưỡng đề bạt quy ước (slice 5, spec §4) ──────────────────────────

/// Độ dài cửa sổ đo một epoch, tính bằng bước world.
pub const EPOCH_STEPS: u64 = 64;

/// Số lần một ký hiệu phải được phát trong epoch để phép đo có nghĩa.
/// Dưới ngưỡng này, "độ chính xác 100%" chỉ là 1/1 — không đề bạt.
pub const MIN_EPOCH_SUPPORT: u64 = 16;

/// Số atom-step nghe được ký hiệu, tối thiểu, để tỉ lệ ăn có nghĩa.
pub const MIN_BENEFIT_SUPPORT: u64 = 8;

/// Độ chính xác tối thiểu dạng phân số `NUM/DEN` = 7/8.
///
/// So bằng nhân chéo số nguyên chứ không chia f64: cùng một bảng đếm
/// luôn cho cùng một quyết định, không phụ thuộc thứ tự làm tròn.
pub const PRECISION_NUM: u64 = 7;
/// Mẫu số của ngưỡng độ chính xác — xem [`PRECISION_NUM`].
pub const PRECISION_DEN: u64 = 8;

/// Số epoch LIÊN TIẾP một nghĩa phải trụ được trước khi đề bạt.
pub const PROMOTION_EPOCHS: u32 = 3;
