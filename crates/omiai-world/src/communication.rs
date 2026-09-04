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

/// Số lớp trạng thái thế giới: 4 hướng + "không có tài nguyên kề" + "đứng trên tài nguyên (beacon)".
pub const N_STATE_CLASSES: usize = 6;

/// Giá trị tín hiệu. **Im lặng LÀ một giá trị**, không phải dữ liệu thiếu:
/// nếu im lặng bị bỏ khỏi bảng đếm thì trần MI bị chặn dưới log₂6 vì lý do
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

/// Lớp trạng thái mà tín hiệu nói về: hướng ô tài nguyên KỀ sender + beacon (đứng trên tài nguyên).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateClass {
    North,
    East,
    South,
    West,
    None,
    Resource, // beacon: atom đang đứng trên ô tài nguyên
}

impl StateClass {
    /// Cột trong bảng joint, theo đúng thứ tự khai báo N,E,S,W,None,Resource.
    pub fn col(self) -> usize {
        match self {
            StateClass::North => 0,
            StateClass::East => 1,
            StateClass::South => 2,
            StateClass::West => 3,
            StateClass::None => 4,
            StateClass::Resource => 5,
        }
    }

    /// Nghịch đảo của [`col`](Self::col); `None` nếu cột ngoài bảng.
    ///
    /// Cần cho đường đi ngược checkpoint → tri thức: `PromotedConvention`
    /// lưu nghĩa dưới dạng chỉ số cột (ổn định qua CBOR) chứ không lưu tên
    /// biến thể, nên load phải dựng lại được `StateClass`.
    pub fn from_col(col: usize) -> Option<Self> {
        Some(match col {
            0 => StateClass::North,
            1 => StateClass::East,
            2 => StateClass::South,
            3 => StateClass::West,
            4 => StateClass::None,
            5 => StateClass::Resource,
            _ => return None,
        })
    }

    /// Id dùng làm tên concept trong `knowledge::graph` — snake_case, ổn
    /// định (đổi id là đổi tên node đã đề bạt trong checkpoint cũ).
    pub fn concept_id(self) -> &'static str {
        match self {
            StateClass::North => "state_res_north",
            StateClass::East => "state_res_east",
            StateClass::South => "state_res_south",
            StateClass::West => "state_res_west",
            StateClass::None => "state_no_resource",
            StateClass::Resource => "state_on_resource",
        }
    }

    /// Nhãn người đọc được của lớp trạng thái.
    pub fn label(self) -> &'static str {
        match self {
            StateClass::North => "resource to the North",
            StateClass::East => "resource to the East",
            StateClass::South => "resource to the South",
            StateClass::West => "resource to the West",
            StateClass::None => "no adjacent resource",
            StateClass::Resource => "standing on a resource",
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

    /// Số lần ký hiệu `sym` được phát (tổng hàng) — độ đỡ của phép đo.
    pub fn symbol_support(&self, sym: Symbol) -> u64 {
        self.joint[SignalValue::Sym(sym).row()].iter().sum()
    }

    /// Lớp trạng thái mà `sym` hay đi kèm nhất, kèm số đếm của nó.
    ///
    /// Hoà thì **cột nhỏ nhất thắng** — quyết định không được phụ thuộc
    /// thứ tự duyệt, nếu không hai lần chạy cùng bảng cho hai nghĩa khác
    /// nhau và mọi kết luận về "quy ước ổn định" mất giá trị.
    pub fn modal_state(&self, sym: Symbol) -> Option<(StateClass, u64)> {
        let row = &self.joint[SignalValue::Sym(sym).row()];
        let (col, &count) = row
            .iter()
            .enumerate()
            .max_by_key(|&(col, &c)| (c, std::cmp::Reverse(col)))?;
        if count == 0 {
            return None;
        }
        StateClass::from_col(col).map(|m| (m, count))
    }
}

