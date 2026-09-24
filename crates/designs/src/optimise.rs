use crate::matching::{plan_match, swr_of_50};
use crate::{Controls, Design, dress};
use antenna_solver::geometry::WireProps;
use antenna_solver::optimise::nelder_mead;
use antenna_solver::solve::SolveResult;
use antenna_solver::solve::{Prepared, sweep_cap};
use antenna_solver::vec::{Vec3, cross, normalise, scale};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Goal {
    pub match_weight: f64,
    pub gain_weight: f64,
    pub fb_weight: f64,
    pub band: f64,
    pub z0: f64,
}

impl Default for Goal {
    fn default() -> Self {
        Goal { match_weight: 1.0, gain_weight: 1.0, fb_weight: 0.0, band: 0.01, z0: 50.0 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Score {
    pub cost: f64,
    pub worst_swr: f64,
    pub gain: f64,
    pub fb: f64,
}

pub struct Problem<'a> {
    pub design: &'static Design,
    pub lam: f64,
    pub wire: f64,
    pub props: WireProps,
    pub controls: &'a Controls,
    pub base: &'a HashMap<String, f64>,
    pub keys: Vec<(String, f64)>,
    pub goal: Goal,
    pub forward: Vec3,
}

impl Problem<'_> {
    pub fn overrides(&self, x: &[f64]) -> HashMap<String, f64> {
        let mut o = self.base.clone();
        for ((k, v0), s) in self.keys.iter().zip(x) {
            o.insert(k.clone(), v0 * s);
        }
        o
    }

    pub fn score(&self, x: &[f64]) -> Option<Score> {
        let fmt = |mm: f64| format!("{mm}");
        let o = self.overrides(x);
        let mut geo =
            self.design.run(self.lam, self.wire, &fmt, &o, self.controls, 1.0).output.solve;
        dress(&mut geo, self.props);
        let model = Prepared::new(&geo, self.lam, self.wire, sweep_cap());
        let r = model.solve(self.lam, self.goal.gain_weight > 0.0 || self.goal.fb_weight > 0.0);
        let plan = plan_match(self.design.id, r.z, self.lam, self.goal.z0);
        let mut worst = swr_of_50(plan.apply(r.z, 1.0, 1.0), self.goal.z0);
        if self.goal.band > 0.0 {
            for s in [1.0 - self.goal.band, 1.0 + self.goal.band] {
                let z = model.solve(self.lam / s, false).z;
                worst = worst.max(swr_of_50(plan.apply(z, s, 1.0), self.goal.z0));
            }
        }
        let g = ((worst - 1.0) / (worst + 1.0)).powi(2);
        let mismatch_db = -10.0 * (1.0 - g).max(1e-6).log10();
        let (gain, fb) = if r.pattern.is_some() {
            let front = r.gain_dbi(self.forward)?;
            let back = rear_lobe(&r, scale(self.forward, -1.0))?;
            (front, (front - back).min(40.0))
        } else {
            (0.0, 0.0)
        };
        let cost = self.goal.match_weight * mismatch_db
            - self.goal.gain_weight * gain
            - self.goal.fb_weight * 0.2 * fb;
        Some(Score { cost, worst_swr: worst, gain, fb })
    }

    pub fn run(
        &self,
        max_evals: usize,
        mut on_best: impl FnMut(&[f64], Score, usize) -> bool,
    ) -> (Vec<f64>, Score) {
        let x0 = vec![1.0; self.keys.len()];
        let best = nelder_mead(
            |x| self.score(x).map(|s| s.cost),
            &x0,
            0.04,
            (0.75, 1.25),
            max_evals,
            |b| match self.score(&b.x) {
                Some(s) => on_best(&b.x, s, b.evals),
                None => true,
            },
        );
        let s = self.score(&best.x).unwrap_or_default();
        (best.x, s)
    }
}

fn rear_lobe(r: &SolveResult, back: Vec3) -> Option<f64> {
    let b = normalise(back);
    let seed = if b[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let u = normalise(cross(b, seed));
    let v = cross(b, u);
    let mut worst = r.gain_dbi(b)?;
    for i in 1..=6 {
        let t = (i as f64 * 10.0).to_radians();
        for j in 0..12 {
            let p = (j as f64 * 30.0).to_radians();
            let d = [0, 1, 2].map(|k| b[k] * t.cos() + (u[k] * p.cos() + v[k] * p.sin()) * t.sin());
            worst = worst.max(r.gain_dbi(d)?);
        }
    }
    Some(worst)
}
