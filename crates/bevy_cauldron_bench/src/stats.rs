//! **The arithmetic a benchmark report states, and nothing else** (BM-03, #413).
//!
//! Engine-free and model-free: a function of numbers, so a report built from recorded `trial.json` files
//! can be checked against values worked out by hand.
//!
//! # pass^k
//!
//! A task run `n` times with `c` successes has `pass^k = C(c, k) / C(n, k)`: the chance that `k` trials
//! drawn without replacement are *all* successes (τ-bench, `10.48550/arXiv.2406.12045`). It falls as `k`
//! grows, and that fall is the point: an agent that passes a task three times in five is not an agent that
//! passes it every time.
//!
//! # The bootstrap, and why it resamples two levels
//!
//! A suite's mean success has two sources of noise, which tasks were chosen and which trials happened to
//! come out, so the interval resamples **tasks** with replacement and then **trials within each resampled
//! task** (the hierarchical bootstrap). Resampling trials alone would report an interval as narrow as the
//! number of trials, which says nothing about whether another set of tasks would give the same mean. The
//! generator is seeded ([`BOOTSTRAP_SEED`]), so one report is one set of numbers on every machine.

/// **How many resamples a bootstrap interval is drawn from.**
pub const BOOTSTRAP_RESAMPLES: usize = 10_000;

/// **The seed every bootstrap starts from**, so a report built twice from the same trials is the same
/// report, and a comparison does not move between two readings of it.
pub const BOOTSTRAP_SEED: u64 = 0x5EED_D4A2_B3C1_0413;

/// **The two tails of a 95% interval**, as fractions of the sorted resamples.
pub const INTERVAL_LOW: f64 = 0.025;
/// See [`INTERVAL_LOW`].
pub const INTERVAL_HIGH: f64 = 0.975;

/// **The percentile a report's "slow" wall time is read at.**
pub const WALL_SLOW_PERCENTILE: f64 = 0.90;

/// **A 95% interval.**
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Interval {
    /// The low end.
    pub lo: f64,
    /// The high end.
    pub hi: f64,
}

impl Interval {
    /// **Whether the interval contains `x`**, ends included.
    pub fn contains(&self, x: f64) -> bool {
        self.lo <= x && x <= self.hi
    }
}

/// **A small seeded generator** (SplitMix64): enough for resampling, and with no dependency.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// A generator that starts at `seed`.
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// **An index in `0..n`**; `0` for an empty range, so a caller that guards nothing cannot panic.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        // 53 high bits make a double in [0, 1): the modulo bias of `% n` for small `n` is not what a
        // bootstrap interval is sensitive to, and this one has none.
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        ((unit * n as f64) as usize).min(n - 1)
    }
}

/// **`pass^k` for a task with `c` successes in `n` trials**: `C(c, k) / C(n, k)`, computed as the product
/// `Π (c − i) / (n − i)` so a large `n` cannot overflow a binomial.
///
/// `1` for `k = 0` (the empty product), and `0` whenever `k > c` or `k > n`.
pub fn pass_hat_k(n: u32, c: u32, k: u32) -> f64 {
    if k > n || c > n || k > c {
        return 0.0;
    }
    (0..k).map(|i| f64::from(c - i) / f64::from(n - i)).product()
}

/// **`pass^k` for every `k` from 1 to `n`.**
pub fn pass_hat_curve(n: u32, c: u32) -> Vec<f64> {
    (1..=n).map(|k| pass_hat_k(n, c, k)).collect()
}

/// **The value at fraction `p` of a sample**, interpolated between the two nearest ranks; `None` for an
/// empty sample.
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.total_cmp(b));
    let position = p.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let below = position.floor() as usize;
    let above = position.ceil() as usize;
    let fraction = position - below as f64;
    Some(sorted[below] + (sorted[above.min(sorted.len() - 1)] - sorted[below]) * fraction)
}

/// **The median**, `None` for an empty sample.
pub fn median(values: &[f64]) -> Option<f64> {
    percentile(values, 0.5)
}

/// **The arithmetic mean**, `None` for an empty sample.
pub fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

/// **One resample of a task's trials**: the mean of `trials.len()` draws with replacement.
fn resampled_mean(trials: &[f64], rng: &mut Rng) -> f64 {
    if trials.is_empty() {
        return 0.0;
    }
    let sum: f64 = (0..trials.len()).map(|_| trials[rng.below(trials.len())]).sum();
    sum / trials.len() as f64
}

/// **The interval of `sorted`**, which the caller has already sorted.
fn interval_of(sorted: &[f64]) -> Option<Interval> {
    percentile(sorted, INTERVAL_LOW).zip(percentile(sorted, INTERVAL_HIGH)).map(|(lo, hi)| Interval { lo, hi })
}

/// **The mean of per-task success, with a 95% bootstrap interval.**
///
/// `tasks[i]` is task `i`'s trials, each `1.0` for a pass and `0.0` for a fail. Tasks with no trial are
/// left out by the caller. `None` when there is nothing to resample.
pub fn bootstrap_mean(tasks: &[Vec<f64>], resamples: usize, seed: u64) -> Option<(f64, Interval)> {
    if tasks.is_empty() || tasks.iter().any(Vec::is_empty) {
        return None;
    }
    let task_means: Vec<f64> = tasks.iter().filter_map(|t| mean(t)).collect();
    let observed = mean(&task_means)?;
    let mut rng = Rng::new(seed);
    let mut draws = Vec::with_capacity(resamples);
    for _ in 0..resamples {
        let total: f64 =
            (0..tasks.len()).map(|_| resampled_mean(&tasks[rng.below(tasks.len())], &mut rng)).sum();
        draws.push(total / tasks.len() as f64);
    }
    draws.sort_by(|a, b| a.total_cmp(b));
    interval_of(&draws).map(|interval| (observed, interval))
}

/// **The difference `b − a` of two runs' mean success on the same tasks, with a paired 95% bootstrap
/// interval.**
///
/// `pairs[i]` is `(a's trials, b's trials)` of task `i`. **The same resampled tasks are used for both
/// runs**, which is what makes the interval about *the change* rather than about each run's own spread:
/// a task both runs find hard cancels out of the difference instead of widening it. Trials are resampled
/// within each side separately, because they are separate runs. `None` when there is nothing to pair.
pub fn paired_bootstrap(
    pairs: &[(Vec<f64>, Vec<f64>)],
    resamples: usize,
    seed: u64,
) -> Option<(f64, Interval)> {
    if pairs.is_empty() || pairs.iter().any(|(a, b)| a.is_empty() || b.is_empty()) {
        return None;
    }
    let observed = mean(&pairs.iter().filter_map(|(a, b)| Some(mean(b)? - mean(a)?)).collect::<Vec<_>>())?;
    let mut rng = Rng::new(seed);
    let mut draws = Vec::with_capacity(resamples);
    for _ in 0..resamples {
        let mut total = 0.0;
        for _ in 0..pairs.len() {
            let (a, b) = &pairs[rng.below(pairs.len())];
            total += resampled_mean(b, &mut rng) - resampled_mean(a, &mut rng);
        }
        draws.push(total / pairs.len() as f64);
    }
    draws.sort_by(|a, b| a.total_cmp(b));
    interval_of(&draws).map(|interval| (observed, interval))
}
