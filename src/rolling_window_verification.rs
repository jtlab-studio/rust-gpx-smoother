/// Rolling Window Verification Module
/// Applies 48-point rolling mean smoothing to ALL routes regardless of elevation gain/km
/// This isolates the smoothing effect to verify if it's causing the extreme reductions

#[derive(Debug, Clone)]
pub struct RollingWindowData {
    pub original_elevations: Vec<f64>,
    pub cumulative_distances: Vec<f64>,
    pub distance_changes: Vec<f64>,
    pub altitude_changes: Vec<f64>,
    pub smoothed_altitude_changes: Vec<f64>,
    pub reconstructed_elevations: Vec<f64>,
    pub original_elevation_gain: f64,
    pub smoothed_elevation_gain: f64,
    pub elevation_gain_per_km: f64,
}

impl RollingWindowData {
    pub fn new(elevations: Vec<f64>, distances: Vec<f64>) -> Self {
        let mut data = RollingWindowData {
            original_elevations: elevations.clone(),
            cumulative_distances: distances.clone(),
            distance_changes: vec![],
            altitude_changes: vec![],
            smoothed_altitude_changes: vec![],
            reconstructed_elevations: vec![],
            original_elevation_gain: 0.0,
            smoothed_elevation_gain: 0.0,
            elevation_gain_per_km: 0.0,
        };
        
        data.process_with_rolling_window();
        data
    }
    
    fn calculate_distance_changes(&mut self) {
        if self.cumulative_distances.is_empty() {
            return;
        }
        
        // First value is the first cumulative distance itself
        self.distance_changes.push(self.cumulative_distances[0]);
        
        for i in 1..self.cumulative_distances.len() {
            self.distance_changes.push(
                self.cumulative_distances[i] - self.cumulative_distances[i - 1]
            );
        }
    }
    
    fn calculate_altitude_changes(&mut self) {
        if self.original_elevations.is_empty() {
            return;
        }
        
        // First entry has no previous value to compare
        self.altitude_changes.push(0.0);
        
        for i in 1..self.original_elevations.len() {
            self.altitude_changes.push(
                self.original_elevations[i] - self.original_elevations[i - 1]
            );
        }
    }
    
    fn calculate_elevation_gain(elevations: &[f64]) -> f64 {
        let mut gain = 0.0;
        for w in elevations.windows(2) {
            let delta = w[1] - w[0];
            if delta > 0.0 {
                gain += delta;
            }
        }
        gain
    }
    
