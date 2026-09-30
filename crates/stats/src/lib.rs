//! `stats` — speedcubing statistics over a list of solves.
//!
//! All times are in milliseconds. Functions are pure and operate on slices of
//! [`store::Solve`]. Statistics use *effective* times: the raw time for `OK`,
//! raw + 2000 ms for `+2`, and "did not finish" for `DNF`.
//!
//! Averages follow the WCA rule: for an average of N solves, the best and worst
//! 5% (rounded up, at least 1 each for N >= 5) are trimmed and the remainder is
//! averaged. A DNF counts as the slowest result; if the number of DNFs exceeds
//! the trim count the average is DNF. `mo3` (mean of 3) trims nothing and is
//! DNF if any of its solves is.

use store::{Penalty, Solve};

/// A statistic that may be unavailable, DNF, or a time in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatValue {
    /// Not enough solves to compute the value.
    None,
    /// The value is a DNF.
    Dnf,
    /// A time in milliseconds.
    Time(f64),
}

impl StatValue {
    /// The time in ms, if this is a `Time`.
    pub fn time(self) -> Option<f64> {
        match self {
            StatValue::Time(t) => Some(t),
            _ => None,
        }
    }
}

/// The effective time of a solve in ms, or `None` for a DNF.
pub fn effective_ms(solve: &Solve) -> Option<i64> {
    match solve.penalty {
        Penalty::Ok => Some(solve.time_ms),
        Penalty::PlusTwo => Some(solve.time_ms + 2000),
        Penalty::Dnf => None,
    }
}

fn effective_all(solves: &[Solve]) -> Vec<Option<i64>> {
    solves.iter().map(effective_ms).collect()
}

/// Extract effective solve times (ms) in chronological order, skipping DNFs.
pub fn times(solves: &[Solve]) -> Vec<i64> {
    solves.iter().filter_map(effective_ms).collect()
}

/// Total number of solves.
pub fn count(solves: &[Solve]) -> usize {
    solves.len()
}

/// Number of solves that count towards the mean (non-DNF).
pub fn mean_counted(solves: &[Solve]) -> usize {
    solves.iter().filter(|s| effective_ms(s).is_some()).count()
}

/// Arithmetic mean of all non-DNF solves (ms), or `None` if there are none.
pub fn mean(solves: &[Solve]) -> Option<f64> {
    let t = times(solves);
    if t.is_empty() {
        return None;
    }
    Some(t.iter().sum::<i64>() as f64 / t.len() as f64)
}

/// Best (minimum) effective single time (ms), ignoring DNFs.
pub fn best(solves: &[Solve]) -> Option<i64> {
    solves.iter().filter_map(effective_ms).min()
}

/// Worst single: DNF when any DNF exists, else the maximum time.
pub fn worst(solves: &[Solve]) -> StatValue {
    if solves.is_empty() {
        return StatValue::None;
    }
    if solves.iter().any(|s| s.penalty == Penalty::Dnf) {
        return StatValue::Dnf;
    }
    solves
        .iter()
        .filter_map(effective_ms)
        .max()
        .map_or(StatValue::None, |m| StatValue::Time(m as f64))
}

/// Population standard deviation of all non-DNF solves (ms).
pub fn stddev(solves: &[Solve]) -> Option<f64> {
    let m = mean(solves)?;
    let t = times(solves);
    let n = t.len() as f64;
    let var = t
        .iter()
        .map(|&x| {
            let d = x as f64 - m;
            d * d
        })
        .sum::<f64>()
        / n;
    Some(var.sqrt())
}

/// Mean of exactly 3 results (no trimming); DNF if any is DNF.
fn mo3_window(window: &[Option<i64>]) -> StatValue {
    if window.len() != 3 {
        return StatValue::None;
    }
    let mut sum = 0i64;
    for w in window {
        match w {
            Some(v) => sum += v,
            None => return StatValue::Dnf,
        }
    }
    StatValue::Time(sum as f64 / 3.0)
}

/// Mean of 3: the mean of the most recent 3 solves (no trimming).
pub fn mo3(solves: &[Solve]) -> StatValue {
    let n = solves.len();
    if n < 3 {
        return StatValue::None;
    }
    mo3_window(&effective_all(&solves[n - 3..]))
}

/// The best mo3 over all windows of 3 (DNF windows ignored).
pub fn best_mo3(solves: &[Solve]) -> StatValue {
    let all = effective_all(solves);
    best_of_windows(&all, 3, mo3_window)
}

