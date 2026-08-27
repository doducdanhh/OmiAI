//! World loop: 5 phase cố định — ca_step, metabolism, agent_act,
//! reproduce_and_evolve, snapshot. Thứ tự cố định bảo đảm resume
//! deterministic; mỗi phase là hàm riêng test được độc lập.

use std::collections::BTreeSet;

use omiai_core::ltl::LtlFormula;
use rand::Rng;
use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};

use crate::agents;
use crate::atoms::Atom;
use crate::communication::{self, Symbol, Vocabulary};
use crate::ecology::MUTATION_PROB;
use crate::registry::{FormulaId, FormulaRegistry, Genome};
use crate::substrate::CellularAutomaton;

/// Genome mặc định cho atom mồi: tìm tài nguyên hoặc ô trống.
fn default_genome_formula() -> LtlFormula {
    LtlFormula::or(LtlFormula::atom("res"), LtlFormula::atom("open"))
}

/// Cấu hình khởi tạo world.
#[derive(Debug, Clone)]
pub struct WorldConfig {
    pub width: usize,
    pub height: usize,
    pub n_initial_atoms: usize,
    /// Density ô tài nguyên lúc khởi tạo (giá trị 2 hoặc 3 ngẫu nhiên).
    pub initial_resources: f64,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            width: 32,
            height: 32,
            n_initial_atoms: 5,
            initial_resources: 0.06,
        }
    }
}

/// Thế giới: lưới CA + registry genome + các atom + RNG deterministic.
#[derive(Debug)]
pub struct World {
    pub ca: CellularAutomaton,
    pub registry: FormulaRegistry,
    pub atoms: Vec<Atom>,
    pub rng: ChaCha8Rng,
    /// Seed gốc (lưu checkpoint để tái tạo `rng`).
    pub rng_seed: u64,
    /// Stream số (mặc định 0, giữ cho tương lai; lưu checkpoint).
    pub rng_stream: u64,
    pub step_count: u64,
    /// Kênh tín hiệu của bước hiện tại, một ô lưới một phần tử: ký hiệu vừa
    /// được nói TẠI ô đó, `None` nếu không ai nói.
    ///
    /// Trạng thái PHÁI SINH: `speak` ghi một lần rồi đóng băng, mọi receiver
    /// đọc cùng một ảnh. KHÔNG lưu checkpoint — `load` khởi tạo toàn `None`
    /// và bước tiếp theo ghi lại đầy đủ trước khi ai đó đọc.
    pub airwave: Vec<Option<Symbol>>,
    /// Bảng đồng xuất hiện (ký hiệu × lớp trạng thái), tích luỹ toàn bộ
    /// vòng đời world. Lưu checkpoint (`communication/vocabulary.cbor`).
    pub vocabulary: Vocabulary,
}

impl World {
    /// Khởi tạo: lưới trống + rải tài nguyên + đặt atom mồi lên ô trống
    /// đầu tiên quét row-major. Toàn bộ randomness qua `self.rng`.
    pub fn new(config: WorldConfig, seed: u64) -> Self {
        let ca = CellularAutomaton::new(config.width, config.height, 4);
        let mut registry = FormulaRegistry::new();
        let default_genome = registry.insert(Genome {
            formula: default_genome_formula(),
            fitness: None,
        });

        let rng = ChaCha8Rng::seed_from_u64(seed);
        let n_cells = config.width.saturating_mul(config.height);
        let mut world = Self {
            ca,
            registry,
            atoms: Vec::new(),
            rng,
            rng_seed: seed,
            rng_stream: 0,
            step_count: 0,
            airwave: vec![None; n_cells],
            vocabulary: Vocabulary::default(),
        };

        // Rải tài nguyên: giá trị 2 hoặc 3.
        for i in 0..n_cells {
            if world.rng.r#gen::<f64>() < config.initial_resources {
                let rich = world.rng.r#gen::<bool>();
                world.ca.cells[i] = if rich { 3 } else { 2 };
            }
        }

