/// TWO-PASS SMOOTHING AND SAVITZKY-GOLAY COMPARISON ANALYSIS
/// 
/// This module implements and compares five approaches:
/// 1. Baseline: Your proven distance-based approach (default)
/// 2. DistBased-3m: Distance-based with 3m interval
/// 3. DistBased-6.1m: Distance-based with 6.1m interval  
/// 4. Two-Pass: Distance-based for gain + 15m distance-based for loss
/// 5. Savitzky-Golay: Traditional signal processing filter
/// 
/// Scoring: Separate gain accuracy and loss accuracy (both vs official gain)
/// Output: Detailed file-by-file CSV + summary comparison

use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;
use crate::distbased_elevation_processor::DistBasedElevationProcessor;

#[derive(Debug, Serialize, Clone)]
pub struct FileComparisonResult {
    filename: String,
    official_gain_m: u32,
    
    // Baseline (default distance-based)
    baseline_gain_m: f32,
    baseline_loss_m: f32,
    baseline_gain_accuracy: f32,
    baseline_loss_accuracy: f32,
    
    // Distance-based 3m
    dist3m_gain_m: f32,
    dist3m_loss_m: f32,
    dist3m_gain_accuracy: f32,
    dist3m_loss_accuracy: f32,
    
    // Distance-based 6.1m
    dist61m_gain_m: f32,
    dist61m_loss_m: f32,
    dist61m_gain_accuracy: f32,
    dist61m_loss_accuracy: f32,
    
    // Two-pass
    twopass_gain_m: f32,
    twopass_loss_m: f32,
    twopass_gain_accuracy: f32,
    twopass_loss_accuracy: f32,
    
    // Savitzky-Golay
    savgol_gain_m: f32,
    savgol_loss_m: f32,
    savgol_gain_accuracy: f32,
    savgol_loss_accuracy: f32,
    
    // Best method for this file
    best_gain_method: String,
    best_loss_method: String,
    best_combined_method: String,
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
    
    // Baseline results (default distance-based)
    baseline_gain: f32,
    baseline_loss: f32,
    baseline_gain_accuracy: f32,
    baseline_loss_accuracy: f32,
    
    // Distance-based 3m results
    dist3m_gain: f32,
    dist3m_loss: f32,
    dist3m_gain_accuracy: f32,
    dist3m_loss_accuracy: f32,
    
    // Distance-based 6.1m results
    dist61m_gain: f32,
    dist61m_loss: f32,
    dist61m_gain_accuracy: f32,
    dist61m_loss_accuracy: f32,
    
    // Two-pass results
    twopass_gain: f32,
    twopass_loss: f32,
    twopass_gain_accuracy: f32,
    twopass_loss_accuracy: f32,
    
    // Savitzky-Golay results
    savgol_gain: f32,
    savgol_loss: f32,
    savgol_gain_accuracy: f32,
    savgol_loss_accuracy: f32,
}

pub fn run_two_pass_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Load GPX data (silent)
    let (gpx_files_data, valid_files) = load_gpx_data(gpx_folder)?;
    
    // Filter files with elevation data and official benchmarks (silent)
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
    
    println!("🔄 Starting Baseline Distance-Based Analysis...");
    println!("🔄 Starting Distance-Based 3m Analysis...");
    println!("🔄 Starting Distance-Based 6.1m Analysis...");
    println!("🔄 Starting Two-Pass Smoothing Analysis...");
    println!("🔄 Starting Savitzky-Golay Filter Analysis...");
    
    // Process with all five methods (silent)
    let results = process_five_methods(&gpx_files_data, &files_with_elevation)?;
    
    // Write detailed file comparison CSV
    let file_comparison_path = Path::new(gpx_folder).join("detailed_file_comparison.csv");
    write_file_comparison_csv(&results, &file_comparison_path)?;
    
    // Write summary results (silent)
    let output_path = Path::new(gpx_folder).join("five_method_comparison.csv");
    write_five_method_results(&results, &output_path)?;
    
    // Print summary
    print_five_method_summary(&results);
    
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
                                        
                                        let file_data = GpxFileData {
                                            filename: filename.clone(),
                                            elevations,
                                            distances,
                                            official_gain,
                                        };
                                        
                                        gpx_data.insert(filename.clone(), file_data);
                                        valid_files.push(filename);
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

