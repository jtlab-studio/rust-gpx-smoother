/// Complete GPX Processing Pipeline
/// 
/// This shows exactly how we parse, clean, and process GPX files
/// for accurate elevation gain calculations.

use std::{fs::File, path::Path};
use std::io::BufReader;
use gpx::{read, Time};
use geo::{HaversineDistance, point};

// Your DistBased processor
use crate::distbased_elevation_processor::DistBasedElevationProcessor;

#[derive(Debug, Clone)]
pub struct GpxProcessingResult {
    pub filename: String,
    pub raw_points: usize,
    pub total_distance_km: f64,
    pub raw_elevation_gain_m: f64,
    pub processed_elevation_gain_m: f64,
    pub terrain_type: String,
    pub average_time_interval_seconds: u32,
    pub elevation_range_m: (f64, f64), // (min, max)
    pub processing_stats: ProcessingStats,
}

#[derive(Debug, Clone)]
pub struct ProcessingStats {
    pub points_with_elevation: usize,
    pub points_with_timestamps: usize,
    pub distance_calculation_method: String,
    pub elevation_cleaning_applied: bool,
    pub gps_quality_indicators: GpsQualityMetrics,
}

#[derive(Debug, Clone)]
pub struct GpsQualityMetrics {
    pub average_point_spacing_m: f64,
    pub elevation_noise_ratio: f64,
    pub time_interval_consistency: f64,
    pub coordinates_precision: u8,
}

/// Main GPX processing function - this is what we use for each file
pub fn process_gpx_file(gpx_path: &Path) -> Result<GpxProcessingResult, Box<dyn std::error::Error>> {
    println!("🔄 Processing: {}", gpx_path.display());
    
    // Step 1: Parse GPX file
    let gpx_data = parse_gpx_file(gpx_path)?;
    
    // Step 2: Extract and validate coordinates
    let coords = extract_coordinates(&gpx_data.gpx)?;
    if coords.is_empty() {
        return Err("No valid coordinates with elevation data found".into());
    }
    
    // Step 3: Calculate distances between points
    let distances = calculate_cumulative_distances(&coords);
    
    // Step 4: Extract elevation data
    let elevations: Vec<f64> = coords.iter().map(|c| c.elevation).collect();
    
    // Step 5: Calculate raw elevation gain (before cleaning)
    let raw_elevation_gain = calculate_raw_elevation_gain(&elevations);
    
    // Step 6: Apply DistBased cleaning and processing
    let processor = DistBasedElevationProcessor::new(elevations.clone(), distances.clone());
    let processed_elevation_gain = processor.get_total_elevation_gain();
    
    // Step 7: Analyze GPS quality metrics
    let gps_quality = analyze_gps_quality(&coords, &gpx_data.timestamps, &distances);
    
    // Step 8: Calculate additional statistics
    let total_distance_km = distances.last().unwrap_or(&0.0) / 1000.0;
    let elevation_range = (
        elevations.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
        elevations.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))
    );
    
    let result = GpxProcessingResult {
        filename: gpx_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        raw_points: coords.len(),
        total_distance_km,
        raw_elevation_gain_m: raw_elevation_gain,
        processed_elevation_gain_m: processed_elevation_gain,
        terrain_type: processor.get_terrain_type().to_string(),
        average_time_interval_seconds: gpx_data.average_time_interval,
        elevation_range_m: elevation_range,
        processing_stats: ProcessingStats {
            points_with_elevation: coords.len(),
            points_with_timestamps: gpx_data.timestamps.iter().filter(|t| t.is_some()).count(),
            distance_calculation_method: "Haversine".to_string(),
            elevation_cleaning_applied: true,
            gps_quality_indicators: gps_quality,
        },
    };
    
    println!("  ✅ Processed: {:.1}km, {:.0}m gain → {:.0}m cleaned, {} terrain", 
             result.total_distance_km, 
             result.raw_elevation_gain_m, 
             result.processed_elevation_gain_m,
             result.terrain_type);
    
    Ok(result)
}

#[derive(Debug)]
struct GpxData {
    gpx: gpx::Gpx,
    timestamps: Vec<Option<Time>>,
    average_time_interval: u32,
}

#[derive(Debug, Clone)]
struct Coordinate {
    latitude: f64,
    longitude: f64,
    elevation: f64,
    timestamp: Option<Time>,
}

/// Step 1: Parse GPX file using the gpx crate
fn parse_gpx_file(path: &Path) -> Result<GpxData, Box<dyn std::error::Error>> {
    let file = File::open(path)
        .map_err(|e| format!("Failed to open GPX file: {}", e))?;
    
    let reader = BufReader::new(file);
    let gpx = read(reader)
        .map_err(|e| format!("Failed to parse GPX: {}", e))?;
    
    // Extract timestamps for quality analysis
    let mut timestamps = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for point in &segment.points {
                timestamps.push(point.time);
            }
        }
    }
    
    let average_time_interval = calculate_average_time_interval(&timestamps);
    
    Ok(GpxData {
        gpx,
        timestamps,
        average_time_interval,
    })
}

