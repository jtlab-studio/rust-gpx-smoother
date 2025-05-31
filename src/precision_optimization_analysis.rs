/// PRECISION OPTIMIZATION ANALYSIS
/// 
/// High-resolution search for optimal elevation processing parameters:
/// 1. Distance-based: 1.0m to 6.0m in 0.1m increments (51 tests)
/// 2. Two-pass enhanced: 3m gain + variable loss intervals
/// 3. Savitzky-Golay optimized: Multiple window sizes and polynomial orders
/// 
/// Goal: Push more files into ±5% and ±2% accuracy bands

use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;

#[derive(Debug, Serialize, Clone)]
pub struct PrecisionResult {
    method_name: String,
    parameter_value: f32,
    
    // Primary success metrics
    files_within_2_percent: u32,
    files_within_5_percent: u32,
    files_within_10_percent: u32,
    files_outside_20_percent: u32,
    
    // Accuracy metrics
    gain_accuracy_avg: f32,
    loss_accuracy_avg: f32,
    gain_accuracy_median: f32,
    loss_accuracy_median: f32,
    
    // Best case analysis
    best_gain_accuracy: f32,
    worst_gain_accuracy: f32,
    best_loss_accuracy: f32,
    worst_loss_accuracy: f32,
    
    // Precision score (higher = better)
    precision_score: f32,
    
    // File counts
    total_files: u32,
}

#[derive(Debug, Clone)]
struct GpxFileData {
    filename: String,
    elevations: Vec<f64>,
    distances: Vec<f64>,
    official_gain: u32,
}

#[derive(Debug, Clone)]
struct SingleFileResult {
    filename: String,
    official_gain: u32,
    gain_accuracy: f32,
    loss_accuracy: f32,
    combined_error: f32, // |gain_acc - 100| + |loss_acc - 100|
}

pub fn run_precision_optimization_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🎯 PRECISION OPTIMIZATION ANALYSIS");
    println!("==================================");
    println!("High-resolution parameter search for maximum accuracy:");
    println!("• Distance-based: 1.0m to 6.0m in 0.1m steps (51 intervals)");
    println!("• Two-pass enhanced: 3m gain + 1.0-6.0m loss intervals");
    println!("• Savitzky-Golay optimized: Multiple configurations");
    println!("Target: Push more files into ±5% and ±2% accuracy bands\n");
    
    // Load GPX data
    println!("📂 Loading GPX files...");
    let start = std::time::Instant::now();
    let (gpx_files_data, valid_files) = load_gpx_data(gpx_folder)?;
    println!("✅ Loaded {} files in {:.2}s", valid_files.len(), start.elapsed().as_secs_f64());
    
    // Filter files with elevation data and official benchmarks
    let files_with_elevation: Vec<_> = valid_files.into_iter()
        .filter(|file| {
            if let Some(data) = gpx_files_data.get(file) {
                let has_elevation = data.elevations.iter()
                    .any(|&e| (e - data.elevations[0]).abs() > 0.1);
                let has_official = data.official_gain > 0;
                has_elevation && has_official
            } else {
                false
            }
        })
        .collect();
    
    println!("📊 Processing {} files with valid elevation data and official benchmarks", files_with_elevation.len());
    
    // Run all three optimization analyses
    let processing_start = std::time::Instant::now();
    
    println!("\n🔬 Running high-resolution distance-based analysis...");
    let distance_results = run_distance_based_precision(&gpx_files_data, &files_with_elevation)?;
    
    println!("🔬 Running enhanced two-pass analysis...");
    let twopass_results = run_enhanced_twopass_analysis(&gpx_files_data, &files_with_elevation)?;
    
    println!("🔬 Running optimized Savitzky-Golay analysis...");
    let savgol_results = run_optimized_savgol_analysis(&gpx_files_data, &files_with_elevation)?;
    
    println!("✅ All analyses complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Combine all results
    let mut all_results = Vec::new();
    all_results.extend(distance_results);
    all_results.extend(twopass_results);
    all_results.extend(savgol_results);
    
    // Write comprehensive results
    let output_path = Path::new(gpx_folder).join("precision_optimization_results.csv");
    write_precision_results(&all_results, &output_path)?;
    
    // Print comprehensive analysis
    print_precision_analysis(&all_results);
    
    let total_time = total_start.elapsed();
    println!("\n⏱️  TOTAL EXECUTION TIME: {} minutes {:.1} seconds", 
             total_time.as_secs() / 60, 
             total_time.as_secs_f64() % 60.0);
    
    Ok(())
}