fn process_five_methods(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<SingleFileResult>, Box<dyn std::error::Error>> {
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    // Process all files with all five methods (silent)
    let all_file_results: Vec<SingleFileResult> = valid_files
        .par_iter()
        .filter_map(|filename| {
            let gpx_data = Arc::clone(&gpx_data_arc);
            
            if let Some(file_data) = gpx_data.get(filename) {
                if file_data.official_gain > 0 {
                    let result = process_single_file_five_methods(file_data);
                    return Some(result);
                }
            }
            None
        })
        .collect();
    
    Ok(all_file_results)
}

fn process_single_file_five_methods(file_data: &GpxFileData) -> SingleFileResult {
    let official_gain = file_data.official_gain as f32;
    
    // METHOD 1: BASELINE - Your proven distance-based approach (default)
    let baseline_processor = DistBasedElevationProcessor::new(
        file_data.elevations.clone(),
        file_data.distances.clone()
    );
    let baseline_gain = baseline_processor.get_total_elevation_gain() as f32;
    let baseline_loss = baseline_processor.get_total_elevation_loss() as f32;
    let baseline_gain_accuracy = (baseline_gain / official_gain) * 100.0;
    let baseline_loss_accuracy = (baseline_loss / official_gain) * 100.0;
    
    // METHOD 2: DISTANCE-BASED 3M - Use 3m interval processing
    let (dist3m_gain, dist3m_loss) = apply_distance_based_custom_interval(
        &file_data.elevations, 
        &file_data.distances, 
        3.0
    );
    let dist3m_gain_accuracy = (dist3m_gain / official_gain) * 100.0;
    let dist3m_loss_accuracy = (dist3m_loss / official_gain) * 100.0;
    
    // METHOD 3: DISTANCE-BASED 6.1M - Use 6.1m interval processing
    let (dist61m_gain, dist61m_loss) = apply_distance_based_custom_interval(
        &file_data.elevations, 
        &file_data.distances, 
        6.1
    );
    let dist61m_gain_accuracy = (dist61m_gain / official_gain) * 100.0;
    let dist61m_loss_accuracy = (dist61m_loss / official_gain) * 100.0;
    
    // METHOD 4: TWO-PASS - Distance-based gain + 15m distance-based loss
    let (twopass_gain, twopass_loss) = apply_two_pass_smoothing(
        &file_data.elevations, 
        &file_data.distances
    );
    let twopass_gain_accuracy = (twopass_gain / official_gain) * 100.0;
    let twopass_loss_accuracy = (twopass_loss / official_gain) * 100.0;
    
    // METHOD 5: SAVITZKY-GOLAY - Traditional signal processing
    let (savgol_gain, savgol_loss) = apply_savitzky_golay_filter(
        &file_data.elevations,
        15.0  // Use 15-point window
    );
    let savgol_gain_accuracy = (savgol_gain / official_gain) * 100.0;
    let savgol_loss_accuracy = (savgol_loss / official_gain) * 100.0;
    
    SingleFileResult {
        filename: file_data.filename.clone(),
        official_gain: file_data.official_gain,
        baseline_gain,
        baseline_loss,
        baseline_gain_accuracy,
        baseline_loss_accuracy,
        dist3m_gain,
        dist3m_loss,
        dist3m_gain_accuracy,
        dist3m_loss_accuracy,
        dist61m_gain,
        dist61m_loss,
        dist61m_gain_accuracy,
        dist61m_loss_accuracy,
        twopass_gain,
        twopass_loss,
        twopass_gain_accuracy,
        twopass_loss_accuracy,
        savgol_gain,
        savgol_loss,
        savgol_gain_accuracy,
        savgol_loss_accuracy,
    }
}

fn apply_distance_based_custom_interval(
    elevations: &[f64],
    distances: &[f64],
    interval: f64
) -> (f32, f32) {
    // Use the same approach as your custom_smoother with specified interval
    let (_uniform_distances, uniform_elevations) = resample_to_uniform_distance(
        elevations, distances, interval
    );
    
    if uniform_elevations.is_empty() {
        return (0.0, 0.0);
    }
    
    // Apply median filter for spike removal
    let median_smoothed = median_filter(&uniform_elevations, 3);
    
    // Apply Gaussian smoothing (adaptive window based on interval)
    let window_size = ((150.0 / interval).round() as usize).max(5).min(30);
    let gaussian_smoothed = gaussian_smooth(&median_smoothed, window_size);
    
    // Apply deadband filtering
    let deadband_threshold = match interval {
        x if x <= 3.0 => 1.5,
        x if x <= 6.0 => 2.0,
        _ => 2.5,
    };
    
    let mut filtered_elevations = vec![gaussian_smoothed[0]];
    let mut last_significant_elevation = gaussian_smoothed[0];
    
    for &elevation in gaussian_smoothed.iter().skip(1) {
        let change = elevation - last_significant_elevation;
        
        if change.abs() >= deadband_threshold {
            filtered_elevations.push(elevation);
            last_significant_elevation = elevation;
        } else {
            filtered_elevations.push(last_significant_elevation);
        }
    }
    
    // Calculate gain and loss
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for window in filtered_elevations.windows(2) {
        let change = window[1] - window[0];
        if change > 0.0 {
            gain += change;
        } else {
            loss += -change;
        }
    }
    
    (gain as f32, loss as f32)
}

fn apply_two_pass_smoothing(
    elevations: &[f64],
    distances: &[f64]
) -> (f32, f32) {
    // PASS 1: Process for elevation gain using standard distance-based approach
    let gain_processor = DistBasedElevationProcessor::new(
        elevations.to_vec(),
        distances.to_vec()
    );
    let processed_gain = gain_processor.get_total_elevation_gain() as f32;
    
    // PASS 2: Process for elevation loss using 15m interval
    let processed_loss = apply_loss_specific_processing(elevations, distances, 15.0);
    
    (processed_gain, processed_loss)
}

fn apply_loss_specific_processing(
    elevations: &[f64],
    distances: &[f64],
    interval: f64
) -> f32 {
    // Custom loss processing with specified interval
    let (_uniform_distances, uniform_elevations) = resample_to_uniform_distance(
        elevations, distances, interval
    );
    
    // Apply median filter for spike removal
    let median_smoothed = median_filter(&uniform_elevations, 3);
    
    // Apply Gaussian smoothing
    let gaussian_smoothed = gaussian_smooth(&median_smoothed, 15);
    
    // Calculate loss
    let mut total_loss = 0.0;
    for window in gaussian_smoothed.windows(2) {
        let change = window[1] - window[0];
        if change < 0.0 {
            total_loss += -change;
        }
    }
    
    total_loss as f32
}

fn apply_savitzky_golay_filter(
    elevations: &[f64],
    window_size: f32
) -> (f32, f32) {
    // Simplified Savitzky-Golay implementation
    let window = (window_size as usize).max(5).min(elevations.len() / 4);
    let smoothed = savitzky_golay_smooth(elevations, window);
    
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for window in smoothed.windows(2) {
        let change = window[1] - window[0];
        if change > 0.0 {
            gain += change;
        } else {
            loss += -change;
        }
    }
    
    (gain as f32, loss as f32)
}

fn savitzky_golay_smooth(data: &[f64], window: usize) -> Vec<f64> {
    if window < 5 || window >= data.len() {
        return data.to_vec();
    }
    
    let mut result = Vec::with_capacity(data.len());
    let half_window = window / 2;
    
    // Savitzky-Golay coefficients for 2nd order polynomial (simplified)
    let coeffs = generate_savgol_coefficients(window);
    
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

fn generate_savgol_coefficients(window: usize) -> Vec<f64> {
    // Simplified Savitzky-Golay coefficients for 2nd order polynomial
    let mut coeffs = vec![1.0; window];
    let center = window / 2;
    
    // Apply triangular weighting (simplified approximation)
    for i in 0..window {
        let distance = (i as f64 - center as f64).abs();
        coeffs[i] = (window as f64 - distance) / window as f64;
    }
    
    coeffs
}

// Helper functions for distance-based processing
fn resample_to_uniform_distance(
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
        
        let elevation = interpolate_elevation_at_distance(elevations, distances, target_distance);
        uniform_elevations.push(elevation);
    }
    
    (uniform_distances, uniform_elevations)
}

fn interpolate_elevation_at_distance(
    elevations: &[f64],
    distances: &[f64],
    target_distance: f64
) -> f64 {
    if target_distance <= 0.0 {
        return elevations[0];
    }
    
    for i in 1..distances.len() {
        if distances[i] >= target_distance {
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

fn gaussian_smooth(data: &[f64], window: usize) -> Vec<f64> {
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

fn write_file_comparison_csv(
    results: &[SingleFileResult],
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header with all methods
    wtr.write_record(&[
        "Filename", "Official_Gain_m",
        // Baseline (default distance-based)
        "Baseline_Gain_m", "Baseline_Loss_m", "Baseline_Gain_Acc_%", "Baseline_Loss_Acc_%",
        // Distance-based 3m
        "Dist3m_Gain_m", "Dist3m_Loss_m", "Dist3m_Gain_Acc_%", "Dist3m_Loss_Acc_%",
        // Distance-based 6.1m  
        "Dist61m_Gain_m", "Dist61m_Loss_m", "Dist61m_Gain_Acc_%", "Dist61m_Loss_Acc_%",
        // Two-pass
        "TwoPass_Gain_m", "TwoPass_Loss_m", "TwoPass_Gain_Acc_%", "TwoPass_Loss_Acc_%",
        // Savitzky-Golay
        "SavGol_Gain_m", "SavGol_Loss_m", "SavGol_Gain_Acc_%", "SavGol_Loss_Acc_%",
        // Best methods
        "Best_Gain_Method", "Best_Loss_Method", "Best_Combined_Method"
    ])?;
    
    // Sort by filename for easier reading
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| a.filename.cmp(&b.filename));
    
    for result in sorted_results {
        // Determine best methods for this file
        let gain_accuracies = [
            ("Baseline", result.baseline_gain_accuracy),
            ("Dist3m", result.dist3m_gain_accuracy),
            ("Dist61m", result.dist61m_gain_accuracy),
            ("TwoPass", result.twopass_gain_accuracy),
            ("SavGol", result.savgol_gain_accuracy),
        ];
        
        let loss_accuracies = [
            ("Baseline", result.baseline_loss_accuracy),
            ("Dist3m", result.dist3m_loss_accuracy),
            ("Dist61m", result.dist61m_loss_accuracy),
            ("TwoPass", result.twopass_loss_accuracy),
            ("SavGol", result.savgol_loss_accuracy),
        ];
        
        let best_gain = gain_accuracies.iter()
            .min_by_key(|(_, acc)| ((acc - 100.0).abs() * 1000.0) as i32)
            .unwrap().0;
            
        let best_loss = loss_accuracies.iter()
            .min_by_key(|(_, acc)| ((acc - 100.0).abs() * 1000.0) as i32)
            .unwrap().0;
        
        // Combined score (simple average of gain and loss accuracy distances from 100%)
        let combined_scores = [
            ("Baseline", (result.baseline_gain_accuracy - 100.0).abs() + (result.baseline_loss_accuracy - 100.0).abs()),
            ("Dist3m", (result.dist3m_gain_accuracy - 100.0).abs() + (result.dist3m_loss_accuracy - 100.0).abs()),
            ("Dist61m", (result.dist61m_gain_accuracy - 100.0).abs() + (result.dist61m_loss_accuracy - 100.0).abs()),
            ("TwoPass", (result.twopass_gain_accuracy - 100.0).abs() + (result.twopass_loss_accuracy - 100.0).abs()),
            ("SavGol", (result.savgol_gain_accuracy - 100.0).abs() + (result.savgol_loss_accuracy - 100.0).abs()),
        ];
        
        let best_combined = combined_scores.iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap().0;
        
        wtr.write_record(&[
            &result.filename,
            &result.official_gain.to_string(),
            // Baseline
            &format!("{:.1}", result.baseline_gain),
            &format!("{:.1}", result.baseline_loss),
            &format!("{:.1}", result.baseline_gain_accuracy),
            &format!("{:.1}", result.baseline_loss_accuracy),
            // Dist3m
            &format!("{:.1}", result.dist3m_gain),
            &format!("{:.1}", result.dist3m_loss),
            &format!("{:.1}", result.dist3m_gain_accuracy),
            &format!("{:.1}", result.dist3m_loss_accuracy),
            // Dist61m
            &format!("{:.1}", result.dist61m_gain),
            &format!("{:.1}", result.dist61m_loss),
            &format!("{:.1}", result.dist61m_gain_accuracy),
            &format!("{:.1}", result.dist61m_loss_accuracy),
            // TwoPass
            &format!("{:.1}", result.twopass_gain),
            &format!("{:.1}", result.twopass_loss),
            &format!("{:.1}", result.twopass_gain_accuracy),
            &format!("{:.1}", result.twopass_loss_accuracy),
            // SavGol
            &format!("{:.1}", result.savgol_gain),
            &format!("{:.1}", result.savgol_loss),
            &format!("{:.1}", result.savgol_gain_accuracy),
            &format!("{:.1}", result.savgol_loss_accuracy),
            // Best methods
            best_gain,
            best_loss,
            best_combined,
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn write_five_method_results(
    results: &[SingleFileResult],
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    // This creates a summary CSV - the detailed one is in write_file_comparison_csv
    let mut wtr = Writer::from_path(output_path)?;
    
    wtr.write_record(&[
        "Summary_Statistics",
        "Baseline_Gain_Acc", "Baseline_Loss_Acc",
        "Dist3m_Gain_Acc", "Dist3m_Loss_Acc", 
        "Dist61m_Gain_Acc", "Dist61m_Loss_Acc",
        "TwoPass_Gain_Acc", "TwoPass_Loss_Acc",
        "SavGol_Gain_Acc", "SavGol_Loss_Acc"
    ])?;
    
    let total_files = results.len() as f32;
    
    // Calculate averages
    let baseline_gain_avg = results.iter().map(|r| r.baseline_gain_accuracy).sum::<f32>() / total_files;
    let baseline_loss_avg = results.iter().map(|r| r.baseline_loss_accuracy).sum::<f32>() / total_files;
    let dist3m_gain_avg = results.iter().map(|r| r.dist3m_gain_accuracy).sum::<f32>() / total_files;
    let dist3m_loss_avg = results.iter().map(|r| r.dist3m_loss_accuracy).sum::<f32>() / total_files;
    let dist61m_gain_avg = results.iter().map(|r| r.dist61m_gain_accuracy).sum::<f32>() / total_files;
    let dist61m_loss_avg = results.iter().map(|r| r.dist61m_loss_accuracy).sum::<f32>() / total_files;
    let twopass_gain_avg = results.iter().map(|r| r.twopass_gain_accuracy).sum::<f32>() / total_files;
    let twopass_loss_avg = results.iter().map(|r| r.twopass_loss_accuracy).sum::<f32>() / total_files;
    let savgol_gain_avg = results.iter().map(|r| r.savgol_gain_accuracy).sum::<f32>() / total_files;
    let savgol_loss_avg = results.iter().map(|r| r.savgol_loss_accuracy).sum::<f32>() / total_files;
    
    wtr.write_record(&[
        "Average_Accuracy_%",
        &format!("{:.1}", baseline_gain_avg), &format!("{:.1}", baseline_loss_avg),
        &format!("{:.1}", dist3m_gain_avg), &format!("{:.1}", dist3m_loss_avg),
        &format!("{:.1}", dist61m_gain_avg), &format!("{:.1}", dist61m_loss_avg),
        &format!("{:.1}", twopass_gain_avg), &format!("{:.1}", twopass_loss_avg),
        &format!("{:.1}", savgol_gain_avg), &format!("{:.1}", savgol_loss_avg),
    ])?;
    
    wtr.flush()?;
    Ok(())
}

fn print_five_method_summary(results: &[SingleFileResult]) {
    println!("\n📊 FIVE-METHOD COMPARISON RESULTS");
    println!("==================================");
    println!("Processed {} files with official elevation data\n", results.len());
    
    // Calculate aggregate statistics for each method
    let total_files = results.len() as f32;
    
    // Method 1: Baseline
    let baseline_gain_acc = results.iter().map(|r| r.baseline_gain_accuracy).sum::<f32>() / total_files;
    let baseline_loss_acc = results.iter().map(|r| r.baseline_loss_accuracy).sum::<f32>() / total_files;
    let baseline_gain_within_10 = results.iter().filter(|r| (r.baseline_gain_accuracy - 100.0).abs() <= 10.0).count();
    let baseline_loss_within_10 = results.iter().filter(|r| (r.baseline_loss_accuracy - 100.0).abs() <= 10.0).count();
    
    // Method 2: Distance-based 3m
    let dist3m_gain_acc = results.iter().map(|r| r.dist3m_gain_accuracy).sum::<f32>() / total_files;
    let dist3m_loss_acc = results.iter().map(|r| r.dist3m_loss_accuracy).sum::<f32>() / total_files;
    let dist3m_gain_within_10 = results.iter().filter(|r| (r.dist3m_gain_accuracy - 100.0).abs() <= 10.0).count();
    let dist3m_loss_within_10 = results.iter().filter(|r| (r.dist3m_loss_accuracy - 100.0).abs() <= 10.0).count();
    
    // Method 3: Distance-based 6.1m
    let dist61m_gain_acc = results.iter().map(|r| r.dist61m_gain_accuracy).sum::<f32>() / total_files;
    let dist61m_loss_acc = results.iter().map(|r| r.dist61m_loss_accuracy).sum::<f32>() / total_files;
    let dist61m_gain_within_10 = results.iter().filter(|r| (r.dist61m_gain_accuracy - 100.0).abs() <= 10.0).count();
    let dist61m_loss_within_10 = results.iter().filter(|r| (r.dist61m_loss_accuracy - 100.0).abs() <= 10.0).count();
    
    // Method 4: Two-Pass
    let twopass_gain_acc = results.iter().map(|r| r.twopass_gain_accuracy).sum::<f32>() / total_files;
    let twopass_loss_acc = results.iter().map(|r| r.twopass_loss_accuracy).sum::<f32>() / total_files;
    let twopass_gain_within_10 = results.iter().filter(|r| (r.twopass_gain_accuracy - 100.0).abs() <= 10.0).count();
    let twopass_loss_within_10 = results.iter().filter(|r| (r.twopass_loss_accuracy - 100.0).abs() <= 10.0).count();
    
    // Method 5: Savitzky-Golay
    let savgol_gain_acc = results.iter().map(|r| r.savgol_gain_accuracy).sum::<f32>() / total_files;
    let savgol_loss_acc = results.iter().map(|r| r.savgol_loss_accuracy).sum::<f32>() / total_files;
    let savgol_gain_within_10 = results.iter().filter(|r| (r.savgol_gain_accuracy - 100.0).abs() <= 10.0).count();
    let savgol_loss_within_10 = results.iter().filter(|r| (r.savgol_loss_accuracy - 100.0).abs() <= 10.0).count();
    
    println!("🏆 COMPARATIVE PERFORMANCE SUMMARY:");
    println!("Method               | Gain Acc% | Loss Acc% | Gain ±10% | Loss ±10% | Combined Score");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Baseline (Default)   | {:8.1} | {:8.1} | {:8}/{} | {:8}/{} | {:13.1}",
             baseline_gain_acc, baseline_loss_acc, baseline_gain_within_10, total_files as usize,
             baseline_loss_within_10, total_files as usize, 
             (baseline_gain_within_10 + baseline_loss_within_10) as f32 / 2.0);
    println!("Distance-Based 3m    | {:8.1} | {:8.1} | {:8}/{} | {:8}/{} | {:13.1}",
             dist3m_gain_acc, dist3m_loss_acc, dist3m_gain_within_10, total_files as usize,
             dist3m_loss_within_10, total_files as usize,
             (dist3m_gain_within_10 + dist3m_loss_within_10) as f32 / 2.0);
    println!("Distance-Based 6.1m  | {:8.1} | {:8.1} | {:8}/{} | {:8}/{} | {:13.1}",
             dist61m_gain_acc, dist61m_loss_acc, dist61m_gain_within_10, total_files as usize,
             dist61m_loss_within_10, total_files as usize,
             (dist61m_gain_within_10 + dist61m_loss_within_10) as f32 / 2.0);
    println!("Two-Pass Smoothing   | {:8.1} | {:8.1} | {:8}/{} | {:8}/{} | {:13.1}",
             twopass_gain_acc, twopass_loss_acc, twopass_gain_within_10, total_files as usize,
             twopass_loss_within_10, total_files as usize,
             (twopass_gain_within_10 + twopass_loss_within_10) as f32 / 2.0);
    println!("Savitzky-Golay       | {:8.1} | {:8.1} | {:8}/{} | {:8}/{} | {:13.1}",
             savgol_gain_acc, savgol_loss_acc, savgol_gain_within_10, total_files as usize,
             savgol_loss_within_10, total_files as usize,
             (savgol_gain_within_10 + savgol_loss_within_10) as f32 / 2.0);
    
    // Overall winner analysis
    let methods = [
        ("Baseline", (baseline_gain_within_10 + baseline_loss_within_10) as f32 / 2.0),
        ("Dist3m", (dist3m_gain_within_10 + dist3m_loss_within_10) as f32 / 2.0),
        ("Dist61m", (dist61m_gain_within_10 + dist61m_loss_within_10) as f32 / 2.0),
        ("Two-Pass", (twopass_gain_within_10 + twopass_loss_within_10) as f32 / 2.0),
        ("Savitzky-Golay", (savgol_gain_within_10 + savgol_loss_within_10) as f32 / 2.0),
    ];
    
    let overall_best = methods.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
    
    println!("\n🥇 OVERALL WINNER: {} (Combined Score: {:.1})", overall_best.0, overall_best.1);
    
    // Method wins analysis
    let mut gain_wins = HashMap::new();
    let mut loss_wins = HashMap::new();
    let mut combined_wins = HashMap::new();
    
    for result in results {
        // Best gain method for this file
        let gain_methods = [
            ("Baseline", result.baseline_gain_accuracy),
            ("Dist3m", result.dist3m_gain_accuracy),
            ("Dist61m", result.dist61m_gain_accuracy),
            ("TwoPass", result.twopass_gain_accuracy),
            ("SavGol", result.savgol_gain_accuracy),
        ];
        let best_gain = gain_methods.iter()
            .min_by_key(|(_, acc)| ((acc - 100.0).abs() * 1000.0) as i32)
            .unwrap().0;
        *gain_wins.entry(best_gain).or_insert(0) += 1;
        
        // Best loss method for this file
        let loss_methods = [
            ("Baseline", result.baseline_loss_accuracy),
            ("Dist3m", result.dist3m_loss_accuracy),
            ("Dist61m", result.dist61m_loss_accuracy),
            ("TwoPass", result.twopass_loss_accuracy),
            ("SavGol", result.savgol_loss_accuracy),
        ];
        let best_loss = loss_methods.iter()
            .min_by_key(|(_, acc)| ((acc - 100.0).abs() * 1000.0) as i32)
            .unwrap().0;
        *loss_wins.entry(best_loss).or_insert(0) += 1;
        
        // Best combined method
        let combined_methods = [
            ("Baseline", (result.baseline_gain_accuracy - 100.0).abs() + (result.baseline_loss_accuracy - 100.0).abs()),
            ("Dist3m", (result.dist3m_gain_accuracy - 100.0).abs() + (result.dist3m_loss_accuracy - 100.0).abs()),
            ("Dist61m", (result.dist61m_gain_accuracy - 100.0).abs() + (result.dist61m_loss_accuracy - 100.0).abs()),
            ("TwoPass", (result.twopass_gain_accuracy - 100.0).abs() + (result.twopass_loss_accuracy - 100.0).abs()),
            ("SavGol", (result.savgol_gain_accuracy - 100.0).abs() + (result.savgol_loss_accuracy - 100.0).abs()),
        ];
        let best_combined = combined_methods.iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap().0;
        *combined_wins.entry(best_combined).or_insert(0) += 1;
    }
    
    println!("\n📈 FILE-BY-FILE WINS:");
    println!("GAIN accuracy wins:");
    for (method, count) in gain_wins {
        println!("   {}: {} files ({:.1}%)", method, count, (count as f32 / total_files) * 100.0);
    }
    
    println!("LOSS accuracy wins:");
    for (method, count) in loss_wins {
        println!("   {}: {} files ({:.1}%)", method, count, (count as f32 / total_files) * 100.0);
    }
    
    println!("COMBINED accuracy wins:");
    for (method, count) in combined_wins {
        println!("   {}: {} files ({:.1}%)", method, count, (count as f32 / total_files) * 100.0);
    }
    
    println!("\n💡 KEY INSIGHTS:");
    println!("• Distance-based 3m vs Default: Gain {:.1}% vs {:.1}%, Loss {:.1}% vs {:.1}%",
             dist3m_gain_acc, baseline_gain_acc, dist3m_loss_acc, baseline_loss_acc);
    println!("• Distance-based 6.1m vs Default: Gain {:.1}% vs {:.1}%, Loss {:.1}% vs {:.1}%",
             dist61m_gain_acc, baseline_gain_acc, dist61m_loss_acc, baseline_loss_acc);
    println!("• Two-Pass vs Default: Gain {:.1}% vs {:.1}%, Loss {:.1}% vs {:.1}%",
             twopass_gain_acc, baseline_gain_acc, twopass_loss_acc, baseline_loss_acc);
    println!("• Savitzky-Golay vs Default: Gain {:.1}% vs {:.1}%, Loss {:.1}% vs {:.1}%",
             savgol_gain_acc, baseline_gain_acc, savgol_loss_acc, baseline_loss_acc);
    
    println!("\n✅ Results saved to:");
    println!("   • detailed_file_comparison.csv (file-by-file results)");
    println!("   • five_method_comparison.csv (summary statistics)");
}