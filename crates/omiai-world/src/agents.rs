//! Agent: atom hành động theo gene LTL của chính nó.
//!
//! Decode policy thuần tuý, không cần World — quan sát một hướng được mã
//! hoá thành các mệnh đề `open/wall/res/occupied`; genome được đánh giá
//! propositional trên từng hướng, hướng đầu tiên thoả (theo thứ tự cố định
//! N,E,S,W) được chọn. Toán tử thời gian của LTL được coi như phần hiện tại
//! (giới hạn đã biết, ghi rõ ở [`eval_current`]).

use std::collections::BTreeMap;

use omiai_core::ltl::LtlFormula;

use crate::atoms::Atom;

/// Hướng trên lưới — thứ tự khai báo cũng là thứ tự ưu tiên khi quyết định.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

pub const ALL_DIRECTIONS: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];

impl Direction {
    /// Delta (dx, dy) của hướng.
    pub fn delta(self) -> (isize, isize) {
        match self {
            Direction::North => (0, -1),
            Direction::East => (1, 0),
            Direction::South => (0, 1),
            Direction::West => (-1, 0),
        }
    }
}

/// Hành động của một agent trong một bước.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Stay,
    Move(Direction),
}

/// Quan sát MỘT ô kề: giá trị ô + có atom khác đang đứng hay không.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Observation {
    pub open: bool,
    pub wall: bool,
    pub res: bool,
    pub occupied: bool,
    /// Giá trị tín hiệu mà atom **nhận** ở ô này (airwave của neighbor).
    /// `None` nếu không có tín hiệu — nguyên tắc spec §3.1: không báo động
    /// nếu airwave rỗng, để valuation không tự "biết" mình đang nghe.
    pub heard: Option<crate::communication::Symbol>,
}

/// Mã hoá giá trị ô lưới (0 trống, 1 cản, ≥2 tài nguyên) + tình trạng chiếm.
pub fn observe(cell_value: u8, occupied: bool) -> Observation {
    Observation {
        open: cell_value == 0 && !occupied,
        wall: cell_value == 1,
        res: cell_value >= 2,
        occupied,
        heard: None,
    }
}

/// Pool tên mệnh đề của gene DI CHUYỂN + tiếng nói. Phiên bản 8 tên cho
/// slice 3: 4 không hướng (agent_act) + 4 `hearK` (receiver thính ứng
/// tín hiệu của neighbor phát cùng bước). Thứ tự: movement 4 + hearK.
pub const MOVEMENT_ATOM_NAMES: [&str; 8] = [
    "open", "wall", "res", "occupied", "hear0", "hear1", "hear2", "hear3",
];

/// Valuation propositional của một quan sát (BTreeMap — thứ tự cố định).
pub fn valuation(obs: &Observation) -> BTreeMap<String, bool> {
    [
        ("open".to_string(), obs.open),
        ("wall".to_string(), obs.wall),
        ("res".to_string(), obs.res),
        ("occupied".to_string(), obs.occupied),
    ]
    .into_iter()
    .collect()
}

/// Mã hoá giá trị ô lưới + tình trạng chiếm + tín hiệu nhận được.
/// `observe` (2 tham số) giữ nguyên `heard = None` để slice 2 không đổi.
pub fn observe_with(cell_value: u8, occupied: bool, heard: Option<crate::communication::Symbol>) -> Observation {
    let mut o = observe(cell_value, occupied);
    o.heard = heard;
    o
}

/// Nhóm nghe của một hướng: có kênh `hearK` không tên hướng.
fn heard_bit(sym: u8) -> &'static str {
    ["hear0", "hear1", "hear2", "hear3"][sym as usize % crate::communication::N_SYMBOLS]
}

