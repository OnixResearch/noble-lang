//! Order statistics for the paired in-memory pipeline lane.

#[derive(Clone, Copy)]
pub(super) struct Summary {
    pub(super) median_us: f64,
    pub(super) iqr_us: f64,
    pub(super) observations: usize,
}

pub(super) struct Samples {
    values_us: Vec<f64>,
}

impl Samples {
    pub(super) fn with_capacity(sample_count: usize) -> Self {
        Self {
            values_us: Vec::with_capacity(sample_count),
        }
    }

    pub(super) fn push_us(&mut self, value_us: f64) {
        self.values_us.push(value_us);
    }

    pub(super) fn summary(&mut self) -> Option<Summary> {
        if self.values_us.is_empty() {
            return None;
        }
        self.values_us
            .sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let count = self.values_us.len();
        let median_us = self.values_us[count / 2];
        let low_us = self.values_us[count / 4];
        let high_us = self.values_us[count * 3 / 4];
        Some(Summary {
            median_us,
            iqr_us: high_us - low_us,
            observations: count,
        })
    }
}
