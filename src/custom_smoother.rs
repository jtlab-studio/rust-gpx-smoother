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
    pub data_quality_issues: Vec<String>,
}

/// Smoothing variant type
#[derive(Debug, Clone, Copy)]
pub enum SmoothingVariant {
    Original,
    Capping,
    Flat21,
    PostCap,
    DistBased,
    SymmetricFixed,
    AdaptiveQuality,
}

#[derive(Debug)]
enum DataQuality {
    Good,
    ArtificialInflation,
    SevereCorruption,
}

impl ElevationData {
    pub fn new(enhanced_altitude: Vec<f64>, cumulative_distance: Vec<f64>) -> Self {
        Self::new_with_variant(enhanced_altitude, cumulative_distance, SmoothingVariant::Original)
    }
    
    pub fn new_with_variant(
        enhanced_altitude: Vec<f64>, 
        cumulative_distance: Vec<f64>,
        variant: SmoothingVariant
    ) -> Self {
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
            data_quality_issues: vec![],
        };
        
        data.calculate_distance_changes();
        data.process_elevation_data_with_variant(variant);
        
        data
    }
    
    fn calculate_distance_changes(&mut self) {
        if self.cumulative_distance.is_empty() {
            println!("      [DEBUG] No cumulative distances to process");
            return;
        }
        
        // Debug info
        let total_distance = self.cumulative_distance.last().unwrap_or(&0.0);
        println!("      [DEBUG] Total distance: {:.1}m, Points: {}", total_distance, self.cumulative_distance.len());
        
        // Check for potential issues
        if *total_distance < 0.1 {
            println!("      [WARNING] Very small total distance: {:.3}m", total_distance);
        }
        
        // ULTRA-SAFE: Use .get() for first element
        if let Some(&first_distance) = self.cumulative_distance.get(0) {
            self.distance_change.push(first_distance);
        } else {
            return; // Safety exit if can't access first element
        }
        
        for i in 1..self.cumulative_distance.len() {
            // ULTRA-SAFE: Use .get() instead of direct indexing
            if let (Some(&current), Some(&previous)) = (
                self.cumulative_distance.get(i), 
                self.cumulative_distance.get(i - 1)
            ) {
                let change = current - previous;
                if change < 0.0 {
                    println!("      [WARNING] Negative distance change at index {}: {:.3}m", i, change);
                }
                self.distance_change.push(change);
            } else {
                // Safety fallback
                self.distance_change.push(0.0);
            }
        }
        
        // Check for unusual patterns
        let zero_changes = self.distance_change.iter().filter(|&&d| d == 0.0).count();
        if zero_changes > self.distance_change.len() / 2 {
            println!("      [WARNING] {} zero distance changes out of {}", zero_changes, self.distance_change.len());
        }
    }
    
    pub fn calculate_altitude_changes(&mut self) {
        if self.enhanced_altitude.is_empty() {
            return;
        }
        
        self.altitude_change.push(0.0);
        
        for i in 1..self.enhanced_altitude.len() {
            // ULTRA-SAFE: Use .get() instead of direct indexing
            if let (Some(&current), Some(&previous)) = (
                self.enhanced_altitude.get(i), 
                self.enhanced_altitude.get(i - 1)
            ) {
                self.altitude_change.push(current - previous);
            } else {
                // Safety fallback
                self.altitude_change.push(0.0);
            }
        }
    }
    
    fn calculate_raw_gain_loss(&self) -> (f64, f64) {
        let mut gain = 0.0;
        let mut loss = 0.0;
        
        for &change in &self.altitude_change {
            if change > 0.0 {
                gain += change;
            } else if change < 0.0 {
                loss += -change;
            }
        }
        
        (gain, loss)
    }
    
    fn calculate_accumulated_ascent_descent(&mut self) {
        self.accumulated_ascent.clear();
        self.accumulated_descent.clear();
        
        let mut ascent_acc = 0.0;
        let mut descent_acc = 0.0;
        
        self.accumulated_ascent.push(0.0);
        self.accumulated_descent.push(0.0);
        
        for i in 1..self.enhanced_altitude.len() {
            // ULTRA-SAFE: Use .get() instead of direct indexing
            if let (Some(&current), Some(&previous)) = (
                self.enhanced_altitude.get(i), 
                self.enhanced_altitude.get(i - 1)
            ) {
                let altitude_diff = current - previous;
                
                if altitude_diff > 0.0 {
                    ascent_acc += altitude_diff;
                    self.accumulated_ascent.push(ascent_acc);
                    self.accumulated_descent.push(descent_acc);
                } else if altitude_diff < 0.0 {
                    descent_acc += -altitude_diff;
                    self.accumulated_descent.push(descent_acc);
                    self.accumulated_ascent.push(ascent_acc);
                } else {
                    self.accumulated_ascent.push(ascent_acc);
                    self.accumulated_descent.push(descent_acc);
                }
            } else {
                // Safety fallback
                self.accumulated_ascent.push(ascent_acc);
                self.accumulated_descent.push(descent_acc);
            }
        }
    }
    
    fn calculate_gradients(&mut self) {
        self.gradient_percent.clear();
        
        // Ensure both vectors have the same length
        let min_len = std::cmp::min(self.altitude_change.len(), self.distance_change.len());
        
        for i in 0..min_len {
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
    
    pub fn rolling_mean(data: &[f64], window: usize) -> Vec<f64> {
        let mut result = vec![];
        
        if data.is_empty() {
            return result;
        }
        
        for i in 0..data.len() {
            let start = if i >= window { i - window + 1 } else { 0 };
            let end = std::cmp::min(i + 1, data.len());
            
            // Ultra-safe bounds checking with .get()
            if start < data.len() && end <= data.len() && start < end {
                let sum: f64 = data.get(start..end)
                    .map(|slice| slice.iter().sum())
                    .unwrap_or(0.0);
                let count = end - start;
                result.push(if count > 0 { sum / count as f64 } else { 0.0 });
            } else {
                // Ultra-safe fallback
                result.push(data.get(i).copied().unwrap_or(0.0));
            }
        }
        
        result
    }
    
    // FIXED: Only applies correction for ratios > 1.1 - preserves good results for balanced files
    pub fn process_elevation_data_adaptive(&mut self) {
        println!("🔍 ADAPTIVE QUALITY-BASED PROCESSING");
        
        // Step 1: Calculate initial altitude changes
        self.calculate_altitude_changes();
        
        // Step 2: Calculate raw gain/loss for quality assessment
        let (raw_gain, raw_loss) = self.calculate_raw_gain_loss();
        
        // Step 3: Detect data quality issues
        let gain_loss_ratio = if raw_loss > 0.0 { raw_gain / raw_loss } else { f64::INFINITY };
        
        println!("   Raw gain: {:.1}m, Raw loss: {:.1}m", raw_gain, raw_loss);
        println!("   Gain/Loss ratio: {:.3}", gain_loss_ratio);
        
        // FIXED: Only apply adaptive correction for ratios > 1.1
        if gain_loss_ratio <= 1.1 {
            println!("   ✅ EXCELLENT RATIO (≤ 1.1) - Using standard 1.9m symmetric processing");
            println!("      No adaptive correction needed - preserving natural elevation profile");
            
            // Use standard symmetric processing with 1.9m interval (which was working great!)
            self.apply_standard_symmetric_processing();
            return;
        }
        
        // For ratios > 1.1, assess severity and apply appropriate correction
        let data_quality = self.assess_data_quality_conservative(raw_gain, raw_loss, gain_loss_ratio);
        println!("   Quality assessment: {:?}", data_quality);
        
        // Step 4: Apply processing based on detected quality
        match data_quality {
            DataQuality::ArtificialInflation => {
                println!("   🔧 MILD INFLATION (ratio 1.1-1.5) - Applying gentle correction");
                self.apply_gentle_inflation_correction(raw_gain, raw_loss);
            },
            DataQuality::SevereCorruption => {
                println!("   🚨 SEVERE CORRUPTION (ratio > 1.5) - Applying moderate correction");
                self.apply_moderate_correction(raw_gain, raw_loss);
            },
            DataQuality::Good => {
                println!("   ✅ Using standard processing");
                self.apply_standard_symmetric_processing();
            }
        }
        
        // Step 5: Recalculate accumulated values from processed altitude_change
        self.recalculate_accumulated_values_from_altitude_changes();
        
        let (final_gain, final_loss) = (
            self.accumulated_ascent.last().unwrap_or(&0.0).clone(),
            self.accumulated_descent.last().unwrap_or(&0.0).clone()
        );
        let final_ratio = if final_loss > 0.0 { final_gain / final_loss } else { f64::INFINITY };
        
        println!("   📊 PROCESSING RESULTS:");
        println!("      Final gain: {:.1}m (was {:.1}m)", final_gain, raw_gain);
        println!("      Final loss: {:.1}m (was {:.1}m)", final_loss, raw_loss);
        println!("      Final ratio: {:.3} (was {:.3})", final_ratio, gain_loss_ratio);
        
        if final_ratio > 1.2 {
            println!("      ⚠️  Still imbalanced - may need stronger correction");
            self.data_quality_issues.push("Persistent gain/loss imbalance after processing".to_string());
        } else if final_ratio >= 0.8 && final_ratio <= 1.2 {
            println!("      ✅ Balanced ratio achieved!");
        }
    }
    
    // NEW: Standard symmetric processing for good files (ratio ≤ 1.1)
    fn apply_standard_symmetric_processing(&mut self) {
        // This is what was working great before - don't mess with it!
        
        println!("      [DEBUG] Applying standard symmetric processing...");
        
        // Step 1: Apply the proven 1.9m symmetric processing
        self.apply_custom_interval_processing_symmetric(1.9);
        
        // Step 2: Calculate final metrics
        self.calculate_gradients();
        self.separate_ascent_descent();
        self.recalculate_accumulated_values();
        self.calculate_overall_gradients();
        
        // Clear any quality issues since this is good data
        self.data_quality_issues.clear();
        self.data_quality_issues.push("Good quality data".to_string());
        
        println!("      [DEBUG] Standard processing complete!");
    }
    
    // FIXED: More conservative assessment - only flag truly problematic files
    fn assess_data_quality_conservative(&mut self, _raw_gain: f64, raw_loss: f64, ratio: f64) -> DataQuality {
        // Clear previous quality issues
        self.data_quality_issues.clear();
        
        if ratio.is_infinite() || raw_loss < 10.0 {
            self.data_quality_issues.push("No meaningful loss data".to_string());
            return DataQuality::SevereCorruption;
        }
        
        // FIXED: More conservative thresholds
        if ratio > 1.5 {
            if ratio > 3.0 {
                self.data_quality_issues.push(format!("Severe gain inflation: {:.1}x expected", ratio));
            } else {
                self.data_quality_issues.push(format!("Moderate gain inflation: {:.1}x expected", ratio));
            }
            return DataQuality::SevereCorruption;
        }
        
        if ratio > 1.1 {
            self.data_quality_issues.push(format!("Mild elevation inflation detected: {:.1}x ratio", ratio));
            
            if self.detect_artificial_patterns() {
                self.data_quality_issues.push("Artificial elevation patterns detected".to_string());
            }
            
            return DataQuality::ArtificialInflation;
        }
        
        DataQuality::Good
    }
    
    fn detect_artificial_patterns(&self) -> bool {
        let mut extreme_gradients = 0;
        let mut total_segments = 0;
        
        let min_len = std::cmp::min(self.altitude_change.len(), self.distance_change.len());
        
        for i in 0..min_len {
            if self.distance_change[i] > 0.0 {
                let gradient = (self.altitude_change[i] / self.distance_change[i]) * 100.0;
                total_segments += 1;
                
                if gradient.abs() > 30.0 {
                    extreme_gradients += 1;
                }
            }
        }
        
        total_segments > 0 && (extreme_gradients as f64 / total_segments as f64) > 0.05
    }
    
    // NEW: Gentle correction for mild inflation (ratio 1.1-1.5)
    fn apply_gentle_inflation_correction(&mut self, _raw_gain: f64, _raw_loss: f64) {
        println!("   🔧 Applying GENTLE correction for mild inflation...");
        
        println!("      📊 Applying light 30-point smoothing...");
        self.altitude_change = Self::rolling_mean(&self.altitude_change, 30);
        
        println!("      ✂️  Applying conservative gradient capping (max 30%)...");
        self.apply_strict_gradient_capping(30.0);
        
        println!("      🚫 Applying small deadband filtering (2m threshold)...");
        self.apply_large_deadband_filtering(2.0);
        
        self.recalculate_accumulated_values_from_altitude_changes();
        
        let processed_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
        let processed_loss = self.accumulated_descent.last().unwrap_or(&0.0).clone();
        let new_ratio = if processed_loss > 0.0 { processed_gain / processed_loss } else { f64::INFINITY };
        
        if new_ratio > 2.0 && processed_loss > 100.0 {
            println!("      ⚖️  Applying very gentle scaling (still quite imbalanced)...");
            self.scale_gain_to_realistic_ratio(processed_loss, 1.3);
            self.recalculate_accumulated_values_from_altitude_changes();
        }
    }
    
    fn apply_moderate_correction(&mut self, _raw_gain: f64, _raw_loss: f64) {
        println!("   🔧 Applying MODERATE correction for significant corruption...");
        
        println!("      📊 Applying moderate 75-point smoothing...");
        self.altitude_change = Self::rolling_mean(&self.altitude_change, 75);
        
        println!("      ✂️  Applying moderate gradient capping (max 22%)...");
        self.apply_strict_gradient_capping(22.0);
        
        println!("      🚫 Applying moderate deadband filtering (4m threshold)...");
        self.apply_large_deadband_filtering(4.0);
        
        self.recalculate_accumulated_values_from_altitude_changes();
        
        let processed_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
        let processed_loss = self.accumulated_descent.last().unwrap_or(&0.0).clone();
        let new_ratio = if processed_loss > 0.0 { processed_gain / processed_loss } else { f64::INFINITY };
        
        if new_ratio > 1.8 && processed_loss > 100.0 {
            println!("      ⚖️  Applying moderate scaling to balance ratio...");
            self.scale_gain_to_realistic_ratio(processed_loss, 1.25);
            self.recalculate_accumulated_values_from_altitude_changes();
        }
    }
    
    fn apply_strict_gradient_capping(&mut self, max_gradient_percent: f64) {
        self.calculate_gradients();
        
        // FIXED: Ensure we don't exceed array bounds
        let min_len = self.gradient_percent.len()
            .min(self.distance_change.len())
            .min(self.altitude_change.len());
        
        for i in 0..min_len {
            if self.distance_change[i] > 0.0 {
                let capped_gradient = self.gradient_percent[i].max(-max_gradient_percent).min(max_gradient_percent);
                self.altitude_change[i] = capped_gradient * self.distance_change[i] / 100.0;
            }
        }
        
        self.calculate_gradients();
    }
    
    fn apply_large_deadband_filtering(&mut self, threshold_meters: f64) {
        let mut filtered_changes = vec![0.0];
        let mut cumulative_change = 0.0;
        
        // FIXED: Ensure we stay within bounds
        for i in 1..self.altitude_change.len() {
            if i < self.altitude_change.len() {  // Extra safety check
                cumulative_change += self.altitude_change[i];
                
                if cumulative_change.abs() >= threshold_meters {
                    filtered_changes.push(cumulative_change);
                    cumulative_change = 0.0;
                } else {
                    filtered_changes.push(0.0);
                }
            }
        }
        
        self.altitude_change = filtered_changes;
    }
    
    fn scale_gain_to_realistic_ratio(&mut self, target_loss: f64, target_ratio: f64) {
        let target_gain = target_loss * target_ratio;
        
        let current_gain: f64 = self.altitude_change.iter()
            .filter(|&&x| x > 0.0)
            .sum();
        
        if current_gain > 0.0 {
            let scale_factor = target_gain / current_gain;
            println!("         Scaling positive changes by factor: {:.3}", scale_factor);
            
            // FIXED: Use iterator to avoid bounds issues
            for change in self.altitude_change.iter_mut() {
                if *change > 0.0 {
                    *change *= scale_factor;
                }
            }
        }
    }
    
    pub fn recalculate_accumulated_values_from_altitude_changes(&mut self) {
        self.accumulated_ascent.clear();
        self.accumulated_descent.clear();
        
        let mut ascent_acc = 0.0;
        let mut descent_acc = 0.0;
        
        for &change in &self.altitude_change {
            if change > 0.0 {
                ascent_acc += change;
            } else if change < 0.0 {
                descent_acc += -change;
            }
            
            self.accumulated_ascent.push(ascent_acc);
            self.accumulated_descent.push(descent_acc);
        }
    }
    
    /// Linear interpolation to resample elevation data onto uniform distance grid
    fn resample_to_uniform_distance(&mut self, interval_meters: f64) -> (Vec<f64>, Vec<f64>) {
        if self.cumulative_distance.is_empty() || self.enhanced_altitude.is_empty() {
            return (vec![], vec![]);
        }
        
        let total_distance = self.cumulative_distance.last().unwrap();
        
        // Safety check: prevent creating too many points
        if interval_meters < 0.1 {
            println!("      [WARNING] Interval too small: {:.3}m, using minimum 0.1m", interval_meters);
            return self.resample_to_uniform_distance(0.1);
        }
        
        let num_points = (total_distance / interval_meters).ceil() as usize + 1;
        
        // Safety check: prevent excessive memory usage
        if num_points > 1_000_000 {
            println!("      [ERROR] Would create {} points! Total distance: {:.1}m, interval: {:.3}m", 
                     num_points, total_distance, interval_meters);
            println!("      [ERROR] Aborting resampling to prevent memory issues");
            return (vec![], vec![]);
        }
        
        println!("      [DEBUG] Resampling: {:.1}m total distance, {:.3}m interval = {} points", 
                 total_distance, interval_meters, num_points);
        
        let mut uniform_distances = Vec::with_capacity(num_points);
        let mut uniform_elevations = Vec::with_capacity(num_points);
        
        for i in 0..num_points {
            let target_distance = i as f64 * interval_meters;
            if target_distance > *total_distance {
                break;
            }
            uniform_distances.push(target_distance);
            
            let elevation = self.interpolate_elevation_at_distance(target_distance);
            uniform_elevations.push(elevation);
        }
        
        (uniform_distances, uniform_elevations)
    }
    
    fn interpolate_elevation_at_distance(&self, target_distance: f64) -> f64 {
        if target_distance <= 0.0 || self.enhanced_altitude.is_empty() {
            return self.enhanced_altitude.get(0).copied().unwrap_or(0.0);
        }
        
        // Check if we have valid cumulative distances
        if self.cumulative_distance.is_empty() {
            return 0.0;
        }
        
        // Check if target is beyond our data
        if let Some(&last_dist) = self.cumulative_distance.last() {
            if target_distance >= last_dist {
                return self.enhanced_altitude.last().copied().unwrap_or(0.0);
            }
        }
        
        for i in 1..self.cumulative_distance.len() {
            if i >= self.cumulative_distance.len() || i >= self.enhanced_altitude.len() {
                // Safety: return last known elevation
                return self.enhanced_altitude.last().copied().unwrap_or(0.0);
            }
            
            if self.cumulative_distance[i] >= target_distance {
                // Safety checks for array access
                if i == 0 || i - 1 >= self.cumulative_distance.len() || i - 1 >= self.enhanced_altitude.len() {
                    return self.enhanced_altitude.get(i).copied().unwrap_or(0.0);
                }
                
                let d1 = self.cumulative_distance[i - 1];
                let d2 = self.cumulative_distance[i];
                let e1 = self.enhanced_altitude[i - 1];
                let e2 = self.enhanced_altitude[i];
                
                if (d2 - d1).abs() < 1e-10 {
                    return e1;
                }
                
                let t = (target_distance - d1) / (d2 - d1);
                return e1 + t * (e2 - e1);
            }
        }
        
        self.enhanced_altitude.last().copied().unwrap_or(0.0)
    }
    
    pub fn apply_symmetric_deadband_filtering(&mut self, threshold_meters: f64) {
        if self.altitude_change.is_empty() {
            return;
        }
        
        let mut filtered_changes = vec![0.0];
        let mut cumulative_elevation_change = 0.0;
        
        for i in 1..self.altitude_change.len() {
            let change = self.altitude_change[i];
            cumulative_elevation_change += change;
            
            if cumulative_elevation_change.abs() >= threshold_meters {
                filtered_changes.push(cumulative_elevation_change);
                cumulative_elevation_change = 0.0;
            } else {
                filtered_changes.push(0.0);
            }
        }
        
        self.altitude_change = filtered_changes;
    }
    
    /// Custom interval processing for testing different distance intervals
    pub fn apply_custom_interval_processing(&mut self, interval_meters: f64) {
        // First calculate terrain type for adaptive parameters
        self.calculate_altitude_changes();
        self.calculate_accumulated_ascent_descent();
        self.calculate_overall_gradients();
        
        let hilliness_ratio = self.overall_uphill_gradient;
        
        // Determine adaptive parameters based on terrain and interval
        let (deadband_threshold, gaussian_window) = if hilliness_ratio < 20.0 {
            let deadband = match interval_meters as u32 {
                1 => 0.8, 3 => 1.0, 6 => 1.2, _ => 1.5,
            };
            let window = ((120.0 / interval_meters).round() as usize).max(5).min(50);
            (deadband, window)
        } else if hilliness_ratio < 40.0 {
            let deadband = match interval_meters as u32 {
                1 => 1.5, 3 => 1.8, 6 => 2.0, _ => 2.5,
            };
            let window = ((150.0 / interval_meters).round() as usize).max(5).min(30);
            (deadband, window)
        } else {
            let deadband = match interval_meters as u32 {
                1 => 2.0, 3 => 1.8, 6 => 1.5, _ => 2.0,
            };
            let window = ((100.0 / interval_meters).round() as usize).max(3).min(20);
            (deadband, window)
        };
        
        // Resample and process
        let (uniform_distances, uniform_elevations) = self.resample_to_uniform_distance(interval_meters);
        if uniform_elevations.is_empty() { return; }
        
        let median_smoothed = Self::median_filter(&uniform_elevations, 3);
        let gaussian_smoothed = Self::gaussian_smooth(&median_smoothed, gaussian_window);
        
        // Update data - SAFE VERSION
        let mut smoothed_altitude_changes = vec![0.0];
        if gaussian_smoothed.len() > 1 {
            for i in 1..gaussian_smoothed.len() {
                // Extra safety check
                if i < gaussian_smoothed.len() && (i - 1) < gaussian_smoothed.len() {
                    smoothed_altitude_changes.push(gaussian_smoothed[i] - gaussian_smoothed[i - 1]);
                } else {
                    smoothed_altitude_changes.push(0.0);
                }
            }
        }
        
        self.enhanced_altitude = gaussian_smoothed;
        self.cumulative_distance = uniform_distances;
        self.altitude_change = smoothed_altitude_changes;
        
        // Safe distance_change assignment
        self.distance_change = vec![interval_meters; self.altitude_change.len()];
        if !self.distance_change.is_empty() && !self.cumulative_distance.is_empty() {
            self.distance_change[0] = self.cumulative_distance[0];
        }
        
        // Apply deadband using legacy asymmetric method for backward compatibility
        self.apply_deadband_filtering(deadband_threshold);
        self.calculate_gradients();
        self.recalculate_accumulated_values_after_smoothing();
    }

    /// NEW: Custom interval processing with SYMMETRIC deadband (FIXED VERSION)
    pub fn apply_custom_interval_processing_symmetric(&mut self, interval_meters: f64) {
        println!("      [DEBUG] Starting symmetric processing with interval: {:.1}m", interval_meters);
        
        self.calculate_altitude_changes();
        self.calculate_accumulated_ascent_descent();
        self.calculate_overall_gradients();
        
        let hilliness_ratio = self.overall_uphill_gradient;
        println!("      [DEBUG] Hilliness ratio: {:.1}", hilliness_ratio);
        
        let (deadband_threshold, gaussian_window) = if hilliness_ratio < 20.0 {
            let deadband = match interval_meters as u32 {
                1 => 0.8, 3 => 1.0, 6 => 1.2, _ => 1.5,
            };
            let window = ((120.0 / interval_meters).round() as usize).max(5).min(50);
            (deadband, window)
        } else if hilliness_ratio < 40.0 {
            let deadband = match interval_meters as u32 {
                1 => 1.5, 3 => 1.8, 6 => 2.0, _ => 2.5,
            };
            let window = ((150.0 / interval_meters).round() as usize).max(5).min(30);
            (deadband, window)
        } else {
            let deadband = match interval_meters as u32 {
                1 => 2.0, 3 => 1.8, 6 => 1.5, _ => 2.0,
            };
            let window = ((100.0 / interval_meters).round() as usize).max(3).min(20);
            (deadband, window)
        };
        
        println!("      [DEBUG] Deadband: {:.1}m, Gaussian window: {}", deadband_threshold, gaussian_window);
        println!("      [DEBUG] Starting resampling...");
        
        let (uniform_distances, uniform_elevations) = self.resample_to_uniform_distance(interval_meters);
        if uniform_elevations.is_empty() { 
            println!("      [DEBUG] No elevations after resampling!");
            return; 
        }
        
        println!("      [DEBUG] Resampled to {} points", uniform_elevations.len());
        
        // Check if resampling created too many points
        if uniform_elevations.len() > 100000 {
            println!("      [WARNING] Too many points after resampling: {}", uniform_elevations.len());
            println!("      [WARNING] This may cause performance issues!");
        }
        
        println!("      [DEBUG] Applying median filter...");
        let median_smoothed = Self::median_filter(&uniform_elevations, 3);
        
        println!("      [DEBUG] Applying Gaussian smoothing...");
        let gaussian_smoothed = Self::gaussian_smooth(&median_smoothed, gaussian_window);
        
        println!("      [DEBUG] Creating altitude changes...");
        
        // SAFE VERSION: Update data with comprehensive bounds checking
        let mut smoothed_altitude_changes = vec![0.0];
        if gaussian_smoothed.len() > 1 {
            for i in 1..gaussian_smoothed.len() {
                // Triple check bounds
                if i < gaussian_smoothed.len() && i > 0 && (i - 1) < gaussian_smoothed.len() {
                    smoothed_altitude_changes.push(gaussian_smoothed[i] - gaussian_smoothed[i - 1]);
                } else {
                    smoothed_altitude_changes.push(0.0);
                }
            }
        }
        
        self.enhanced_altitude = gaussian_smoothed;
        self.cumulative_distance = uniform_distances;
        self.altitude_change = smoothed_altitude_changes;
        
        // Safe distance_change assignment
        self.distance_change = vec![interval_meters; self.altitude_change.len()];
        if !self.distance_change.is_empty() && !self.cumulative_distance.is_empty() {
            self.distance_change[0] = self.cumulative_distance[0];
        }
        
        println!("      [DEBUG] Applying symmetric deadband filtering...");
        self.apply_symmetric_deadband_filtering(deadband_threshold);
        
        println!("      [DEBUG] Calculating gradients...");
        self.calculate_gradients();
        
        println!("      [DEBUG] Recalculating accumulated values...");
        self.recalculate_accumulated_values_after_smoothing();
        
        println!("      [DEBUG] Symmetric processing complete!");
    }
    
    fn median_filter(data: &[f64], window: usize) -> Vec<f64> {
        let mut result = Vec::with_capacity(data.len());
        
        if data.is_empty() {
            return result;
        }
        
        // Debug for large datasets
        if data.len() > 10000 {
            println!("      [DEBUG] Median filter on {} points...", data.len());
        }
        
        let half_window = window / 2;
        
        for i in 0..data.len() {
            // Progress indicator for large datasets
            if data.len() > 10000 && i % 5000 == 0 {
                println!("      [DEBUG] Median filter progress: {}/{}", i, data.len());
            }
            
            let start = if i >= half_window { i - half_window } else { 0 };
            let end = std::cmp::min(i + half_window + 1, data.len()); // Use +1 for end to avoid exclusive bound issues
            
            // Extra safety check
            if start < data.len() && end <= data.len() && start < end {
                let mut window_data: Vec<f64> = data[start..end].to_vec();
                window_data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                
                let median = if window_data.len() % 2 == 0 {
                    if window_data.len() >= 2 {
                        (window_data[window_data.len() / 2 - 1] + window_data[window_data.len() / 2]) / 2.0
                    } else if window_data.len() == 1 {
                        window_data[0]
                    } else {
                        data[i] // Fallback
                    }
                } else {
                    if window_data.len() > 0 {
                        window_data[window_data.len() / 2]
                    } else {
                        data[i] // Fallback
                    }
                };
                
                result.push(median);
            } else {
                // Fallback: just use the original value
                result.push(data[i]);
            }
        }
        
        result
    }
    
    fn gaussian_smooth(data: &[f64], window: usize) -> Vec<f64> {
        let mut result = Vec::with_capacity(data.len());
        
        if data.is_empty() {
            return result;
        }
        
        // Debug for large datasets
        if data.len() > 10000 {
            println!("      [DEBUG] Gaussian smooth on {} points, window {}", data.len(), window);
        }
        
        let sigma = window as f64 / 6.0;
        let half_window = window / 2;
        
        for i in 0..data.len() {
            // Progress indicator for large datasets
            if data.len() > 10000 && i % 5000 == 0 {
                println!("      [DEBUG] Gaussian smooth progress: {}/{}", i, data.len());
            }
            
            let start = if i >= half_window { i - half_window } else { 0 };
            let end = std::cmp::min(i + half_window + 1, data.len()); // Use +1 for end to avoid exclusive bound issues
            
            // Extra safety check
            if start < data.len() && end <= data.len() && start < end {
                let mut weighted_sum = 0.0;
                let mut weight_sum = 0.0;
                
                for j in start..end {
                    if j < data.len() { // Extra bounds check
                        let distance = (j as f64 - i as f64).abs();
                        let weight = (-0.5 * (distance / sigma).powi(2)).exp();
                        
                        weighted_sum += data[j] * weight;
                        weight_sum += weight;
                    }
                }
                
                if weight_sum > 0.0 {
                    result.push(weighted_sum / weight_sum);
                } else {
                    result.push(data[i]); // Fallback
                }
            } else {
                // Fallback: just use the original value
                result.push(data[i]);
            }
        }
        
        result
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
        self.accumulated_ascent.clear();
        self.accumulated_descent.clear();
        
        let mut ascent_acc = 0.0;
        let mut descent_acc = 0.0;
        
        for i in 0..self.ascent.len() {
            ascent_acc += self.ascent[i];
            descent_acc += self.descent[i].abs();
            
            self.accumulated_ascent.push(ascent_acc);
            self.accumulated_descent.push(descent_acc);
        }
    }
    
    fn recalculate_accumulated_values_after_smoothing(&mut self) {
        self.accumulated_ascent.clear();
        self.accumulated_descent.clear();
        
        let mut ascent_acc = 0.0;
        let mut descent_acc = 0.0;
        
        for &change in &self.altitude_change {
            if change > 0.0 {
                ascent_acc += change;
            } else if change < 0.0 {
                descent_acc += -change;
            }
            
            self.accumulated_ascent.push(ascent_acc);
            self.accumulated_descent.push(descent_acc);
        }
    }
    
    fn process_elevation_data_with_variant(&mut self, variant: SmoothingVariant) {
        if matches!(variant, SmoothingVariant::AdaptiveQuality) {
            self.process_elevation_data_adaptive();
            return;
        }
        
        self.calculate_altitude_changes();
        self.calculate_accumulated_ascent_descent();
        self.calculate_gradients();
        self.calculate_overall_gradients();
        
        if matches!(variant, SmoothingVariant::SymmetricFixed) {
            self.apply_custom_interval_processing_symmetric(1.9);
            return;
        }
        
        if matches!(variant, SmoothingVariant::DistBased) {
            self.apply_distance_based_processing();
            return;
        }
        
        // Apply smoothing based on variant
        let hilliness_ratio = self.overall_uphill_gradient;
        
        match variant {
            SmoothingVariant::Original => {
                if hilliness_ratio < 20.0 {
                    self.altitude_change = Self::rolling_mean(&self.altitude_change, 83);
                } else {
                    self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
                }
            },
            SmoothingVariant::Capping => {
                self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
            },
            SmoothingVariant::Flat21 => {
                if hilliness_ratio < 20.0 {
                    self.altitude_change = Self::rolling_mean(&self.altitude_change, 21);
                } else {
                    self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
                }
            },
            SmoothingVariant::PostCap => {
                self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
            },
            _ => {
                // Default processing
                self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
            }
        }
        
        self.calculate_gradients();
        self.separate_ascent_descent();
        self.recalculate_accumulated_values();
        
        let total_distance_km = self.cumulative_distance.last().unwrap_or(&0.0) / 1000.0;
        if total_distance_km > 0.0 {
            self.overall_uphill_gradient = self.accumulated_ascent.last().unwrap_or(&0.0) / total_distance_km;
            self.overall_downhill_gradient = self.accumulated_descent.last().unwrap_or(&0.0) / total_distance_km;
        }
    }
    
    fn apply_distance_based_processing(&mut self) {
        let original_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
        
        let total_distance_km = self.cumulative_distance.last().unwrap_or(&0.0) / 1000.0;
        let gain_per_km = if total_distance_km > 0.0 { original_gain / total_distance_km } else { 0.0 };
        
        let terrain_type = if gain_per_km < 12.0 {
            "flat"
        } else if gain_per_km < 30.0 {
            "rolling"  
        } else if gain_per_km < 60.0 {
            "hilly"
        } else {
            "mountainous"
        };
        
        let (smoothing_window, max_gradient, spike_threshold) = match terrain_type {
            "flat" => (90, 6.0, 3.0),
            "rolling" => (45, 12.0, 4.0),
            "hilly" => (21, 18.0, 6.0),
            "mountainous" => (15, 25.0, 8.0),
            _ => (45, 12.0, 4.0),
        };
        
        self.apply_terrain_adaptive_smoothing(smoothing_window, max_gradient, spike_threshold);
    }
    
    fn apply_terrain_adaptive_smoothing(&mut self, window: usize, max_gradient: f64, spike_threshold: f64) {
        if self.altitude_change.is_empty() || self.altitude_change.len() < 3 {
            return;
        }
        
        let mut smoothed_changes = self.altitude_change.clone();
        
        // Step 1: Remove obvious GPS spikes (sudden jumps) - with bounds checking
        if smoothed_changes.len() >= 3 {
            for i in 1..(smoothed_changes.len().saturating_sub(1)) {
                if i > 0 && i < smoothed_changes.len() - 1 {
                    let prev_change = smoothed_changes[i-1];
                    let curr_change = smoothed_changes[i];
                    let next_change = smoothed_changes[i+1];
                    
                    if curr_change.abs() > spike_threshold && 
                       (curr_change > 0.0) != (prev_change > 0.0) && 
                       (curr_change > 0.0) != (next_change > 0.0) {
                        smoothed_changes[i] = (prev_change + next_change) / 2.0;
                    }
                }
            }
        }
        
        // Step 2: Apply rolling window smoothing - with extra careful bounds checking
        let mut windowed_changes = smoothed_changes.clone();
        let half_window = window / 2;
        
        for i in 0..windowed_changes.len() {
            let start = if i >= half_window { i - half_window } else { 0 };
            let end = std::cmp::min(i + half_window, smoothed_changes.len());
            
            // Triple check bounds
            if start < smoothed_changes.len() && end <= smoothed_changes.len() && start < end {
                let window_sum: f64 = smoothed_changes[start..end].iter().sum();
                let window_count = end - start;
                if window_count > 0 {
                    windowed_changes[i] = window_sum / window_count as f64;
                }
            }
        }
        
        // Step 3: Gradient capping with elevation gain preservation
        let original_total_gain: f64 = self.altitude_change.iter()
            .filter(|&&x| x > 0.0)
            .sum();
        
        // FIXED: Ensure we don't exceed array bounds
        let min_len = windowed_changes.len().min(self.distance_change.len());
        
        for i in 0..min_len {
            if self.distance_change[i] > 0.0 {
                let gradient_percent = (windowed_changes[i] / self.distance_change[i]) * 100.0;
                
                if gradient_percent > max_gradient {
                    windowed_changes[i] = max_gradient * self.distance_change[i] / 100.0;
                } else if gradient_percent < -max_gradient {
                    windowed_changes[i] = -max_gradient * self.distance_change[i] / 100.0;
                }
            }
        }
        
        // Step 4: Preserve total elevation gain for hilly routes
        let processed_total_gain: f64 = windowed_changes.iter()
            .filter(|&&x| x > 0.0)
            .sum();
            
        if processed_total_gain < original_total_gain * 0.75 && original_total_gain > 500.0 && processed_total_gain > 0.0 {
            let scaling_factor = (original_total_gain * 0.85) / processed_total_gain;
            for change in &mut windowed_changes {
                if *change > 0.0 {
                    *change *= scaling_factor;
                }
            }
        }
        
        self.altitude_change = windowed_changes;
        self.recalculate_derived_values();
    }
    
    fn recalculate_derived_values(&mut self) {
        // Recalculate gradients with bounds checking
        self.gradient_percent.clear();
        let min_len = std::cmp::min(self.altitude_change.len(), self.distance_change.len());
        
        for i in 0..min_len {
            if self.distance_change[i] == 0.0 {
                self.gradient_percent.push(0.0);
            } else {
                self.gradient_percent.push(
                    (self.altitude_change[i] / self.distance_change[i]) * 100.0
                );
            }
        }
        
        // Recalculate ascent/descent
        self.ascent.clear();
        self.descent.clear();
        for &change in &self.altitude_change {
            if change > 0.0 {
                self.ascent.push(change);
                self.descent.push(0.0);
            } else {
                self.ascent.push(0.0);
                self.descent.push(change);
            }
        }
        
        // Recalculate accumulated values - safe version
        self.accumulated_ascent.clear();
        self.accumulated_descent.clear();
        let mut ascent_acc = 0.0;
        let mut descent_acc = 0.0;
        
        let max_len = std::cmp::max(self.ascent.len(), self.descent.len());
        for i in 0..max_len {
            if i < self.ascent.len() {
                ascent_acc += self.ascent[i];
            }
            if i < self.descent.len() {
                descent_acc += self.descent[i].abs();
            }
            self.accumulated_ascent.push(ascent_acc);
            self.accumulated_descent.push(descent_acc);
        }
    }
    
    pub fn process_elevation_data(&mut self) {
        self.process_elevation_data_with_variant(SmoothingVariant::Original);
    }
    
    pub fn get_total_elevation_gain(&self) -> f64 {
        self.accumulated_ascent.last().unwrap_or(&0.0).clone()
    }
    
    pub fn get_total_elevation_loss(&self) -> f64 {
        self.accumulated_descent.last().unwrap_or(&0.0).clone()
    }
    
    pub fn get_gain_loss_ratio(&self) -> f64 {
        let gain = self.get_total_elevation_gain();
        let loss = self.get_total_elevation_loss();
        
        if loss > 0.0 {
            gain / loss
        } else {
            f64::INFINITY
        }
    }
    
    pub fn get_data_quality_issues(&self) -> &Vec<String> {
        &self.data_quality_issues
    }
    
    /// LEGACY: Original asymmetric deadband - KEPT FOR BACKWARD COMPATIBILITY
    /// NOTE: This method causes severe loss under-estimation and should be avoided
    #[allow(dead_code)]
    fn apply_deadband_filtering(&mut self, threshold_meters: f64) {
        if self.altitude_change.is_empty() {
            return;
        }
        
        let mut filtered_changes = Vec::with_capacity(self.altitude_change.len());
        let mut cumulative_climb = 0.0;
        let mut last_significant_idx = 0;
        
        filtered_changes.push(0.0); // First change is always 0
        
        for i in 1..self.altitude_change.len() {
            let change = self.altitude_change[i];
            
            if change > 0.0 {
                cumulative_climb += change;
                
                if cumulative_climb >= threshold_meters {
                    // Distribute the accumulated climb
                    let segments_to_fill = i - last_significant_idx;
                    if segments_to_fill > 0 {
                        let climb_per_segment = cumulative_climb / segments_to_fill as f64;
                        
                        // Fill from last_significant_idx + 1 to i (inclusive)
                        for j in (last_significant_idx + 1)..=i {
                            if j < filtered_changes.len() {
                                filtered_changes[j] = climb_per_segment;
                            } else if j == i {
                                // We're at the current index, safe to push
                                filtered_changes.push(climb_per_segment);
                            }
                        }
                        
                        // Ensure we have the right length
                        while filtered_changes.len() <= i {
                            filtered_changes.push(climb_per_segment);
                        }
                    } else {
                        filtered_changes.push(cumulative_climb);
                    }
                    
                    cumulative_climb = 0.0;
                    last_significant_idx = i;
                } else {
                    filtered_changes.push(0.0);
                }
            } else {
                // Descents are preserved as-is
                filtered_changes.push(change);
                if cumulative_climb > 0.0 {
                    cumulative_climb = 0.0;
                    last_significant_idx = i;
                }
            }
        }
        
        // Handle any remaining accumulated climb
        if cumulative_climb > 0.0 && last_significant_idx < filtered_changes.len() {
            let remaining_segments = filtered_changes.len() - last_significant_idx;
            if remaining_segments > 0 {
                let climb_per_segment = cumulative_climb / remaining_segments as f64;
                for j in (last_significant_idx + 1)..filtered_changes.len() {
                    filtered_changes[j] = climb_per_segment;
                }
            }
        }
        
        // Ensure the result has the same length as the input
        filtered_changes.resize(self.altitude_change.len(), 0.0);
        
        self.altitude_change = filtered_changes;
    }
}

pub fn create_custom_original(elevations: Vec<f64>, distances: Vec<f64>) -> ElevationData {
    ElevationData::new_with_variant(elevations, distances, SmoothingVariant::Original)
}

pub fn create_custom_capping(elevations: Vec<f64>, distances: Vec<f64>) -> ElevationData {
    ElevationData::new_with_variant(elevations, distances, SmoothingVariant::Capping)
}

pub fn create_custom_flat21(elevations: Vec<f64>, distances: Vec<f64>) -> ElevationData {
    ElevationData::new_with_variant(elevations, distances, SmoothingVariant::Flat21)
}

pub fn create_custom_postcap(elevations: Vec<f64>, distances: Vec<f64>) -> ElevationData {
    ElevationData::new_with_variant(elevations, distances, SmoothingVariant::PostCap)
}

pub fn create_custom_distbased(elevations: Vec<f64>, distances: Vec<f64>) -> ElevationData {
    ElevationData::new_with_variant(elevations, distances, SmoothingVariant::DistBased)
}

pub fn create_custom_symmetric_fixed(elevations: Vec<f64>, distances: Vec<f64>) -> ElevationData {
    ElevationData::new_with_variant(elevations, distances, SmoothingVariant::SymmetricFixed)
}

pub fn create_custom_adaptive_quality(elevations: Vec<f64>, distances: Vec<f64>) -> ElevationData {
    ElevationData::new_with_variant(elevations, distances, SmoothingVariant::AdaptiveQuality)
}