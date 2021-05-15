use crate::statistic::algorithm::WelfordOnline;

/// Representation of a dataset using measures of dispersion - range, variance & standard deviation.
#[derive(Debug, Clone, PartialOrd, PartialEq)]
pub struct Dispersion {
    pub range: Range,
    pub recurrence_relation_m: f64,
    pub variance: f64,
    pub std_dev: f64
}

impl Default for Dispersion {
    fn default() -> Self {
        Self {
            range: Range::default(),
            recurrence_relation_m: 0.0,
            variance: 0.0,
            std_dev: 0.0
        }
    }
}

impl Dispersion {
    /// Iteratively updates the measures of Dispersion given the previous mean, new mean, new value,
    /// and the dataset count.
    pub fn update(&mut self, prev_mean: f64, new_mean: f64, new_value: f64, value_count: usize) {
        // Update Range
        self.range.update(new_value);

        // Update Welford Online recurrence relation M
        self.recurrence_relation_m = WelfordOnline::calculate_recurrence_relation_m(
            self.recurrence_relation_m, prev_mean, new_value, new_mean);

        // Update Population Variance
        self.variance = WelfordOnline::calculate_population_variance(
            self.recurrence_relation_m, value_count);

        // Update Standard Deviation
        self.std_dev = self.variance.sqrt();
    }
}

/// Measure of dispersion providing the highest and lowest value of a dataset. Lazy evaluation is
/// used when calculating the range between them via the calculate() function.
#[derive(Debug, Clone, PartialOrd, PartialEq)]
pub struct Range {
    pub activated: bool,
    pub highest: f64,
    pub lowest: f64,
}

impl Default for Range {
    fn default() -> Self {
        Self {
            activated: false,
            highest: 0.0,
            lowest: 0.0,
        }
    }
}

impl Range {
    /// Initialises the Range with the provided first value of the dataset.
    fn init(first_value: f64) -> Self {
        Self {
            activated: false,
            highest: first_value,
            lowest: first_value,
        }
    }

    /// Iteratively updates the Range given the next value in the dataset.
    fn update(&mut self, new_value: f64) {
        match self.activated {
            true => {
                if new_value > self.highest {
                    self.highest = new_value;
                }

                if new_value < self.lowest {
                    self.lowest = new_value;
                }
            }
            false => {
                self.activated = true;
                self.highest = new_value;
                self.lowest = new_value;
            }
        }
    }

    /// Calculates the range between the highest and lowest value of a dataset. Provided to
    /// allow lazy evaluation.
    fn calculate(&self) -> f64 {
        self.highest - self.lowest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispersion() {
        let mut dispersion = Dispersion::default();

        // Dataset  = [1.1, 1.2, 1.3, 1.4, 0.6]
        // Means    = [1.1, 1.15, 1.2, 1.25, 1.12]
        // Inputs:
        struct UpdateInput { prev_mean: f64, new_mean: f64, new_value: f64, value_count: usize }
        let input_1 = UpdateInput{ prev_mean: 0.0, new_mean: 1.1, new_value: 1.1, value_count: 1 };
        let input_2 = UpdateInput{ prev_mean: 1.1, new_mean: 1.15, new_value: 1.2, value_count: 2 };
        let input_3 = UpdateInput{ prev_mean: 1.15, new_mean: 1.2, new_value: 1.3, value_count: 3 };
        let input_4 = UpdateInput{ prev_mean: 1.2, new_mean: 1.25, new_value: 1.4, value_count: 4 };
        let input_5 = UpdateInput{ prev_mean: 1.25, new_mean: 1.12, new_value: 0.6, value_count: 5 };
        let inputs = vec![input_1, input_2, input_3, input_4, input_5];

        // Expected Outputs:
        // Recurrence_M = [0.0, 0.005, ~0.02, ~0.05, 0.388]
        // Variance     = [0.0, 0.0025, ~1/150, ~0.0125, 0.0776]
        // Std. Dev     = [0.0, 0.05, ~(6.sqrt()/30), ~(5.sqrt()/20), ~(194.sqrt()/50)]
        let output_1 = Dispersion{ range: Range { activated: true, highest: 1.1, lowest: 1.1 },
            recurrence_relation_m: 0.0, variance: 0.0, std_dev: 0.0 };

        let output_2 = Dispersion{ range: Range { activated: true, highest: 1.2, lowest: 1.1 },
            recurrence_relation_m: 0.005, variance: 0.0025, std_dev: 0.05 };

        let output_3 = Dispersion{ range: Range { activated: true, highest: 1.3, lowest: 1.1 },
            recurrence_relation_m: 0.02, variance: 1.0/150.0, std_dev: (6.0_f64.sqrt()/30.0) };

        let output_4 = Dispersion{ range: Range { activated: true, highest: 1.4, lowest: 1.1 },
            recurrence_relation_m: 0.05, variance: 0.0125, std_dev: (5.0_f64.sqrt()/20.0) };

        let output_5 = Dispersion{ range: Range { activated: true, highest: 1.4, lowest: 0.6 },
            recurrence_relation_m: 0.388, variance: 0.0776, std_dev: (194.0_f64.sqrt()/50.0) };

        let outputs = vec![output_1, output_2, output_3, output_4, output_5];

        for (input, out) in inputs.into_iter().zip(outputs.into_iter()) {
            dispersion.update(input.prev_mean, input.new_mean, input.new_value, input.value_count);

            // Range
            assert_eq!(dispersion.range.activated, out.range.activated);
            assert_eq!(dispersion.range.highest, out.range.highest);
            assert_eq!(dispersion.range.lowest, out.range.lowest);

            // Floating Point Comparisons
            let recurrence_diff = dispersion.recurrence_relation_m - out.recurrence_relation_m;
            assert!(recurrence_diff < 1e-10);

            let variance_diff = dispersion.variance - out.variance;
            assert!(variance_diff < 1e-10);

            let standard_dev_diff = dispersion.std_dev - out.std_dev;
            assert!(standard_dev_diff < 1e-10);
        }
    }

    #[test]
    fn range() {
        let dataset  = [0.1, 1.01, 1.02, 1.03, 1.04, 1.05, 1.06, 1.07, 9999.0];
        let mut actual_range = Range::default();

        for &value in &dataset {
            actual_range.update(value);
        }

        let expected_range = Range {
            activated: true,
            highest: 9999.0,
            lowest: 0.1,
        };

        assert_eq!(actual_range, expected_range);
        assert_eq!(actual_range.calculate(), 9998.9);
    }
}