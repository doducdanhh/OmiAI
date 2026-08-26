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