/// Bộ đếm ích lợi của một epoch (spec slice-5 §3).
///
/// Ích lợi được đo bằng **kết quả sinh thái** (ăn được tài nguyên), không
/// bằng tương quan ký hiệu↔trạng thái — MI đã lo phần tương quan. Một
/// atom-step nghe hai ký hiệu cộng vào cả hai hàng, nên
/// `Σ heard_steps ≠` dân số; đó là chủ ý, bộ đếm là *theo ký hiệu*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BenefitCounters {
    /// Số atom-step mà cờ `hear{s}` bật.
    pub heard_steps: [u64; N_SYMBOLS],
    /// Trong số đó, số lần atom ăn được tài nguyên.
    pub heard_feeds: [u64; N_SYMBOLS],
    /// Số atom-step không nghe ký hiệu nào.
    pub quiet_steps: u64,
    /// Trong số đó, số lần ăn được.
    pub quiet_feeds: u64,
}

impl BenefitCounters {
    /// Ghi một atom-step: nghe những gì, có ăn được không.
    pub fn record(&mut self, hear: &[bool; N_SYMBOLS], fed: bool) {
        let mut any = false;
        for (s, &on) in hear.iter().enumerate() {
            if on {
                any = true;
                self.heard_steps[s] += 1;
                if fed {
                    self.heard_feeds[s] += 1;
                }
            }
        }
        if !any {
            self.quiet_steps += 1;
            if fed {
                self.quiet_feeds += 1;
            }
        }
    }

    /// Ký hiệu `sym` có ích trong epoch này chưa: tỉ lệ ăn khi nghe `sym`
    /// ≥ tỉ lệ ăn khi im ắng, so bằng nhân chéo `u128` (không float ⇒
    /// cùng bảng đếm luôn cho cùng quyết định trên mọi máy).
    ///
    /// Không có nền so sánh (`quiet_steps == 0`) thì chỉ đạt khi thật sự
    /// đã ăn được lần nào lúc nghe.
    pub fn benefits(&self, sym: Symbol) -> bool {
        let s = sym as usize;
        if s >= N_SYMBOLS || self.heard_steps[s] < crate::ecology::MIN_BENEFIT_SUPPORT {
            return false;
        }
        if self.quiet_steps == 0 {
            return self.heard_feeds[s] > 0;
        }
        u128::from(self.heard_feeds[s]) * u128::from(self.quiet_steps)
            >= u128::from(self.quiet_feeds) * u128::from(self.heard_steps[s])
    }
}

/// Theo dõi quy ước theo **epoch** và đề bạt khi đủ ổn định (spec §4).
///
/// Không rút RNG ở bất kỳ đường nào: đề bạt là hàm thuần của bảng đếm,
/// nên resume từ checkpoint cho đúng cùng kết luận.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConventionTracker {
    /// Số epoch đã đóng.
    pub epoch_index: u64,
    /// Số bước đã đi trong epoch hiện tại.
    pub steps_in_epoch: u64,
    /// Bảng đếm CỦA RIÊNG epoch hiện tại — khác `World::vocabulary` (tích
    /// luỹ toàn run). Quy ước "ổn định qua nhiều epoch" chỉ đo được khi
    /// từng epoch được đo riêng.
    pub epoch_vocab: Vocabulary,
    pub benefit: BenefitCounters,
    /// Nghĩa đang giữ streak cho từng ký hiệu (chỉ số cột), `None` = đứt.
    pub streak_meaning: [Option<u8>; N_SYMBOLS],
    /// Số epoch liên tiếp nghĩa tương ứng trụ được.
    pub streak_len: [u32; N_SYMBOLS],
    /// Các quy ước đã đề bạt, theo thứ tự đề bạt.
    pub promoted: Vec<PromotedConvention>,
}

impl ConventionTracker {
    /// Ghi một mẫu (tín hiệu, trạng thái) vào epoch hiện tại.
    pub fn record_signal(&mut self, signal: SignalValue, state: StateClass) {
        self.epoch_vocab.record(signal, state);
    }

    /// Ghi một atom-step vào bộ đếm ích lợi của epoch hiện tại.
    pub fn record_benefit(&mut self, hear: &[bool; N_SYMBOLS], fed: bool) {
        self.benefit.record(hear, fed);
    }

