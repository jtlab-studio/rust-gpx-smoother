/// NUCLEAR SPIKE REMOVAL - Simplified brute force approach
/// Complex logic is broken - going back to basics with AGGRESSIVE raw ratio thresholds

use crate::custom_smoother::{ElevationData, create_custom_distbased_adaptive};

#[derive(Debug, Clone)]
pub struct GpsQualityMetrics {
    pub raw_elevation_gain: f64,
    pub distance_km: f64,
    pub raw_ratio: f64,
    pub needs_spike_removal: bool,
    pub quality_score: f64,
    pub decision_reason: String,
}

impl GpsQualityMetrics {
    pub fn analyze_gps_quality(
        elevations: &[f64], 
        distances: &[f64], 
        _timestamps: Option<&[f64]>
    ) -> Self {
        let raw_elevation_gain = calculate_raw_elevation_gain(elevations);
        let distance_km = distances.last().unwrap_or(&0.0) / 1000.0;
        
        // NUCLEAR SIMPLIFICATION - Direct ratio calculation based on distance only
        let expected_max_gain = distance_km * 20.0; // 20m/km maximum reasonable gain for ANY terrain
        
        let raw_ratio = if expected_max_gain > 0.0 { 
            raw_elevation_gain / expected_max_gain 
        } else { 
            1.0 
        };
        
        // NUCLEAR DECISION MAKING - Catch EVERYTHING suspicious
        let (needs_spike_removal, decision_reason) = if raw_ratio > 2.5 {
            (true, format!("EXTREME: {:.1}x expected gain ({:.0}m vs {:.0}m max)", raw_ratio, raw_elevation_gain, expected_max_gain))
        } else if raw_ratio > 1.8 {
            (true, format!("VERY HIGH: {:.1}x expected gain ({:.0}m vs {:.0}m max)", raw_ratio, raw_elevation_gain, expected_max_gain))
        } else if raw_ratio > 1.3 {
            (true, format!("HIGH: {:.1}x expected gain ({:.0}m vs {:.0}m max)", raw_ratio, raw_elevation_gain, expected_max_gain))
        } else if raw_ratio > 1.1 {
            (true, format!("SUSPICIOUS: {:.1}x expected gain ({:.0}m vs {:.0}m max)", raw_ratio, raw_elevation_gain, expected_max_gain))
        } else {
            (false, format!("OK: {:.1}x expected gain ({:.0}m vs {:.0}m max)", raw_ratio, raw_elevation_gain, expected_max_gain))
        };
        
        println!("💥 NUCLEAR GPS Analysis:");
        println!("  Distance: {:.1}km, Raw gain: {:.0}m", distance_km, raw_elevation_gain);
        println!("  Max reasonable: {:.0}m, Ratio: {:.1}x", expected_max_gain, raw_ratio);
        println!("  Decision: {} - {}", if needs_spike_removal { "💥 NUKE SPIKES" } else { "SKIP" }, decision_reason);
        
        GpsQualityMetrics {
            raw_elevation_gain,
            distance_km,
            raw_ratio,
            needs_spike_removal,
            quality_score: raw_ratio,
            decision_reason,
        }
    }
}

fn calculate_raw_elevation_gain(elevations: &[f64]) -> f64 {
    elevations.windows(2)
        .map(|w| if w[1] > w[0] { w[1] - w[0] } else { 0.0 })
        .sum()
}

/// Nuclear smart processing - BRUTE FORCE spike removal
pub fn smart_spike_distbased(
    elevations: Vec<f64>, 
    distances: Vec<f64>,
    timestamps: Option<Vec<f64>>
) -> f64 {
    println!("💥 Running NUCLEAR BRUTE FORCE Spike Analysis...");
    
    let quality_metrics = GpsQualityMetrics::analyze_gps_quality(
        &elevations, 
        &distances, 
        timestamps.as_deref()
    );
    
    if quality_metrics.needs_spike_removal {
        println!("💥 Applying NUCLEAR Spike Removal");
        println!("   Reason: {}", quality_metrics.decision_reason);
        
        // Apply NUCLEAR spike removal - multiple aggressive passes
        let spike_removed = nuclear_spike_removal(&elevations, quality_metrics.raw_ratio);
        
        // Apply DistBased processing
        let distbased_result = create_custom_distbased_adaptive(spike_removed, distances);
        distbased_result.get_total_elevation_gain()
    } else {
        println!("🎯 Applying DistBased only");
        println!("   Reason: {}", quality_metrics.decision_reason);
        
        // Skip spike removal, apply DistBased directly
        let distbased_result = create_custom_distbased_adaptive(elevations, distances);
        distbased_result.get_total_elevation_gain()
    }
}