        // Đặt atom mồi lên các ô trống đầu tiên (row-major).
        let occupied = occupied_set(&world.atoms);
        let mut placed = 0;
        for i in 0..n_cells {
            if placed >= config.n_initial_atoms {
                break;
            }
            let (x, y) = (i % config.width, i / config.width);
            if world.ca.cells[i] == 0 && !occupied.contains(&(x, y)) {
                // Thứ tự rút RNG là hợp đồng: rải tài nguyên xong mới tới
                // voice, một atom một lượt, theo thứ tự đặt.
                let voice = random_voice(&mut world.registry, &mut world.rng);
                world.atoms.push(Atom {
                    pos: (x, y),
                    energy: 0.5,
                    gene: default_genome,
                    age: 0,
                    voice,
                });
                placed += 1;
            }
        }
        world
    }

    /// Một bước world: 7 phase theo thứ tự cố định.
    ///
    /// Thứ tự: ca_step → metabolism → speak → agent_act → reproduce_and_evolve
    /// → team_reward → snapshot
    ///
    /// `speak` nằm SAU `metabolism` (atom chết trong bước này không nói) và
    /// TRƯỚC `agent_act` (tín hiệu ảnh hưởng ngay hành động cùng bước).
    /// `team_reward` chạy sau `reproduce_and_evolve` để phần thưởng tính trên
    /// dân số sau sinh sản.
    pub fn step(&mut self) {
        self.ca_step();
        self.metabolism();
        self.speak();
        self.agent_act();
        self.reproduce_and_evolve();
        self.team_reward();
        self.snapshot();
    }

    /// Phase 1: môi trường tiến hoá một bước Margolus.
    pub fn ca_step(&mut self) {
        self.ca.step();
    }

    /// Phase 2: trừ năng lượng, loại atom chết.
    pub fn metabolism(&mut self) {
        self.atoms.retain_mut(|atom| atom.metabolize());
    }

    /// Phase 3: mỗi atom còn sống quan sát vùng lân cận, giải mã voice gene
    /// thành một ký hiệu (hoặc im lặng), và ghi vào `airwave` tại ô của
    /// chính nó. Đồng thời ghi mẫu (ký hiệu, lớp trạng thái) vào
    /// `vocabulary` — **cùng một ảnh quan sát**, nên MI đo đúng cái sender
    /// thấy lúc nói (spec §5).
    ///
    /// KHÔNG rút RNG. Buffer cục bộ rồi gán một lần: ghi trực tiếp vào
    /// `self.airwave` sẽ cho atom sau nghe atom trước và làm ký hiệu phụ
    /// thuộc thứ tự `Vec`.
    pub fn speak(&mut self) {
        let (width, height) = (self.ca.width, self.ca.height);
        let mut airwave: Vec<Option<Symbol>> = vec![None; width * height];
        let occupied = occupied_set(&self.atoms);
        let cells = self.ca.cells.clone();

        for atom in &self.atoms {
            let cell = |x: usize, y: usize| cells[y * width + x];
            let occ =
                |x: usize, y: usize| occupied.contains(&(x, y)) && (x, y) != atom.pos;
            let obs =
                agents::observe_surroundings(atom.pos, width, height, &cell, &occ);
            let val = communication::neighbourhood_valuation(&obs);
            let signal = communication::decode_voice(&atom.voice, &self.registry, &val);
            // Pass self cell value for beacon detection
            let self_cell = cells[atom.pos.1 * width + atom.pos.0];
            let state = communication::state_class(&obs, self_cell);
            // Ghi MỌI atom sống, kể cả atom câm (hàng Silent) — nếu không,
            // `total` không còn là dân số và MI của thế giới câm thành NaN.
            self.vocabulary.record(signal, state);
            if let crate::communication::SignalValue::Sym(sym) = signal {
                let idx = atom.pos.1 * width + atom.pos.0;
                debug_assert!(airwave[idx].is_none(), "hai atom cùng ô");
                airwave[idx] = Some(sym);
            }
        }

        self.airwave = airwave;
    }

    /// Phase 4: mỗi atom quan sát → decode genome → hành động, duyệt
    /// theo thứ tự Vec (deterministic). Ăn tài nguyên: ô ≥ 2 → cộng
    /// năng lượng, ô về 0.
    pub fn agent_act(&mut self) {
        let width = self.ca.width;
        let height = self.ca.height;
        // airwave đã đóng băng ở phase speak; ảnh cục bộ để closure không
        // vay `self` trong lúc ta sửa `self.atoms`.
        let airwave = self.airwave.clone();
        for i in 0..self.atoms.len() {
            let (pos, gene) = {
                let atom = &self.atoms[i];
                (atom.pos, atom.gene)
            };
            let formula = match self.registry.get(gene) {
                Some(g) => g.formula.clone(),
                None => continue, // genome mất (không xảy ra ở slice này)
            };

            let occupied = occupied_set(&self.atoms);
            let cells = self.ca.cells.clone();
            let cell = |x: usize, y: usize| cells[y * width + x];
            let occ = |x: usize, y: usize| occupied.contains(&(x, y)) && (x, y) != pos;
            let heard = |x: usize, y: usize| airwave[y * width + x];
            let obs = agents::observe_surroundings_hearing(
                pos, width, height, &cell, &occ, &heard,
            );

            let action = agents::decide_with_hear(&formula, &obs);
            let target = agents::target_of(&self.atoms[i], action);
            if target != pos && target.0 < width && target.1 < height {
                let ti = target.1 * width + target.0;
                let tv = self.ca.cells[ti];
                if tv == 0 || tv >= 2 {
                    let still_occupied = self.atoms.iter().any(|a| a.pos == target);
                    if !still_occupied {
                        self.atoms[i].pos = target;
                        if tv >= 2 {
                            self.atoms[i].feed(tv);
                            self.ca.cells[ti] = 0;
                        }
                    }
                }
            }
        }
    }

    /// Phase 4: sinh sản qua ngưỡng + đột biến gene.
    pub fn reproduce_and_evolve(&mut self) {
        let mut children: Vec<Atom> = Vec::new();
        // `taken` = vị trí atom hiện hữu + vị trí con vừa đặt trong phase
        // này (tránh hai cha chọn cùng ô). Cập nhật ngay sau mỗi lần sinh.
        let mut taken = occupied_set(&self.atoms);
        for atom in self.atoms.iter_mut() {
            atom.age += 1;
            if !atom.can_split() {
                continue;
            }
            let in_bounds = |x: usize, y: usize| x < self.ca.width && y < self.ca.height;
            let is_taken = |x: usize, y: usize| taken.contains(&(x, y));
            let Some((sx, sy)) = crate::atoms::first_free_neighbor(atom.pos, &in_bounds, &is_taken)
            else {
                continue;
            };
            // Chỉ sinh lên ô giá trị 0 (YAGNI: không sinh-ăn-always).
            if self.ca.cells[sy * self.ca.width + sx] != 0 {
                continue;
            }
            let child_gene = if self.rng.r#gen::<f64>() < MUTATION_PROB {
                let mutated = match self.registry.get(atom.gene) {
                    Some(g) => mutate_formula(&g.formula, &mut self.rng),
                    None => continue, // genome mất (không xảy ra ở slice này)
                };
                self.registry.insert(Genome {
                    formula: mutated,
                    fitness: None,
                })
            } else {
                atom.gene
            };
            // Trừ năng lượng cha LÀ BƯỚC CUỐI: ô đặt con đã chắc chắn, nên
            // không có đường nào làm năng lượng biến mất mà chẳng sinh ai.
            let Some(child_energy) = atom.split_energy() else {
                continue; // can_split() ở trên đã đảm bảo Some
            };
            taken.insert((sx, sy));
            children.push(Atom {
                pos: (sx, sy),
                energy: child_energy,
                gene: child_gene,
                age: 0,
                voice: Vec::new(),
            });
        }
        self.atoms.extend(children);
    }

    /// Phase 6: Team reward — nếu vocabulary MI > threshold thì cộng năng lượng
    /// cho tất cả atom còn sống. Khuyến khích hội tụ ngôn ngữ chung.
    pub fn team_reward(&mut self) {
        use crate::ecology::{TEAM_MI_THRESHOLD, TEAM_REWARD_ENERGY};
        if self.vocabulary.mutual_information() >= TEAM_MI_THRESHOLD {
            for atom in &mut self.atoms {
                atom.energy = (atom.energy + TEAM_REWARD_ENERGY).min(crate::ecology::ENERGY_MAX);
            }
        }
    }

    /// Phase 7: đóng băng bước.
    pub fn snapshot(&mut self) {
        self.step_count += 1;
    }
}