    /// Ứng viên của epoch hiện tại cho ký hiệu `sym`: đủ độ đỡ, đủ độ
    /// chính xác, và có ích. Trả `None` nếu thiếu bất kỳ điều kiện nào.
    pub fn candidate(&self, sym: Symbol) -> Option<(StateClass, u64, u64)> {
        let total = self.epoch_vocab.symbol_support(sym);
        if total < crate::ecology::MIN_EPOCH_SUPPORT {
            return None;
        }
        let (meaning, hits) = self.epoch_vocab.modal_state(sym)?;
        let precise = u128::from(hits) * u128::from(crate::ecology::PRECISION_DEN)
            >= u128::from(total) * u128::from(crate::ecology::PRECISION_NUM);
        if !precise || !self.benefit.benefits(sym) {
            return None;
        }
        Some((meaning, hits, total))
    }

    /// Đếm một bước world; đóng epoch khi đủ [`EPOCH_STEPS`] bước.
    ///
    /// Trả về các quy ước **mới** được đề bạt ở lần đóng này (rỗng ở mọi
    /// bước khác) — người gọi đem chúng sang `knowledge::graph`.
    ///
    /// [`EPOCH_STEPS`]: crate::ecology::EPOCH_STEPS
    pub fn note_step(&mut self) -> Vec<PromotedConvention> {
        self.steps_in_epoch += 1;
        if self.steps_in_epoch < crate::ecology::EPOCH_STEPS {
            return Vec::new();
        }
        self.close_epoch()
    }

    /// Đóng epoch: cập nhật streak từng ký hiệu, đề bạt cái nào đủ lâu,
    /// rồi xoá bảng đếm để epoch sau đo độc lập.
    pub fn close_epoch(&mut self) -> Vec<PromotedConvention> {
        let mut newly = Vec::new();
        for s in 0..N_SYMBOLS {
            let sym = s as Symbol;
            match self.candidate(sym) {
                Some((meaning, hits, total)) => {
                    let col = meaning.col() as u8;
                    if self.streak_meaning[s] == Some(col) {
                        self.streak_len[s] += 1;
                    } else {
                        self.streak_meaning[s] = Some(col);
                        self.streak_len[s] = 1;
                    }
                    if self.streak_len[s] >= crate::ecology::PROMOTION_EPOCHS {
                        let record = PromotedConvention {
                            symbol: sym,
                            meaning_col: col,
                            epoch: self.epoch_index,
                            streak: self.streak_len[s],
                            precision_hits: hits,
                            precision_total: total,
                            heard_steps: self.benefit.heard_steps[s],
                            heard_feeds: self.benefit.heard_feeds[s],
                            quiet_steps: self.benefit.quiet_steps,
                            quiet_feeds: self.benefit.quiet_feeds,
                        };
                        // Idempotent theo cặp (ký hiệu, nghĩa): quy ước đã
                        // có tên rồi thì không sinh node trùng mỗi epoch.
                        let known = self
                            .promoted
                            .iter()
                            .any(|p| p.symbol == sym && p.meaning_col == col);
                        if !known {
                            self.promoted.push(record.clone());
                            newly.push(record);
                        }
                    }
                }
                None => {
                    // Nghĩa đứt: streak về 0, phải gây dựng lại từ đầu.
                    self.streak_meaning[s] = None;
                    self.streak_len[s] = 0;
                }
            }
        }
        self.epoch_index += 1;
        self.steps_in_epoch = 0;
        self.epoch_vocab = Vocabulary::default();
        self.benefit = BenefitCounters::default();
        newly
    }
}

/// Một quy ước đã được đề bạt, **kèm bằng chứng đã đo**.
///
///
/// Mọi số là số nguyên (tử/mẫu), không float: file checkpoint phải đọc
/// lại y nguyên trên máy khác, và một node tri thức nói "chính xác 15/16"
/// kiểm lại được, còn "0.9375" thì tuỳ cách in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotedConvention {
    pub symbol: Symbol,
    /// Nghĩa dưới dạng chỉ số cột — xem [`StateClass::from_col`].
    pub meaning_col: u8,
    /// Epoch mà điều kiện đủ lần cuối (epoch đề bạt).
    pub epoch: u64,
    /// Số epoch liên tiếp nghĩa này trụ được tính tới lúc đề bạt.
    pub streak: u32,
    /// Độ chính xác = `precision_hits / precision_total` trong epoch đó.
    pub precision_hits: u64,
    pub precision_total: u64,
    /// Bằng chứng ích lợi: tỉ lệ ăn khi nghe so với khi im ắng.
    pub heard_steps: u64,
    pub heard_feeds: u64,
    pub quiet_steps: u64,
    pub quiet_feeds: u64,
}

