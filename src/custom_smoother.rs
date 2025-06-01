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
    pub data_quality_issues: Vec<String>, // NEW: Track detected data quality issues
}

/// Smoothing variant type
#[derive(Debug, Clone, Copy)]
pub enum SmoothingVariant {
    Original,           // Adaptive 83/5-point with conditional capping
    Capping,            // 5-point smoothing + capping for ALL routes
    Flat21,             // 21-point for flat, 5-point for hilly
    PostCap,            // 5-point + capping + 83-point post-capping smoothing
    DistBased,          // Distance-based uniform resampling + distance-aware processing
    SymmetricFixed,     // Distance-based with symmetric deadband filtering (FIXED VERSION)
    AdaptiveQuality,    // NEW: Adaptive processing based on data quality detection
}

#[derive(Debug)]
enum DataQuality {
    Good,                    // Ratio < 1.1
    ArtificialInflation,     // Ratio 1.1 - 2.0
    SevereCorruption,        // Ratio > 2.0
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
        
        // Calculate distance changes
        data.calculate_distance_changes();
        
        // Process elevation data with specified variant
        data.process_elevation_data_with_variant(variant);
        
        data
    }
    
    fn calculate_distance_changes(&mut self) {
        if self.cumulative_distance.is_empty() {
            return;
        }
        
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
        
        self.altitude_change.push(0.0);
        
        for i in 1..self.enhanced_altitude.len() {
            self.altitude_change.push(
                self.enhanced_altitude[i] - self.enhanced_altitude[i - 1]
            );
        }
    }
    
    fn calculate_raw_gain_loss(&self) -> (f64, f64) {
        let mut gain = 0.0;
        let mut loss = 0.0;
        
        for &change in &self.altitude_change {
            if change > 0.0 {
                gain += change;
            } else if change < 0.0 {
                loss += -change; // Make loss positive
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
            let altitude_diff = self.enhanced_altitude[i] - self.enhanced_altitude[i - 1];
            
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
    
    // FIXED: NEW: Adaptive processing based on data quality detection
    // NOTE: Uses MODERATE correction parameters to avoid over-processing
    // - Moderate smoothing (50-100 points, not 200-500)
    // - Reasonable gradient caps (18-25%, not 8-15%)  
    // - Small deadband filters (3-5m, not 8-15m)
    // - Conservative scaling targets (1.2-1.3, not 1.0-1.1)
    fn process_elevation_data_adaptive(&mut self) {
        println!("🔍 ADAPTIVE QUALITY-BASED PROCESSING");
        
        // Step 1: Calculate initial altitude changes
        self.calculate_altitude_changes();
        
        // Step 2: Calculate raw gain/loss for quality assessment
        let (raw_gain, raw_loss) = self.calculate_raw_gain_loss();
        
        // Step 3: Detect data quality issues
        let gain_loss_ratio = if raw_loss > 0.0 { raw_gain / raw_loss } else { f64::INFINITY };
        let data_quality = self.assess_data_quality(raw_gain, raw_loss, gain_loss_ratio);
        
        println!("   Raw gain: {:.1}m, Raw loss: {:.1}m", raw_gain, raw_loss);
        println!("   Gain/Loss ratio: {:.3}", gain_loss_ratio);
        println!("   Quality: {:?}", data_quality);
        
        // Step 4: Apply processing based on detected quality
        match data_quality {
            DataQuality::Good => {
                println!("   ✅ Applying standard processing (ratio < 1.1)");
                self.apply_standard_processing();
            },
            DataQuality::ArtificialInflation => {
                println!("   🚨 ARTIFICIAL INFLATION DETECTED! Applying moderate correction");
                self.apply_aggressive_inflation_correction(raw_gain, raw_loss);
            },
            DataQuality::SevereCorruption => {
                println!("   💀 SEVERE CORRUPTION! Applying strong correction");
                self.apply_maximum_correction(raw_gain, raw_loss);
            }
        }
        
        // Step 5: CRITICAL FIX - Recalculate accumulated values from processed altitude_change
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
            println!("      ⚠️  Still imbalanced - may need even more aggressive processing");
            self.data_quality_issues.push("Persistent gain/loss imbalance after processing".to_string());
        } else if final_ratio >= 0.8 && final_ratio <= 1.2 {
            println!("      ✅ Balanced ratio achieved!");
        }
    }
    
    fn assess_data_quality(&mut self, _raw_gain: f64, raw_loss: f64, ratio: f64) -> DataQuality {
        // Clear previous quality issues
        self.data_quality_issues.clear();
        
        if ratio.is_infinite() || raw_loss < 10.0 {
            self.data_quality_issues.push("No meaningful loss data".to_string());
            return DataQuality::SevereCorruption;
        }
        
        if ratio > 2.0 {
            self.data_quality_issues.push(format!("Severe gain inflation: {:.1}x expected", ratio));
            return DataQuality::SevereCorruption;
        }
        
        if ratio > 1.1 {
            self.data_quality_issues.push(format!("Artificial elevation inflation detected: {:.1}x ratio", ratio));
            
            // Additional checks for artificial inflation patterns
            if self.detect_artificial_patterns() {
                self.data_quality_issues.push("Artificial elevation patterns detected".to_string());
            }
            
            return DataQuality::ArtificialInflation;
        }
        
        DataQuality::Good
    }
    
    fn detect_artificial_patterns(&self) -> bool {
        // Check for unrealistic gradient patterns that indicate artificial data
        let mut extreme_gradients = 0;
        let mut total_segments = 0;
        
        for i in 0..self.altitude_change.len() {
            if self.distance_change[i] > 0.0 {
                let gradient = (self.altitude_change[i] / self.distance_change[i]) * 100.0;
                total_segments += 1;
                
                // Count segments with unrealistic gradients (>30% or <-30%)
                if gradient.abs() > 30.0 {
                    extreme_gradients += 1;
                }
            }
        }
        
        // If more than 5% of segments have extreme gradients, likely artificial
        total_segments > 0 && (extreme_gradients as f64 / total_segments as f64) > 0.05
    }
    
    fn apply_standard_processing(&mut self) {
        // Use the original logic for good quality data
        self.calculate_accumulated_ascent_descent();
        self.calculate_gradients();
        self.calculate_overall_gradients();
        
        let hilliness_ratio = self.overall_uphill_gradient;
        
        // Light smoothing for flat routes
        if hilliness_ratio < 20.0 {
            self.altitude_change = Self::rolling_mean(&self.altitude_change, 83);
        } else {
            self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
        }
        
        self.calculate_gradients();
        self.apply_standard_gradient_capping();
        self.separate_ascent_descent();
        self.recalculate_accumulated_values();
    }
    
    fn apply_aggressive_inflation_correction(&mut self, _raw_gain: f64, _raw_loss: f64) {
        println!("   🔧 Applying MODERATE correction for artificial inflation...");
        
        // Step 1: MODERATE smoothing to remove artificial spikes (was 200, now 30-50)
        println!("      📊 Applying moderate 50-point smoothing...");
        self.altitude_change = Self::rolling_mean(&self.altitude_change, 50);
        
        // Step 2: Reasonable gradient capping (was 15%, now 25%)
        println!("      ✂️  Applying reasonable gradient capping (max 25%)...");
        self.apply_strict_gradient_capping(25.0);
        
        // Step 3: SMALLER deadband filtering (was 8m, now 3m)
        println!("      🚫 Applying moderate deadband filtering (3m threshold)...");
        self.apply_large_deadband_filtering(3.0);
        
        // Step 4: CRITICAL FIX - Recalculate accumulated from modified altitude_change
        self.recalculate_accumulated_values_from_altitude_changes();
        
        let processed_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
        let processed_loss = self.accumulated_descent.last().unwrap_or(&0.0).clone();
        let new_ratio = if processed_loss > 0.0 { processed_gain / processed_loss } else { f64::INFINITY };
        
        // Step 5: GENTLER scaling - only if severely imbalanced
        if new_ratio > 1.5 && processed_loss > 100.0 {
            println!("      ⚖️  Applying gentle scaling to balance ratio...");
            self.scale_gain_to_realistic_ratio(processed_loss, 1.2); // More conservative target
            // Recalculate again after scaling
            self.recalculate_accumulated_values_from_altitude_changes();
        }
    }
    
    fn apply_maximum_correction(&mut self, _raw_gain: f64, raw_loss: f64) {
        println!("   💀 Applying STRONG correction for severe corruption...");
        
        // Strong but not nuclear: More aggressive than moderate but not extreme (was 500, now 100)
        println!("      🌪️  Applying strong 100-point smoothing...");
        self.altitude_change = Self::rolling_mean(&self.altitude_change, 100);
        
        println!("      ✂️  Applying strong gradient capping (max 18%)...");
        self.apply_strict_gradient_capping(18.0);
        
        println!("      🚫 Applying strong deadband filtering (5m threshold)...");
        self.apply_large_deadband_filtering(5.0);
        
        // CRITICAL FIX - Recalculate from modified altitude_change
        self.recalculate_accumulated_values_from_altitude_changes();
        
        // More conservative scaling for severe cases
        if raw_loss > 500.0 && raw_loss < 15000.0 {
            let processed_loss = self.accumulated_descent.last().unwrap_or(&0.0).clone();
            let processed_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
            let current_ratio = if processed_loss > 0.0 { processed_gain / processed_loss } else { f64::INFINITY };
            
            // Only scale if still very imbalanced
            if current_ratio > 2.0 {
                println!("      🎯 Scaling gain to match loss (target ratio: 1.3)...");
                self.scale_gain_to_realistic_ratio(processed_loss, 1.3); // Less aggressive target
                // Recalculate again after scaling
                self.recalculate_accumulated_values_from_altitude_changes();
            }
        }
    }
    
    fn apply_strict_gradient_capping(&mut self, max_gradient_percent: f64) {
        self.calculate_gradients();
        
        for i in 0..self.gradient_percent.len() {
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
        
        for i in 1..self.altitude_change.len() {
            cumulative_change += self.altitude_change[i];
            
            if cumulative_change.abs() >= threshold_meters {
                filtered_changes.push(cumulative_change);
                cumulative_change = 0.0;
            } else {
                filtered_changes.push(0.0);
            }
        }
        
        self.altitude_change = filtered_changes;
    }
    
    fn scale_gain_to_realistic_ratio(&mut self, target_loss: f64, target_ratio: f64) {
        let target_gain = target_loss * target_ratio;
        
        // Scale all positive altitude changes proportionally
        let current_gain: f64 = self.altitude_change.iter()
            .filter(|&&x| x > 0.0)
            .sum();
        
        if current_gain > 0.0 {
            let scale_factor = target_gain / current_gain;
            println!("         Scaling positive changes by factor: {:.3}", scale_factor);
            
            for change in &mut self.altitude_change {
                if *change > 0.0 {
                    *change *= scale_factor;
                }
            }
        }
    }
    
    // CRITICAL FIX: New method to recalculate accumulated values from altitude_change
    fn recalculate_accumulated_values_from_altitude_changes(&mut self) {
        self.accumulated_ascent.clear();
        self.accumulated_descent.clear();
        
        let mut ascent_acc = 0.0;
        let mut descent_acc = 0.0;
        
        for &change in &self.altitude_change {
            if change > 0.0 {
                ascent_acc += change;
            } else if change < 0.0 {
                descent_acc += -change; // Make positive
            }
            
            self.accumulated_ascent.push(ascent_acc);
            self.accumulated_descent.push(descent_acc);
        }
    }
    
    fn apply_standard_gradient_capping(&mut self) {
        let hilliness_ratio = self.overall_uphill_gradient;
        
        let thresholds = vec![
            (20.0, 15.0, 12.0),
            (30.0, 20.0, 15.0),
            (40.0, 25.0, 20.0),
            (50.0, 32.0, 27.0),
            (60.0, 35.0, 31.0),
            (f64::INFINITY, 40.0, 36.0),
        ];
        
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
        
        self.calculate_gradients();
    }

    /// Linear interpolation to resample elevation data onto uniform distance grid
    fn resample_to_uniform_distance(&mut self, interval_meters: f64) -> (Vec<f64>, Vec<f64>) {
        if self.cumulative_distance.is_empty() || self.enhanced_altitude.is_empty() {
            return (vec![], vec![]);
        }
        
        let total_distance = self.cumulative_distance.last().unwrap();
        let num_points = (total_distance / interval_meters).ceil() as usize + 1;
        
        let mut uniform_distances = Vec::with_capacity(num_points);
        let mut uniform_elevations = Vec::with_capacity(num_points);
        
        // Generate uniform distance grid
        for i in 0..num_points {
            let target_distance = i as f64 * interval_meters;
            if target_distance > *total_distance {
                break;
            }
            uniform_distances.push(target_distance);
            
            // Interpolate elevation at this distance
            let elevation = self.interpolate_elevation_at_distance(target_distance);
            uniform_elevations.push(elevation);
        }
        
        (uniform_distances, uniform_elevations)
    }
    
    /// Linear interpolation to find elevation at specific distance
    fn interpolate_elevation_at_distance(&self, target_distance: f64) -> f64 {
        if target_distance <= 0.0 {
            return self.enhanced_altitude[0];
        }
        
        // Find the two points that bracket our target distance
        for i in 1..self.cumulative_distance.len() {
            if self.cumulative_distance[i] >= target_distance {
                let d1 = self.cumulative_distance[i - 1];
                let d2 = self.cumulative_distance[i];
                let e1 = self.enhanced_altitude[i - 1];
                let e2 = self.enhanced_altitude[i];
                
                if (d2 - d1).abs() < 1e-10 {
                    return e1; // Avoid division by zero
                }
                
                // Linear interpolation
                let t = (target_distance - d1) / (d2 - d1);
                return e1 + t * (e2 - e1);
            }
        }
        
        // If we're past the end, return the last elevation
        *self.enhanced_altitude.last().unwrap()
    }
    
    /// FIXED: Symmetric deadband filtering - treats gains and losses equally
    /// This replaces the asymmetric version that was causing loss under-estimation
    fn apply_symmetric_deadband_filtering(&mut self, threshold_meters: f64) {
        if self.altitude_change.is_empty() {
            return;
        }
        
        let mut filtered_changes = vec![0.0]; // Start with first change as 0
        let mut cumulative_elevation_change = 0.0;
        
        for i in 1..self.altitude_change.len() {
            let change = self.altitude_change[i];
            cumulative_elevation_change += change;
            
            // Check if the absolute cumulative change exceeds threshold
            if cumulative_elevation_change.abs() >= threshold_meters {
                // Register the cumulative change and reset
                filtered_changes.push(cumulative_elevation_change);
                cumulative_elevation_change = 0.0;
            } else {
                // Change below threshold - don't register it yet
                filtered_changes.push(0.0);
            }
        }
        
        self.altitude_change = filtered_changes;
    }
    
    fn apply_distance_based_processing(&mut self) {
        let original_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
        
        // IMPROVED: Better terrain classification
        let total_distance_km = self.cumulative_distance.last().unwrap_or(&0.0) / 1000.0;
        let gain_per_km = if total_distance_km > 0.0 { original_gain / total_distance_km } else { 0.0 };
        
        // CHANGE: More nuanced terrain thresholds
        let terrain_type = if gain_per_km < 12.0 {
            "flat"
        } else if gain_per_km < 30.0 {
            "rolling"  
        } else if gain_per_km < 60.0 {
            "hilly"
        } else {
            "mountainous"
        };
        
        // CHANGE: Terrain-adaptive smoothing parameters
        let (smoothing_window, max_gradient, spike_threshold) = match terrain_type {
            "flat" => (90, 6.0, 3.0),           // Aggressive smoothing for flat
            "rolling" => (45, 12.0, 4.0),       // Moderate for rolling
            "hilly" => (21, 18.0, 6.0),         // Conservative for hilly
            "mountainous" => (15, 25.0, 8.0),   // Minimal smoothing for mountains
            _ => (45, 12.0, 4.0),
        };
        
        // CHANGE: Apply terrain-specific processing
        self.apply_terrain_adaptive_smoothing(smoothing_window, max_gradient, spike_threshold);
        
        let _processed_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
    }
    
    fn apply_smoothing_variant(&mut self, variant: SmoothingVariant) {
        let hilliness_ratio = self.overall_uphill_gradient;
        let _original_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
        
        match variant {
            SmoothingVariant::Original => {
                // Original adaptive smoothing
                if hilliness_ratio < 20.0 {
                    self.altitude_change = Self::rolling_mean(&self.altitude_change, 83);
                } else {
                    self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
                }
            },
            
            SmoothingVariant::Capping => {
                // Always 5-point smoothing, always apply capping
                self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
            },
            
            SmoothingVariant::Flat21 => {
                // 21-point for flat, 5-point for hilly
                if hilliness_ratio < 20.0 {
                    self.altitude_change = Self::rolling_mean(&self.altitude_change, 21);
                } else {
                    self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
                }
            },
            
            SmoothingVariant::PostCap => {
                // Always 5-point smoothing, capping will be applied, then 83-point post-capping smoothing
                self.altitude_change = Self::rolling_mean(&self.altitude_change, 5);
            },
            
            SmoothingVariant::DistBased => {
                // Distance-based processing - uses LEGACY asymmetric deadband
                self.apply_distance_based_processing();
                return; // Skip the normal smoothing path
            },
            
            SmoothingVariant::SymmetricFixed => {
                // NEW: Distance-based processing with SYMMETRIC deadband (FIXED VERSION)
                self.apply_distance_based_processing_symmetric();
                return; // Skip the normal smoothing path
            },
            
            SmoothingVariant::AdaptiveQuality => {
                // NEW: Adaptive processing based on data quality
                self.process_elevation_data_adaptive();
                return; // Complete processing, skip normal path
            },
        }
        
        self.calculate_gradients();
        self.recalculate_accumulated_values_after_smoothing();
        
        let _smoothed_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
    }
    
    /// NEW: Distance-based processing with symmetric deadband filtering
    fn apply_distance_based_processing_symmetric(&mut self) {
        let original_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
        
        // Better terrain classification
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
        
        // Terrain-adaptive smoothing parameters
        let (smoothing_window, max_gradient, spike_threshold) = match terrain_type {
            "flat" => (90, 6.0, 3.0),
            "rolling" => (45, 12.0, 4.0),
            "hilly" => (21, 18.0, 6.0),
            "mountainous" => (15, 25.0, 8.0),
            _ => (45, 12.0, 4.0),
        };
        
        // Apply terrain-specific processing with SYMMETRIC deadband
        self.apply_terrain_adaptive_smoothing_symmetric(smoothing_window, max_gradient, spike_threshold);
        
        let _processed_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
    }
    
    /// NEW: Terrain adaptive smoothing with symmetric deadband
    fn apply_terrain_adaptive_smoothing_symmetric(&mut self, window: usize, max_gradient: f64, spike_threshold: f64) {
        if self.altitude_change.is_empty() {
            return;
        }
        
        // Smart spike detection for hilly terrain
        let mut smoothed_changes = self.altitude_change.clone();
        
        // Step 1: Remove obvious GPS spikes (sudden jumps)
        for i in 1..smoothed_changes.len()-1 {
            let prev_change = smoothed_changes[i-1];
            let curr_change = smoothed_changes[i];
            let next_change = smoothed_changes[i+1];
            
            // Detect spikes based on terrain-specific threshold
            if curr_change.abs() > spike_threshold && 
               (curr_change > 0.0) != (prev_change > 0.0) && 
               (curr_change > 0.0) != (next_change > 0.0) {
                // This looks like a GPS spike - interpolate
                smoothed_changes[i] = (prev_change + next_change) / 2.0;
            }
        }
        
        // Step 2: Apply rolling window smoothing (terrain-adaptive)
        let mut windowed_changes = smoothed_changes.clone();
        for i in 0..windowed_changes.len() {
            let start = if i >= window/2 { i - window/2 } else { 0 };
            let end = if i + window/2 < windowed_changes.len() { i + window/2 } else { windowed_changes.len() - 1 };
            
            let window_sum: f64 = smoothed_changes[start..=end].iter().sum();
            let window_count = end - start + 1;
            windowed_changes[i] = window_sum / window_count as f64;
        }
        
        // Step 3: Gradient capping with elevation gain preservation
        let original_total_gain: f64 = self.altitude_change.iter()
            .filter(|&&x| x > 0.0)
            .sum();
            
        for i in 0..windowed_changes.len() {
            if self.distance_change[i] > 0.0 {
                let gradient_percent = (windowed_changes[i] / self.distance_change[i]) * 100.0;
                
                // Only cap if gradient is unreasonably high
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
            
        // If we lost too much elevation gain, scale it back up
        if processed_total_gain < original_total_gain * 0.75 && original_total_gain > 500.0 {
            let scaling_factor = (original_total_gain * 0.85) / processed_total_gain;
            for change in &mut windowed_changes {
                if *change > 0.0 {
                    *change *= scaling_factor;
                }
            }
        }
        
        // Apply the smoothed changes
        self.altitude_change = windowed_changes;
        
        // Step 5: Apply SYMMETRIC deadband filtering (NEW - FIXES THE MAIN ISSUE)
        let deadband_threshold = match self.overall_uphill_gradient {
            x if x < 20.0 => 1.5,  // Flat terrain
            x if x < 40.0 => 2.0,  // Hilly terrain
            _ => 1.5,              // Mountainous terrain
        };
        
        self.apply_symmetric_deadband_filtering(deadband_threshold);
        
        self.recalculate_derived_values();
    }
    
    fn apply_terrain_adaptive_smoothing(&mut self, window: usize, max_gradient: f64, spike_threshold: f64) {
        if self.altitude_change.is_empty() {
            return;
        }
        
        // IMPROVEMENT: Smart spike detection for hilly terrain
        let mut smoothed_changes = self.altitude_change.clone();
        
        // Step 1: Remove obvious GPS spikes (sudden jumps)
        for i in 1..smoothed_changes.len()-1 {
            let prev_change = smoothed_changes[i-1];
            let curr_change = smoothed_changes[i];
            let next_change = smoothed_changes[i+1];
            
            // CHANGE: Detect spikes based on terrain-specific threshold
            if curr_change.abs() > spike_threshold && 
               (curr_change > 0.0) != (prev_change > 0.0) && 
               (curr_change > 0.0) != (next_change > 0.0) {
                // This looks like a GPS spike - interpolate
                smoothed_changes[i] = (prev_change + next_change) / 2.0;
            }
        }
        
        // Step 2: Apply rolling window smoothing (terrain-adaptive)
        let mut windowed_changes = smoothed_changes.clone();
        for i in 0..windowed_changes.len() {
            let start = if i >= window/2 { i - window/2 } else { 0 };
            let end = if i + window/2 < windowed_changes.len() { i + window/2 } else { windowed_changes.len() - 1 };
            
            let window_sum: f64 = smoothed_changes[start..=end].iter().sum();
            let window_count = end - start + 1;
            windowed_changes[i] = window_sum / window_count as f64;
        }
        
        // Step 3: CHANGE - Gradient capping with elevation gain preservation
        let original_total_gain: f64 = self.altitude_change.iter()
            .filter(|&&x| x > 0.0)
            .sum();
            
        for i in 0..windowed_changes.len() {
            if self.distance_change[i] > 0.0 {
                let gradient_percent = (windowed_changes[i] / self.distance_change[i]) * 100.0;
                
                // IMPROVEMENT: Only cap if gradient is unreasonably high
                if gradient_percent > max_gradient {
                    windowed_changes[i] = max_gradient * self.distance_change[i] / 100.0;
                } else if gradient_percent < -max_gradient {
                    windowed_changes[i] = -max_gradient * self.distance_change[i] / 100.0;
                }
            }
        }
        
        // Step 4: CRITICAL - Preserve total elevation gain for hilly routes
        let processed_total_gain: f64 = windowed_changes.iter()
            .filter(|&&x| x > 0.0)
            .sum();
            
        // CHANGE: If we lost too much elevation gain, scale it back up
        if processed_total_gain < original_total_gain * 0.75 && original_total_gain > 500.0 {
            let scaling_factor = (original_total_gain * 0.85) / processed_total_gain;
            for change in &mut windowed_changes {
                if *change > 0.0 {
                    *change *= scaling_factor;
                }
            }
        }
        
        // Apply the smoothed changes
        self.altitude_change = windowed_changes;
        self.recalculate_derived_values();
    }
    
    fn recalculate_derived_values(&mut self) {
        // Recalculate gradients
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
        
        // Recalculate accumulated values
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
    
    fn apply_gradient_capping_variant(&mut self, variant: SmoothingVariant) {
        // Distance-based variants handle their own processing
        if matches!(variant, SmoothingVariant::DistBased | SmoothingVariant::SymmetricFixed | SmoothingVariant::AdaptiveQuality) {
            return;
        }
        
        let hilliness_ratio = self.overall_uphill_gradient;
        
        let should_apply_capping = match variant {
            SmoothingVariant::Original => hilliness_ratio >= 20.0,  // Only hilly routes
            SmoothingVariant::Capping => true,                     // ALL routes
            SmoothingVariant::Flat21 => hilliness_ratio >= 20.0,   // Only hilly routes  
            SmoothingVariant::PostCap => true,                     // ALL routes (needed for post-capping smoothing)
            _ => false,
        };
        
        if !should_apply_capping {
            return;
        }
        
        let _pre_capping_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
        
        // Define thresholds: (limit, max positive gradient, max negative gradient)
        let thresholds = vec![
            (30.0, 20.0, 15.0),
            (40.0, 25.0, 20.0),
            (50.0, 32.0, 27.0),
            (60.0, 35.0, 31.0),
            (f64::INFINITY, 40.0, 36.0),
        ];
        
        for (limit, max_up, max_down) in thresholds {
            if hilliness_ratio < limit {
                let _capped_count = 0;
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
        
        self.calculate_gradients();
        self.recalculate_accumulated_values_after_smoothing();
        
        let _post_capping_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
    }
    
    fn apply_post_capping_smoothing(&mut self, variant: SmoothingVariant) {
        if !matches!(variant, SmoothingVariant::PostCap) {
            return; // Only apply post-capping smoothing for PostCap variant
        }
        
        let _pre_post_smoothing_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
        
        // Apply 83-point smoothing to the capped altitude changes
        self.altitude_change = Self::rolling_mean(&self.altitude_change, 83);
        self.calculate_gradients();
        self.recalculate_accumulated_values_after_smoothing();
        
        let _post_post_smoothing_gain = self.accumulated_ascent.last().unwrap_or(&0.0).clone();
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
        // For adaptive quality processing, use special handling
        if matches!(variant, SmoothingVariant::AdaptiveQuality) {
            self.process_elevation_data_adaptive();
            return;
        }
        
        // Step 1: Calculate initial altitude changes
        self.calculate_altitude_changes();
        
        // Step 2: Calculate accumulated ascent and descent
        self.calculate_accumulated_ascent_descent();
        let _initial_gain = self.accumulated_ascent.last().unwrap_or(&0.0);
        
        // Step 3: Calculate initial gradients
        self.calculate_gradients();
        
        // Step 4: Calculate overall gradients (determines flat vs hilly)
        self.calculate_overall_gradients();
        
        // Step 5: Apply smoothing based on variant
        self.apply_smoothing_variant(variant);
        
        // For distance-based variants, processing is complete at this point
        if matches!(variant, SmoothingVariant::DistBased | SmoothingVariant::SymmetricFixed) {
            let _final_gain = self.accumulated_ascent.last().unwrap_or(&0.0);
            return;
        }
        
        // Step 6: Apply gradient capping based on variant
        self.apply_gradient_capping_variant(variant);
        
        // Step 7: Apply post-capping smoothing (only for PostCap variant)
        self.apply_post_capping_smoothing(variant);
        
        // Step 8: Separate into ascent and descent
        self.separate_ascent_descent();
        
        // Step 9: Final recalculation of accumulated values
        self.recalculate_accumulated_values();
        
        // Step 10: Final overall gradients calculation
        let total_distance_km = self.cumulative_distance.last().unwrap_or(&0.0) / 1000.0;
        if total_distance_km > 0.0 {
            self.overall_uphill_gradient = self.accumulated_ascent.last().unwrap_or(&0.0) / total_distance_km;
            self.overall_downhill_gradient = self.accumulated_descent.last().unwrap_or(&0.0) / total_distance_km;
        }
        
        let _final_gain = self.accumulated_ascent.last().unwrap_or(&0.0);
    }
    
    // Legacy method for backward compatibility
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
        
        // Update data
        let mut smoothed_altitude_changes = vec![0.0];
        for i in 1..gaussian_smoothed.len() {
            smoothed_altitude_changes.push(gaussian_smoothed[i] - gaussian_smoothed[i - 1]);
        }
        
        self.enhanced_altitude = gaussian_smoothed;
        self.cumulative_distance = uniform_distances;
        self.altitude_change = smoothed_altitude_changes;
        self.distance_change = vec![interval_meters; self.altitude_change.len()];
        self.distance_change[0] = self.cumulative_distance[0];
        
        // Apply deadband using existing method (still asymmetric for backward compatibility)
        self.apply_deadband_filtering(deadband_threshold);
        self.calculate_gradients();
        self.recalculate_accumulated_values_after_smoothing();
    }
    
    /// NEW: Custom interval processing with SYMMETRIC deadband (FIXED VERSION)
    pub fn apply_custom_interval_processing_symmetric(&mut self, interval_meters: f64) {
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
        
        // Update data
        let mut smoothed_altitude_changes = vec![0.0];
        for i in 1..gaussian_smoothed.len() {
            smoothed_altitude_changes.push(gaussian_smoothed[i] - gaussian_smoothed[i - 1]);
        }
        
        self.enhanced_altitude = gaussian_smoothed;
        self.cumulative_distance = uniform_distances;
        self.altitude_change = smoothed_altitude_changes;
        self.distance_change = vec![interval_meters; self.altitude_change.len()];
        self.distance_change[0] = self.cumulative_distance[0];
        
        // Apply SYMMETRIC deadband filtering (FIXED VERSION)
        self.apply_symmetric_deadband_filtering(deadband_threshold);
        self.calculate_gradients();
        self.recalculate_accumulated_values_after_smoothing();
    }
    
    /// Median filter for spike removal
    fn median_filter(data: &[f64], window: usize) -> Vec<f64> {
        let mut result = Vec::with_capacity(data.len());
        
        for i in 0..data.len() {
            let start = if i >= window / 2 { i - window / 2 } else { 0 };
            let end = if i + window / 2 < data.len() { i + window / 2 } else { data.len() - 1 };
            
            let mut window_data: Vec<f64> = data[start..=end].to_vec();
            window_data.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            let median = if window_data.len() % 2 == 0 {
                (window_data[window_data.len() / 2 - 1] + window_data[window_data.len() / 2]) / 2.0
            } else {
                window_data[window_data.len() / 2]
            };
            
            result.push(median);
        }
        
        result
    }
    
    /// Savitzky-Golay-like smoothing (simplified Gaussian)
    fn gaussian_smooth(data: &[f64], window: usize) -> Vec<f64> {
        let mut result = Vec::with_capacity(data.len());
        let sigma = window as f64 / 6.0; // Standard deviation
        
        for i in 0..data.len() {
            let start = if i >= window / 2 { i - window / 2 } else { 0 };
            let end = if i + window / 2 < data.len() { i + window / 2 } else { data.len() - 1 };
            
            let mut weighted_sum = 0.0;
            let mut weight_sum = 0.0;
            
            for j in start..=end {
                let distance = (j as f64 - i as f64).abs();
                let weight = (-0.5 * (distance / sigma).powi(2)).exp();
                
                weighted_sum += data[j] * weight;
                weight_sum += weight;
            }
            
            result.push(weighted_sum / weight_sum);
        }
        
        result
    }
    
    /// LEGACY: Original asymmetric deadband - KEPT FOR BACKWARD COMPATIBILITY
    /// NOTE: This method causes severe loss under-estimation and should be avoided
    #[allow(dead_code)]
    fn apply_deadband_filtering(&mut self, threshold_meters: f64) {
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
                    let climb_per_segment = cumulative_climb / (i - last_significant_idx) as f64;
                    for j in (last_significant_idx + 1)..=i {
                        if j < filtered_changes.len() {
                            filtered_changes[j] = climb_per_segment;
                        } else {
                            filtered_changes.push(climb_per_segment);
                        }
                    }
                    cumulative_climb = 0.0;
                    last_significant_idx = i;
                } else {
                    filtered_changes.push(0.0);
                }
            } else {
                // ⚠️ PROBLEM: Descents are preserved as-is (no deadband filtering)
                filtered_changes.push(change);
                if cumulative_climb > 0.0 {
                    // Reset climb accumulator on descent
                    cumulative_climb = 0.0;
                    last_significant_idx = i;
                }
            }
        }
        
        // Handle any remaining accumulated climb
        if cumulative_climb > 0.0 {
            let climb_per_segment = cumulative_climb / (filtered_changes.len() - last_significant_idx) as f64;
            for j in (last_significant_idx + 1)..filtered_changes.len() {
                filtered_changes[j] = climb_per_segment;
            }
        }
        
        self.altitude_change = filtered_changes;
    }
}

/// Convenience functions for each variant
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

/// NEW: Create with FIXED symmetric deadband processing
pub fn create_custom_symmetric_fixed(elevations: Vec<f64>, distances: Vec<f64>) -> ElevationData {
    ElevationData::new_with_variant(elevations, distances, SmoothingVariant::SymmetricFixed)
}

/// NEW: Create with adaptive quality-based processing
pub fn create_custom_adaptive_quality(elevations: Vec<f64>, distances: Vec<f64>) -> ElevationData {
    ElevationData::new_with_variant(elevations, distances, SmoothingVariant::AdaptiveQuality)
}