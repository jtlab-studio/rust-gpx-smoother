/// Universal Hybrid Smoothing Algorithm
/// Combines Custom and Enhanced Combo Conservative approaches based on terrain analysis
/// Works for any GPX file without hardcoded route-specific optimizations
use crate::custom_smoother::ElevationData;
use crate::enhanced_combo_smoother::{enhanced_universal_smooth_conservative, calculate_elevation_gain_loss_enhanced};

#[derive(Debug, Clone)]
pub struct TerrainAnalysis {
    pub total_elevation_gain: f64,
    pub total_elevation_loss: f64,
    pub total_distance_km: f64,
    pub elevation_gain_per_km: f64,
    pub elevation_loss_per_km: f64,
    pub elevation_variability: f64,
    pub max_gradient: f64,
    pub avg_gradient: f64,
    pub terrain_roughness: f64,
    pub terrain_type: TerrainType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerrainType {
    VeryFlat,       // Minimal elevation change
    Flat,           // Low elevation change
    Undulating,     // Moderate, rolling terrain
    Hilly,          // Significant elevation changes
    Mountainous,    // Very significant elevation changes
}

#[derive(Debug, Clone)]
pub struct HybridConfig {
    /// Terrain classification thresholds
    pub very_flat_threshold_m_per_km: f64,
    pub flat_threshold_m_per_km: f64,
    pub undulating_threshold_m_per_km: f64,
    pub hilly_threshold_m_per_km: f64,
    /// Roughness threshold (variability indicator)
    pub roughness_threshold: f64,
    /// Smoothing intensity factors for each terrain type
    pub very_flat_smoothing_intensity: f64,
    pub flat_smoothing_intensity: f64,
    pub undulating_smoothing_intensity: f64,
    pub hilly_smoothing_intensity: f64,
    pub mountainous_smoothing_intensity: f64,
}

impl Default for HybridConfig {
    fn default() -> Self {
        HybridConfig {
            // Universal thresholds based on elevation gain per km
            very_flat_threshold_m_per_km: 8.0,
            flat_threshold_m_per_km: 20.0,
            undulating_threshold_m_per_km: 40.0,
            hilly_threshold_m_per_km: 70.0,
            roughness_threshold: 5.0,
            // Smoothing intensities (0.0 = no smoothing, 1.0 = full smoothing)
            very_flat_smoothing_intensity: 0.1,      // Minimal smoothing
            flat_smoothing_intensity: 0.3,           // Light smoothing
            undulating_smoothing_intensity: 0.5,     // Moderate smoothing
            hilly_smoothing_intensity: 0.7,          // Strong smoothing
            mountainous_smoothing_intensity: 0.8,    // Very strong smoothing
        }
    }
}

impl HybridConfig {
    /// Conservative - preserves more original character
    pub fn conservative() -> Self {
        HybridConfig {
            very_flat_threshold_m_per_km: 6.0,
            flat_threshold_m_per_km: 15.0,
            undulating_threshold_m_per_km: 30.0,
            hilly_threshold_m_per_km: 55.0,
            very_flat_smoothing_intensity: 0.05,
            flat_smoothing_intensity: 0.2,
            undulating_smoothing_intensity: 0.4,
            hilly_smoothing_intensity: 0.6,
            mountainous_smoothing_intensity: 0.7,
            ..Default::default()
        }
    }
    
