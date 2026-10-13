use faer::dyn_stack::{MemBuffer, MemStack, StackReq};
use faer::linalg::lu::partial_pivoting::{factor, solve};
use faer::prelude::{Reborrow, ReborrowMut};
use faer::{MatMut, Par};
use num_complex::Complex64 as C64;

pub struct System {
    pub a: Vec<C64>,
    pub n: usize,
    pub w: usize,
}

impl System {
    pub fn zeros(n: usize) -> Self {
        let w = n + 1;
        Self { a: vec![C64::new(0.0, 0.0); n * w], n, w }
    }

    pub fn rhs_mut(&mut self, row: usize) -> &mut C64 {
        &mut self.a[row * self.w + self.n]
    }
}

pub type Currents = Vec<C64>;

#[cfg(not(target_arch = "wasm32"))]
fn par(n: usize) -> Par {
    let wanted = (n * n / 1_690_000).max(1);
    match wanted.min(rayon::current_num_threads()) {
        0 | 1 => Par::Seq,
        t => Par::rayon(t),
    }
}

#[cfg(target_arch = "wasm32")]
fn par(_: usize) -> Par {
    Par::Seq
}

pub fn solve_system(mut sys: System) -> Currents {
    let (n, w) = (sys.n, sys.w);
    if n == 0 {
        return Vec::new();
    }
    let mut x: Vec<C64> = (0..n).map(|r| sys.a[r * w + n]).collect();
    let par = par(n);
    let mut perm = vec![0usize; n];
    let mut perm_inv = vec![0usize; n];
    let mut buf = MemBuffer::new(StackReq::any_of(&[
        factor::lu_in_place_scratch::<usize, C64>(n, n, par, Default::default()),
        solve::solve_transpose_in_place_scratch::<usize, C64>(n, 1, par),
    ]));
    let stack = MemStack::new(&mut buf);
    let rhs = MatMut::from_column_major_slice_mut(&mut x, n, 1);
    let mut transposed = MatMut::from_column_major_slice_with_stride_mut(&mut sys.a, n, n, w);
    let (_, p) = factor::lu_in_place(
        transposed.rb_mut(),
        &mut perm,
        &mut perm_inv,
        par,
        stack,
        Default::default(),
    );
    solve::solve_transpose_in_place(transposed.rb(), transposed.rb(), p, rhs, par, stack);
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_that_needs_row_swaps_is_solved() {
        let n = 40;
        let mut sys = System::zeros(n);
        for r in 0..n {
            let partner = (r + n / 2) % n;
            sys.a[r * sys.w + partner] = C64::new(1.0 + r as f64, 0.5);
            sys.a[r * sys.w + (r + 1) % n] += C64::new(0.0, 0.01 * r as f64);
            *sys.rhs_mut(r) = C64::new(r as f64, -1.0);
        }
        let a = sys.a.clone();
        let x = solve_system(sys);
        for r in 0..n {
            let ax: C64 = (0..n).map(|j| a[r * (n + 1) + j] * x[j]).sum();
            assert!((ax - a[r * (n + 1) + n]).norm() < 1e-12, "row {r}: {ax}");
        }
    }
}
