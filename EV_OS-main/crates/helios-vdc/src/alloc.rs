//! Box-constrained weighted least-squares control allocation.
//!
//! minimise  J(u) = sum_i w_i u_i^2 + sum_j q_j (B_j . u - v_j)^2
//! s.t.      lo_i <= u_i <= hi_i
//!
//! Solved by the redistributed weighted pseudo-inverse (Bodson, "Evaluation of
//! optimization methods for control allocation", JGCD 25(4), 2002): solve the
//! unconstrained normal equations on the free set, clamp every violating
//! variable to its bound, remove it from the free set, repeat. Terminates in at
//! most N iterations; deterministic run-time, which is what we need on the VCU.

/// Maximum number of allocation variables (four corners).
pub const N: usize = 4;
/// Number of virtual controls: [Fx, Mz].
pub const M: usize = 2;

#[derive(Clone, Copy, Debug)]
pub struct Problem {
    pub n: usize,
    /// Effectiveness matrix rows (virtual control j, variable i).
    pub b: [[f32; N]; M],
    pub v: [f32; M],
    pub q: [f32; M],
    pub w: [f32; N],
    pub lo: [f32; N],
    pub hi: [f32; N],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Solution {
    pub u: [f32; N],
    /// Achieved virtual controls B u.
    pub achieved: [f32; M],
    pub iterations: u8,
    pub saturated: [bool; N],
}

/// In-place Cholesky solve of the k x k SPD system A x = y. Returns false if A is not SPD.
fn cholesky_solve(a: &mut [[f32; N]; N], y: &mut [f32; N], k: usize) -> bool {
    for j in 0..k {
        let mut d = a[j][j];
        for p in 0..j {
            d -= a[j][p] * a[j][p];
        }
        if !(d > 1e-12) {
            return false;
        }
        let d = libm::sqrtf(d);
        a[j][j] = d;
        for i in (j + 1)..k {
            let mut s = a[i][j];
            for p in 0..j {
                s -= a[i][p] * a[j][p];
            }
            a[i][j] = s / d;
        }
    }
    // forward: L z = y
    for i in 0..k {
        let mut s = y[i];
        for p in 0..i {
            s -= a[i][p] * y[p];
        }
        y[i] = s / a[i][i];
    }
    // backward: L^T x = z
    for i in (0..k).rev() {
        let mut s = y[i];
        for p in (i + 1)..k {
            s -= a[p][i] * y[p];
        }
        y[i] = s / a[i][i];
    }
    true
}

pub fn solve(p: &Problem) -> Solution {
    let n = p.n.min(N);
    let mut u = [0.0f32; N];
    let mut fixed = [false; N];
    let mut iterations = 0u8;
    for i in 0..n {
        // Degenerate box (lo == hi, e.g. undriven + no brake) is fixed up front.
        if p.hi[i] - p.lo[i] <= 1e-6 {
            fixed[i] = true;
            u[i] = 0.5 * (p.lo[i] + p.hi[i]);
        }
    }
    for _ in 0..=n {
        iterations += 1;
        // Map free variables to a compact index set.
        let mut idx = [0usize; N];
        let mut k = 0;
        for i in 0..n {
            if !fixed[i] {
                idx[k] = i;
                k += 1;
            }
        }
        if k == 0 {
            break;
        }
        // Residual target after the fixed variables' contribution.
        let mut r = [0.0f32; M];
        for j in 0..M {
            let mut s = p.v[j];
            for i in 0..n {
                if fixed[i] {
                    s -= p.b[j][i] * u[i];
                }
            }
            r[j] = s;
        }
        // Normal equations H x = g on the free set.
        let mut h = [[0.0f32; N]; N];
        let mut g = [0.0f32; N];
        for a in 0..k {
            let ia = idx[a];
            for c in 0..k {
                let ic = idx[c];
                let mut s = if a == c { p.w[ia] } else { 0.0 };
                for j in 0..M {
                    s += p.q[j] * p.b[j][ia] * p.b[j][ic];
                }
                h[a][c] = s;
            }
            let mut s = 0.0;
            for j in 0..M {
                s += p.q[j] * p.b[j][ia] * r[j];
            }
            g[a] = s;
        }
        if !cholesky_solve(&mut h, &mut g, k) {
            // Numerically degenerate: fall back to the projection of zero (safe: no torque).
            for a in 0..k {
                u[idx[a]] = helios_core::math::clamp(0.0, p.lo[idx[a]], p.hi[idx[a]]);
            }
            break;
        }
        let mut violated = false;
        for a in 0..k {
            let i = idx[a];
            let x = g[a];
            if x > p.hi[i] {
                u[i] = p.hi[i];
                fixed[i] = true;
                violated = true;
            } else if x < p.lo[i] {
                u[i] = p.lo[i];
                fixed[i] = true;
                violated = true;
            } else {
                u[i] = x;
            }
        }
        if !violated {
            break;
        }
    }
    let mut achieved = [0.0f32; M];
    for j in 0..M {
        for i in 0..n {
            achieved[j] += p.b[j][i] * u[i];
        }
    }
    let mut saturated = [false; N];
    for i in 0..n {
        saturated[i] = (u[i] - p.hi[i]).abs() < 1e-3 || (u[i] - p.lo[i]).abs() < 1e-3;
    }
    Solution { u, achieved, iterations, saturated }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quad problem in normalised units (forces in kN), the same scaling the VDC uses
    /// so that H = W + B^T Q B stays well conditioned in f32.
    fn quad(fx_kn: f32, mz_knm: f32, cap_kn: f32) -> Problem {
        let yl = 0.83;
        Problem {
            n: 4,
            b: [[1.0, 1.0, 1.0, 1.0], [-1.0, 1.0, -1.0, 1.0]],
            v: [fx_kn, mz_knm / yl],
            q: [1.0, 1.0],
            w: [1e-3; 4],
            lo: [-cap_kn; 4],
            hi: [cap_kn; 4],
        }
    }

    #[test]
    fn feasible_demand_is_met_and_split_evenly() {
        let s = solve(&quad(8.0, 0.0, 6.0));
        assert!((s.achieved[0] - 8.0).abs() / 8.0 < 0.005, "{:?}", s);
        for i in 1..4 {
            assert!((s.u[i] - s.u[0]).abs() < 1e-3, "{:?}", s.u);
        }
    }

    #[test]
    fn yaw_moment_produces_left_right_split() {
        // Positive Mz (CCW, left turn) needs more force on the right wheels (y < 0).
        let s = solve(&quad(4.0, 3.0, 6.0));
        assert!(s.u[1] > s.u[0] && s.u[3] > s.u[2], "{:?}", s.u);
        assert!((s.achieved[1] * 0.83 - 3.0).abs() < 0.02, "{:?}", s.achieved);
    }

    #[test]
    fn bounds_respected_and_constrained_optimum_found() {
        // RL capped at 1 kN by TC, demand 16 kN / 0 Nm. With FL saturated at 6 kN and
        // FR = RR = x: Fx = 7 + 2x, Mz/yl = 2x - 7. Minimising (2x-9)^2 + (2x-7)^2
        // gives 2x = 8 => Fx = 15 kN, Mz/yl = 1 kN: the exact constrained LS optimum.
        let mut p = quad(16.0, 0.0, 6.0);
        p.hi[2] = 1.0;
        let s = solve(&p);
        for i in 0..4 {
            assert!(s.u[i] <= p.hi[i] + 1e-4 && s.u[i] >= p.lo[i] - 1e-4);
        }
        assert!((s.achieved[0] - 15.0).abs() < 0.02, "{:?}", s);
        assert!((s.achieved[1] - 1.0).abs() < 0.02, "{:?}", s);
    }

    #[test]
    fn infeasible_demand_saturates_gracefully() {
        let s = solve(&quad(40.0, 0.0, 6.0));
        for i in 0..4 {
            assert!((s.u[i] - 6.0).abs() < 1e-4);
        }
        assert!(s.iterations as usize <= 5);
    }

    #[test]
    fn degenerate_boxes_are_fixed() {
        let mut p = quad(5.0, 0.0, 6.0);
        p.lo[0] = 0.0;
        p.hi[0] = 0.0;
        p.lo[1] = 0.0;
        p.hi[1] = 0.0;
        let s = solve(&p);
        assert!(s.u[0].abs() < 1e-6 && s.u[1].abs() < 1e-6);
        assert!((s.achieved[0] - 5.0).abs() < 0.01);
    }
}