    /// Aggressive - more smoothing across all terrain types
    pub fn aggressive() -> Self {
        HybridConfig {
            very_flat_threshold_m_per_km: 12.0,
            flat_threshold_m_per_km: 30.0,
            undulating_threshold_m_per_km: 60.0,
            hilly_threshold_m_per_km: 100.0,
            very_flat_smoothing_intensity: 0.2,
            flat_smoothing_intensity: 0.4,
            undulating_smoothing_intensity: 0.6,
            hilly_smoothing_intensity: 0.8,
            mountainous_smoothing_intensity: 0.9,
            ..Default::default()
        }
    }
}

/// Analyze terrain characteristics using only elevation and distance data
pub fn analyze_terrain(distances: &[f64], elevations: &[f64]) -> TerrainAnalysis {
    if distances.len() != elevations.len() || distances.len() < 3 {
        return TerrainAnalysis {
            total_elevation_gain: 0.0,
            total_elevation_loss: 0.0,
            total_distance_km: 0.0,
            elevation_gain_per_km: 0.0,
            elevation_loss_per_km: 0.0,
            elevation_variability: 0.0,
            max_gradient: 0.0,
            avg_gradient: 0.0,
            terrain_roughness: 0.0,
            terrain_type: TerrainType::VeryFlat,
        };
    }
    
    // Calculate basic metrics
    let (total_gain, total_loss) = calculate_elevation_gain_loss(elevations);
    let total_distance_km = distances.last().unwrap() / 1000.0;
    
    let elevation_gain_per_km = if total_distance_km > 0.0 {
        total_gain / total_distance_km
    } else {
        0.0
    };
    
    let elevation_loss_per_km = if total_distance_km > 0.0 {
        total_loss / total_distance_km
    } else {
        0.0
    };
    
    // Calculate gradients
    let gradients: Vec<f64> = distances.windows(2)
        .zip(elevations.windows(2))
        .map(|(d_win, e_win)| {
            let dx = d_win[1] - d_win[0];
            if dx.abs() < 1e-10 {
                0.0
            } else {
                (e_win[1] - e_win[0]) / dx * 100.0
            }
        })
        .collect();
    
    let max_gradient = gradients.iter().map(|g| g.abs()).fold(0.0, f64::max);
    let avg_gradient = gradients.iter().map(|g| g.abs()).sum::<f64>() / gradients.len() as f64;
    
    // Calculate elevation variability (how much elevation bounces around)
    let elevation_changes: Vec<f64> = elevations.windows(2)
        .map(|w| w[1] - w[0])
        .collect();
    
    let mean_change = elevation_changes.iter().sum::<f64>() / elevation_changes.len() as f64;
    let variance = elevation_changes.iter()
        .map(|x| (x - mean_change).powi(2))
        .sum::<f64>() / elevation_changes.len() as f64;
    let elevation_variability = variance.sqrt();
    
    // Calculate terrain roughness (how frequently elevation changes direction)
    let direction_changes = elevation_changes.windows(2)
        .filter(|pair| (pair[0] > 0.0) != (pair[1] > 0.0)) // Sign change
        .count();
    let terrain_roughness = direction_changes as f64 / elevation_changes.len() as f64;
    
    // Classify terrain based on multiple factors
    let terrain_type = classify_terrain_universal(
        elevation_gain_per_km,
        elevation_loss_per_km,
        max_gradient,
        avg_gradient,
        terrain_roughness
    );
    
    TerrainAnalysis {
        total_elevation_gain: total_gain,
        total_elevation_loss: total_loss,
        total_distance_km,
        elevation_gain_per_km,
        elevation_loss_per_km,
        elevation_variability,
        max_gradient,
        avg_gradient,
        terrain_roughness,
        terrain_type,
    }
}

fn classify_terrain_universal(
    gain_per_km: f64,
    loss_per_km: f64,
    max_gradient: f64,
    avg_gradient: f64,
    roughness: f64
) -> TerrainType {
    let total_elevation_per_km = gain_per_km + loss_per_km;
    
    // Use multiple factors for classification
    if total_elevation_per_km < 8.0 && max_gradient < 4.0 && avg_gradient < 1.5 {
        TerrainType::VeryFlat
    } else if total_elevation_per_km < 20.0 && max_gradient < 8.0 && avg_gradient < 3.0 {
        TerrainType::Flat
    } else if total_elevation_per_km < 40.0 && max_gradient < 15.0 {
        TerrainType::Undulating
    } else if total_elevation_per_km < 70.0 && max_gradient < 25.0 {
        TerrainType::Hilly
    } else {
        TerrainType::Mountainous
    }
}

/// Universal hybrid smoothing that adapts to any terrain
pub fn hybrid_smooth(
    distances: &[f64], 
    elevations: &[f64], 
    config: &HybridConfig
) -> Vec<f64> {
    if distances.len() != elevations.len() || distances.len() < 5 {
        return elevations.to_vec();
    }
    
    // Analyze terrain
    let terrain = analyze_terrain(distances, elevations);
    
    // Determine smoothing intensity based on terrain type
    let smoothing_intensity = match terrain.terrain_type {
        TerrainType::VeryFlat => config.very_flat_smoothing_intensity,
        TerrainType::Flat => config.flat_smoothing_intensity,
        TerrainType::Undulating => config.undulating_smoothing_intensity,
        TerrainType::Hilly => config.hilly_smoothing_intensity,
        TerrainType::Mountainous => config.mountainous_smoothing_intensity,
    };
    
    // Apply appropriate smoothing method based on terrain
    match terrain.terrain_type {
        TerrainType::VeryFlat | TerrainType::Flat => {
            // Use Custom smoothing for flatter terrain
            let custom_data = ElevationData::new(elevations.to_vec(), distances.to_vec());
            let mut custom_smoothed = Vec::with_capacity(elevations.len());
            custom_smoothed.push(elevations[0]);
            
            // Reconstruct from custom processed altitude changes
            for i in 1..custom_data.altitude_change.len() {
                let prev_elevation = custom_smoothed[i - 1];
                custom_smoothed.push(prev_elevation + custom_data.altitude_change[i]);
            }
            
            // Blend custom result with original based on smoothing intensity
            elevations.iter()
                .zip(custom_smoothed.iter())
                .map(|(&original, &smoothed)| {
                    smoothing_intensity * smoothed + (1.0 - smoothing_intensity) * original
                })
                .collect()
        },
        
        TerrainType::Undulating | TerrainType::Hilly | TerrainType::Mountainous => {
            // Use Enhanced Combo Conservative for hillier terrain
            let enhanced_smoothed = enhanced_universal_smooth_conservative(distances, elevations);
            
            // Blend enhanced result with original based on smoothing intensity
            elevations.iter()
                .zip(enhanced_smoothed.iter())
                .map(|(&original, &smoothed)| {
                    smoothing_intensity * smoothed + (1.0 - smoothing_intensity) * original
                })
                .collect()
        }
    }
}

/// Convenience functions with different configurations
pub fn hybrid_smooth_auto(distances: &[f64], elevations: &[f64]) -> Vec<f64> {
    hybrid_smooth(distances, elevations, &HybridConfig::default())
}

pub fn hybrid_smooth_conservative(distances: &[f64], elevations: &[f64]) -> Vec<f64> {
    hybrid_smooth(distances, elevations, &HybridConfig::conservative())
}

pub fn hybrid_smooth_aggressive(distances: &[f64], elevations: &[f64]) -> Vec<f64> {
    hybrid_smooth(distances, elevations, &HybridConfig::aggressive())
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

pub fn calculate_hybrid_elevation_gain_loss(elevations: &[f64]) -> (f64, f64) {
    calculate_elevation_gain_loss(elevations)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_universal_terrain_classification() {
        // Very flat terrain
        let flat_distances = vec![0.0, 10000.0, 20000.0, 30000.0, 40000.0];
        let flat_elevations = vec![100.0, 102.0, 101.0, 103.0, 104.0]; // ~4m total over 40km
        let flat_analysis = analyze_terrain(&flat_distances, &flat_elevations);
        assert_eq!(flat_analysis.terrain_type, TerrainType::VeryFlat);
        
        // Hilly terrain  
        let hilly_distances = vec![0.0, 5000.0, 10000.0, 15000.0, 20000.0];
        let hilly_elevations = vec![100.0, 200.0, 150.0, 250.0, 180.0]; // High elevation changes
        let hilly_analysis = analyze_terrain(&hilly_distances, &hilly_elevations);
        assert!(matches!(hilly_analysis.terrain_type, TerrainType::Hilly | TerrainType::Mountainous));
    }
    
    #[test]
    fn test_hybrid_smoothing_intensity() {
        let distances = vec![0.0, 1000.0, 2000.0, 3000.0, 4000.0];
        let elevations = vec![100.0, 105.0, 103.0, 108.0, 106.0];
        
        let conservative = hybrid_smooth_conservative(&distances, &elevations);
        let aggressive = hybrid_smooth_aggressive(&distances, &elevations);
        
        // Conservative should be closer to original than aggressive
        let conservative_diff: f64 = elevations.iter()
            .zip(conservative.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
            
        let aggressive_diff: f64 = elevations.iter()
            .zip(aggressive.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
            
        assert!(conservative_diff < aggressive_diff);
    }
}