/// NUCLEAR spike removal - DESTROY ALL SPIKES with extreme prejudice
fn nuclear_spike_removal(elevations: &[f64], raw_ratio: f64) -> Vec<f64> {
    if elevations.len() < 3 {
        return elevations.to_vec();
    }
    
    let original_gain = calculate_raw_elevation_gain(elevations);
    let mut result = elevations.to_vec();
    let mut total_spikes_removed = 0;
    
    // NUCLEAR THRESHOLDS - based on how bad the data is
    let thresholds = if raw_ratio > 2.5 {
        vec![6.0, 4.0, 3.0, 2.0, 1.5]  // EXTREME case - 5 passes
    } else if raw_ratio > 1.8 {
        vec![5.0, 3.5, 2.5, 1.5]       // VERY HIGH - 4 passes
    } else if raw_ratio > 1.3 {
        vec![4.0, 3.0, 2.0]            // HIGH - 3 passes
    } else {
        vec![3.0, 2.0]                 // SUSPICIOUS - 2 passes
    };
    
    println!("💥 NUCLEAR SPIKE REMOVAL: {} passes for {:.1}x ratio", thresholds.len(), raw_ratio);
    
    for (pass, &threshold_m) in thresholds.iter().enumerate() {
        let mut pass_spikes = 0;
        
        for i in 1..result.len() - 1 {
            let prev = result[i - 1];
            let curr = result[i];
            let next = result[i + 1];
            
            let up_change = curr - prev;
            let down_change = next - curr;
            
            // NUCLEAR spike detection - catch EVERYTHING
            if up_change.abs() > threshold_m || down_change.abs() > threshold_m {
                if up_change.signum() != down_change.signum() && 
                   (up_change.abs() > threshold_m * 0.5 && down_change.abs() > threshold_m * 0.5) {
                    
                    // NUCLEAR interpolation - replace with average
                    result[i] = (prev + next) / 2.0;
                    pass_spikes += 1;
                }
            }
        }
        
        total_spikes_removed += pass_spikes;
        println!("💥 Pass {} ({:.1}m): {} spikes NUKED", pass + 1, threshold_m, pass_spikes);
        
        if pass_spikes == 0 { break; }
    }
    
    // ADDITIONAL NUCLEAR SMOOTHING for extreme cases
    if raw_ratio > 2.0 {
        println!("💥 Applying NUCLEAR smoothing for extreme case");
        
        // 5-point smoothing for very bad data
        let mut smoothed = result.clone();
        for i in 2..result.len() - 2 {
            smoothed[i] = (result[i-2] + result[i-1] + result[i] + result[i+1] + result[i+2]) / 5.0;
        }
        
        // Blend based on severity
        let blend = ((raw_ratio - 2.0) / 2.0).min(0.4);
        for i in 0..result.len() {
            result[i] = result[i] * (1.0 - blend) + smoothed[i] * blend;
        }
    }
    
    let final_gain = calculate_raw_elevation_gain(&result);
    
    println!("💥 NUCLEAR SPIKE REMOVAL COMPLETE:");
    println!("   {} total spikes DESTROYED", total_spikes_removed);
    println!("   {:.1}m → {:.1}m ({:+.1}% change)", 
             original_gain, final_gain, 
             ((final_gain - original_gain) / original_gain) * 100.0);
    
    result
}

/// Nuclear version for backward compatibility
pub fn simple_spike_removal_only(elevations: &[f64], distances: &[f64]) -> Vec<f64> {
    let quality_metrics = GpsQualityMetrics::analyze_gps_quality(elevations, distances, None);
    
    if quality_metrics.needs_spike_removal {
        nuclear_spike_removal(elevations, quality_metrics.raw_ratio)
    } else {
        elevations.to_vec()
    }
}