/// Tập vị trí đang bị chiếm (BTreeSet — thứ tự cố định).
pub fn occupied_set(atoms: &[Atom]) -> BTreeSet<(usize, usize)> {
    atoms.iter().map(|a| a.pos).collect()
}

/// Độ sâu AST của formula.
fn depth(f: &LtlFormula) -> usize {
    match f {
        LtlFormula::True_ | LtlFormula::False_ | LtlFormula::Atom(_) => 1,
        LtlFormula::Not(g)
        | LtlFormula::Next(g)
        | LtlFormula::Eventually(g)
        | LtlFormula::Globally(g) => 1 + depth(g),
        LtlFormula::And(a, b)
        | LtlFormula::Or(a, b)
        | LtlFormula::Until(a, b)
        | LtlFormula::Release(a, b) => 1 + depth(a).max(depth(b)),
    }
}

/// Đột biến cấu trúc với pool tên cho trước. Các biến đổi: đổi atom thành
/// atom khác **trong pool**, đảo And↔Or / Until↔Release / X↔F↔G khi đi xuống
/// qua node đó. Không xoá cấu trúc — genome luôn còn đánh giá được, và độ
/// sâu không tăng (mỗi biến đổi giữ arity).
pub fn mutate_formula_with(
    f: &LtlFormula,
    rng: &mut ChaCha8Rng,
    names: &[&str],
) -> LtlFormula {
    debug_assert!(!names.is_empty(), "pool tên rỗng thì không đột biến được");
    match f {
        LtlFormula::Atom(_) => {
            let name = names[rng.gen_range(0..names.len())];
            LtlFormula::atom(name)
        }
        LtlFormula::Not(g) => LtlFormula::Not(Box::new(mutate_formula_with(g, rng, names))),
        LtlFormula::And(a, b) | LtlFormula::Or(a, b) => {
            let (a2, b2) = (
                mutate_formula_with(a, rng, names),
                mutate_formula_with(b, rng, names),
            );
            if rng.r#gen::<bool>() {
                LtlFormula::Or(Box::new(a2), Box::new(b2))
            } else {
                LtlFormula::And(Box::new(a2), Box::new(b2))
            }
        }
        LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
            let inner = mutate_formula_with(g, rng, names);
            match rng.gen_range(0..3) {
                0 => LtlFormula::Next(Box::new(inner)),
                1 => LtlFormula::Eventually(Box::new(inner)),
                _ => LtlFormula::Globally(Box::new(inner)),
            }
        }
        LtlFormula::Until(p, q) | LtlFormula::Release(p, q) => {
            let p2 = mutate_formula_with(p, rng, names);
            let q2 = mutate_formula_with(q, rng, names);
            if rng.r#gen::<bool>() {
                LtlFormula::Until(Box::new(p2), Box::new(q2))
            } else {
                LtlFormula::Release(Box::new(p2), Box::new(q2))
            }
        }
        LtlFormula::True_ | LtlFormula::False_ => f.clone(), // leaf giữ nguyên
    }
}