/// Bật cờ `hearK` cho từng ký hiệu đang được nghe trên 4 kênh.
pub fn hear_flags(obs_by_dir: &[(Direction, Observation)]) -> [bool; crate::communication::N_SYMBOLS] {
    let mut flags = [false; crate::communication::N_SYMBOLS];
    for (_, obs) in obs_by_dir {
        if let Some(sym) = obs.heard {
            flags[sym as usize % crate::communication::N_SYMBOLS] = true;
        }
    }
    flags
}

/// Valuation 8 tên cho policy di chuyển + tiếng nói: 4 không hướng từ
/// Observation + 4 `hearK` từ cờ nghe.
pub fn valuation_with_hear(
    obs: &Observation,
    hear: &[bool; crate::communication::N_SYMBOLS],
) -> BTreeMap<String, bool> {
    let mut v = valuation(obs);
    for (k, &set) in hear.iter().enumerate() {
        v.insert(heard_bit(k as u8).to_string(), set);
    }
    v
}

/// Chọn hành động với thông tin nghe được — cập nhật cờ hear một lần rồi
/// đánh giá trên valuation mở rộng. Hướng đầu tiên passable + thoả sẽ được chọn.
pub fn decide_with_hear(
    formula: &LtlFormula,
    obs_by_dir: &[(Direction, Observation)],
) -> Action {
    let hear = hear_flags(obs_by_dir);
    for (dir, obs) in obs_by_dir {
        let passable = !obs.wall && !obs.occupied;
        if passable && eval_current(formula, &valuation_with_hear(obs, &hear)) {
            return Action::Move(*dir);
        }
    }
    Action::Stay
}

/// Quan sát 4 hướng có nhận thức biết. `heard(x,y)` trả về tín hiệu mà
/// neighbor tại (x,y) đang phát (airwave của neighbor).
pub fn observe_surroundings_hearing(
    pos: (usize, usize),
    width: usize,
    height: usize,
    cell: &dyn Fn(usize, usize) -> u8,
    occupied: &dyn Fn(usize, usize) -> bool,
    heard: &dyn Fn(usize, usize) -> Option<crate::communication::Symbol>,
) -> Vec<(Direction, Observation)> {
    ALL_DIRECTIONS
        .iter()
        .map(|&dir| {
            let (dx, dy) = dir.delta();
            let (x, y) = (pos.0 as isize + dx, pos.1 as isize + dy);
            if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
                (dir, observe_with(1, false, None))
            } else {
                let (x, y) = (x as usize, y as usize);
                (dir, observe_with(cell(x, y), occupied(x, y), heard(x, y)))
            }
        })
        .collect()
}

/// Đánh giá propositional của LtlFormula trên trạng thái hiện tại.
///
/// Ngữ nghĩa toán tử thời gian ở đây (giới hạn đã biết của policy decode):
/// - `X φ`, `F φ`, `G φ` → đánh giá φ ngay bây giờ.
/// - `φ U ψ` → ψ ngay bây giờ; `φ R ψ` → φ ∧ ψ ngay bây giờ.
/// - Atom không có trong valuation → false.
pub fn eval_current(f: &LtlFormula, val: &BTreeMap<String, bool>) -> bool {
    match f {
        LtlFormula::True_ => true,
        LtlFormula::False_ => false,
        LtlFormula::Atom(name) => val.get(name).copied().unwrap_or(false),
        LtlFormula::Not(g) => !eval_current(g, val),
        LtlFormula::And(a, b) => eval_current(a, val) && eval_current(b, val),
        LtlFormula::Or(a, b) => eval_current(a, val) || eval_current(b, val),
        LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
            eval_current(g, val)
        }
        LtlFormula::Until(_, q) => eval_current(q, val),
        LtlFormula::Release(p, q) => eval_current(p, val) && eval_current(q, val),
    }
}

/// Chọn hành động: hướng đầu tiên (theo thứ tự trong `obs_by_dir`) mà
/// genome thoả VÀ ô đi được (không cản, không bị chiếm).
///
/// `obs_by_dir` thường là `ALL_DIRECTIONS` zipped với quan sát — thứ tự
/// đưa vào chính là thứ tự ưu tiên.
pub fn decide(formula: &LtlFormula, obs_by_dir: &[(Direction, Observation)]) -> Action {
    for (dir, obs) in obs_by_dir {
        let passable = !obs.wall && !obs.occupied;
        if passable && eval_current(formula, &valuation(obs)) {
            return Action::Move(*dir);
        }
    }
    Action::Stay
}

