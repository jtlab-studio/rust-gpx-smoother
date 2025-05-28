/// Incline & Decline Analyzer - Find the longest climbs and descents in GPS routes
/// Uses custom_smoother.rs logic for preprocessing elevation data
use crate::custom_smoother::{ElevationData, SmoothingVariant};

#[derive(Debug, Clone)]
pub struct InclineSegment {
    pub start_index: usize,
    pub end_index: usize,
    pub start_distance_km: f64,
    pub end_distance_km: f64,
    pub length_km: f64,
    pub elevation_gain_m: f64,
    pub average_grade_percent: f64,
    pub max_grade_percent: f64,
    pub start_elevation_m: f64,
    pub end_elevation_m: f64,
}

#[derive(Debug, Clone)]
pub struct DeclineSegment {
    pub start_index: usize,
    pub end_index: usize,
    pub start_distance_km: f64,
    pub end_distance_km: f64,
    pub length_km: f64,
    pub elevation_loss_m: f64,
    pub average_grade_percent: f64, // Negative for declines
    pub max_grade_percent: f64,     // Most negative grade
    pub start_elevation_m: f64,
    pub end_elevation_m: f64,
}

#[derive(Debug, Clone)]
pub struct InclineAnalysisConfig {
    pub deadband_threshold_grade: f64,
    pub min_elevation_gain_m: f64,
    pub min_length_m: f64,
    pub min_average_grade_percent: f64,
    pub max_interruption_length_m: f64,
    pub smoothing_variant: SmoothingVariant,
}