    fn calculate_elevation_gain_from_changes(altitude_changes: &[f64]) -> f64 {
        altitude_changes.iter()
            .filter(|&&change| change > 0.0)
            .sum()
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
    
    fn reconstruct_elevations_from_changes(&mut self) {
        if self.smoothed_altitude_changes.is_empty() {
            return;
        }
        
        self.reconstructed_elevations.clear();
        self.reconstructed_elevations.push(self.original_elevations[0]); // Start with original first elevation
        
        for i in 1..self.smoothed_altitude_changes.len() {
            let prev_elevation = self.reconstructed_elevations[i - 1];
            self.reconstructed_elevations.push(prev_elevation + self.smoothed_altitude_changes[i]);
        }
    }
    
    fn process_with_rolling_window(&mut self) {
        println!("=== ROLLING WINDOW VERIFICATION ===");
        
        // Step 1: Calculate distance changes
        self.calculate_distance_changes();
        
        // Step 2: Calculate altitude changes from original elevations
        self.calculate_altitude_changes();
        
        // Step 3: Calculate original elevation gain
        self.original_elevation_gain = Self::calculate_elevation_gain(&self.original_elevations);
        
        // Step 4: Calculate elevation gain per km
        let total_distance_km = self.cumulative_distances.last().unwrap_or(&0.0) / 1000.0;
        self.elevation_gain_per_km = if total_distance_km > 0.0 {
            self.original_elevation_gain / total_distance_km
        } else {
            0.0
        };
        
        println!("Original elevation gain: {:.1}m", self.original_elevation_gain);
        println!("Elevation gain per km: {:.2}m/km", self.elevation_gain_per_km);
        println!("Total distance: {:.1}km", total_distance_km);
        
        // Step 5: Apply 48-point rolling mean to altitude changes (INDISCRIMINATELY)
        println!("Applying 48-point rolling mean to altitude changes...");
        self.smoothed_altitude_changes = Self::rolling_mean(&self.altitude_changes, 48);
        
        // Step 6: Calculate elevation gain from smoothed altitude changes
        let smoothed_gain_from_changes = Self::calculate_elevation_gain_from_changes(&self.smoothed_altitude_changes);
        println!("Elevation gain from smoothed changes: {:.1}m", smoothed_gain_from_changes);
        
        // Step 7: Reconstruct elevations from smoothed altitude changes
        self.reconstruct_elevations_from_changes();
        
        // Step 8: Calculate final elevation gain from reconstructed elevations
        self.smoothed_elevation_gain = Self::calculate_elevation_gain(&self.reconstructed_elevations);
        
        println!("Final smoothed elevation gain: {:.1}m", self.smoothed_elevation_gain);
        println!("Reduction: {:.1}m → {:.1}m ({:.1}% reduction)", 
                 self.original_elevation_gain, 
                 self.smoothed_elevation_gain,
                 (1.0 - self.smoothed_elevation_gain / self.original_elevation_gain) * 100.0);
        
        // Step 9: Analysis
        if self.elevation_gain_per_km < 20.0 {
            println!("Route classification: FLAT (<20m/km) - this should get smoothing only");
        } else {
            println!("Route classification: HILLY (>=20m/km) - this should get smoothing + capping");
        }
        
        println!("=== ROLLING WINDOW VERIFICATION COMPLETE ===");
    }
    
    pub fn get_original_elevation_gain(&self) -> f64 {
        self.original_elevation_gain
    }
    
    pub fn get_smoothed_elevation_gain(&self) -> f64 {
        self.smoothed_elevation_gain
    }
    
    pub fn get_elevation_gain_per_km(&self) -> f64 {
        self.elevation_gain_per_km
    }
    
    pub fn get_reduction_percentage(&self) -> f64 {
        if self.original_elevation_gain > 0.0 {
            (1.0 - self.smoothed_elevation_gain / self.original_elevation_gain) * 100.0
        } else {
            0.0
        }
    }
}

/// Apply 48-point rolling window smoothing to any route regardless of m/km
pub fn rolling_window_smooth_indiscriminate(elevations: &[f64], distances: &[f64]) -> Vec<f64> {
    let data = RollingWindowData::new(elevations.to_vec(), distances.to_vec());
    data.reconstructed_elevations
}

/// Get elevation gain after applying 48-point rolling window smoothing
pub fn rolling_window_elevation_gain(elevations: &[f64], distances: &[f64]) -> f64 {
    let data = RollingWindowData::new(elevations.to_vec(), distances.to_vec());
    data.get_smoothed_elevation_gain()
}

/// Analyze the impact of 48-point rolling window smoothing
pub fn analyze_rolling_window_impact(elevations: &[f64], distances: &[f64]) -> RollingWindowData {
    RollingWindowData::new(elevations.to_vec(), distances.to_vec())
}

pub fn calculate_rolling_window_elevation_gain_loss(elevations: &[f64]) -> (f64, f64) {
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for w in elevations.windows(2) {
        let delta = w[1] - w[0];
        if delta > 0.0 {
            gain += delta;
        } else {
            loss += -delta;
        }
    }
    
    (gain, loss)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rolling_window_basic() {
        let elevations = vec![100.0, 102.0, 105.0, 103.0, 107.0, 110.0, 108.0, 112.0];
        let distances = vec![0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0];
        
        let analysis = analyze_rolling_window_impact(&elevations, &distances);
        
        assert!(analysis.get_original_elevation_gain() > 0.0);
        assert!(analysis.get_smoothed_elevation_gain() >= 0.0);
        assert!(analysis.get_smoothed_elevation_gain() <= analysis.get_original_elevation_gain());
    }
    
    #[test]
    fn test_rolling_window_flat_route() {
        // Simulate a flat route with small elevation changes
        let elevations = vec![50.0, 52.0, 51.0, 53.0, 52.0, 54.0, 53.0, 55.0];
        let distances = vec![0.0, 5000.0, 10000.0, 15000.0, 20000.0, 25000.0, 30000.0, 35000.0]; // 35km
        
        let analysis = analyze_rolling_window_impact(&elevations, &distances);
        
        // Should be classified as flat
        assert!(analysis.get_elevation_gain_per_km() < 20.0);
        
        // Smoothing should reduce gain but not eliminate it
        let reduction = analysis.get_reduction_percentage();
        assert!(reduction >= 0.0 && reduction < 90.0); // Should not reduce by more than 90%
    }
    
    #[test]
    fn test_rolling_mean_function() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let smoothed = RollingWindowData::rolling_mean(&data, 3);
        
        assert_eq!(smoothed.len(), data.len());
        // First element should be just itself (window size 1)
        assert_eq!(smoothed[0], 1.0);
        // Second element should be average of first two
        assert_eq!(smoothed[1], 1.5);
        // Third element should be average of first three
        assert_eq!(smoothed[2], 2.0);
    }
}

