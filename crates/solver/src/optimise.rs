pub struct Best {
    pub x: Vec<f64>,
    pub cost: f64,
    pub evals: usize,
}

pub fn nelder_mead(
    mut f: impl FnMut(&[f64]) -> Option<f64>,
    x0: &[f64],
    step: f64,
    bounds: (f64, f64),
    max_evals: usize,
    mut on_best: impl FnMut(&Best) -> bool,
) -> Best {
    let n = x0.len();
    let clamp =
        |x: Vec<f64>| x.into_iter().map(|v| v.clamp(bounds.0, bounds.1)).collect::<Vec<_>>();
    let mut evals = 0;
    let mut eval = |x: &[f64], evals: &mut usize| {
        *evals += 1;
        f(x).filter(|c| c.is_finite()).unwrap_or(f64::INFINITY)
    };
    let mut simplex: Vec<(Vec<f64>, f64)> = Vec::with_capacity(n + 1);
    let c0 = eval(x0, &mut evals);
    simplex.push((x0.to_vec(), c0));
    for i in 0..n {
        let mut x = x0.to_vec();
        x[i] += if x[i] + step <= bounds.1 { step } else { -step };
        let x = clamp(x);
        let c = eval(&x, &mut evals);
        simplex.push((x, c));
    }
    let mut best = Best { x: x0.to_vec(), cost: c0, evals };
    while evals < max_evals {
        simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
        if simplex[0].1 < best.cost {
            best = Best { x: simplex[0].0.clone(), cost: simplex[0].1, evals };
            if !on_best(&best) {
                break;
            }
        }
        let spread = simplex[n].1 - simplex[0].1;
        let size = simplex
            .iter()
            .skip(1)
            .map(|(x, _)| {
                x.iter().zip(&simplex[0].0).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max)
            })
            .fold(0.0, f64::max);
        if spread.abs() < 1e-5 && size < 1e-4 {
            break;
        }
        let centroid: Vec<f64> = (0..n)
            .map(|j| simplex[..n].iter().map(|(x, _)| x[j]).sum::<f64>() / n as f64)
            .collect();
        let worst = simplex[n].clone();
        let along =
            |t: f64| clamp((0..n).map(|j| centroid[j] + t * (worst.0[j] - centroid[j])).collect());
        let xr = along(-1.0);
        let cr = eval(&xr, &mut evals);
        if cr < simplex[0].1 {
            let xe = along(-2.0);
            let ce = eval(&xe, &mut evals);
            simplex[n] = if ce < cr { (xe, ce) } else { (xr, cr) };
        } else if cr < simplex[n - 1].1 {
            simplex[n] = (xr, cr);
        } else {
            let (xc, cc) = if cr < worst.1 {
                let x = along(-0.5);
                let c = eval(&x, &mut evals);
                (x, c)
            } else {
                let x = along(0.5);
                let c = eval(&x, &mut evals);
                (x, c)
            };
            if cc < worst.1.min(cr) {
                simplex[n] = (xc, cc);
            } else {
                let b = simplex[0].0.clone();
                for s in simplex.iter_mut().skip(1) {
                    let x = clamp((0..n).map(|j| b[j] + 0.5 * (s.0[j] - b[j])).collect());
                    let c = eval(&x, &mut evals);
                    *s = (x, c);
                }
            }
        }
    }
    simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
    if simplex[0].1 < best.cost {
        best = Best { x: simplex[0].0.clone(), cost: simplex[0].1, evals };
    }
    best.evals = evals;
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_bottom_of_a_rosenbrock_valley() {
        let f = |x: &[f64]| Some((1.0 - x[0]).powi(2) + 100.0 * (x[1] - x[0] * x[0]).powi(2));
        let b = nelder_mead(f, &[-1.2, 1.0], 0.1, (-5.0, 5.0), 2000, |_| true);
        assert!((b.x[0] - 1.0).abs() < 1e-2 && (b.x[1] - 1.0).abs() < 2e-2, "{:?}", b.x);
    }

    #[test]
    fn respects_the_bounds() {
        let f = |x: &[f64]| Some((x[0] - 3.0).powi(2));
        let b = nelder_mead(f, &[1.0], 0.1, (0.5, 1.5), 500, |_| true);
        assert!((b.x[0] - 1.5).abs() < 1e-6);
    }
}