/// Step 2: Extract coordinates with elevation data
fn extract_coordinates(gpx: &gpx::Gpx) -> Result<Vec<Coordinate>, Box<dyn std::error::Error>> {
    let mut coords = Vec::new();
    
    for track in &gpx.tracks {
        for segment in &track.segments {
            for point in &segment.points {
                // Only include points with elevation data
                if let Some(elevation) = point.elevation {
                    coords.push(Coordinate {
                        latitude: point.point().y(),
                        longitude: point.point().x(),
                        elevation,
                        timestamp: point.time,
                    });
                }
            }
        }
    }
    
    if coords.is_empty() {
        return Err("No trackpoints with elevation data found in GPX file".into());
    }
    
    println!("  📍 Extracted {} points with elevation data", coords.len());
    Ok(coords)
}

/// Step 3: Calculate cumulative distances using Haversine formula
fn calculate_cumulative_distances(coords: &[Coordinate]) -> Vec<f64> {
    let mut distances = vec![0.0];
    
    for i in 1..coords.len() {
        let prev = &coords[i - 1];
        let curr = &coords[i];
        
        let point_a = point!(x: prev.longitude, y: prev.latitude);
        let point_b = point!(x: curr.longitude, y: curr.latitude);
        
        let segment_distance = point_a.haversine_distance(&point_b);
        distances.push(distances[i - 1] + segment_distance);
    }
    
    distances
}

/// Step 4: Calculate raw elevation gain (before DistBased cleaning)
fn calculate_raw_elevation_gain(elevations: &[f64]) -> f64 {
    elevations.windows(2)
        .map(|window| if window[1] > window[0] { window[1] - window[0] } else { 0.0 })
        .sum()
}

/// Step 5: Analyze GPS data quality
fn analyze_gps_quality(
    coords: &[Coordinate], 
    timestamps: &[Option<Time>], 
    distances: &[f64]
) -> GpsQualityMetrics {
    // Calculate average point spacing
    let average_spacing = if coords.len() > 1 {
        distances.last().unwrap() / (coords.len() - 1) as f64
    } else {
        0.0
    };
    
    // Calculate elevation noise ratio
    let elevation_noise = calculate_elevation_noise_ratio(coords);
    
    // Calculate time interval consistency
    let time_consistency = calculate_time_consistency(timestamps);
    
    // Estimate coordinate precision (rough approximation)
    let coord_precision = estimate_coordinate_precision(coords);
    
    GpsQualityMetrics {
        average_point_spacing_m: average_spacing,
        elevation_noise_ratio: elevation_noise,
        time_interval_consistency: time_consistency,
        coordinates_precision: coord_precision,
    }
}

fn calculate_elevation_noise_ratio(coords: &[Coordinate]) -> f64 {
    if coords.len() < 10 {
        return 0.0;
    }
    
    let elevations: Vec<f64> = coords.iter().map(|c| c.elevation).collect();
    
    // Calculate total elevation variation
    let total_variation: f64 = elevations.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .sum();
    
    // Calculate smoothed variation (5-point moving average)
    let window_size = 5;
    let mut smoothed = Vec::new();
    
    for i in 0..elevations.len() {
        let start = if i >= window_size/2 { i - window_size/2 } else { 0 };
        let end = (i + window_size/2 + 1).min(elevations.len());
        let avg = elevations[start..end].iter().sum::<f64>() / (end - start) as f64;
        smoothed.push(avg);
    }
    
    let smooth_variation: f64 = smoothed.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .sum();
    
    // Noise ratio = (total - smooth) / total
    if total_variation > 0.0 {
        (total_variation - smooth_variation) / total_variation
    } else {
        0.0
    }
}

fn calculate_time_consistency(timestamps: &[Option<Time>]) -> f64 {
    let valid_times: Vec<&Time> = timestamps.iter().filter_map(|t| t.as_ref()).collect();
    
    if valid_times.len() < 3 {
        return 0.0;
    }
    
    let mut intervals = Vec::new();
    for i in 1..valid_times.len() {
        if let (Ok(t1), Ok(t2)) = (valid_times[i-1].format(), valid_times[i].format()) {
            if let (Ok(dt1), Ok(dt2)) = (
                t1.parse::<chrono::DateTime<chrono::Utc>>(),
                t2.parse::<chrono::DateTime<chrono::Utc>>()
            ) {
                let interval = dt2.signed_duration_since(dt1).num_seconds();
                if interval > 0 && interval < 3600 {
                    intervals.push(interval as f64);
                }
            }
        }
    }
    
    if intervals.is_empty() {
        return 0.0;
    }
    
    // Calculate coefficient of variation (lower = more consistent)
    let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
    let variance = intervals.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>() / intervals.len() as f64;
    
    let std_dev = variance.sqrt();
    
    // Return consistency score (1.0 = perfect, 0.0 = very inconsistent)
    if mean > 0.0 {
        (1.0 - (std_dev / mean).min(1.0)).max(0.0)
    } else {
        0.0
    }
}

