#[derive(Debug, Clone)]
pub struct ElevationData {
    pub enhanced_altitude: Vec<f64>,
    pub cumulative_distance: Vec<f64>,
    pub distance_change: Vec<f64>,
    pub altitude_change: Vec<f64>,
    pub gradient_percent: Vec<f64>,
    pub accumulated_ascent: Vec<f64>,
    pub accumulated_descent: Vec<f64>,
    pub ascent: Vec<f64>,
    pub descent: Vec<f64>,
    pub overall_uphill_gradient: f64,
    pub overall_downhill_gradient: f64,
}

impl ElevationData {
    pub fn new(enhanced_altitude: Vec<f64>, cumulative_distance: Vec<f64>) -> Self {
        let mut data = ElevationData {
            enhanced_altitude,
            cumulative_distance,
            distance_change: vec![],
            altitude_change: vec![],
            gradient_percent: vec![],
            accumulated_ascent: vec![],
            accumulated_descent: vec![],
            ascent: vec![],
            descent: vec![],
            overall_uphill_gradient: 0.0,
            overall_downhill_gradient: 0.0,
        };
        
        // Calculate distance changes
        data.calculate_distance_changes();
        
        // Process elevation data with smoothing
        data.process_elevation_data();
        
        data
    }
    
    fn calculate_distance_changes(&mut self) {
        if self.cumulative_distance.is_empty() {
            return;
        }
        
        // First value is the first cumulative distance itself
        self.distance_change.push(self.cumulative_distance[0]);
        
        for i in 1..self.cumulative_distance.len() {
            self.distance_change.push(
                self.cumulative_distance[i] - self.cumulative_distance[i - 1]
            );
        }
    }
    
    fn calculate_altitude_changes(&mut self) {
        if self.enhanced_altitude.is_empty() {
            return;
        }
        
        // First entry has no previous value to compare
        self.altitude_change.push(0.0);
        
        for i in 1..self.enhanced_altitude.len() {
            self.altitude_change.push(
                self.enhanced_altitude[i] - self.enhanced_altitude[i - 1]
            );
        }
    }
    
    fn calculate_accumulated_ascent_descent(&mut self) {
        self.accumulated_ascent.push(0.0);
        self.accumulated_descent.push(0.0);
        
        for i in 1..self.enhanced_altitude.len() {
            let altitude_diff = self.enhanced_altitude[i] - self.enhanced_altitude[i - 1];
            
            if altitude_diff > 0.0 {
                // Ascending
                self.accumulated_ascent.push(
                    self.accumulated_ascent.last().unwrap() + altitude_diff
                );
                self.accumulated_descent.push(*self.accumulated_descent.last().unwrap());
            } else if altitude_diff < 0.0 {
                // Descending
                self.accumulated_descent.push(
                    self.accumulated_descent.last().unwrap() - altitude_diff
                );
                self.accumulated_ascent.push(*self.accumulated_ascent.last().unwrap());
            } else {
                // No change
                self.accumulated_ascent.push(*self.accumulated_ascent.last().unwrap());
                self.accumulated_descent.push(*self.accumulated_descent.last().unwrap());
            }
        }
    }
    
    fn calculate_gradients(&mut self) {
        self.gradient_percent.clear();
        
        for i in 0..self.altitude_change.len() {
            if self.distance_change[i] == 0.0 {
                self.gradient_percent.push(0.0);
            } else {
                self.gradient_percent.push(
                    (self.altitude_change[i] / self.distance_change[i]) * 100.0
                );
            }
        }
    }
    
    fn calculate_overall_gradients(&mut self) {
        let total_distance_km = self.cumulative_distance.last().unwrap_or(&0.0) / 1000.0;
        
        if total_distance_km > 0.0 {
            self.overall_uphill_gradient = self.accumulated_ascent.last().unwrap_or(&0.0) / total_distance_km;
            self.overall_downhill_gradient = self.accumulated_descent.last().unwrap_or(&0.0) / total_distance_km;
        }
    }
    
    fn rolling_mean(data: &[f64], window: usize) -> Vec<f64> {
        let mut result = vec![];
        
        for i in 0..data.len() {
            let start = if i >= window { i - window + 1 } else { 0 };
            let end = i + 1;
            
            let sum: f64 = data[start..end].iter().sum();
            let count = end - start;
            
            result.push(sum / count as f64);
        }
        
        result
    }
    
    fn apply_gradient_smoothing(&mut self) {
        let hilliness_ratio = self.overall_uphill_gradient;
        
        // Apply rolling mean smoothing only if hilliness ratio is below 20m/km
        if hilliness_ratio < 20.0 {
            self.altitude_change = Self::rolling_mean(&self.altitude_change, 83);
            self.calculate_gradients();
        }
    }
    
