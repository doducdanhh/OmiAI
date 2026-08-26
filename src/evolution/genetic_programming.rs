//! Cartesian Genetic Programming (CGP) with an Asynchronous Island Model.
//!
//! Each node computes a function of earlier nodes / inputs. Islands evolve
//! in parallel (`rayon`) and periodically migrate elite individuals.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;

use super::fitness::Fitness;
use super::mutation;
use super::selection::{self, SelectionStrategy};

/// Built-in CGP function set (id → arity).
pub const FN_ADD: usize = 0;
pub const FN_SUB: usize = 1;
pub const FN_MUL: usize = 2;
pub const FN_DIV: usize = 3;
pub const FN_SIN: usize = 4;
pub const FN_NEG: usize = 5;
pub const NUM_FUNCTIONS: usize = 6;

fn arity(fn_id: usize) -> usize {
    match fn_id % NUM_FUNCTIONS {
        FN_SIN | FN_NEG => 1,
        _ => 2,
    }
}

/// A CGP node: function id + connection indices (into inputs+prior nodes).
#[derive(Debug, Clone)]
pub struct CgpNode {
    pub function_id: usize,
    pub inputs: Vec<usize>,
}

/// A CGP genotype / phenotype program.
#[derive(Debug, Clone)]
pub struct GeneticProgram {
    pub n_inputs: usize,
    pub nodes: Vec<CgpNode>,
    pub outputs: Vec<usize>,
}

impl GeneticProgram {
    /// Random program with `n_nodes` computational nodes.
    pub fn random(n_inputs: usize, n_nodes: usize, n_outputs: usize, rng: &mut impl Rng) -> Self {
        let mut nodes = Vec::with_capacity(n_nodes);
        for i in 0..n_nodes {
            let max_src = n_inputs + i; // can connect to inputs and prior nodes
            let fid = rng.gen_range(0..NUM_FUNCTIONS);
            let a = arity(fid);
            let inputs: Vec<usize> = (0..a)
                .map(|_| {
                    if max_src == 0 {
                        0
                    } else {
                        rng.gen_range(0..max_src)
                    }
                })
                .collect();
            nodes.push(CgpNode {
                function_id: fid,
                inputs,
            });
        }
        let total = n_inputs + n_nodes;
        let outputs = (0..n_outputs)
            .map(|_| {
                if total == 0 {
                    0
                } else {
                    rng.gen_range(0..total)
                }
            })
            .collect();
        Self {
            n_inputs,
            nodes,
            outputs,
        }
    }

    /// Evaluate the program on an input vector.
    pub fn eval(&self, inputs: &[f64]) -> Vec<f64> {
        let mut values = vec![0.0; self.n_inputs + self.nodes.len()];
        for i in 0..self.n_inputs {
            values[i] = inputs.get(i).copied().unwrap_or(0.0);
        }
        for (i, node) in self.nodes.iter().enumerate() {
            let idx = self.n_inputs + i;
            let a = node
                .inputs
                .first()
                .map(|&j| values.get(j).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let b = node
                .inputs
                .get(1)
                .map(|&j| values.get(j).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            values[idx] = apply_fn(node.function_id, a, b);
        }
        self.outputs
            .iter()
            .map(|&o| values.get(o).copied().unwrap_or(0.0))
            .collect()
    }

    /// Point-mutate function ids and connections.
    pub fn mutate(&mut self, rate: f64, rng: &mut impl Rng) {
        for (i, node) in self.nodes.iter_mut().enumerate() {
            if rng.r#gen::<f64>() < rate {
                node.function_id = rng.gen_range(0..NUM_FUNCTIONS);
                let a = arity(node.function_id);
                let max_src = self.n_inputs + i;
                node.inputs = (0..a)
                    .map(|_| {
                        if max_src == 0 {
                            0
                        } else {
                            rng.gen_range(0..max_src.max(1))
                        }
                    })
                    .collect();
            } else {
                let max_src = self.n_inputs + i;
                for inp in node.inputs.iter_mut() {
                    if rng.r#gen::<f64>() < rate && max_src > 0 {
                        *inp = rng.gen_range(0..max_src);
                    }
                }
            }
        }
        let total = self.n_inputs + self.nodes.len();
        mutation::mutate_int(&mut self.outputs, rate, 0, total.saturating_sub(1), rng);
    }

