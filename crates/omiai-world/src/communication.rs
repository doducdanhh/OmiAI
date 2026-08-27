//! Ngôn ngữ nổi sinh: bảng ký hiệu, giải mã voice gene, thước đo MI.
//!
//! Module này **thuần tuý**: không biết `World`, không rút RNG, không I/O.
//! Nhờ vậy thước đo MI kiểm được bằng bảng dựng tay có đáp số chính xác —
//! điều kiện để mọi kết luận của lát cắt 3 đáng tin.

use std::collections::BTreeMap;

use omiai_core::ltl::LtlFormula;
use serde::{Deserialize, Serialize};

use crate::agents::{Direction, Observation, eval_current};
use crate::registry::{FormulaId, FormulaRegistry};

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
        for (s, row) in self.joint.iter().enumerate() {
            r[s] = row.iter().sum();
        }
        r
    }

    fn col_sums(&self) -> [u64; N_STATE_CLASSES] {
        let mut c = [0; N_STATE_CLASSES];
        for (m, slot) in c.iter_mut().enumerate() {
            *slot = self.joint.iter().map(|row| row[m]).sum();
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
        for (s, row) in self.joint.iter().enumerate() {
            for (m, &count) in row.iter().enumerate() {
                if count == 0 {
                    continue;
                }
                let c = count as f64;
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
    "open_n",
    "wall_n",
    "res_n",
    "occupied_n", //
    "open_e",
    "wall_e",
    "res_e",
    "occupied_e", //
    "open_s",
    "wall_s",
    "res_s",
    "occupied_s", //
    "open_w",
    "wall_w",
    "res_w",
    "occupied_w",
];

/// Valuation 16 mệnh đề của cả vùng lân cận — miền đánh giá của voice arm.
///
/// KHÔNG chứa `hear*`: voice không được phụ thuộc cái nghe được, xem spec
/// §2.4 (mọi atom phát cùng lúc nên đọc airwave đang-ghi-dở sẽ làm ký hiệu
/// phụ thuộc thứ tự `Vec`).
pub fn neighbourhood_valuation(obs_by_dir: &[(Direction, Observation)]) -> BTreeMap<String, bool> {
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
        for (s, row) in joint.iter_mut().enumerate() {
            row[s] = 10;
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

    use crate::agents::{Direction, observe};
    use crate::registry::Genome;

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
                reg.insert(Genome {
                    formula: LtlFormula::atom(*n),
                    fitness: None,
                })
            })
            .collect();
        // Tài nguyên ở E và S → arm 1 (res_e) là arm đầu tiên thoả.
        let val = neighbourhood_valuation(&obs4([(0, false), (2, false), (2, false), (0, false)]));
        assert_eq!(decode_voice(&arms, &reg, &val), SignalValue::Sym(1));
    }

    #[test]
    fn decode_voice_silent_when_no_arm_fires_or_atom_is_mute() {
        let mut reg = FormulaRegistry::new();
        let arms: Vec<_> = (0..N_SYMBOLS)
            .map(|_| {
                reg.insert(Genome {
                    formula: LtlFormula::atom("res_n"),
                    fitness: None,
                })
            })
            .collect();
        let val = neighbourhood_valuation(&obs4([(0, false), (0, false), (0, false), (0, false)]));
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
        assert!(
            printed.contains("res_n"),
            "hạt giống phải nói tên có hướng: {printed}"
        );
        let val = neighbourhood_valuation(&obs4([(2, false), (0, false), (0, false), (0, false)]));
        assert!(
            crate::agents::eval_current(&f, &val),
            "hạt giống phải bắn được"
        );
    }
}