fn estimate_coordinate_precision(coords: &[Coordinate]) -> u8 {
    if coords.is_empty() {
        return 0;
    }
    
    // Find the smallest non-zero difference in coordinates
    let mut min_lat_diff = f64::INFINITY;
    let mut min_lon_diff = f64::INFINITY;
    
    for i in 1..coords.len().min(100) { // Check first 100 points
        let lat_diff = (coords[i].latitude - coords[i-1].latitude).abs();
        let lon_diff = (coords[i].longitude - coords[i-1].longitude).abs();
        
        if lat_diff > 0.0 && lat_diff < min_lat_diff {
            min_lat_diff = lat_diff;
        }
        if lon_diff > 0.0 && lon_diff < min_lon_diff {
            min_lon_diff = lon_diff;
        }
    }
    
    // Estimate decimal places based on smallest difference
    let min_diff = min_lat_diff.min(min_lon_diff);
    if min_diff == f64::INFINITY {
        return 6; // Default assumption
    }
    
    // Count decimal places
    let precision = (-min_diff.log10()).ceil() as u8;
    precision.min(10).max(3) // Reasonable bounds
}

fn calculate_average_time_interval(timestamps: &[Option<Time>]) -> u32 {
    let valid_timestamps: Vec<&Time> = timestamps.iter()
        .filter_map(|ts| ts.as_ref())
        .collect();
    
    if valid_timestamps.len() < 2 {
        return 0;
    }
    
    let mut intervals = Vec::new();
    for i in 1..valid_timestamps.len() {
        let time1_result = valid_timestamps[i-1].format();
        let time2_result = valid_timestamps[i].format();
        
        if let (Ok(time1_str), Ok(time2_str)) = (time1_result, time2_result) {
            let dt1_result = time1_str.parse::<chrono::DateTime<chrono::Utc>>();
            let dt2_result = time2_str.parse::<chrono::DateTime<chrono::Utc>>();
            
            if let (Ok(dt1), Ok(dt2)) = (dt1_result, dt2_result) {
                let interval = dt2.signed_duration_since(dt1);
                let seconds = interval.num_seconds();
                
                if seconds > 0 && seconds <= 3600 {
                    intervals.push(seconds as f64);
                }
            }
        }
    }
    
    if intervals.is_empty() {
        return 0;
    }
    
    let average_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
    average_interval.round() as u32
}

/// Batch process multiple GPX files
pub fn process_gpx_directory(directory: &Path) -> Result<Vec<GpxProcessingResult>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("gpx") {
            match process_gpx_file(&path) {
                Ok(result) => {
                    results.push(result);
                },
                Err(e) => {
                    eprintln!("❌ Error processing {}: {}", path.display(), e);
                }
            }
        }
    }
    
    Ok(results)
}

/// Generate processing summary
pub fn generate_processing_summary(results: &[GpxProcessingResult]) {
    println!("\n📊 GPX PROCESSING SUMMARY");
    println!("=========================");
    println!("Total files processed: {}", results.len());
    
    let total_distance: f64 = results.iter().map(|r| r.total_distance_km).sum();
    let total_points: usize = results.iter().map(|r| r.raw_points).sum();
    
    println!("Total distance: {:.1}km", total_distance);
    println!("Total GPS points: {}", total_points);
    
    // Terrain type distribution
    let mut terrain_counts = std::collections::HashMap::new();
    for result in results {
        *terrain_counts.entry(&result.terrain_type).or_insert(0) += 1;
    }
    
    println!("\nTerrain distribution:");
    for (terrain, count) in terrain_counts {
        println!("  {}: {} files", terrain, count);
    }
    
    // Average processing improvements
    let raw_gains: Vec<f64> = results.iter().map(|r| r.raw_elevation_gain_m).collect();
    let processed_gains: Vec<f64> = results.iter().map(|r| r.processed_elevation_gain_m).collect();
    
    let avg_raw = raw_gains.iter().sum::<f64>() / raw_gains.len() as f64;
    let avg_processed = processed_gains.iter().sum::<f64>() / processed_gains.len() as f64;
    
    println!("\nElevation processing:");
    println!("  Average raw gain: {:.1}m", avg_raw);
    println!("  Average processed gain: {:.1}m", avg_processed);
    println!("  Average cleaning effect: {:.1}%", ((avg_processed - avg_raw) / avg_raw * 100.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_coordinate_extraction() {
        // Test coordinate extraction logic
        let coords = vec![
            Coordinate { latitude: 40.0, longitude: -74.0, elevation: 100.0, timestamp: None },
            Coordinate { latitude: 40.01, longitude: -74.01, elevation: 105.0, timestamp: None },
        ];
        
        let distances = calculate_cumulative_distances(&coords);
        assert_eq!(distances.len(), 2);
        assert_eq!(distances[0], 0.0);
        assert!(distances[1] > 0.0);
    }
    
    #[test]
    fn test_elevation_gain_calculation() {
        let elevations = vec![100.0, 105.0, 103.0, 108.0, 110.0];
        let raw_gain = calculate_raw_elevation_gain(&elevations);
        assert_eq!(raw_gain, 13.0); // 5 + 0 + 5 + 2 = 12, but 103->108 is 5, so 5+5+2 = 12
    }
}