/// Quan sát 4 hướng quanh một atom trên lưới.
///
/// `cell(x, y)` đọc giá trị ô (ngoài biên coi là cản); `occupied(x, y)` tra
/// vị trí atom khác. Trả về cặp (Direction, Observation) theo ALL_DIRECTIONS.
pub fn observe_surroundings(
    pos: (usize, usize),
    width: usize,
    height: usize,
    cell: &dyn Fn(usize, usize) -> u8,
    occupied: &dyn Fn(usize, usize) -> bool,
) -> Vec<(Direction, Observation)> {
    ALL_DIRECTIONS
        .iter()
        .map(|&dir| {
            let (dx, dy) = dir.delta();
            let (x, y) = (pos.0 as isize + dx, pos.1 as isize + dy);
            if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
                (dir, observe(1, false)) // ngoài biên = cản
            } else {
                let (x, y) = (x as usize, y as usize);
                (dir, observe(cell(x, y), occupied(x, y)))
            }
        })
        .collect()
}

/// Vị trí đích nếu action là di chuyển (không tự kiểm tra tính hợp lệ —
/// caller trong world_loop kiểm tra lần cuối trước khi áp dụng).
pub fn target_of(atom: &Atom, action: Action) -> (usize, usize) {
    match action {
        Action::Stay => atom.pos,
        Action::Move(dir) => {
            let (dx, dy) = dir.delta();
            (
                (atom.pos.0 as isize + dx) as usize,
                (atom.pos.1 as isize + dy) as usize,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- observe / valuation ---

    #[test]
    fn observe_maps_cell_values() {
        assert!(observe(0, false).open);
        assert!(observe(1, false).wall);
        assert!(observe(2, false).res);
        assert!(observe(3, false).res);
        assert!(observe(0, true).occupied);
        assert!(!observe(0, true).open);
    }

    // --- eval_current ---

    #[test]
    fn eval_atom_lookup_and_default_false() {
        let mut val = valuation(&observe(2, false));
        assert!(eval_current(&LtlFormula::atom("res"), &val));
        assert!(!eval_current(&LtlFormula::atom("wall"), &val));
        // Atom lạ → false.
        assert!(!eval_current(&LtlFormula::atom("nonexistent"), &val));
        val.insert("extra".to_string(), true);
        assert!(eval_current(&LtlFormula::atom("extra"), &val));
    }

    #[test]
    fn eval_boolean_connectives() {
        let val = valuation(&observe(0, false)); // chỉ open=true
        let open = LtlFormula::atom("open");
        let wall = LtlFormula::atom("wall");
        assert!(eval_current(
            &LtlFormula::and(open.clone(), LtlFormula::True_),
            &val
        ));
        assert!(eval_current(
            &LtlFormula::or(open.clone(), wall.clone()),
            &val
        ));
        assert!(!eval_current(
            &LtlFormula::and(open.clone(), wall.clone()),
            &val
        ));
        assert!(eval_current(&LtlFormula::neg(wall.clone()), &val));
        // implies(p,q) = ¬p ∨ q — không có constructor riêng trong LtlFormula.
        let implies = LtlFormula::or(LtlFormula::neg(open), LtlFormula::True_);
        assert!(eval_current(&implies, &val));
    }

    #[test]
    fn eval_temporal_operators_fall_back_to_present() {
        let val = valuation(&observe(2, false)); // res=true
        let res = LtlFormula::atom("res");
        assert!(eval_current(&LtlFormula::f(res.clone()), &val));
        assert!(eval_current(&LtlFormula::g(res.clone()), &val));
        assert!(eval_current(&LtlFormula::x(res.clone()), &val));
        assert!(eval_current(
            &LtlFormula::until(LtlFormula::False_, res.clone()),
            &val
        ));
        assert!(eval_current(
            &LtlFormula::release(res.clone(), res.clone()),
            &val
        ));
    }

    // --- decide ---

    fn obs_open() -> Observation {
        observe(0, false)
    }
    fn obs_res() -> Observation {
        observe(3, false)
    }
    fn obs_wall() -> Observation {
        observe(1, false)
    }

    #[test]
    fn genome_seeking_resource_moves_to_first_resource() {
        // Genome "muốn tài nguyên": res ∨ open
        let genome = LtlFormula::or(LtlFormula::atom("res"), LtlFormula::atom("open"));
        // N=open, E=res → N thoả trước (thứ tự ưu tiên).
        let obs = vec![
            (Direction::North, obs_open()),
            (Direction::East, obs_res()),
            (Direction::South, obs_wall()),
            (Direction::West, obs_wall()),
        ];
        assert_eq!(decide(&genome, &obs), Action::Move(Direction::North));

        // N=cản → E=res được chọn.
        let obs2 = vec![(Direction::North, obs_wall()), (Direction::East, obs_res())];
        assert_eq!(decide(&genome, &obs2), Action::Move(Direction::East));
    }

    #[test]
    fn blocked_cells_are_skipped_even_if_formula_matches() {
        // Genome khớp cả wall lẫn open: N thoả formula nhưng là ô cản
        // (không đi được) → bị bỏ qua; E=open được chọn.
        let genome = LtlFormula::or(LtlFormula::atom("wall"), LtlFormula::atom("open"));
        let obs = vec![
            (Direction::North, obs_wall()),
            (Direction::East, obs_open()),
        ];
        assert_eq!(decide(&genome, &obs), Action::Move(Direction::East));

        // Genome chỉ match ô cản → mọi hướng bị chặn/bỏ qua → Stay.
        let wall_only = LtlFormula::atom("wall");
        assert_eq!(
            decide(&wall_only, &[(Direction::North, obs_open())]),
            Action::Stay
        );
    }

    #[test]
    fn occupied_cells_are_skipped() {
        let genome = LtlFormula::atom("open");
        let obs = vec![
            (Direction::North, observe(0, true)), // trống nhưng bị chiếm
            (Direction::East, observe(0, false)),
        ];
        assert_eq!(decide(&genome, &obs), Action::Move(Direction::East));
    }

    #[test]
    fn no_match_means_stay() {
        let genome = LtlFormula::atom("res");
        let obs = vec![
            (Direction::North, obs_open()),
            (Direction::East, obs_wall()),
        ];
        assert_eq!(decide(&genome, &obs), Action::Stay);
    }

    // --- observe_surroundings ---

    #[test]
    fn surroundings_read_grid_and_bounds_as_wall() {
        // Lưới 3x3, tất cả trống; atom ở (0,0): W và N ngoài biên = cản.
        let cell = |_: usize, _: usize| 0u8;
        let occ = |_: usize, _: usize| false;
        let obs = observe_surroundings((0, 0), 3, 3, &cell, &occ);
        assert_eq!(obs.len(), 4);
        assert!(obs[0].1.wall); // North ngoài biên
        assert!(obs[1].1.open); // East
        assert!(obs[2].1.open); // South
        assert!(obs[3].1.wall); // West ngoài biên
    }

    // --- target_of ---

    #[test]
    fn target_of_move_applies_delta() {
        let mut a = Atom {
            pos: (2, 2),
            energy: 0.5,
            gene: crate::registry::FormulaId::from_slot(0),
            age: 3,
            voice: Vec::new(),
        };
        assert_eq!(target_of(&a, Action::Move(Direction::North)), (2, 1));
        assert_eq!(target_of(&a, Action::Stay), (2, 2));
        a.pos = (0, 0);
        // Wrap usize khi lùi biên — caller phải kiểm tra biên trước.
        assert_eq!(
            target_of(&a, Action::Move(Direction::West)),
            (usize::MAX, 0)
        );
    }

    use crate::communication::N_SYMBOLS;

    #[test]
    fn movement_pool_grows_with_symbol_count() {
        assert_eq!(MOVEMENT_ATOM_NAMES.len(), 4 + N_SYMBOLS);
        for k in 0..N_SYMBOLS {
            assert!(MOVEMENT_ATOM_NAMES.contains(&format!("hear{k}").as_str()));
        }
    }

    #[test]
    fn hear_flags_aggregate_over_all_directions() {
        let obs = vec![
            (Direction::North, observe_with(0, false, Some(2))),
            (Direction::East, observe_with(0, false, None)),
            (Direction::South, observe_with(0, false, Some(0))),
            (Direction::West, observe_with(0, false, Some(2))),
        ];
        let flags = hear_flags(&obs);
        assert_eq!(flags, [true, false, true, false]);
    }

    #[test]
    fn valuation_with_hear_merges_cell_and_hear_names() {
        let obs = observe_with(2, false, Some(1));
        let val = valuation_with_hear(&obs, &[false, true, false, false]);
        assert_eq!(val.len(), 4 + N_SYMBOLS);
        assert!(val["res"]);
        assert!(val["hear1"]);
        assert!(!val["hear0"]);
        let val2 = valuation_with_hear(&obs, &[false; N_SYMBOLS]);
        assert!(!val2["hear1"]);
    }

    #[test]
    fn decide_with_hear_follows_the_signal() {
        let formula =
            LtlFormula::and(LtlFormula::atom("open"), LtlFormula::atom("hear2"));
        let silent: Vec<_> = ALL_DIRECTIONS
            .iter()
            .map(|&d| (d, observe_with(0, false, None)))
            .collect();
        assert_eq!(decide_with_hear(&formula, &silent), Action::Stay);

        let mut heard = silent.clone();
        heard[3] = (Direction::West, observe_with(0, false, Some(2)));
        assert_eq!(decide_with_hear(&formula, &heard), Action::Move(Direction::North));
    }

    #[test]
    fn decide_ignores_hear_names_and_stays_slice2_behaviour() {
        let formula = LtlFormula::atom("hear0");
        let obs: Vec<_> = ALL_DIRECTIONS
            .iter()
            .map(|&d| (d, observe_with(0, false, Some(0))))
            .collect();
        assert_eq!(decide(&formula, &obs), Action::Stay);
        assert_eq!(decide_with_hear(&formula, &obs), Action::Move(Direction::North));
    }

    #[test]
    fn observe_surroundings_hearing_reads_neighbour_cells_only() {
        let obs = observe_surroundings_hearing(
            (1, 1),
            3,
            3,
            &|_x, _y| 0,
            &|_x, _y| false,
            &|x, y| if (x, y) == (1, 1) { Some(3) } else { None },
        );
        assert!(obs.iter().all(|(_, o)| o.heard.is_none()), "atom tự nghe mình");
        assert_eq!(hear_flags(&obs), [false; N_SYMBOLS]);

        let obs = observe_surroundings_hearing(
            (1, 1),
            3,
            3,
            &|_x, _y| 0,
            &|_x, _y| false,
            &|x, y| if (x, y) == (2, 1) { Some(3) } else { None },
        );
        assert_eq!(obs[1].1.heard, Some(3));
        assert_eq!(hear_flags(&obs), [false, false, false, true]);
    }

    #[test]
    fn out_of_bounds_neighbour_hears_nothing() {
        let obs = observe_surroundings_hearing(
            (0, 0),
            3,
            3,
            &|_x, _y| 0,
            &|_x, _y| false,
            &|_x, _y| Some(1),
        );
        assert!(obs[0].1.wall && obs[0].1.heard.is_none());
        assert_eq!(obs[1].1.heard, Some(1));
    }
}