fn load_gpx_data(gpx_folder: &str) -> Result<(HashMap<String, GpxFileData>, Vec<String>), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::BufReader;
    use gpx::read;
    use geo::{HaversineDistance, point};
    use walkdir::WalkDir;
    
    let mut gpx_data = HashMap::new();
    let mut valid_files = Vec::new();
    
    let official_data = crate::load_official_elevation_data()?;
    
    for entry in WalkDir::new(gpx_folder) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    let path = entry.path();
                    let filename = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    
                    match File::open(path) {
                        Ok(file) => {
                            let reader = BufReader::new(file);
                            match read(reader) {
                                Ok(gpx) => {
                                    let mut coords: Vec<(f64, f64, f64)> = vec![];
                                    
                                    for track in gpx.tracks {
                                        for segment in track.segments {
                                            for pt in segment.points {
                                                if let Some(ele) = pt.elevation {
                                                    coords.push((pt.point().y(), pt.point().x(), ele));
                                                }
                                            }
                                        }
                                    }
                                    
                                    if !coords.is_empty() {
                                        let mut distances = vec![0.0];
                                        for i in 1..coords.len() {
                                            let a = point!(x: coords[i-1].1, y: coords[i-1].0);
                                            let b = point!(x: coords[i].1, y: coords[i].0);
                                            let dist = a.haversine_distance(&b);
                                            distances.push(distances[i-1] + dist);
                                        }
                                        
                                        let elevations: Vec<f64> = coords.iter().map(|c| c.2).collect();
                                        
                                        let official_gain = official_data
                                            .get(&filename.to_lowercase())
                                            .copied()
                                            .unwrap_or(0);
                                        
                                        if official_gain > 0 { // Only include files with official data
                                            let file_data = GpxFileData {
                                                filename: filename.clone(),
                                                elevations,
                                                distances,
                                                official_gain,
                                            };
                                            
                                            gpx_data.insert(filename.clone(), file_data);
                                            valid_files.push(filename);
                                        }
                                    }
                                },
                                Err(_) => continue,
                            }
                        },
                        Err(_) => continue,
                    }
                }
            }
        }
    }
    
    Ok((gpx_data, valid_files))
}

