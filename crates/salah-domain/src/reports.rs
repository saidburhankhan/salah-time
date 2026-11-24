//! Arithmetic shared by the session reports.

/// Compensated sum of distraction contributions in ascending value order.
#[derive(Default)]
pub struct DistractionTotal {
    sum: Option<f64>,
    correction: f64,
}

impl DistractionTotal {
    pub fn add(&mut self, value: f64) {
        let sum = self.sum.unwrap_or(0.);
        let next = sum + value;
        if next.is_finite() {
            self.correction += if sum.abs() >= value.abs() {
                (sum - next) + value
            } else {
                (value - next) + sum
            };
        }
        self.sum = Some(next);
    }

    pub fn finish(self) -> Option<f64> {
        self.sum.map(|sum| sum + self.correction)
    }
}

pub fn distraction_total(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut values: Vec<_> = values.collect();
    values.sort_unstable_by(f64::total_cmp);
    let mut total = DistractionTotal::default();
    for value in values {
        total.add(value);
    }
    total.finish()
}

pub fn distractions_per_hour(total: Option<f64>, minutes: i64) -> Option<f64> {
    total
        .filter(|_| minutes > 0)
        .map(|sum| sum * 60. / minutes as f64)
}
