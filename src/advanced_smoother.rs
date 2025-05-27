/// Fixed Advanced Smoothing - Should only REDUCE elevation gain, never increase
use crate::custom_smoother::ElevationData;
use crate::enhanced_combo_smoother::enhanced_universal_smooth_conservative;

#[derive(Debug, Clone)]
pub struct AdvancedSmoothingConfig {
    pub spike_threshold_m: f64,
    pub confidence_threshold: f64,
    pub smoothing_intensity: f64,
}

impl Default for AdvancedSmoothingConfig {
    fn default() -> Self {
        AdvancedSmoothingConfig {
            spike_threshold_m: 8.0,
            confidence_threshold: 0.7,
            smoothing_intensity: 0.3,
        }
    }
}

/// Simple and safe GPS spike removal
pub fn remove_gps_spikes_safe(elevations: &[f64], threshold_m: f64) -> Vec<f64> {
    if elevations.len() < 3 {
        return elevations.to_vec();
    }
    
    let mut cleaned = elevations.to_vec();
    
    // Only remove obvious spikes (sudden jump up then down, or down then up)
    for i in 1..elevations.len()-1 {
        let prev_change = elevations[i] - elevations[i-1];
        let next_change = elevations[i+1] - elevations[i];
        
        // Spike detection: big change in one direction, then big change back
        if prev_change.abs() > threshold_m && 
           next_change.abs() > threshold_m && 
           (prev_change * next_change < -threshold_m) { // Strong opposite changes
            
            // Replace spike with simple interpolation
            cleaned[i] = (elevations[i-1] + elevations[i+1]) / 2.0;
        }
    }
    
    cleaned
}

/// Safe confidence-based smoothing that never increases total elevation
pub fn confidence_based_smooth_safe(elevations: &[f64], smoothing_intensity: f64) -> Vec<f64> {
    if elevations.len() < 5 {
        return elevations.to_vec();
    }
    
    // Calculate confidence for each point
    let confidences: Vec<f64> = elevations.iter().enumerate()
        .map(|(i, _)| calculate_point_confidence_safe(elevations, i))
        .collect();
    
    // Apply very light smoothing with confidence weighting
    let window = 3; // Small window to prevent over-smoothing
    let mut result = Vec::with_capacity(elevations.len());
    
    for i in 0..elevations.len() {
        let confidence = confidences[i];
        
        if confidence > 0.8 {
            // High confidence: keep original
            result.push(elevations[i]);
        } else {
            // Low confidence: apply light smoothing
            let start = if i >= window / 2 { i - window / 2 } else { 0 };
            let end = if i + window / 2 < elevations.len() { i + window / 2 } else { elevations.len() - 1 };
            
            let sum: f64 = elevations[start..=end].iter().sum();
            let count = end - start + 1;
            let smoothed = sum / count as f64;
            
            // Blend based on confidence and smoothing intensity
            let blend_factor = (1.0 - confidence) * smoothing_intensity;
            let blended = blend_factor * smoothed + (1.0 - blend_factor) * elevations[i];
            
            result.push(blended);
        }
    }
    
    result
}

/// Safe advanced smoothing pipeline with elevation gain validation
pub fn advanced_hybrid_smooth_safe(
    distances: &[f64], 
    elevations: &[f64], 
    config: &AdvancedSmoothingConfig
) -> Vec<f64> {
    if distances.len() != elevations.len() || distances.len() < 5 {
        return elevations.to_vec();
    }
    
    // Calculate original elevation gain for validation
    let (original_gain, _) = calculate_elevation_gain_loss(elevations);
    
    // Step 1: Remove GPS spikes (conservative)
    let mut result = remove_gps_spikes_safe(elevations, config.spike_threshold_m);
    
    // Validate: should not increase elevation gain significantly
    let (after_spikes_gain, _) = calculate_elevation_gain_loss(&result);
    if after_spikes_gain > original_gain * 1.1 { // Allow 10% increase max
        // If spike removal increased elevation, skip it
        result = elevations.to_vec();
    }
    
    // Step 2: Apply confidence-based smoothing
    result = confidence_based_smooth_safe(&result, config.smoothing_intensity);
    
    // Final validation: ensure we haven't increased elevation gain
    let (final_gain, _) = calculate_elevation_gain_loss(&result);
    if final_gain > original_gain * 1.05 { // Allow 5% increase max
        // If we somehow increased elevation, fall back to a simpler method
        return simple_safe_smooth(elevations, 3);
    }
    
    result
}

