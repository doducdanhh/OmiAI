//! Ngôn ngữ nổi sinh: bảng ký hiệu, giải mã voice gene, thước đo MI.
//!
//! Module này **thuần tuý**: không biết `World`, không rút RNG, không I/O.
//! Nhờ vậy thước đo MI kiểm được bằng bảng dựng tay có đáp số chính xác —
//! điều kiện để mọi kết luận của lát cắt 3 đáng tin.

use serde::{Deserialize, Serialize};

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
}