/// Đột biến gene DI CHUYỂN — wrapper giữ chữ ký slice 2, pool 8 tên.
pub fn mutate_formula(f: &LtlFormula, rng: &mut ChaCha8Rng) -> LtlFormula {
    mutate_formula_with(f, rng, &crate::agents::MOVEMENT_ATOM_NAMES)
}

/// Sinh voice ngẫu nhiên: đúng `N_SYMBOLS` arm, mỗi arm là một đột biến
/// độc lập của hạt giống trên pool voice, chèn vào registry.
///
/// Thứ tự rút RNG (arm 0 → arm K-1) là hợp đồng: đổi thứ tự là đổi mọi
/// quỹ đạo của mọi seed đã lưu.
pub fn random_voice(
    registry: &mut FormulaRegistry,
    rng: &mut ChaCha8Rng,
) -> Vec<FormulaId> {
    let seed = crate::communication::voice_seed_formula();
    (0..crate::communication::N_SYMBOLS)
        .map(|_| {
            let f = mutate_formula_with(
                &seed,
                rng,
                &crate::communication::VOICE_ATOM_NAMES,
            );
            registry.insert(Genome { formula: f, fitness: None })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecology::{ENERGY_MAX, METABOLIC_COST, REPRODUCE_THRESHOLD};
    use crate::communication::{voice_seed_formula, VOICE_ATOM_NAMES};
    use crate::registry::FormulaId;
    use crate::registry::FormulaRegistry;

    fn small_world(seed: u64) -> World {
        World::new(
            WorldConfig {
                width: 8,
                height: 8,
                n_initial_atoms: 2,
                initial_resources: 0.1,
            },
            seed,
        )
    }

    #[test]
    fn new_world_places_atoms_on_empty_cells() {
        let w = small_world(7);
        assert_eq!(w.atoms.len(), 2);
        // 1 genome mặc định + 2 atom * N_SYMBOLS arm voice
        assert_eq!(w.registry.len(), 1 + 2 * crate::communication::N_SYMBOLS);
        for a in &w.atoms {
            assert_eq!(w.ca.cells[a.pos.1 * 8 + a.pos.0], 0);
            assert_eq!(a.age, 0);
            assert!((a.energy - 0.5).abs() < 1e-12);
        }
    }

    #[test]
    fn new_world_is_deterministic_same_seed() {
        let a = small_world(42);
        let b = small_world(42);
        assert_eq!(a.ca.cells, b.ca.cells);
        assert_eq!(a.atoms, b.atoms);
        assert_ne!(small_world(42).rng_seed, small_world(43).rng_seed);
    }

    #[test]
    fn metabolism_removes_starved_atoms() {
        let mut w = small_world(1);
        w.atoms.clear();
        w.atoms.push(Atom {
            pos: (0, 0),
            energy: METABOLIC_COST - 0.001,
            gene: FormulaId::from_slot(0),
            age: 0,
            voice: Vec::new(),
        });
        w.metabolism();
        assert!(w.atoms.is_empty());
    }

    #[test]
    fn agent_act_eats_resource_and_clears_cell() {
        let mut w = World::new(
            WorldConfig {
                width: 4,
                height: 4,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            5,
        );
        // Atom ở (1,0), genome mặc định (res ∨ open); đặt tài nguyên bên E.
        let gene = FormulaId::from_slot(0);
        w.atoms.push(Atom {
            pos: (1, 0),
            energy: 0.5,
            gene,
            age: 0,
            voice: Vec::new(),
        });
        w.ca.cells[2] = 3; // (x=2, y=0) — East neighbor
        let before = w.atoms[0].energy;

        w.agent_act();

        assert_eq!(w.atoms[0].pos, (2, 0));
        assert_eq!(w.ca.cells[2], 0); // đã ăn
        assert!(w.atoms[0].energy > before);
        assert!(w.atoms[0].energy <= ENERGY_MAX);
    }

    #[test]
    fn agent_act_blocked_by_other_atom() {
        let mut w = World::new(
            WorldConfig {
                width: 4,
                height: 4,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            5,
        );
        let gene = FormulaId::from_slot(0);
        // Atom A (duyệt trước) ở (1,0); atom B ở (2,0) — East của A.
        w.atoms.push(Atom {
            pos: (1, 0),
            energy: 0.5,
            gene,
            age: 0,
            voice: Vec::new(),
        });
        w.atoms.push(Atom {
            pos: (2, 0),
            energy: 0.5,
            gene,
            age: 0,
            voice: Vec::new(),
        });
        // Lưới trống hoàn toàn: A muốn đi N (ưu tiên cao nhất trống).
        w.agent_act();
        // A không thể đứng yên nếu có hướng trống — kiểm tra A rời (1,0)
        // và không đè lên B.
        assert_ne!(w.atoms[0].pos, (2, 0));
    }

    #[test]
    fn reproduce_splits_at_threshold_when_space() {
        let mut w = World::new(
            WorldConfig {
                width: 4,
                height: 4,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            9,
        );
        let gene = FormulaId::from_slot(0);
        w.atoms.push(Atom {
            pos: (1, 1),
            energy: REPRODUCE_THRESHOLD,
            gene,
            age: 0,
            voice: Vec::new(),
        });
        let parent_before = w.atoms[0].energy;

        w.reproduce_and_evolve();

        assert_eq!(w.atoms.len(), 2);
        assert!(w.atoms[0].energy < parent_before);
        assert!((w.atoms[0].energy + w.atoms[1].energy - parent_before).abs() < 1e-12);
        // Con kế thừa gene cha HOẶC genome đột biến mới — cả hai đều phải
        // hợp lệ trong registry (MUTATION_PROB = 0.3 nên không khẳng định
        // cứng gene nào ở đây).
        assert!(w.registry.get(w.atoms[1].gene).is_some());
    }

    #[test]
    fn reproduce_no_space_no_child() {
        let mut w = World::new(
            WorldConfig {
                width: 2,
                height: 2,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            11,
        );
        let gene = FormulaId::from_slot(0);
        // Chiếm cả 4 ô → không còn ô kề trống.
        for pos in [(0, 0), (1, 0), (0, 1)] {
            w.atoms.push(Atom {
                pos,
                energy: 0.3,
                gene,
                age: 0,
                voice: Vec::new(),
            });
        }
        w.atoms.push(Atom {
            pos: (1, 1),
            energy: REPRODUCE_THRESHOLD,
            gene,
            age: 0,
            voice: Vec::new(),
        });
        w.reproduce_and_evolve();
        assert_eq!(w.atoms.len(), 4); // không ai sinh được
        // Sinh sản thất bại KHÔNG được ăn mất năng lượng của cha.
        assert_eq!(w.atoms[3].energy, REPRODUCE_THRESHOLD);
    }

    #[test]
    fn step_increments_counter_and_runs_phases() {
        let mut w = small_world(3);
        w.step();
        assert_eq!(w.step_count, 1);
        assert!(w.atoms.iter().all(|a| a.age == 1));
    }

    #[test]
    fn same_seed_same_trajectory() {
        let mut a = small_world(77);
        let mut b = small_world(77);
        for _ in 0..20 {
            a.step();
            b.step();
        }
        assert_eq!(a.ca.cells, b.ca.cells);
        assert_eq!(a.atoms, b.atoms);
        assert_eq!(a.step_count, b.step_count);
        assert_eq!(a.vocabulary, b.vocabulary);
        assert_eq!(a.airwave, b.airwave);
    }

    #[test]
    fn mutate_formula_bounded_depth_and_valid() {
        let mut rng = ChaCha8Rng::seed_from_u64(4);
        let base = LtlFormula::and(
            LtlFormula::atom("open"),
            LtlFormula::g(LtlFormula::atom("res")),
        );
        for _ in 0..50 {
            let m = mutate_formula(&base, &mut rng);
            // Đột biến chỉ hoán đổi node cùng arity nên depth không tăng.
            assert!(depth(&m) <= depth(&base));
        }
    }

    #[test]
    fn energy_stays_finite_and_bounded_over_run() {
        let mut w = small_world(21);
        for _ in 0..15 {
            w.step();
        }
        // Mọi energy phải finite và trong [0, ENERGY_MAX]; ăn/sinh sản chỉ
        // chuyển giao năng lượng giữa atom và lưới, metabolism chỉ trừ.
        assert!(
            w.atoms
                .iter()
                .all(|a| a.energy.is_finite() && (0.0..=ENERGY_MAX).contains(&a.energy))
        );
    }

    /// Thu mọi tên atom xuất hiện trong công thức.
    fn atom_names(f: &LtlFormula, out: &mut Vec<String>) {
        match f {
            LtlFormula::Atom(n) => out.push(n.clone()),
            LtlFormula::True_ | LtlFormula::False_ => {}
            LtlFormula::Not(g)
            | LtlFormula::Next(g)
            | LtlFormula::Eventually(g)
            | LtlFormula::Globally(g) => atom_names(g, out),
            LtlFormula::And(a, b)
            | LtlFormula::Or(a, b)
            | LtlFormula::Until(a, b)
            | LtlFormula::Release(a, b) => {
                atom_names(a, out);
                atom_names(b, out);
            }
        }
    }

    #[test]
    fn mutate_with_pool_only_emits_pool_names() {
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let seed = voice_seed_formula();
        for _ in 0..64 {
            let m = mutate_formula_with(&seed, &mut rng, &VOICE_ATOM_NAMES);
            let mut names = Vec::new();
            atom_names(&m, &mut names);
            assert!(!names.is_empty());
            for n in names {
                assert!(
                    VOICE_ATOM_NAMES.contains(&n.as_str()),
                    "đột biến voice rò tên ngoài pool: {n}"
                );
            }
        }
    }

    #[test]
    fn mutate_formula_still_uses_movement_pool() {
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let base = default_genome_formula();
        for _ in 0..64 {
            let m = mutate_formula(&base, &mut rng);
            let mut names = Vec::new();
            atom_names(&m, &mut names);
            for n in names {
                assert!(
                    crate::agents::MOVEMENT_ATOM_NAMES.contains(&n.as_str()),
                    "đột biến di chuyển rò tên ngoài pool: {n}"
                );
            }
        }
    }

    #[test]
    fn random_voice_has_full_arity_bounded_depth_and_is_deterministic() {
        let mut reg_a = FormulaRegistry::new();
        let mut rng_a = ChaCha8Rng::seed_from_u64(99);
        let voice_a = random_voice(&mut reg_a, &mut rng_a);

        assert_eq!(voice_a.len(), crate::communication::N_SYMBOLS);
        let seed_depth = depth(&voice_seed_formula());
        for id in &voice_a {
            let f = &reg_a.get(*id).expect("arm phải có trong registry").formula;
            assert!(depth(f) <= seed_depth, "đột biến không được làm sâu thêm");
        }

        // Cùng seed → cùng voice (bit-exact resume phụ thuộc điều này).
        let mut reg_b = FormulaRegistry::new();
        let mut rng_b = ChaCha8Rng::seed_from_u64(99);
        let voice_b = random_voice(&mut reg_b, &mut rng_b);
        assert_eq!(voice_a, voice_b);
        assert_eq!(reg_a.genomes_in_order(), reg_b.genomes_in_order());

        // Khác seed → gần như chắc chắn khác.
        let mut reg_c = FormulaRegistry::new();
        let mut rng_c = ChaCha8Rng::seed_from_u64(100);
        let _voice_c = random_voice(&mut reg_c, &mut rng_c);
        assert_ne!(reg_a.genomes_in_order(), reg_c.genomes_in_order());
    }

    #[test]
    fn random_voice_arms_are_decodable() {
        // Voice sinh ngẫu nhiên phải phát được ký hiệu trên valuation thật —
        // nếu pool sai, mọi arm false và dân số câm mà không lỗi nào nổi lên.
        let mut reg = FormulaRegistry::new();
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let mut fired = 0;
        for _ in 0..32 {
            let voice = random_voice(&mut reg, &mut rng);
            let obs = crate::agents::observe_surroundings(
                (1, 1),
                3,
                3,
                &|x, _y| if x == 2 { 2 } else { 0 },
                &|_x, _y| false,
            );
            let val = crate::communication::neighbourhood_valuation(&obs);
            if crate::communication::decode_voice(&voice, &reg, &val)
                != crate::communication::SignalValue::Silent
            {
                fired += 1;
            }
        }
        assert!(fired > 0, "32 voice ngẫu nhiên mà không ai phát được gì ⇒ pool sai");
    }

    /// Voice quy ước hoàn hảo: arm k bắn đúng khi tài nguyên ở hướng k.
    fn convention_voice(reg: &mut FormulaRegistry) -> Vec<FormulaId> {
        ["res_n", "res_e", "res_s", "res_w"]
            .iter()
            .map(|n| {
                reg.insert(Genome { formula: LtlFormula::atom(*n), fitness: None })
            })
            .collect()
    }

    fn empty_world(w: usize, h: usize, seed: u64) -> World {
        World::new(
            WorldConfig {
                width: w,
                height: h,
                n_initial_atoms: 0,
                initial_resources: 0.0,
            },
            seed,
        )
    }

    #[test]
    fn new_world_gives_every_seed_atom_a_full_voice() {
        let w = small_world(7);
        assert_eq!(w.atoms.len(), 2);
        for a in &w.atoms {
            assert!(a.voice_is_valid() && !a.is_mute());
            for id in &a.voice {
                assert!(w.registry.get(*id).is_some(), "arm phải nằm trong registry");
            }
        }
        // airwave đúng kích thước lưới và trống khi chưa ai nói.
        assert_eq!(w.airwave.len(), 8 * 8);
        assert!(w.airwave.iter().all(|c| c.is_none()));
        assert_eq!(w.vocabulary, Vocabulary::default());
    }

    #[test]
    fn speak_writes_airwave_at_speaker_cell_only() {
        let mut w = empty_world(4, 4, 5);
        let voice = convention_voice(&mut w.registry);
        w.atoms.push(Atom {
            pos: (1, 1),
            energy: 0.5,
            gene: FormulaId::from_slot(0),
            age: 0,
            voice,
        });
        w.ca.cells[6] = 2; // tài nguyên phía Đông của (1,1) = (2,1)

        w.speak();

        assert_eq!(w.airwave[5], Some(1), "phải nói ký hiệu 1 = Đông");
        assert_eq!(
            w.airwave.iter().filter(|c| c.is_some()).count(),
            1,
            "chỉ ô của người nói mới có tiếng"
        );
        assert_eq!(w.vocabulary.total, 1);
        assert_eq!(
            w.vocabulary.joint[crate::communication::SignalValue::Sym(1).row()][crate::communication::StateClass::East.col()],
            1
        );
    }

    #[test]
    fn speak_records_mute_atoms_as_silence() {
        let mut w = empty_world(4, 4, 5);
        w.atoms.push(Atom {
            pos: (1, 1),
            energy: 0.5,
            gene: FormulaId::from_slot(0),
            age: 0,
            voice: Vec::new(),
        });
        w.ca.cells[6] = 2; // (1, 1) East neighbor

        w.speak();

        assert!(w.airwave.iter().all(|c| c.is_none()));
        assert_eq!(w.vocabulary.total, 1, "atom câm vẫn được đếm");
        assert_eq!(
            w.vocabulary.joint[crate::communication::SignalValue::Silent.row()][crate::communication::StateClass::East.col()],
            1
        );
        assert_eq!(w.vocabulary.mutual_information(), 0.0);
    }

    #[test]
    fn speak_consumes_no_rng() {
        // Hợp đồng bit-exact resume: word_pos không được nhúc nhích.
        let mut w = small_world(13);
        let before = w.rng.get_word_pos();
        w.speak();
        assert_eq!(w.rng.get_word_pos(), before, "speak rút RNG là mất bit-exact");
    }

    #[test]
    fn speak_is_order_independent_within_a_step() {
        // Đảo thứ tự Vec atom không được đổi airwave lẫn vocabulary. Nếu
        // speak ghi trực tiếp vào self.airwave, atom sau sẽ nghe atom trước
        // và test này vỡ.
        let mut a = empty_world(5, 5, 17);
        let voice_a = convention_voice(&mut a.registry);
        let mut b = empty_world(5, 5, 17);
        let voice_b = convention_voice(&mut b.registry);
        let mk = |pos, voice: &Vec<FormulaId>| Atom {
            pos,
            energy: 0.5,
            gene: FormulaId::from_slot(0),
            age: 0,
            voice: voice.clone(),
        };
        a.atoms = vec![mk((1, 1), &voice_a), mk((2, 1), &voice_a), mk((3, 1), &voice_a)];
        b.atoms = vec![mk((3, 1), &voice_b), mk((1, 1), &voice_b), mk((2, 1), &voice_b)];
        for w in [&mut a, &mut b] {
            w.ca.cells[9] = 3; // (1, 1) North neighbor in 5x5 grid
        }

        a.speak();
        b.speak();

        assert_eq!(a.airwave, b.airwave);
        assert_eq!(a.vocabulary, b.vocabulary);
    }

    #[test]
    fn step_runs_six_phases_and_speaks() {
        let mut w = small_world(3);
        w.step();
        assert_eq!(w.step_count, 1);
        assert!(w.atoms.iter().all(|a| a.age == 1));
        // speak đã chạy: mọi atom sống lúc speak được ghi.
        assert!(w.vocabulary.total >= 1);
    }

    #[test]
    fn vocabulary_total_accumulates_across_steps() {
        let mut w = small_world(23);
        let mut expected = 0u64;
        for _ in 0..10 {
            w.ca_step();
            w.metabolism();
            expected += w.atoms.len() as u64; // dân số ĐÚNG lúc speak
            w.speak();
            w.agent_act();
            w.reproduce_and_evolve();
            w.team_reward();
            w.snapshot();
        }
        assert_eq!(w.vocabulary.total, expected);
    }

    #[test]
    fn agent_act_reacts_to_heard_signal() {
        // Atom nghe được ký hiệu 2 thì đi; không nghe thì đứng yên. Lưới
        // trống nên khác biệt duy nhất là airwave.
        let mut w = empty_world(5, 5, 31);
        let gene = w.registry.insert(Genome {
            formula: LtlFormula::and(
                LtlFormula::atom("open"),
                LtlFormula::atom("hear2"),
            ),
            fitness: None,
        });
        w.atoms.push(Atom {
            pos: (2, 2),
            energy: 0.5,
            gene,
            age: 0,
            voice: Vec::new(),
        });

        w.airwave = vec![None; 25];
        w.agent_act();
        assert_eq!(w.atoms[0].pos, (2, 2), "không nghe gì thì đứng yên");

        w.airwave = vec![None; 25];
        w.airwave[2 * 5 + 1] = Some(2); // ô kề phía Tây có tiếng
        w.agent_act();
        assert_ne!(w.atoms[0].pos, (2, 2), "nghe được thì phải hành động");
    }
}