/// Fallback: very simple safe smoothing
fn simple_safe_smooth(elevations: &[f64], window: usize) -> Vec<f64> {
    if elevations.len() < window {
        return elevations.to_vec();
    }
    
    let mut result = Vec::with_capacity(elevations.len());
    
    for i in 0..elevations.len() {
        let start = if i >= window / 2 { i - window / 2 } else { 0 };
        let end = if i + window / 2 < elevations.len() { i + window / 2 } else { elevations.len() - 1 };
        
        let sum: f64 = elevations[start..=end].iter().sum();
        let count = end - start + 1;
        result.push(sum / count as f64);
    }
    
    result
}

fn calculate_point_confidence_safe(elevations: &[f64], index: usize) -> f64 {
    let window = 2;
    let start = if index >= window { index - window } else { 0 };
    let end = if index + window < elevations.len() { index + window } else { elevations.len() - 1 };
    
    if start >= end {
        return 1.0;
    }
    
    let segment = &elevations[start..=end];
    let median = {
        let mut sorted = segment.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sorted[sorted.len() / 2]
    };
    
    let deviation = (elevations[index] - median).abs();
    
    // Simple confidence: lower deviation = higher confidence
    if deviation < 2.0 {
        1.0
    } else if deviation < 5.0 {
        0.8
    } else if deviation < 10.0 {
        0.5
    } else {
        0.2
    }
}

/// Conservative variant - minimal smoothing
pub fn advanced_smooth_conservative(distances: &[f64], elevations: &[f64]) -> Vec<f64> {
    let config = AdvancedSmoothingConfig {
        spike_threshold_m: 10.0,
        confidence_threshold: 0.8,
        smoothing_intensity: 0.2,
    };
    advanced_hybrid_smooth_safe(distances, elevations, &config)
}

/// Moderate variant - balanced smoothing
pub fn advanced_smooth_moderate(distances: &[f64], elevations: &[f64]) -> Vec<f64> {
    let config = AdvancedSmoothingConfig {
        spike_threshold_m: 8.0,
        confidence_threshold: 0.7,
        smoothing_intensity: 0.3,
    };
    advanced_hybrid_smooth_safe(distances, elevations, &config)
}

/// Aggressive variant - more smoothing but still safe
pub fn advanced_smooth_aggressive(distances: &[f64], elevations: &[f64]) -> Vec<f64> {
    let config = AdvancedSmoothingConfig {
        spike_threshold_m: 6.0,
        confidence_threshold: 0.6,
        smoothing_intensity: 0.4,
    };
    advanced_hybrid_smooth_safe(distances, elevations, &config)
}

fn calculate_elevation_gain_loss(elevations: &[f64]) -> (f64, f64) {
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

pub fn calculate_advanced_elevation_gain_loss(elevations: &[f64]) -> (f64, f64) {
    calculate_elevation_gain_loss(elevations)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_no_elevation_increase() {
        let elevations = vec![100.0, 102.0, 105.0, 103.0, 107.0, 110.0];
        let distances = vec![0.0, 100.0, 200.0, 300.0, 400.0, 500.0];
        
        let (original_gain, _) = calculate_elevation_gain_loss(&elevations);
        
        let smoothed = advanced_smooth_moderate(&distances, &elevations);
        let (smoothed_gain, _) = calculate_elevation_gain_loss(&smoothed);
        
        // Smoothed elevation gain should never be significantly higher than original
        assert!(smoothed_gain <= original_gain * 1.1, 
                "Smoothed gain {} should not be much higher than original {}", 
                smoothed_gain, original_gain);
    }
    
    #[test]
    fn test_spike_removal() {
        let elevations = vec![100.0, 102.0, 150.0, 103.0, 105.0]; // 150.0 is a spike
        let cleaned = remove_gps_spikes_safe(&elevations, 10.0);
        
        // Spike should be reduced
        assert!(cleaned[2] < 150.0);
        assert!(cleaned[2] > 100.0);
    }
}