    /// Island-model evolutionary search.
    ///
    /// Evolves `islands` sub-populations of size `population_size / islands`
    /// for `generations`, migrating elites every 5 generations.
    pub fn evolve<F>(
        population_size: usize,
        islands: usize,
        generations: usize,
        n_inputs: usize,
        n_nodes: usize,
        n_outputs: usize,
        fitness_fn: F,
        seed: u64,
    ) -> GeneticProgram
    where
        F: Fn(&GeneticProgram) -> Fitness + Sync,
    {
        let islands = islands.max(1);
        let pop_per = (population_size / islands).max(4);
        let mut rng = ChaCha8Rng::seed_from_u64(seed);

        // Init islands
        let mut island_pops: Vec<Vec<GeneticProgram>> = (0..islands)
            .map(|i| {
                let mut r = ChaCha8Rng::seed_from_u64(seed.wrapping_add(i as u64 * 997));
                (0..pop_per)
                    .map(|_| GeneticProgram::random(n_inputs, n_nodes, n_outputs, &mut r))
                    .collect()
            })
            .collect();

        for generation in 0..generations {
            // Parallel island evolution (one generation each)
            island_pops
                .par_iter_mut()
                .enumerate()
                .for_each(|(isle, pop)| {
                    let mut r = ChaCha8Rng::seed_from_u64(
                        seed.wrapping_add((generation as u64 + 1) * 10007 + isle as u64),
                    );
                    let fitnesses: Vec<Fitness> = pop.iter().map(|ind| fitness_fn(ind)).collect();
                    let strategy = SelectionStrategy::Tournament { k: 3 };
                    let best_i = fitnesses
                        .iter()
                        .enumerate()
                        .max_by(|(_, a), (_, b)| {
                            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    let mut next = vec![pop[best_i].clone()];
                    while next.len() < pop.len() {
                        let i = selection::select(&fitnesses, strategy, &mut r);
                        let mut child = pop[i].clone();
                        child.mutate(0.15, &mut r);
                        next.push(child);
                    }
                    *pop = next;
                });

            // Spatial migration every 5 gens
            if generation % 5 == 4 && islands > 1 {
                let elites: Vec<GeneticProgram> = island_pops
                    .iter()
                    .map(|pop| {
                        pop.iter()
                            .max_by(|a, b| {
                                fitness_fn(a)
                                    .partial_cmp(&fitness_fn(b))
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .cloned()
                            .unwrap_or_else(|| pop[0].clone())
                    })
                    .collect();
                for i in 0..islands {
                    let src = (i + islands - 1) % islands;
                    // Replace worst with neighbor elite
                    if let Some(worst) = island_pops[i]
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| {
                            fitness_fn(a)
                                .partial_cmp(&fitness_fn(b))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(idx, _)| idx)
                    {
                        island_pops[i][worst] = elites[src].clone();
                    }
                }
            }
            let _ = &mut rng; // keep seeded path warm
        }

        // Global best
        island_pops
            .into_iter()
            .flatten()
            .max_by(|a, b| {
                fitness_fn(a)
                    .partial_cmp(&fitness_fn(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or_else(|| GeneticProgram::random(n_inputs, n_nodes, n_outputs, &mut rng))
    }
}

fn apply_fn(id: usize, a: f64, b: f64) -> f64 {
    match id % NUM_FUNCTIONS {
        FN_ADD => a + b,
        FN_SUB => a - b,
        FN_MUL => a * b,
        FN_DIV => {
            if b.abs() < 1e-9 {
                a
            } else {
                a / b
            }
        }
        FN_SIN => a.sin(),
        FN_NEG => -a,
        _ => a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::fitness::mse_to_fitness;

    #[test]
    fn cgp_symbolic_regression_toy() {
        // Target: f(x) = x (identity) — easy
        let data: Vec<(f64, f64)> = (-5..=5).map(|i| (i as f64, i as f64)).collect();
        let best = GeneticProgram::evolve(
            32,
            2,
            15,
            1,
            8,
            1,
            |prog| {
                let preds: Vec<f64> = data.iter().map(|(x, _)| prog.eval(&[*x])[0]).collect();
                let targets: Vec<f64> = data.iter().map(|(_, y)| *y).collect();
                mse_to_fitness(&preds, &targets)
            },
            7,
        );
        let err: f64 = data
            .iter()
            .map(|(x, y)| (best.eval(&[*x])[0] - y).abs())
            .sum::<f64>()
            / data.len() as f64;
        assert!(err < 2.0, "mean abs err={err}");
    }
}