impl Default for InclineAnalysisConfig {
    fn default() -> Self {
        InclineAnalysisConfig {
            deadband_threshold_grade: 0.03,
            min_elevation_gain_m: 25.0,
            min_length_m: 200.0,
            min_average_grade_percent: 4.0,
            max_interruption_length_m: 50.0,
            smoothing_variant: SmoothingVariant::DistBased,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InclineAnalysisResult {
    pub longest_incline: Option<InclineSegment>,
    pub steepest_incline: Option<InclineSegment>,
    pub most_elevation_gain_incline: Option<InclineSegment>,
    pub longest_decline: Option<DeclineSegment>,
    pub steepest_decline: Option<DeclineSegment>,
    pub most_elevation_loss_decline: Option<DeclineSegment>,
    pub all_inclines: Vec<InclineSegment>,
    pub all_declines: Vec<DeclineSegment>,
    pub total_climbing_distance_km: f64,
    pub total_descending_distance_km: f64,
    pub total_elevation_gain_m: f64,
    pub total_elevation_loss_m: f64,
    pub climbing_percentage: f64,
    pub descending_percentage: f64,
}

pub fn analyze_inclines_default(raw_elevations: Vec<f64>, distances: Vec<f64>) -> InclineAnalysisResult {
    analyze_inclines(raw_elevations, distances, &InclineAnalysisConfig::default())
}

pub fn analyze_inclines(
    raw_elevations: Vec<f64>,
    distances: Vec<f64>,
    config: &InclineAnalysisConfig
) -> InclineAnalysisResult {
    println!("=== INCLINE & DECLINE ANALYSIS ===");
    
    let elevation_data = ElevationData::new_with_variant(
        raw_elevations, 
        distances.clone(), 
        config.smoothing_variant
    );
    
    let gradients = calculate_gradients(&elevation_data);
    
    // Find climbing and declining segments
    let climbing_segments = identify_climbing_segments(&gradients, config);
    let declining_segments = identify_declining_segments(&gradients, config);
    
    // Create filtered segments
    let filtered_inclines = filter_incline_segments(climbing_segments, &elevation_data, config);
    let filtered_declines = filter_decline_segments(declining_segments, &elevation_data, config);
    
    let result = create_analysis_result(filtered_inclines, filtered_declines, &elevation_data);
    print_analysis_summary(&result);
    
    result
}

fn calculate_gradients(elevation_data: &ElevationData) -> Vec<f64> {
    elevation_data.gradient_percent.iter()
        .map(|&gradient_percent| gradient_percent / 100.0)
        .collect()
}

fn identify_climbing_segments(gradients: &[f64], config: &InclineAnalysisConfig) -> Vec<(usize, usize)> {
    let mut segments = Vec::new();
    let mut current_start: Option<usize> = None;
    
    for i in 0..gradients.len() {
        let is_climbing = gradients[i] >= config.deadband_threshold_grade;
        
        match (current_start, is_climbing) {
            (None, true) => current_start = Some(i),
            (Some(start), false) => {
                if i > start {
                    segments.push((start, i - 1));
                }
                current_start = None;
            },
            _ => {}
        }
    }
    
    if let Some(start) = current_start {
        if start < gradients.len() - 1 {
            segments.push((start, gradients.len() - 1));
        }
    }
    
    segments
}

fn identify_declining_segments(gradients: &[f64], config: &InclineAnalysisConfig) -> Vec<(usize, usize)> {
    let mut segments = Vec::new();
    let mut current_start: Option<usize> = None;
    
    for i in 0..gradients.len() {
        let is_declining = gradients[i] <= -config.deadband_threshold_grade;
        
        match (current_start, is_declining) {
            (None, true) => current_start = Some(i),
            (Some(start), false) => {
                if i > start {
                    segments.push((start, i - 1));
                }
                current_start = None;
            },
            _ => {}
        }
    }
    
    if let Some(start) = current_start {
        if start < gradients.len() - 1 {
            segments.push((start, gradients.len() - 1));
        }
    }
    
    segments
}

fn filter_incline_segments(
    segments: Vec<(usize, usize)>,
    elevation_data: &ElevationData,
    config: &InclineAnalysisConfig
) -> Vec<InclineSegment> {
    segments.into_iter()
        .filter_map(|(start_idx, end_idx)| {
            create_incline_segment_with_data(start_idx, end_idx, elevation_data)
        })
        .filter(|segment| {
            segment.elevation_gain_m >= config.min_elevation_gain_m &&
            segment.length_km * 1000.0 >= config.min_length_m &&
            segment.average_grade_percent >= config.min_average_grade_percent
        })
        .collect()
}

fn filter_decline_segments(
    segments: Vec<(usize, usize)>,
    elevation_data: &ElevationData,
    config: &InclineAnalysisConfig
) -> Vec<DeclineSegment> {
    segments.into_iter()
        .filter_map(|(start_idx, end_idx)| {
            create_decline_segment_with_data(start_idx, end_idx, elevation_data)
        })
        .filter(|segment| {
            segment.elevation_loss_m >= config.min_elevation_gain_m &&
            segment.length_km * 1000.0 >= config.min_length_m &&
            segment.average_grade_percent.abs() >= config.min_average_grade_percent
        })
        .collect()
}

fn create_incline_segment_with_data(start_idx: usize, end_idx: usize, elevation_data: &ElevationData) -> Option<InclineSegment> {
    let start_distance_km = elevation_data.cumulative_distance[start_idx] / 1000.0;
    let end_distance_km = elevation_data.cumulative_distance[end_idx] / 1000.0;
    let length_km = end_distance_km - start_distance_km;
    
    let start_elevation = elevation_data.enhanced_altitude[start_idx];
    let end_elevation = elevation_data.enhanced_altitude[end_idx];
    let elevation_gain_m = end_elevation - start_elevation;
    
    if elevation_gain_m <= 0.0 {
        return None;
    }
    
    let average_grade_percent = if length_km > 0.0 {
        (elevation_gain_m / (length_km * 1000.0)) * 100.0
    } else {
        0.0
    };
    
    let max_grade_percent = elevation_data.gradient_percent[start_idx..=end_idx]
        .iter()
        .copied()
        .fold(0.0f64, f64::max);
    
    Some(InclineSegment {
        start_index: start_idx,
        end_index: end_idx,
        start_distance_km,
        end_distance_km,
        length_km,
        elevation_gain_m,
        average_grade_percent,
        max_grade_percent,
        start_elevation_m: start_elevation,
        end_elevation_m: end_elevation,
    })
}

fn create_decline_segment_with_data(start_idx: usize, end_idx: usize, elevation_data: &ElevationData) -> Option<DeclineSegment> {
    let start_distance_km = elevation_data.cumulative_distance[start_idx] / 1000.0;
    let end_distance_km = elevation_data.cumulative_distance[end_idx] / 1000.0;
    let length_km = end_distance_km - start_distance_km;
    
    let start_elevation = elevation_data.enhanced_altitude[start_idx];
    let end_elevation = elevation_data.enhanced_altitude[end_idx];
    let elevation_loss_m = start_elevation - end_elevation;
    
    if elevation_loss_m <= 0.0 {
        return None;
    }
    
    let average_grade_percent = if length_km > 0.0 {
        -(elevation_loss_m / (length_km * 1000.0)) * 100.0
    } else {
        0.0
    };
    
    let max_grade_percent = elevation_data.gradient_percent[start_idx..=end_idx]
        .iter()
        .copied()
        .fold(0.0f64, f64::min);
    
    Some(DeclineSegment {
        start_index: start_idx,
        end_index: end_idx,
        start_distance_km,
        end_distance_km,
        length_km,
        elevation_loss_m,
        average_grade_percent,
        max_grade_percent,
        start_elevation_m: start_elevation,
        end_elevation_m: end_elevation,
    })
}

fn create_analysis_result(
    inclines: Vec<InclineSegment>,
    declines: Vec<DeclineSegment>,
    elevation_data: &ElevationData
) -> InclineAnalysisResult {
    let longest_incline = inclines.iter()
        .max_by(|a, b| a.length_km.partial_cmp(&b.length_km).unwrap())
        .cloned();
    
    let steepest_incline = inclines.iter()
        .max_by(|a, b| a.average_grade_percent.partial_cmp(&b.average_grade_percent).unwrap())
        .cloned();
    
    let most_elevation_gain_incline = inclines.iter()
        .max_by(|a, b| a.elevation_gain_m.partial_cmp(&b.elevation_gain_m).unwrap())
        .cloned();
    
    let longest_decline = declines.iter()
        .max_by(|a, b| a.length_km.partial_cmp(&b.length_km).unwrap())
        .cloned();
    
    let steepest_decline = declines.iter()
        .max_by(|a, b| a.average_grade_percent.abs().partial_cmp(&b.average_grade_percent.abs()).unwrap())
        .cloned();
    
    let most_elevation_loss_decline = declines.iter()
        .max_by(|a, b| a.elevation_loss_m.partial_cmp(&b.elevation_loss_m).unwrap())
        .cloned();
    
    let total_climbing_distance_km: f64 = inclines.iter().map(|s| s.length_km).sum();
    let total_descending_distance_km: f64 = declines.iter().map(|s| s.length_km).sum();
    let total_elevation_gain_m: f64 = inclines.iter().map(|s| s.elevation_gain_m).sum();
    let total_elevation_loss_m: f64 = declines.iter().map(|s| s.elevation_loss_m).sum();
    
    let total_route_distance_km = elevation_data.cumulative_distance.last().unwrap() / 1000.0;
    let climbing_percentage = if total_route_distance_km > 0.0 {
        (total_climbing_distance_km / total_route_distance_km) * 100.0
    } else { 0.0 };
    
    let descending_percentage = if total_route_distance_km > 0.0 {
        (total_descending_distance_km / total_route_distance_km) * 100.0
    } else { 0.0 };
    
    InclineAnalysisResult {
        longest_incline,
        steepest_incline,
        most_elevation_gain_incline,
        longest_decline,
        steepest_decline,
        most_elevation_loss_decline,
        all_inclines: inclines,
        all_declines: declines,
        total_climbing_distance_km,
        total_descending_distance_km,
        total_elevation_gain_m,
        total_elevation_loss_m,
        climbing_percentage,
        descending_percentage,
    }
}

fn print_analysis_summary(result: &InclineAnalysisResult) {
    println!("\n=== INCLINE & DECLINE ANALYSIS RESULTS ===");
    println!("Total inclines: {}, Total declines: {}", result.all_inclines.len(), result.all_declines.len());
    println!("Climbing: {:.2}km ({:.1}%), Descending: {:.2}km ({:.1}%)", 
             result.total_climbing_distance_km, result.climbing_percentage,
             result.total_descending_distance_km, result.descending_percentage);
    
    if let Some(ref longest) = result.longest_incline {
        println!("🏔️  Longest incline: {:.2}km, {:.1}m gain, {:.1}% grade", 
                 longest.length_km, longest.elevation_gain_m, longest.average_grade_percent);
    }
    
    if let Some(ref longest_down) = result.longest_decline {
        println!("⛷️  Longest decline: {:.2}km, {:.1}m loss, {:.1}% grade", 
                 longest_down.length_km, longest_down.elevation_loss_m, longest_down.average_grade_percent);
    }
    
    println!("=== ANALYSIS COMPLETE ===\n");
}