/// Number of solves trimmed from *each* end for an average of `n` solves,
/// per the WCA rule: 5% rounded up, at least 1 for n >= 5.
fn trim_count(n: usize) -> usize {
    let t = (n as f64 * 0.05).ceil() as usize;
    t.max(1)
}

/// Trimmed average of exactly `window` (WCA average). DNF sorts as slowest;
/// the result is DNF when DNFs exceed the trim count.
fn wca_average_of(window: &[Option<i64>]) -> StatValue {
    let n = window.len();
    if n < 3 {
        return StatValue::None;
    }
    let t = trim_count(n);
    if 2 * t >= n {
        return StatValue::None;
    }
    let dnfs = window.iter().filter(|w| w.is_none()).count();
    if dnfs > t {
        return StatValue::Dnf;
    }
    let mut sorted: Vec<i64> = window.iter().map(|w| w.unwrap_or(i64::MAX)).collect();
    sorted.sort_unstable();
    let kept = &sorted[t..n - t];
    let sum: i64 = kept.iter().sum();
    StatValue::Time(sum as f64 / kept.len() as f64)
}

/// Minimum computed value over all sliding windows of `n`, ignoring DNF/None.
fn best_of_windows(all: &[Option<i64>], n: usize, f: fn(&[Option<i64>]) -> StatValue) -> StatValue {
    if all.len() < n {
        return StatValue::None;
    }
    let mut best: Option<f64> = None;
    for w in all.windows(n) {
        if let StatValue::Time(avg) = f(w) {
            best = Some(best.map_or(avg, |b| b.min(avg)));
        }
    }
    best.map_or(StatValue::None, StatValue::Time)
}

/// The "current" average of N: the WCA average of the most recent `n` solves.
pub fn current_aon(solves: &[Solve], n: usize) -> StatValue {
    if solves.len() < n {
        return StatValue::None;
    }
    wca_average_of(&effective_all(&solves[solves.len() - n..]))
}

/// The "best" average of N over history: the minimum WCA average across all
/// sliding windows of `n` consecutive solves (DNF averages ignored).
pub fn best_aon(solves: &[Solve], n: usize) -> StatValue {
    best_of_windows(&effective_all(solves), n, wca_average_of)
}

/// For every solve, the WCA average of N ending at that solve
/// (`StatValue::None` for the first `n - 1` solves).
pub fn rolling_aon(solves: &[Solve], n: usize) -> Vec<StatValue> {
    let all = effective_all(solves);
    (0..all.len())
        .map(|i| {
            if i + 1 < n {
                StatValue::None
            } else {
                wca_average_of(&all[i + 1 - n..=i])
            }
        })
        .collect()
}

/// Convenience: current Ao5.
pub fn ao5(solves: &[Solve]) -> StatValue {
    current_aon(solves, 5)
}
/// Convenience: current Ao12.
pub fn ao12(solves: &[Solve]) -> StatValue {
    current_aon(solves, 12)
}
/// Convenience: current Ao100.
pub fn ao100(solves: &[Solve]) -> StatValue {
    current_aon(solves, 100)
}

/// A snapshot of all headline statistics for display.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub count: usize,
    /// Number of solves counted in `mean` (non-DNF).
    pub mean_counted: usize,
    pub mean: Option<f64>,
    /// The most recent solve's effective time (DNF if it is a DNF).
    pub current_single: StatValue,
    pub best: Option<i64>,
    pub worst: StatValue,
    pub stddev: Option<f64>,
    pub mo3: StatValue,
    pub best_mo3: StatValue,
    pub ao5: StatValue,
    pub ao12: StatValue,
    pub ao100: StatValue,
    pub best_ao5: StatValue,
    pub best_ao12: StatValue,
    pub best_ao100: StatValue,
}