impl PromotedConvention {
    pub fn meaning(&self) -> Option<StateClass> {
        StateClass::from_col(self.meaning_col as usize)
    }

    /// Id concept của ký hiệu, ví dụ `symbol_1`.
    pub fn symbol_concept_id(&self) -> String {
        format!("symbol_{}", self.symbol)
    }

    /// Id concept của chính quy ước, ví dụ `convention_sym1_state_res_east`.
    pub fn concept_id(&self) -> String {
        let meaning = self.meaning().map_or("state_unknown", |m| m.concept_id());
        format!("convention_sym{}_{}", self.symbol, meaning)
    }

    /// Nhãn tự mang bằng chứng: đọc node là biết vì sao nó ở đó.
    pub fn label(&self) -> String {
        let meaning = self.meaning().map_or("unknown", |m| m.label());
        format!(
            "sym{} ⇒ {} (epoch {}, {} epochs stable, precision {}/{}, feed rate {}/{} heard vs {}/{} quiet)",
            self.symbol,
            meaning,
            self.epoch,
            self.streak,
            self.precision_hits,
            self.precision_total,
            self.heard_feeds,
            self.heard_steps,
            self.quiet_feeds,
            self.quiet_steps,
        )
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
///
/// Nếu atom đứng trên ô tài nguyên (giá trị 2 hoặc 3) → trả về Resource (beacon).
pub fn state_class(obs_by_dir: &[(Direction, Observation)], self_cell_value: u8) -> StateClass {
    // Beacon: atom đang đứng trên tài nguyên
    if self_cell_value >= 2 {
        return StateClass::Resource;
    }
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
    /// log₂6 — trần MI khi 5 giá trị tín hiệu song ánh với 6 lớp trạng thái.
    const LOG2_6: f64 = 2.584962500721156;

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
        let w_state = [1u64, 2, 3, 4, 5, 6]; // 6 state classes
        let mut joint = [[0u64; N_STATE_CLASSES]; N_SIGNAL_VALUES];
        for s in 0..N_SIGNAL_VALUES {
            for m in 0..N_STATE_CLASSES {
                joint[s][m] = w[s] * w_state[m];
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
        joint[1] = [7, 3, 11, 5, 2, 0];
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
        assert_eq!(state_class(&obs, 0), StateClass::East);
        // Không có tài nguyên kề → None.
        let obs = obs4([(0, false), (0, false), (1, false), (0, true)]);
        assert_eq!(state_class(&obs, 0), StateClass::None);
    }

    #[test]
    fn state_class_ignores_resource_value() {
        // N giá trị 2 thắng E giá trị 3: thứ tự quyết định, không phải độ giàu.
        let obs = obs4([(2, false), (3, false), (0, false), (0, false)]);
        assert_eq!(state_class(&obs, 0), StateClass::North);
    }

    #[test]
    fn state_class_counts_resource_under_another_atom() {
        // ca_step (Margolus) đẩy được tài nguyên vào ô đang bị chiếm; ô đó
        // vẫn là tài nguyên. `res` và `occupied` là hai mệnh đề độc lập.
        let obs = obs4([(0, false), (2, true), (0, false), (0, false)]);
        assert_eq!(state_class(&obs, 0), StateClass::East);
        let val = neighbourhood_valuation(&obs);
        assert!(val["res_e"] && val["occupied_e"]);
    }

    #[test]
    fn state_class_radius_is_exactly_one() {
        // 4 ô kề trống ⇒ None, bất kể có gì cách hai ô: mảng quan sát chỉ
        // chứa 4 ô kề nên "xa hơn" về mặt cấu trúc không vào được.
        let obs = obs4([(0, false), (0, false), (0, false), (0, false)]);
        assert_eq!(state_class(&obs, 0), StateClass::None);
    }

    #[test]
    fn state_class_beacon_on_resource() {
        // Atom đứng trên ô tài nguyên (giá trị 2) → Resource beacon
        let obs = obs4([(0, false), (0, false), (0, false), (0, false)]);
        assert_eq!(state_class(&obs, 2), StateClass::Resource);
        // Atom đứng trên ô tài nguyên giá trị 3 → Resource beacon
        assert_eq!(state_class(&obs, 3), StateClass::Resource);
        // Atom đứng trên ô không phải tài nguyên → None
        assert_eq!(state_class(&obs, 0), StateClass::None);
        assert_eq!(state_class(&obs, 1), StateClass::None);
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

    // ── slice 5: nghĩa ↔ cột, ích lợi, đề bạt ─────────────────────────

    #[test]
    fn state_class_col_round_trips_and_names_are_unique() {
        let all = [
            StateClass::North,
            StateClass::East,
            StateClass::South,
            StateClass::West,
            StateClass::None,
            StateClass::Resource,
        ];
        assert_eq!(all.len(), N_STATE_CLASSES);
        let mut ids = std::collections::BTreeSet::new();
        for m in all {
            assert_eq!(StateClass::from_col(m.col()), Some(m));
            assert!(ids.insert(m.concept_id()), "id trùng: {}", m.concept_id());
            assert!(!m.label().is_empty());
        }
        assert_eq!(StateClass::from_col(N_STATE_CLASSES), None);
    }

    #[test]
    fn modal_state_breaks_ties_toward_the_lowest_column() {
        let mut joint = [[0u64; N_STATE_CLASSES]; N_SIGNAL_VALUES];
        // Sym(0) hoà 5–5 giữa cột 1 và cột 3 → cột 1 phải thắng.
        joint[1][1] = 5;
        joint[1][3] = 5;
        let v = vocab_from(joint);
        assert_eq!(v.modal_state(0), Some((StateClass::East, 5)));
        assert_eq!(v.symbol_support(0), 10);
        // Ký hiệu chưa bao giờ phát → không có nghĩa nào.
        assert_eq!(v.modal_state(2), None);
        assert_eq!(v.symbol_support(2), 0);
    }

    #[test]
    fn benefit_counters_split_heard_and_quiet_steps() {
        let mut b = BenefitCounters::default();
        let mut hear = [false; N_SYMBOLS];
        hear[1] = true;
        hear[2] = true;
        b.record(&hear, true); // nghe 1 và 2, ăn được → cộng cả hai hàng
        b.record(&[false; N_SYMBOLS], false);
        b.record(&[false; N_SYMBOLS], true);

        assert_eq!(b.heard_steps[1], 1);
        assert_eq!(b.heard_steps[2], 1);
        assert_eq!(b.heard_feeds[1], 1);
        assert_eq!(b.heard_steps[0], 0);
        assert_eq!(b.quiet_steps, 2);
        assert_eq!(b.quiet_feeds, 1);
    }

    /// Bơm bộ đếm ích lợi tới một tỉ lệ chính xác cho trước.
    fn benefit_with(
        sym: Symbol,
        heard_steps: u64,
        heard_feeds: u64,
        quiet_steps: u64,
        quiet_feeds: u64,
    ) -> BenefitCounters {
        let mut b = BenefitCounters::default();
        let s = sym as usize;
        b.heard_steps[s] = heard_steps;
        b.heard_feeds[s] = heard_feeds;
        b.quiet_steps = quiet_steps;
        b.quiet_feeds = quiet_feeds;
        b
    }

    #[test]
    fn benefit_criterion_compares_feed_rates_exactly() {
        use crate::ecology::MIN_BENEFIT_SUPPORT;
        let n = MIN_BENEFIT_SUPPORT;
        // 3/8 = 0.375 > 1/10 = 0.1 → có ích.
        assert!(benefit_with(0, n, 3, 10, 1).benefits(0));
        // 1/8 = 0.125 < 3/10 = 0.3 → vô ích.
        assert!(!benefit_with(0, n, 1, 10, 3).benefits(0));
        // Bằng nhau (2/8 = 5/20) → vẫn tính là có ích (tiêu chí là ≥).
        assert!(benefit_with(0, n, 2, 20, 5).benefits(0));
    }

    #[test]
    fn benefit_needs_support_and_handles_missing_baseline() {
        use crate::ecology::MIN_BENEFIT_SUPPORT;
        // Dưới độ đỡ: 1/1 = 100% cũng không tính.
        assert!(!benefit_with(0, MIN_BENEFIT_SUPPORT - 1, 1, 10, 0).benefits(0));
        // Không có nền so sánh: chỉ đạt nếu thật sự ăn được lần nào.
        assert!(benefit_with(0, MIN_BENEFIT_SUPPORT, 1, 0, 0).benefits(0));
        assert!(!benefit_with(0, MIN_BENEFIT_SUPPORT, 0, 0, 0).benefits(0));
        // Ký hiệu ngoài bảng chữ không bao giờ có ích.
        assert!(!benefit_with(0, MIN_BENEFIT_SUPPORT, 8, 0, 0).benefits(N_SYMBOLS as Symbol));
    }

    /// Nạp một epoch "sạch": `n` lần phát `sym` đúng nghĩa `state`, cộng
    /// bộ đếm ích lợi vượt ngưỡng rõ ràng (4/8 khi nghe vs 1/10 khi im).
    fn stack_clean_epoch(t: &mut ConventionTracker, sym: Symbol, state: StateClass, n: u64) {
        for _ in 0..n {
            t.record_signal(SignalValue::Sym(sym), state);
        }
        let mut hear = [false; N_SYMBOLS];
        hear[sym as usize] = true;
        for i in 0..crate::ecology::MIN_BENEFIT_SUPPORT {
            t.record_benefit(&hear, i % 2 == 0);
        }
        for i in 0..10 {
            t.record_benefit(&[false; N_SYMBOLS], i == 0);
        }
    }

    #[test]
    fn tracker_promotes_only_after_enough_consecutive_epochs() {
        use crate::ecology::{MIN_EPOCH_SUPPORT, PROMOTION_EPOCHS};
        let mut t = ConventionTracker::default();
        for e in 1..=PROMOTION_EPOCHS {
            stack_clean_epoch(&mut t, 1, StateClass::East, MIN_EPOCH_SUPPORT);
            assert!(t.candidate(1).is_some(), "epoch {e} phải là ứng viên");
            let newly = t.close_epoch();
            if e < PROMOTION_EPOCHS {
                assert!(newly.is_empty(), "epoch {e}: đề bạt quá sớm");
                assert_eq!(t.streak_len[1], e);
            } else {
                assert_eq!(newly.len(), 1, "epoch {e}: phải đề bạt đúng 1 quy ước");
                let p = &newly[0];
                assert_eq!(p.symbol, 1);
                assert_eq!(p.meaning(), Some(StateClass::East));
                assert_eq!(p.streak, PROMOTION_EPOCHS);
                assert_eq!(p.precision_hits, MIN_EPOCH_SUPPORT);
                assert_eq!(p.precision_total, MIN_EPOCH_SUPPORT);
            }
        }
        // Đóng epoch xoá bảng đếm để epoch sau đo độc lập.
        assert_eq!(t.epoch_index, u64::from(PROMOTION_EPOCHS));
        assert_eq!(t.epoch_vocab, Vocabulary::default());
        assert_eq!(t.benefit, BenefitCounters::default());
        assert_eq!(t.steps_in_epoch, 0);
    }

    #[test]
    fn changing_meaning_restarts_the_streak() {
        use crate::ecology::{MIN_EPOCH_SUPPORT, PROMOTION_EPOCHS};
        let mut t = ConventionTracker::default();
        for _ in 0..PROMOTION_EPOCHS - 1 {
            stack_clean_epoch(&mut t, 0, StateClass::North, MIN_EPOCH_SUPPORT);
            assert!(t.close_epoch().is_empty());
        }
        // Nghĩa đổi sang South ⇒ streak về 1, không đề bạt.
        stack_clean_epoch(&mut t, 0, StateClass::South, MIN_EPOCH_SUPPORT);
        assert!(t.close_epoch().is_empty(), "đổi nghĩa mà vẫn đề bạt");
        assert_eq!(t.streak_len[0], 1);
        assert_eq!(t.streak_meaning[0], Some(StateClass::South.col() as u8));
    }

    #[test]
    fn a_silent_epoch_breaks_the_streak_to_zero() {
        use crate::ecology::MIN_EPOCH_SUPPORT;
        let mut t = ConventionTracker::default();
        stack_clean_epoch(&mut t, 0, StateClass::North, MIN_EPOCH_SUPPORT);
        assert!(t.close_epoch().is_empty());
        assert_eq!(t.streak_len[0], 1);
        // Epoch không ai nói gì: ứng viên biến mất, streak về 0.
        assert!(t.close_epoch().is_empty());
        assert_eq!(t.streak_len[0], 0);
        assert_eq!(t.streak_meaning[0], None);
    }

    #[test]
    fn below_support_never_promotes_however_precise() {
        use crate::ecology::{MIN_EPOCH_SUPPORT, PROMOTION_EPOCHS};
        let mut t = ConventionTracker::default();
        for _ in 0..PROMOTION_EPOCHS + 2 {
            // Chính xác 100% nhưng chỉ (support − 1) lần phát.
            stack_clean_epoch(&mut t, 2, StateClass::West, MIN_EPOCH_SUPPORT - 1);
            assert!(t.candidate(2).is_none());
            assert!(t.close_epoch().is_empty());
        }
        assert!(t.promoted.is_empty());
    }

    #[test]
    fn imprecise_symbol_never_promotes() {
        use crate::ecology::PROMOTION_EPOCHS;
        let mut t = ConventionTracker::default();
        for _ in 0..PROMOTION_EPOCHS + 1 {
            // 10 lần North + 10 lần South: độ đỡ 20 ≥ 16 nhưng chính xác
            // chỉ 10/20 = 50% < 7/8.
            stack_clean_epoch(&mut t, 3, StateClass::North, 10);
            for _ in 0..10 {
                t.record_signal(SignalValue::Sym(3), StateClass::South);
            }
            assert!(t.candidate(3).is_none());
            assert!(t.close_epoch().is_empty());
        }
        assert!(t.promoted.is_empty());
    }

    #[test]
    fn promotion_is_idempotent_and_carries_its_evidence() {
        use crate::ecology::{MIN_EPOCH_SUPPORT, PROMOTION_EPOCHS};
        let mut t = ConventionTracker::default();
        let mut total_newly = 0;
        for _ in 0..PROMOTION_EPOCHS + 4 {
            stack_clean_epoch(&mut t, 1, StateClass::East, MIN_EPOCH_SUPPORT);
            total_newly += t.close_epoch().len();
        }
        assert_eq!(total_newly, 1, "cùng một quy ước chỉ được đề bạt một lần");
        assert_eq!(t.promoted.len(), 1);

        let p = &t.promoted[0];
        assert_eq!(p.symbol_concept_id(), "symbol_1");
        assert_eq!(p.concept_id(), "convention_sym1_state_res_east");
        let label = p.label();
        // Node phải tự mang bằng chứng: nghĩa, epoch, phân số chính xác,
        // và cả hai tỉ lệ ăn — đọc node là kiểm lại được vì sao nó ở đó.
        for needle in ["resource to the East", "precision", "16/16", "4/8", "1/10"] {
            assert!(label.contains(needle), "label thiếu {needle}: {label}");
        }
    }

    #[test]
    fn note_step_closes_the_epoch_exactly_on_the_boundary() {
        use crate::ecology::{EPOCH_STEPS, MIN_EPOCH_SUPPORT, PROMOTION_EPOCHS};
        let mut t = ConventionTracker::default();
        for epoch in 0..PROMOTION_EPOCHS {
            stack_clean_epoch(&mut t, 0, StateClass::North, MIN_EPOCH_SUPPORT);
            for step in 1..EPOCH_STEPS {
                assert!(t.note_step().is_empty(), "chưa tới biên mà đã đóng");
                assert_eq!(t.steps_in_epoch, step);
            }
            let newly = t.note_step(); // bước thứ EPOCH_STEPS
            assert_eq!(t.epoch_index, u64::from(epoch) + 1);
            assert_eq!(t.steps_in_epoch, 0);
            if epoch + 1 == PROMOTION_EPOCHS {
                assert_eq!(newly.len(), 1);
            } else {
                assert!(newly.is_empty());
            }
        }
    }
}
