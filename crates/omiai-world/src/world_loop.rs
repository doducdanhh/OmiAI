//! World loop: 5 phase cố định — ca_step, metabolism, agent_act,
//! reproduce_and_evolve, snapshot. Thứ tự cố định bảo đảm resume
//! deterministic; mỗi phase là hàm riêng test được độc lập.

use std::collections::BTreeSet;

use omiai_core::ltl::LtlFormula;
use rand::Rng;
use rand_chacha::{rand_core::SeedableRng, ChaCha8Rng};

use crate::agents;
use crate::atoms::Atom;
use crate::ecology::MUTATION_PROB;
use crate::registry::{FormulaRegistry, Genome};
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
        let mut world = Self {
            ca,
            registry,
            atoms: Vec::new(),
            rng,
            rng_seed: seed,
            rng_stream: 0,
            step_count: 0,
        };

        // Rải tài nguyên: giá trị 2 hoặc 3.
        let n_cells = config.width.saturating_mul(config.height);
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
                world.atoms.push(Atom {
                    pos: (x, y),
                    energy: 0.5,
                    gene: default_genome,
                    age: 0,
                });
                placed += 1;
            }
        }
        world
    }

    /// Một bước world: 5 phase theo thứ tự cố định.
    pub fn step(&mut self) {
        self.ca_step();
        self.metabolism();
        self.agent_act();
        self.reproduce_and_evolve();
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

    /// Phase 3: mỗi atom quan sát → decode genome → hành động, duyệt
    /// theo thứ tự Vec (deterministic). Ăn tài nguyên: ô ≥ 2 → cộng
    /// năng lượng, ô về 0.
    pub fn agent_act(&mut self) {
        let width = self.ca.width;
        let height = self.ca.height;
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
            let occ =
                |x: usize, y: usize| occupied.contains(&(x, y)) && (x, y) != pos;
            let obs =
                agents::observe_surroundings(pos, width, height, &cell, &occ);

            let action = agents::decide(&formula, &obs);
            let target = agents::target_of(&self.atoms[i], action);
            if target != pos && target.0 < width && target.1 < height {
                let ti = target.1 * width + target.0;
                let tv = self.ca.cells[ti];
                if tv == 0 || tv >= 2 {
                    let still_occupied =
                        self.atoms.iter().any(|a| a.pos == target);
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
            if let Some(child_energy) = atom.split_energy() {
                let in_bounds =
                    |x: usize, y: usize| x < self.ca.width && y < self.ca.height;
                let is_taken = |x: usize, y: usize| taken.contains(&(x, y));
                if let Some((sx, sy)) = crate::atoms::first_free_neighbor(
                    atom.pos,
                    &in_bounds,
                    &is_taken,
                ) {
                    // Chỉ sinh lên ô giá trị 0 (YAGNI: không sinh-ăn-always).
                    let cell_v = self.ca.cells[sy * self.ca.width + sx];
                    if cell_v == 0 {
                        let child_gene = if self.rng.r#gen::<f64>() < MUTATION_PROB
                        {
                            let mutated = match self.registry.get(atom.gene) {
                                Some(g) => mutate_formula(
                                    &g.formula,
                                    &mut self.rng,
                                ),
                                None => continue,
                            };
                            self.registry
                                .insert(Genome { formula: mutated, fitness: None })
                        } else {
                            atom.gene
                        };
                        taken.insert((sx, sy));
                        children.push(Atom {
                            pos: (sx, sy),
                            energy: child_energy,
                            gene: child_gene,
                            age: 0,
                        });
                    }
                }
            }
        }
        self.atoms.extend(children);
    }

    /// Phase 5: đóng băng bước.
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

/// Đột biến cấu trúc: chọn ngẫu nhiên một biến đổi an toàn. Các biến đổi:
/// đổi atom thành atom khác, đảo And↔Or / Until↔Release / X↔F↔G khi đi
/// xuống qua node đó. Không xoá cấu trúc — genome luôn còn đánh giá được.
pub fn mutate_formula(f: &LtlFormula, rng: &mut ChaCha8Rng) -> LtlFormula {
    const ATOM_NAMES: [&str; 4] = ["open", "wall", "res", "occupied"];
    match f {
        LtlFormula::Atom(_) => {
            let name = ATOM_NAMES[rng.gen_range(0..ATOM_NAMES.len())];
            LtlFormula::atom(name)
        }
        LtlFormula::Not(g) => {
            LtlFormula::Not(Box::new(mutate_formula(g, rng)))
        }
        LtlFormula::And(a, b) | LtlFormula::Or(a, b) => {
            let (a2, b2) = (mutate_formula(a, rng), mutate_formula(b, rng));
            if rng.r#gen::<bool>() {
                LtlFormula::Or(Box::new(a2), Box::new(b2))
            } else {
                LtlFormula::And(Box::new(a2), Box::new(b2))
            }
        }
        LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
            let inner = mutate_formula(g, rng);
            match rng.gen_range(0..3) {
                0 => LtlFormula::Next(Box::new(inner)),
                1 => LtlFormula::Eventually(Box::new(inner)),
                _ => LtlFormula::Globally(Box::new(inner)),
            }
        }
        LtlFormula::Until(p, q) | LtlFormula::Release(p, q) => {
            let p2 = mutate_formula(p, rng);
            let q2 = mutate_formula(q, rng);
            if rng.r#gen::<bool>() {
                LtlFormula::Until(Box::new(p2), Box::new(q2))
            } else {
                LtlFormula::Release(Box::new(p2), Box::new(q2))
            }
        }
        LtlFormula::True_ | LtlFormula::False_ => f.clone(), // leaf giữ nguyên
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecology::{ENERGY_MAX, METABOLIC_COST, REPRODUCE_THRESHOLD};
    use crate::registry::FormulaId;

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
        assert_eq!(w.registry.len(), 1); // genome mặc định dùng chung
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
        w.atoms.push(Atom { pos: (1, 0), energy: 0.5, gene, age: 0 });
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
        w.atoms.push(Atom { pos: (1, 0), energy: 0.5, gene, age: 0 });
        w.atoms.push(Atom { pos: (2, 0), energy: 0.5, gene, age: 0 });
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
        w.atoms
            .push(Atom { pos: (1, 1), energy: REPRODUCE_THRESHOLD, gene, age: 0 });
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
            w.atoms.push(Atom { pos, energy: 0.3, gene, age: 0 });
        }
        w.atoms
            .push(Atom { pos: (1, 1), energy: REPRODUCE_THRESHOLD, gene, age: 0 });
        w.reproduce_and_evolve();
        assert_eq!(w.atoms.len(), 4); // không ai sinh được
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
        assert!(w
            .atoms
            .iter()
            .all(|a| a.energy.is_finite() && (0.0..=ENERGY_MAX).contains(&a.energy)));
    }
}
