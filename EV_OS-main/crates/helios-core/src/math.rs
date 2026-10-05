//! Discrete-time control primitives. All are allocation-free and `Copy`.

/// Clamp that is NaN-safe: a NaN input returns `lo` (fail towards the low/safe bound).
#[inline]
pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x.is_nan() {
        lo
    } else if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

#[inline]
pub fn sat_sym(x: f32, lim: f32) -> f32 {
    clamp(x, -lim, lim)
}

#[inline]
pub fn sqrt(x: f32) -> f32 {
    libm::sqrtf(x)
}
#[inline]
pub fn atan2(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}
#[inline]
pub fn sin(x: f32) -> f32 {
    libm::sinf(x)
}
#[inline]
pub fn cos(x: f32) -> f32 {
    libm::cosf(x)
}
#[inline]
pub fn exp(x: f32) -> f32 {
    libm::expf(x)
}
#[inline]
pub fn abs(x: f32) -> f32 {
    libm::fabsf(x)
}
#[inline]
pub fn powf(x: f32, y: f32) -> f32 {
    libm::powf(x, y)
}

/// Linear interpolation on a monotonically increasing breakpoint table, clamped at the ends.
pub fn interp1(xs: &[f32], ys: &[f32], x: f32) -> f32 {
    debug_assert_eq!(xs.len(), ys.len());
    let n = xs.len();
    if n == 0 {
        return 0.0;
    }
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    let mut i = 1;
    while i < n && xs[i] < x {
        i += 1;
    }
    let (x0, x1, y0, y1) = (xs[i - 1], xs[i], ys[i - 1], ys[i]);
    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
}

/// First-order low-pass, exact zero-order-hold discretisation:
/// y[k] = y[k-1] + (1 - e^{-dt/tau}) (u[k] - y[k-1]).
#[derive(Clone, Copy, Debug)]
pub struct Lpf1 {
    pub y: f32,
    alpha: f32,
}

impl Lpf1 {
    pub fn new(tau_s: f32, dt_s: f32, y0: f32) -> Self {
        let alpha = if tau_s <= 0.0 { 1.0 } else { 1.0 - exp(-dt_s / tau_s) };
        Self { y: y0, alpha }
    }
    #[inline]
    pub fn step(&mut self, u: f32) -> f32 {
        self.y += self.alpha * (u - self.y);
        self.y
    }
}

/// Symmetric or asymmetric slew-rate limiter (units/s).
#[derive(Clone, Copy, Debug)]
pub struct RateLimiter {
    pub y: f32,
    pub rise: f32,
    pub fall: f32,
}

impl RateLimiter {
    pub const fn new(y0: f32, rise_per_s: f32, fall_per_s: f32) -> Self {
        Self { y: y0, rise: rise_per_s, fall: fall_per_s }
    }
    #[inline]
    pub fn step(&mut self, target: f32, dt_s: f32) -> f32 {
        let up = self.rise * dt_s;
        let dn = self.fall * dt_s;
        let d = clamp(target - self.y, -dn, up);
        self.y += d;
        self.y
    }
}

/// PI controller with conditional-integration anti-windup and output clamp.
#[derive(Clone, Copy, Debug)]
pub struct Pi {
    pub kp: f32,
    pub ki: f32,
    pub lo: f32,
    pub hi: f32,
    pub integ: f32,
}

impl Pi {
    pub const fn new(kp: f32, ki: f32, lo: f32, hi: f32) -> Self {
        Self { kp, ki, lo, hi, integ: 0.0 }
    }
    pub fn reset(&mut self) {
        self.integ = 0.0;
    }
    pub fn step(&mut self, err: f32, dt_s: f32) -> f32 {
        let p = self.kp * err;
        let u_unsat = p + self.integ + self.ki * err * dt_s;
        let u = clamp(u_unsat, self.lo, self.hi);
        // Only integrate when not saturated, or when the error drives out of saturation.
        let saturated_hi = u_unsat > self.hi && err > 0.0;
        let saturated_lo = u_unsat < self.lo && err < 0.0;
        if !(saturated_hi || saturated_lo) {
            self.integ = clamp(self.integ + self.ki * err * dt_s, self.lo, self.hi);
        }
        u
    }
}

/// Counts consecutive true samples; `active()` once `n` reached. Used for debouncing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Debounce {
    count: u32,
    pub n: u32,
}

impl Debounce {
    pub const fn new(n: u32) -> Self {
        Self { count: 0, n }
    }
    #[inline]
    pub fn step(&mut self, cond: bool) -> bool {
        if cond {
            self.count = self.count.saturating_add(1);
        } else {
            self.count = 0;
        }
        self.active()
    }
    #[inline]
    pub fn active(&self) -> bool {
        self.count >= self.n
    }
    pub fn reset(&mut self) {
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lpf_step_response_matches_exponential() {
        let dt = 0.001;
        let tau = 0.02;
        let mut f = Lpf1::new(tau, dt, 0.0);
        for _ in 0..20 {
            f.step(1.0);
        }
        let expected = 1.0 - libm::expf(-0.02 / 0.02);
        assert!((f.y - expected).abs() < 1e-5, "{} vs {}", f.y, expected);
    }

    #[test]
    fn clamp_nan_goes_low() {
        assert_eq!(clamp(f32::NAN, -1.0, 2.0), -1.0);
    }

    #[test]
    fn pi_antiwindup_recovers() {
        let mut pi = Pi::new(1.0, 10.0, -1.0, 1.0);
        for _ in 0..1000 {
            pi.step(5.0, 0.01);
        }
        assert!(pi.integ <= 1.0);
        let u = pi.step(-0.5, 0.01);
        assert!(u < 1.0);
    }

    #[test]
    fn interp_clamps_and_interpolates() {
        let xs = [0.0, 1.0, 2.0];
        let ys = [0.0, 10.0, 0.0];
        assert_eq!(interp1(&xs, &ys, -1.0), 0.0);
        assert!((interp1(&xs, &ys, 0.5) - 5.0).abs() < 1e-6);
        assert!((interp1(&xs, &ys, 1.5) - 5.0).abs() < 1e-6);
    }
}