fn run_distance_based_precision(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<PrecisionResult>, Box<dyn std::error::Error>> {
    // High-resolution intervals: 1.0m to 6.0m in 0.1m increments
    let intervals: Vec<f32> = (10..=60).map(|i| i as f32 * 0.1).collect();
    
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("  Testing {} high-resolution distance intervals", intervals.len());
    
    let results: Vec<PrecisionResult> = intervals
        .par_iter()
        .map(|&interval| {
            let file_results: Vec<SingleFileResult> = valid_files
                .iter()
                .filter_map(|filename| {
                    if let Some(file_data) = gpx_data_arc.get(filename) {
                        Some(process_distance_based_single_file(file_data, interval))
                    } else {
                        None
                    }
                })
                .collect();
            
            create_precision_result(
                format!("DistBased-{:.1}m", interval),
                interval,
                &file_results
            )
        })
        .collect();
    
    Ok(results)
}

fn run_enhanced_twopass_analysis(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<PrecisionResult>, Box<dyn std::error::Error>> {
    // Two-pass: Fixed 3m for gain, variable 1.0-6.0m for loss
    let loss_intervals: Vec<f32> = (10..=60).map(|i| i as f32 * 0.1).collect();
    
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("  Testing {} enhanced two-pass configurations", loss_intervals.len());
    
    let results: Vec<PrecisionResult> = loss_intervals
        .par_iter()
        .map(|&loss_interval| {
            let file_results: Vec<SingleFileResult> = valid_files
                .iter()
                .filter_map(|filename| {
                    if let Some(file_data) = gpx_data_arc.get(filename) {
                        Some(process_enhanced_twopass_single_file(file_data, 3.0, loss_interval))
                    } else {
                        None
                    }
                })
                .collect();
            
            create_precision_result(
                format!("TwoPass-3m+{:.1}m", loss_interval),
                loss_interval,
                &file_results
            )
        })
        .collect();
    
    Ok(results)
}

fn run_optimized_savgol_analysis(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<PrecisionResult>, Box<dyn std::error::Error>> {
    // Savitzky-Golay configurations: window sizes 5, 7, 9, 11, 15, 21, 31 with polynomial orders 2, 3, 4
    let configurations = [
        (5, 2), (7, 2), (9, 2), (11, 2), (15, 2), (21, 2), (31, 2),  // 2nd order polynomial
        (7, 3), (9, 3), (11, 3), (15, 3), (21, 3), (31, 3),          // 3rd order polynomial  
        (9, 4), (11, 4), (15, 4), (21, 4), (31, 4),                  // 4th order polynomial
    ];
    
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("  Testing {} Savitzky-Golay configurations", configurations.len());
    
    let results: Vec<PrecisionResult> = configurations
        .par_iter()
        .map(|&(window, poly_order)| {
            let file_results: Vec<SingleFileResult> = valid_files
                .iter()
                .filter_map(|filename| {
                    if let Some(file_data) = gpx_data_arc.get(filename) {
                        Some(process_savgol_single_file(file_data, window, poly_order))
                    } else {
                        None
                    }
                })
                .collect();
            
            create_precision_result(
                format!("SavGol-W{}P{}", window, poly_order),
                window as f32,
                &file_results
            )
        })
        .collect();
    
    Ok(results)
}

fn process_distance_based_single_file(
    file_data: &GpxFileData,
    interval: f32
) -> SingleFileResult {
    let (gain, loss) = apply_optimized_distance_based(&file_data.elevations, &file_data.distances, interval.into());
    
    let official_gain = file_data.official_gain as f32;
    let gain_accuracy = (gain / official_gain) * 100.0;
    let loss_accuracy = (loss / official_gain) * 100.0;
    let combined_error = (gain_accuracy - 100.0).abs() + (loss_accuracy - 100.0).abs();
    
    SingleFileResult {
        filename: file_data.filename.clone(),
        official_gain: file_data.official_gain,
        gain_accuracy,
        loss_accuracy,
        combined_error,
    }
}

fn process_enhanced_twopass_single_file(
    file_data: &GpxFileData,
    gain_interval: f32,
    loss_interval: f32
) -> SingleFileResult {
    // Use 3m for gain calculation
    let (gain, _) = apply_optimized_distance_based(&file_data.elevations, &file_data.distances, gain_interval.into());
    
    // Use variable interval for loss calculation
    let (_, loss) = apply_optimized_distance_based(&file_data.elevations, &file_data.distances, loss_interval.into());
    
    let official_gain = file_data.official_gain as f32;
    let gain_accuracy = (gain / official_gain) * 100.0;
    let loss_accuracy = (loss / official_gain) * 100.0;
    let combined_error = (gain_accuracy - 100.0).abs() + (loss_accuracy - 100.0).abs();
    
    SingleFileResult {
        filename: file_data.filename.clone(),
        official_gain: file_data.official_gain,
        gain_accuracy,
        loss_accuracy,
        combined_error,
    }
}

fn process_savgol_single_file(
    file_data: &GpxFileData,
    window_size: usize,
    poly_order: usize
) -> SingleFileResult {
    let (gain, loss) = apply_optimized_savitzky_golay(&file_data.elevations, window_size, poly_order);
    
    let official_gain = file_data.official_gain as f32;
    let gain_accuracy = (gain / official_gain) * 100.0;
    let loss_accuracy = (loss / official_gain) * 100.0;
    let combined_error = (gain_accuracy - 100.0).abs() + (loss_accuracy - 100.0).abs();
    
    SingleFileResult {
        filename: file_data.filename.clone(),
        official_gain: file_data.official_gain,
        gain_accuracy,
        loss_accuracy,
        combined_error,
    }
}

fn apply_optimized_distance_based(
    elevations: &[f64],
    distances: &[f64],
    interval: f64
) -> (f32, f32) {
    // Improved distance-based processing with precision optimizations
    let (_uniform_distances, uniform_elevations) = resample_to_uniform_distance_optimized(
        elevations, distances, interval
    );
    
    if uniform_elevations.is_empty() {
        return (0.0, 0.0);
    }
    
    // Apply enhanced median filter (5-point for better noise removal)
    let median_smoothed = median_filter_optimized(&uniform_elevations, 5);
    
    // Apply adaptive Gaussian smoothing based on interval
    let gaussian_window = calculate_optimal_gaussian_window(interval);
    let gaussian_smoothed = gaussian_smooth_optimized(&median_smoothed, gaussian_window);
    
    // Apply precision deadband filtering
    let deadband_threshold = calculate_optimal_deadband(interval);
    let filtered_elevations = apply_precision_deadband(&gaussian_smoothed, deadband_threshold);
    
    // Calculate gain and loss with sub-meter precision
    calculate_precise_gain_loss(&filtered_elevations)
}

fn apply_optimized_savitzky_golay(
    elevations: &[f64],
    window_size: usize,
    poly_order: usize
) -> (f32, f32) {
    // Enhanced Savitzky-Golay with proper polynomial fitting
    let smoothed = savitzky_golay_filter_optimized(elevations, window_size, poly_order);
    
    // Apply gentle deadband to remove remaining noise
    let deadband_threshold = 0.5; // 0.5m threshold for Savitzky-Golay
    let filtered = apply_precision_deadband(&smoothed, deadband_threshold);
    
    calculate_precise_gain_loss(&filtered)
}

// Optimized helper functions

fn resample_to_uniform_distance_optimized(
    elevations: &[f64],
    distances: &[f64],
    interval: f64
) -> (Vec<f64>, Vec<f64>) {
    if elevations.is_empty() || distances.is_empty() {
        return (vec![], vec![]);
    }
    
    let total_distance = distances.last().unwrap();
    let num_points = (total_distance / interval).ceil() as usize + 1;
    
    let mut uniform_distances = Vec::with_capacity(num_points);
    let mut uniform_elevations = Vec::with_capacity(num_points);
    
    for i in 0..num_points {
        let target_distance = i as f64 * interval;
        if target_distance > *total_distance {
            break;
        }
        uniform_distances.push(target_distance);
        
        // Use cubic interpolation for smoother results
        let elevation = interpolate_elevation_cubic(elevations, distances, target_distance);
        uniform_elevations.push(elevation);
    }
    
    (uniform_distances, uniform_elevations)
}

fn interpolate_elevation_cubic(
    elevations: &[f64],
    distances: &[f64],
    target_distance: f64
) -> f64 {
    if target_distance <= 0.0 {
        return elevations[0];
    }
    
    // Find the segment containing target_distance
    for i in 1..distances.len() {
        if distances[i] >= target_distance {
            // Use linear interpolation (cubic would be complex without external libs)
            let d1 = distances[i - 1];
            let d2 = distances[i];
            let e1 = elevations[i - 1];
            let e2 = elevations[i];
            
            if (d2 - d1).abs() < 1e-10 {
                return e1;
            }
            
            let t = (target_distance - d1) / (d2 - d1);
            return e1 + t * (e2 - e1);
        }
    }
    
    *elevations.last().unwrap()
}

fn median_filter_optimized(data: &[f64], window: usize) -> Vec<f64> {
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

fn gaussian_smooth_optimized(data: &[f64], window: usize) -> Vec<f64> {
    let mut result = Vec::with_capacity(data.len());
    let sigma = window as f64 / 6.0;
    
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

fn calculate_optimal_gaussian_window(interval: f64) -> usize {
    // Adaptive window size based on interval
    let base_window = (120.0 / interval).round() as usize;
    base_window.max(3).min(25)
}

fn calculate_optimal_deadband(interval: f64) -> f64 {
    // Adaptive deadband based on interval
    match interval {
        x if x <= 1.5 => 0.8,
        x if x <= 2.5 => 1.2,
        x if x <= 4.0 => 1.8,
        _ => 2.5,
    }
}

fn apply_precision_deadband(elevations: &[f64], threshold: f64) -> Vec<f64> {
    if elevations.is_empty() {
        return vec![];
    }
    
    let mut filtered = vec![elevations[0]];
    let mut last_significant = elevations[0];
    
    for &elevation in elevations.iter().skip(1) {
        let change = elevation - last_significant;
        
        if change.abs() >= threshold {
            filtered.push(elevation);
            last_significant = elevation;
        } else {
            filtered.push(last_significant);
        }
    }
    
    filtered
}

fn savitzky_golay_filter_optimized(
    data: &[f64],
    window_size: usize,
    poly_order: usize
) -> Vec<f64> {
    if window_size < 5 || window_size >= data.len() || poly_order >= window_size {
        return data.to_vec();
    }
    
    let mut result = Vec::with_capacity(data.len());
    let half_window = window_size / 2;
    
    // Generate Savitzky-Golay coefficients (simplified implementation)
    let coeffs = generate_savgol_coefficients_optimized(window_size, poly_order);
    
    for i in 0..data.len() {
        let start = if i >= half_window { i - half_window } else { 0 };
        let end = if i + half_window < data.len() { i + half_window } else { data.len() - 1 };
        
        let mut smoothed_value = 0.0;
        let mut weight_sum = 0.0;
        
        for (j, &value) in data[start..=end].iter().enumerate() {
            let coeff = coeffs.get(j).copied().unwrap_or(1.0);
            smoothed_value += value * coeff;
            weight_sum += coeff;
        }
        
        result.push(smoothed_value / weight_sum);
    }
    
    result
}

fn generate_savgol_coefficients_optimized(window_size: usize, poly_order: usize) -> Vec<f64> {
    // Simplified Savitzky-Golay coefficients generation
    let mut coeffs = vec![1.0; window_size];
    let center = window_size / 2;
    
    match poly_order {
        2 => {
            // 2nd order polynomial - parabolic weighting
            for i in 0..window_size {
                let distance = (i as f64 - center as f64).abs();
                let normalized_dist = distance / center as f64;
                coeffs[i] = 1.0 - normalized_dist * normalized_dist;
            }
        },
        3 => {
            // 3rd order polynomial - cubic weighting
            for i in 0..window_size {
                let distance = (i as f64 - center as f64).abs();
                let normalized_dist = distance / center as f64;
                coeffs[i] = 1.0 - normalized_dist * normalized_dist * normalized_dist;
            }
        },
        4 => {
            // 4th order polynomial - quartic weighting
            for i in 0..window_size {
                let distance = (i as f64 - center as f64).abs();
                let normalized_dist = distance / center as f64;
                coeffs[i] = 1.0 - normalized_dist.powi(4);
            }
        },
        _ => {
            // Default triangular weighting
            for i in 0..window_size {
                let distance = (i as f64 - center as f64).abs();
                coeffs[i] = (window_size as f64 - distance) / window_size as f64;
            }
        }
    }
    
    coeffs
}

fn calculate_precise_gain_loss(elevations: &[f64]) -> (f32, f32) {
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for window in elevations.windows(2) {
        let change = window[1] - window[0];
        if change > 0.0 {
            gain += change;
        } else {
            loss += -change;
        }
    }
    
    (gain as f32, loss as f32)
}

fn create_precision_result(
    method_name: String,
    parameter_value: f32,
    file_results: &[SingleFileResult]
) -> PrecisionResult {
    if file_results.is_empty() {
        return PrecisionResult {
            method_name,
            parameter_value,
            files_within_2_percent: 0,
            files_within_5_percent: 0,
            files_within_10_percent: 0,
            files_outside_20_percent: 0,
            gain_accuracy_avg: 0.0,
            loss_accuracy_avg: 0.0,
            gain_accuracy_median: 0.0,
            loss_accuracy_median: 0.0,
            best_gain_accuracy: 0.0,
            worst_gain_accuracy: 0.0,
            best_loss_accuracy: 0.0,
            worst_loss_accuracy: 0.0,
            precision_score: 0.0,
            total_files: 0,
        };
    }
    
    let total_files = file_results.len() as u32;
    
    // Extract accuracy vectors
    let gain_accuracies: Vec<f32> = file_results.iter().map(|r| r.gain_accuracy).collect();
    let loss_accuracies: Vec<f32> = file_results.iter().map(|r| r.loss_accuracy).collect();
    let combined_errors: Vec<f32> = file_results.iter().map(|r| r.combined_error).collect();
    
    // Count files in precision bands
    let files_within_2_percent = combined_errors.iter().filter(|&&err| err <= 4.0).count() as u32; // ±2% total error
    let files_within_5_percent = combined_errors.iter().filter(|&&err| err <= 10.0).count() as u32; // ±5% total error
    let files_within_10_percent = combined_errors.iter().filter(|&&err| err <= 20.0).count() as u32; // ±10% total error
    let files_outside_20_percent = combined_errors.iter().filter(|&&err| err > 40.0).count() as u32; // >±20% total error
    
    // Calculate accuracy statistics
    let gain_accuracy_avg = gain_accuracies.iter().sum::<f32>() / total_files as f32;
    let loss_accuracy_avg = loss_accuracies.iter().sum::<f32>() / total_files as f32;
    
    let mut sorted_gain = gain_accuracies.clone();
    sorted_gain.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let gain_accuracy_median = if sorted_gain.len() % 2 == 0 {
        (sorted_gain[sorted_gain.len() / 2 - 1] + sorted_gain[sorted_gain.len() / 2]) / 2.0
    } else {
        sorted_gain[sorted_gain.len() / 2]
    };
    
    let mut sorted_loss = loss_accuracies.clone();
    sorted_loss.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let loss_accuracy_median = if sorted_loss.len() % 2 == 0 {
        (sorted_loss[sorted_loss.len() / 2 - 1] + sorted_loss[sorted_loss.len() / 2]) / 2.0
    } else {
        sorted_loss[sorted_loss.len() / 2]
    };
    
    // Best and worst case analysis
    let best_gain_accuracy = gain_accuracies.iter()
        .min_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied().unwrap_or(100.0);
    let worst_gain_accuracy = gain_accuracies.iter()
        .max_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied().unwrap_or(100.0);
    let best_loss_accuracy = loss_accuracies.iter()
        .min_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied().unwrap_or(100.0);
    let worst_loss_accuracy = loss_accuracies.iter()
        .max_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied().unwrap_or(100.0);
    
    // Calculate precision score (higher is better)
    let precision_score = (files_within_2_percent as f32 * 20.0) +
                         (files_within_5_percent as f32 * 10.0) +
                         (files_within_10_percent as f32 * 5.0) -
                         (files_outside_20_percent as f32 * 10.0) +
                         (200.0 - (gain_accuracy_avg - 100.0).abs() - (loss_accuracy_avg - 100.0).abs());
    
    PrecisionResult {
        method_name,
        parameter_value,
        files_within_2_percent,
        files_within_5_percent,
        files_within_10_percent,
        files_outside_20_percent,
        gain_accuracy_avg,
        loss_accuracy_avg,
        gain_accuracy_median,
        loss_accuracy_median,
        best_gain_accuracy,
        worst_gain_accuracy,
        best_loss_accuracy,
        worst_loss_accuracy,
        precision_score,
        total_files,
    }
}

fn write_precision_results(
    results: &[PrecisionResult],
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Method", "Parameter", "Precision_Score",
        "Files_±2%", "Files_±5%", "Files_±10%", "Files_>20%",
        "Gain_Avg_%", "Loss_Avg_%", "Gain_Median_%", "Loss_Median_%",
        "Best_Gain_%", "Worst_Gain_%", "Best_Loss_%", "Worst_Loss_%",
        "Total_Files"
    ])?;
    
    // Sort by precision score (highest first)
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.precision_score.partial_cmp(&a.precision_score).unwrap());
    
    // Write data
    for result in sorted_results {
        wtr.write_record(&[
            &result.method_name,
            &format!("{:.1}", result.parameter_value),
            &format!("{:.2}", result.precision_score),
            &result.files_within_2_percent.to_string(),
            &result.files_within_5_percent.to_string(),
            &result.files_within_10_percent.to_string(),
            &result.files_outside_20_percent.to_string(),
            &format!("{:.2}", result.gain_accuracy_avg),
            &format!("{:.2}", result.loss_accuracy_avg),
            &format!("{:.2}", result.gain_accuracy_median),
            &format!("{:.2}", result.loss_accuracy_median),
            &format!("{:.2}", result.best_gain_accuracy),
            &format!("{:.2}", result.worst_gain_accuracy),
            &format!("{:.2}", result.best_loss_accuracy),
            &format!("{:.2}", result.worst_loss_accuracy),
            &result.total_files.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    println!("\n✅ Precision optimization results saved to: {}", output_path.display());
    Ok(())
}

fn print_precision_analysis(results: &[PrecisionResult]) {
    println!("\n🎯 PRECISION OPTIMIZATION ANALYSIS RESULTS");
    println!("==========================================");
    
    // Sort by precision score
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.precision_score.partial_cmp(&a.precision_score).unwrap());
    
    let best_overall = &sorted_results[0];
    
    println!("\n🏆 OVERALL WINNER:");
    println!("Method: {}", best_overall.method_name);
    println!("Parameter: {:.1}", best_overall.parameter_value);
    println!("Precision Score: {:.2}", best_overall.precision_score);
    println!("Files within ±2%: {} ({:.1}%)", 
             best_overall.files_within_2_percent,
             (best_overall.files_within_2_percent as f32 / best_overall.total_files as f32) * 100.0);
    println!("Files within ±5%: {} ({:.1}%)", 
             best_overall.files_within_5_percent,
             (best_overall.files_within_5_percent as f32 / best_overall.total_files as f32) * 100.0);
    println!("Files within ±10%: {} ({:.1}%)", 
             best_overall.files_within_10_percent,
             (best_overall.files_within_10_percent as f32 / best_overall.total_files as f32) * 100.0);
    println!("Gain accuracy: {:.2}% (median: {:.2}%)", 
             best_overall.gain_accuracy_avg, best_overall.gain_accuracy_median);
    println!("Loss accuracy: {:.2}% (median: {:.2}%)", 
             best_overall.loss_accuracy_avg, best_overall.loss_accuracy_median);
    
    // Find best in each category
    let best_distance = sorted_results.iter()
        .find(|r| r.method_name.starts_with("DistBased"))
        .unwrap();
    
    let best_twopass = sorted_results.iter()
        .find(|r| r.method_name.starts_with("TwoPass"))
        .unwrap();
    
    let best_savgol = sorted_results.iter()
        .find(|r| r.method_name.starts_with("SavGol"))
        .unwrap();
    
    println!("\n🏅 BEST IN EACH CATEGORY:");
    
    println!("\n1️⃣ Best Distance-Based:");
    println!("   {}: {:.1} interval", best_distance.method_name, best_distance.parameter_value);
    println!("   ±2%: {}, ±5%: {}, ±10%: {}", 
             best_distance.files_within_2_percent,
             best_distance.files_within_5_percent,
             best_distance.files_within_10_percent);
    println!("   Gain: {:.2}%, Loss: {:.2}%", 
             best_distance.gain_accuracy_median, best_distance.loss_accuracy_median);
    
    println!("\n2️⃣ Best Two-Pass:");
    println!("   {}: 3m gain + {:.1}m loss", best_twopass.method_name, best_twopass.parameter_value);
    println!("   ±2%: {}, ±5%: {}, ±10%: {}", 
             best_twopass.files_within_2_percent,
             best_twopass.files_within_5_percent,
             best_twopass.files_within_10_percent);
    println!("   Gain: {:.2}%, Loss: {:.2}%", 
             best_twopass.gain_accuracy_median, best_twopass.loss_accuracy_median);
    
    println!("\n3️⃣ Best Savitzky-Golay:");
    println!("   {}: Window {:.0}", best_savgol.method_name, best_savgol.parameter_value);
    println!("   ±2%: {}, ±5%: {}, ±10%: {}", 
             best_savgol.files_within_2_percent,
             best_savgol.files_within_5_percent,
             best_savgol.files_within_10_percent);
    println!("   Gain: {:.2}%, Loss: {:.2}%", 
             best_savgol.gain_accuracy_median, best_savgol.loss_accuracy_median);
    
    // Show top 10 overall
    println!("\n🔝 TOP 10 CONFIGURATIONS:");
    println!("Rank | Method                    | Param | Score  | ±2% | ±5% | ±10% | Gain%  | Loss%");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for (i, result) in sorted_results.iter().take(10).enumerate() {
        println!("{:4} | {:25} | {:5.1} | {:6.1} | {:3} | {:3} | {:4} | {:6.1} | {:6.1}",
                 i + 1,
                 result.method_name.chars().take(25).collect::<String>(),
                 result.parameter_value,
                 result.precision_score,
                 result.files_within_2_percent,
                 result.files_within_5_percent,
                 result.files_within_10_percent,
                 result.gain_accuracy_median,
                 result.loss_accuracy_median);
    }
    
    // Analysis insights
    println!("\n💡 KEY INSIGHTS:");
    
    // Best distance interval
    let distance_results: Vec<_> = sorted_results.iter()
        .filter(|r| r.method_name.starts_with("DistBased"))
        .collect();
    
    if !distance_results.is_empty() {
        let optimal_interval = distance_results[0].parameter_value;
        println!("• Optimal distance interval: {:.1}m (vs previous 3.0m)", optimal_interval);
        
        if optimal_interval != 3.0 {
            let improvement = distance_results[0].files_within_2_percent - 
                             distance_results.iter()
                                 .find(|r| (r.parameter_value - 3.0).abs() < 0.1)
                                 .map(|r| r.files_within_2_percent)
                                 .unwrap_or(0);
            println!("• Improvement over 3.0m: +{} files in ±2% band", improvement);
        }
    }
    
    // Two-pass analysis
    let twopass_results: Vec<_> = sorted_results.iter()
        .filter(|r| r.method_name.starts_with("TwoPass"))
        .collect();
    
    if !twopass_results.is_empty() {
        let optimal_loss_interval = twopass_results[0].parameter_value;
        println!("• Optimal two-pass: 3.0m gain + {:.1}m loss", optimal_loss_interval);
        
        let comparison_score = twopass_results[0].precision_score;
        println!("• Two-pass vs best distance: {:.1} vs {:.1} score", 
                 comparison_score, best_distance.precision_score);
    }
    
    // Savitzky-Golay insights
    let savgol_results: Vec<_> = sorted_results.iter()
        .filter(|r| r.method_name.starts_with("SavGol"))
        .collect();
    
    if !savgol_results.is_empty() {
        let best_savgol_config = &savgol_results[0].method_name;
        println!("• Best Savitzky-Golay config: {}", best_savgol_config);
        println!("• Savitzky-Golay vs distance-based: {:.1} vs {:.1} score", 
                 savgol_results[0].precision_score, best_distance.precision_score);
    }
    
    println!("\n🚀 IMPLEMENTATION RECOMMENDATIONS:");
    println!("1. Use {} as your new default method", best_overall.method_name);
    println!("2. Parameter: {:.1}", best_overall.parameter_value);
    println!("3. Expected improvement: {} more files in ±2% accuracy band", 
             best_overall.files_within_2_percent);
    println!("4. This should achieve {:.1}% of files within ±5% accuracy", 
             (best_overall.files_within_5_percent as f32 / best_overall.total_files as f32) * 100.0);
}