/// Compute the full summary.
pub fn summary(solves: &[Solve]) -> Summary {
    let current_single = match solves.last() {
        None => StatValue::None,
        Some(s) => effective_ms(s).map_or(StatValue::Dnf, |v| StatValue::Time(v as f64)),
    };
    Summary {
        count: count(solves),
        mean_counted: mean_counted(solves),
        mean: mean(solves),
        current_single,
        best: best(solves),
        worst: worst(solves),
        stddev: stddev(solves),
        mo3: mo3(solves),
        best_mo3: best_mo3(solves),
        ao5: ao5(solves),
        ao12: ao12(solves),
        ao100: ao100(solves),
        best_ao5: best_aon(solves, 5),
        best_ao12: best_aon(solves, 12),
        best_ao100: best_aon(solves, 100),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(times: &[i64]) -> Vec<Solve> {
        times
            .iter()
            .enumerate()
            .map(|(i, &t)| Solve {
                id: i as i64 + 1,
                session_id: 1,
                time_ms: t,
                scramble: String::new(),
                created_at: i as i64,
                penalty: Penalty::Ok,
            })
            .collect()
    }

    /// Build solves from `(time, penalty)` pairs.
    fn mkp(items: &[(i64, Penalty)]) -> Vec<Solve> {
        let mut v = mk(&items.iter().map(|i| i.0).collect::<Vec<_>>());
        for (s, i) in v.iter_mut().zip(items) {
            s.penalty = i.1;
        }
        v
    }

    const OK: Penalty = Penalty::Ok;
    const DNF: Penalty = Penalty::Dnf;

    #[test]
    fn effective_ok_plus_two_dnf() {
        let s = mkp(&[(12_340, OK), (12_340, Penalty::PlusTwo), (12_340, DNF)]);
        assert_eq!(effective_ms(&s[0]), Some(12_340));
        assert_eq!(effective_ms(&s[1]), Some(14_340));
        assert_eq!(s[1].time_ms, 12_340, "raw time is unchanged");
        assert_eq!(effective_ms(&s[2]), None);
    }

    #[test]
    fn empty_stats_are_none() {
        let s = mk(&[]);
        assert_eq!(mean(&s), None);
        assert_eq!(best(&s), None);
        assert_eq!(worst(&s), StatValue::None);
        assert_eq!(stddev(&s), None);
        assert_eq!(mo3(&s), StatValue::None);
        assert_eq!(ao5(&s), StatValue::None);
    }

    #[test]
    fn mean_best_worst() {
        let s = mk(&[1000, 2000, 3000]);
        assert_eq!(mean(&s), Some(2000.0));
        assert_eq!(best(&s), Some(1000));
        assert_eq!(worst(&s), StatValue::Time(3000.0));
    }

    #[test]
    fn plus_two_counts_in_stats() {
        let s = mkp(&[(1000, Penalty::PlusTwo), (3000, OK)]);
        assert_eq!(best(&s), Some(3000));
        assert_eq!(worst(&s), StatValue::Time(3000.0));
        assert_eq!(mean(&s), Some(3000.0));
    }

    #[test]
    fn best_single_ignores_dnf() {
        let s = mkp(&[(9000, DNF), (11_000, OK)]);
        assert_eq!(best(&s), Some(11_000));
    }

    #[test]
    fn worst_is_dnf_when_any_dnf() {
        let s = mkp(&[(9000, DNF), (11_000, OK)]);
        assert_eq!(worst(&s), StatValue::Dnf);
    }

    #[test]
    fn mean_and_stddev_exclude_dnf_and_report_counted() {
        let s = mkp(&[(1000, OK), (5000, DNF), (3000, OK)]);
        assert_eq!(mean(&s), Some(2000.0));
        assert_eq!(mean_counted(&s), 2);
        assert_eq!(stddev(&s), Some(1000.0));
        assert_eq!(summary(&s).mean_counted, 2);
        assert_eq!(summary(&s).count, 3);
    }

    #[test]
    fn all_dnf_has_no_best_or_mean() {
        let s = mkp(&[(1000, DNF), (2000, DNF)]);
        assert_eq!(best(&s), None);
        assert_eq!(mean(&s), None);
        assert_eq!(worst(&s), StatValue::Dnf);
    }

    #[test]
    fn stddev_known() {
        let s = mk(&[1000, 2000, 3000]);
        let sd = stddev(&s).unwrap();
        let expected = (2_000_000.0f64 / 3.0).sqrt();
        assert!((sd - expected).abs() < 1e-6, "sd={sd} expected={expected}");
    }

    #[test]
    fn mo3_uses_last_three() {
        let s = mk(&[5000, 1000, 2000, 3000]);
        assert_eq!(mo3(&s), StatValue::Time(2000.0));
    }

    #[test]
    fn mo3_dnf_if_any_dnf() {
        let s = mkp(&[(1000, OK), (2000, DNF), (3000, OK)]);
        assert_eq!(mo3(&s), StatValue::Dnf);
        assert_eq!(best_mo3(&s), StatValue::None);
    }

    #[test]
    fn best_mo3_ignores_dnf_windows() {
        let s = mkp(&[(9000, OK), (9000, OK), (9000, OK), (1000, DNF), (2000, OK)]);
        assert_eq!(best_mo3(&s), StatValue::Time(9000.0));
    }

    #[test]
    fn ao5_trims_best_and_worst() {
        let s = mk(&[1000, 2000, 3000, 4000, 5000]);
        assert_eq!(ao5(&s), StatValue::Time(3000.0));
    }

    #[test]
    fn ao5_uses_most_recent_five() {
        let s = mk(&[1000, 2000, 3000, 4000, 5000, 6000]);
        assert_eq!(ao5(&s), StatValue::Time(4000.0));
    }

    #[test]
    fn ao5_requires_five() {
        let s = mk(&[1000, 2000, 3000, 4000]);
        assert_eq!(ao5(&s), StatValue::None);
    }

    #[test]
    fn ao5_one_dnf_is_trimmed_as_worst() {
        let s = mkp(&[
            (10_000, OK),
            (11_000, OK),
            (12_000, OK),
            (13_000, OK),
            (1, DNF),
        ]);
        assert_eq!(ao5(&s), StatValue::Time(12_000.0));
    }

    #[test]
    fn ao5_two_dnfs_is_dnf() {
        let s = mkp(&[
            (10_000, OK),
            (11_000, DNF),
            (12_000, OK),
            (13_000, OK),
            (1, DNF),
        ]);
        assert_eq!(ao5(&s), StatValue::Dnf);
    }

    #[test]
    fn ao12_trims_one_each_end() {
        let times: Vec<i64> = (1..=12).map(|x| x * 1000).collect();
        let s = mk(&times);
        assert_eq!(ao12(&s), StatValue::Time(6500.0));
    }

    #[test]
    fn ao100_trims_five_each_end() {
        let times: Vec<i64> = (1..=100).map(|x| x * 1000).collect();
        let s = mk(&times);
        assert_eq!(ao100(&s), StatValue::Time(50500.0));
    }

    #[test]
    fn ao100_tolerates_up_to_five_dnfs() {
        let mut items: Vec<(i64, Penalty)> = (1..=100).map(|x| (x * 1000, OK)).collect();
        for item in items.iter_mut().take(5) {
            item.1 = DNF;
        }
        let s = mkp(&items);
        assert!(matches!(ao100(&s), StatValue::Time(_)));
        items[5].1 = DNF;
        assert_eq!(ao100(&mkp(&items)), StatValue::Dnf);
    }

    #[test]
    fn best_aon_finds_minimum_window() {
        let s = mk(&[
            10000, 10000, 10000, 10000, 10000, 1000, 2000, 3000, 4000, 5000,
        ]);
        assert_eq!(best_aon(&s, 5), StatValue::Time(3000.0));
        assert_eq!(ao5(&s), StatValue::Time(3000.0));
    }

    #[test]
    fn best_aon_ignores_dnf_averages() {
        // Windows: [1,2,3,4,5] -> 3s; then a DNF pair makes later windows DNF.
        let s = mkp(&[
            (1000, OK),
            (2000, OK),
            (3000, OK),
            (4000, OK),
            (5000, OK),
            (1, DNF),
            (1, DNF),
        ]);
        assert_eq!(best_aon(&s, 5), StatValue::Time(3000.0));
        assert_eq!(ao5(&s), StatValue::Dnf);
    }

    #[test]
    fn best_aon_requires_n() {
        let s = mk(&[1000, 2000, 3000]);
        assert_eq!(best_aon(&s, 5), StatValue::None);
    }

    #[test]
    fn rolling_aon_per_solve() {
        let s = mk(&[1000, 2000, 3000, 4000, 5000, 6000]);
        let r = rolling_aon(&s, 5);
        assert_eq!(r.len(), 6);
        assert!(r[..4].iter().all(|v| *v == StatValue::None));
        assert_eq!(r[4], StatValue::Time(3000.0));
        assert_eq!(r[5], StatValue::Time(4000.0));
        assert_eq!(*r.last().unwrap(), ao5(&s));
    }

    #[test]
    fn summary_is_consistent() {
        let times: Vec<i64> = (1..=12).map(|x| x * 1000).collect();
        let s = mk(&times);
        let sum = summary(&s);
        assert_eq!(sum.count, 12);
        assert_eq!(sum.best, Some(1000));
        assert_eq!(sum.worst, StatValue::Time(12000.0));
        assert_eq!(sum.ao12, StatValue::Time(6500.0));
        assert_eq!(sum.ao100, StatValue::None);
        assert!(matches!(sum.ao5, StatValue::Time(_)));
        assert_eq!(sum.current_single, StatValue::Time(12000.0));
    }
}