    fn apply_gradient_capping(&mut self) {
        let hilliness_ratio = self.overall_uphill_gradient;
        
        // Define thresholds: (limit, max positive gradient, max negative gradient)
        let thresholds = vec![
            (20.0, 15.0, 12.0),
            (30.0, 20.0, 15.0),
            (40.0, 25.0, 20.0),
            (50.0, 32.0, 27.0),
            (60.0, 35.0, 31.0),
            (f64::INFINITY, 40.0, 36.0),
        ];
        
        // Apply capping for hillier routes (20m/km and above)
        for (limit, max_up, max_down) in thresholds {
            if hilliness_ratio < limit {
                for i in 0..self.gradient_percent.len() {
                    if self.gradient_percent[i] > max_up {
                        self.altitude_change[i] = max_up * self.distance_change[i] / 100.0;
                    } else if self.gradient_percent[i] < -max_down {
                        self.altitude_change[i] = -max_down * self.distance_change[i] / 100.0;
                    }
                }
                break;
            }
        }
        
        // Recalculate gradients after capping
        self.calculate_gradients();
    }
    
    fn separate_ascent_descent(&mut self) {
        self.ascent.clear();
        self.descent.clear();
        
        for &alt_change in &self.altitude_change {
            if alt_change > 0.0 {
                self.ascent.push(alt_change);
                self.descent.push(0.0);
            } else if alt_change < 0.0 {
                self.ascent.push(0.0);
                self.descent.push(alt_change);
            } else {
                self.ascent.push(0.0);
                self.descent.push(0.0);
            }
        }
    }
    
    fn recalculate_accumulated_values(&mut self) {
        // Clear existing values
        self.accumulated_ascent.clear();
        self.accumulated_descent.clear();
        
        // Recalculate based on smoothed ascent/descent
        let mut ascent_acc = 0.0;
        let mut descent_acc = 0.0;
        
        for i in 0..self.ascent.len() {
            ascent_acc += self.ascent[i];
            descent_acc += self.descent[i].abs();
            
            self.accumulated_ascent.push(ascent_acc);
            self.accumulated_descent.push(descent_acc);
        }
    }
    
    pub fn process_elevation_data(&mut self) {
        // Step 1: Calculate initial altitude changes
        self.calculate_altitude_changes();
        
        // Step 2: Calculate accumulated ascent and descent
        self.calculate_accumulated_ascent_descent();
        
        // Step 3: Calculate initial gradients
        self.calculate_gradients();
        
        // Step 4: Calculate overall gradients
        self.calculate_overall_gradients();
        
        // Step 5: Apply smoothing if applicable
        self.apply_gradient_smoothing();
        
        // Step 6: Apply gradient capping based on terrain type
        self.apply_gradient_capping();
        
        // Step 7: Separate into ascent and descent
        self.separate_ascent_descent();
        
        // Step 8: Recalculate accumulated values based on smoothed data
        self.recalculate_accumulated_values();
        
        // Step 9: Recalculate overall gradients with smoothed values
        let total_distance_km = self.cumulative_distance.last().unwrap_or(&0.0) / 1000.0;
        if total_distance_km > 0.0 {
            self.overall_uphill_gradient = self.accumulated_ascent.last().unwrap_or(&0.0) / total_distance_km;
            self.overall_downhill_gradient = self.accumulated_descent.last().unwrap_or(&0.0) / total_distance_km;
        }
    }
    
    pub fn get_total_elevation_gain(&self) -> f64 {
        self.accumulated_ascent.last().unwrap_or(&0.0).clone()
    }
    
    pub fn get_total_elevation_loss(&self) -> f64 {
        self.accumulated_descent.last().unwrap_or(&0.0).clone()
    }
    
    // Additional analysis functions
    pub fn get_gradient_rolling_average(&self, window_seconds: usize) -> Vec<f64> {
        Self::rolling_mean(&self.gradient_percent, window_seconds)
    }
    
    pub fn classify_gradient(&self, gradient: f64) -> &'static str {
        match gradient {
            g if (-2.0..=2.0).contains(&g) => "Flat",
            g if (-7.0..-2.0).contains(&g) => "Slight downhill",
            g if (-12.0..-7.0).contains(&g) => "Downhill",
            g if (-17.0..-12.0).contains(&g) => "Quite downhill",
            g if (-22.0..-17.0).contains(&g) => "Dangerously downhill",
            g if g < -22.0 => "Extreme downhill",
            g if (2.0..7.0).contains(&g) => "Slight uphill",
            g if (7.0..12.0).contains(&g) => "Uphill",
            g if (12.0..17.0).contains(&g) => "Quite uphill",
            g if (17.0..22.0).contains(&g) => "Kilian uphill",
            g if g >= 22.0 => "Walking uphill",
            _ => "Unknown",
        }
    }
    
    pub fn get_gradient_classifications(&self) -> Vec<&'static str> {
        self.gradient_percent.iter()
            .map(|&g| self.classify_gradient(g))
            .collect()
    }
}
