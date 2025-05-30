use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;
use crate::custom_smoother::{ElevationData, SmoothingVariant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessingMethod {
    Standard,              // Standard distance-based (baseline)
    AsymmetricInterval,    // Different intervals for gain/loss
    DirectionalDeadzone,   // Different thresholds for gain/loss
    LossCompensation,      // Apply correction factor to loss
    GradientBased,         // Protect steep sections
    TwoPass,              // Separate gain/loss passes
    HybridSelective,      // Selective smoothing based on variance
}

#[derive(Debug, Serialize, Clone)]
pub struct MethodResult {
    method: String,
    parameters: String,
    // Accuracy scores
    score_98_102: u32,
    score_95_105: u32,
    score_90_110: u32,
    score_85_115: u32,
    score_80_120: u32,
    files_outside_80_120: u32,
    weighted_accuracy_score: f32,
    // Gain/Loss balance metrics
    gain_loss_balance_score: f32,
    files_balanced_85_115: u32,
    files_balanced_70_130: u32,
    avg_gain_loss_ratio: f32,
    median_gain_loss_ratio: f32,
    // Traditional metrics
    average_accuracy: f32,
    median_accuracy: f32,
    worst_accuracy: f32,
    best_accuracy: f32,
    std_deviation: f32,
    success_rate: f32,
    // Gain/loss metrics
    avg_raw_gain: f32,
    avg_raw_loss: f32,
    avg_processed_gain: f32,
    avg_processed_loss: f32,
    total_raw_elevation_loss: f32,
    loss_reduction_percent: f32,
    gain_reduction_percent: f32,
    // Combined scores
    combined_score: f32,
    loss_preservation_score: f32,
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
struct ProcessingResult {
    accuracy: f32,
    raw_gain: f32,
    raw_loss: f32,
    processed_gain: f32,
    processed_loss: f32,
    gain_loss_ratio: f32,
}

pub fn run_asymmetric_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🔬 ASYMMETRIC & ALTERNATIVE PROCESSING ANALYSIS");
    println!("==============================================");
    println!("Comparing methods to preserve elevation loss while maintaining gain accuracy\n");
    
    // Load GPX data
    println!("📂 Loading GPX files...");
    let start = std::time::Instant::now();
    let (gpx_files_data, valid_files) = load_gpx_data(gpx_folder)?;
    println!("✅ Loaded {} files in {:.2}s", valid_files.len(), start.elapsed().as_secs_f64());
    
    // Filter files with elevation data
    let files_with_elevation: Vec<_> = valid_files.into_iter()
        .filter(|file| {
            if let Some(data) = gpx_files_data.get(file) {
                let has_elevation = data.elevations.iter()
                    .any(|&e| (e - data.elevations[0]).abs() > 0.1);
                has_elevation
            } else {
                false
            }
        })
        .collect();
    
    println!("📊 Processing {} files with valid elevation data", files_with_elevation.len());
    
    // Define test configurations
    let test_configs = vec![
        // Standard baseline
        (ProcessingMethod::Standard, vec![2.275]),
        
        // Asymmetric intervals (gain_interval, loss_interval)
        (ProcessingMethod::AsymmetricInterval, vec![2.275, 6.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.275, 4.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.275, 8.0]),
        
        // Directional deadzone (gain_threshold, loss_threshold)
        (ProcessingMethod::DirectionalDeadzone, vec![0.5, 0.1]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.3, 0.05]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.7, 0.2]),
        
        // Loss compensation (interval, compensation_factor)
        (ProcessingMethod::LossCompensation, vec![2.275, 1.5]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.8]),
        (ProcessingMethod::LossCompensation, vec![2.275, 2.0]),
        
        // Gradient-based (interval, gradient_threshold)
        (ProcessingMethod::GradientBased, vec![2.275, 5.0]),
        (ProcessingMethod::GradientBased, vec![2.275, 7.0]),
        (ProcessingMethod::GradientBased, vec![2.275, 10.0]),
        
        // Two-pass (gain_interval, loss_interval)
        (ProcessingMethod::TwoPass, vec![2.275, 0.5]),
        (ProcessingMethod::TwoPass, vec![2.275, 1.0]),
        (ProcessingMethod::TwoPass, vec![2.275, 0.1]),
        
        // Hybrid selective (interval, variance_threshold)
        (ProcessingMethod::HybridSelective, vec![2.275, 2.0]),
        (ProcessingMethod::HybridSelective, vec![2.275, 5.0]),
        (ProcessingMethod::HybridSelective, vec![2.275, 10.0]),
    ];
    
    // Process all methods
    let processing_start = std::time::Instant::now();
    let results = process_all_methods(&gpx_files_data, &files_with_elevation, &test_configs)?;
    println!("✅ Processing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write results
    let output_path = Path::new(gpx_folder).join("asymmetric_methods_comparison.csv");
    write_comparison_results(&results, &output_path)?;
    
    // Print summary
    print_comparison_summary(&results);
    
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

fn process_all_methods(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String],
    test_configs: &[(ProcessingMethod, Vec<f64>)]
) -> Result<Vec<MethodResult>, Box<dyn std::error::Error>> {
    let gpx_data_arc = Arc::new(gpx_data.clone());
    let total_configs = test_configs.len();
    let total_files = valid_files.len();
    
    println!("\n🚀 Processing {} methods × {} files", total_configs, total_files);
    println!("⚡ Using parallel processing on {} cores", num_cpus::get());
    
    let mut all_results = Vec::new();
    
    for (idx, (method, params)) in test_configs.iter().enumerate() {
        println!("\n[{}/{}] Processing {:?} with params {:?}", idx + 1, total_configs, method, params);
        
        let file_results: Vec<ProcessingResult> = valid_files
            .par_iter()
            .filter_map(|filename| {
                let gpx_data = Arc::clone(&gpx_data_arc);
                
                if let Some(file_data) = gpx_data.get(filename) {
                    if file_data.official_gain > 0 {
                        return Some(process_single_file(file_data, *method, params));
                    }
                }
                None
            })
            .collect();
        
        if !file_results.is_empty() {
            let method_result = create_method_result(*method, params, &file_results);
            all_results.push(method_result);
        }
    }
    
    Ok(all_results)
}

fn process_single_file(
    file_data: &GpxFileData,
    method: ProcessingMethod,
    params: &[f64]
) -> ProcessingResult {
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&file_data.elevations);
    
    let (processed_gain, processed_loss) = match method {
        ProcessingMethod::Standard => {
            process_standard(file_data, params[0])
        },
        ProcessingMethod::AsymmetricInterval => {
            process_asymmetric_interval(file_data, params[0], params[1])
        },
        ProcessingMethod::DirectionalDeadzone => {
            process_directional_deadzone(file_data, params[0], params[1])
        },
        ProcessingMethod::LossCompensation => {
            let (gain, loss) = process_standard(file_data, params[0]);
            (gain, loss * params[1])
        },
        ProcessingMethod::GradientBased => {
            process_gradient_based(file_data, params[0], params[1])
        },
        ProcessingMethod::TwoPass => {
            process_two_pass(file_data, params[0], params[1])
        },
        ProcessingMethod::HybridSelective => {
            process_hybrid_selective(file_data, params[0], params[1])
        },
    };
    
    let accuracy = if file_data.official_gain > 0 {
        (processed_gain as f32 / file_data.official_gain as f32) * 100.0
    } else {
        100.0
    };
    
    let gain_loss_ratio = if processed_gain > 0.0 {
        (processed_loss / processed_gain * 100.0)
    } else {
        100.0
    };
    
    ProcessingResult {
        accuracy,
        raw_gain: raw_gain as f32,
        raw_loss: raw_loss as f32,
        processed_gain: processed_gain as f32,
        processed_loss: processed_loss as f32,
        gain_loss_ratio: gain_loss_ratio as f32,
    }
}

fn process_standard(file_data: &GpxFileData, interval: f64) -> (f64, f64) {
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval);
    
    (elevation_data.get_total_elevation_gain(), elevation_data.get_total_elevation_loss())
}

fn process_asymmetric_interval(
    file_data: &GpxFileData, 
    gain_interval: f64, 
    loss_interval: f64
) -> (f64, f64) {
    // Identify ascending and descending segments
    let mut segments = Vec::new();
    let mut current_start = 0;
    let mut is_ascending = true;
    
    for i in 1..file_data.elevations.len() {
        let trend_changed = (file_data.elevations[i] > file_data.elevations[i-1]) != is_ascending;
        
        if trend_changed || i == file_data.elevations.len() - 1 {
            segments.push((current_start, i, is_ascending));
            current_start = i;
            is_ascending = !is_ascending;
        }
    }
    
    let mut total_gain = 0.0;
    let mut total_loss = 0.0;
    
    // Process each segment with appropriate interval
    for (start, end, ascending) in segments {
        if end <= start {
            continue;
        }
        
        let segment_elevations = file_data.elevations[start..=end].to_vec();
        let segment_distances = file_data.distances[start..=end].to_vec();
        
        // Normalize distances to start at 0
        let base_distance = segment_distances[0];
        let normalized_distances: Vec<f64> = segment_distances.iter()
            .map(|d| d - base_distance)
            .collect();
        
        let interval = if ascending { gain_interval } else { loss_interval };
        
        let mut segment_data = ElevationData::new_with_variant(
            segment_elevations,
            normalized_distances,
            SmoothingVariant::DistBased
        );
        
        segment_data.apply_custom_interval_processing(interval);
        
        total_gain += segment_data.get_total_elevation_gain();
        total_loss += segment_data.get_total_elevation_loss();
    }
    
    (total_gain, total_loss)
}

fn process_directional_deadzone(
    file_data: &GpxFileData,
    gain_threshold: f64,
    loss_threshold: f64
) -> (f64, f64) {
    // First apply standard smoothing
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(2.275);
    let smoothed_elevations = elevation_data.enhanced_altitude.clone();
    
    // Apply directional deadzone
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for i in 1..smoothed_elevations.len() {
        let delta = smoothed_elevations[i] - smoothed_elevations[i-1];
        
        if delta > gain_threshold {
            gain += delta;
        } else if delta < -loss_threshold {
            loss += -delta;
        }
    }
    
    (gain, loss)
}

fn process_gradient_based(
    file_data: &GpxFileData,
    interval: f64,
    gradient_threshold: f64
) -> (f64, f64) {
    let mut protected_indices = Vec::new();
    
    // Identify steep sections
    for i in 1..file_data.elevations.len() {
        let distance_diff = file_data.distances[i] - file_data.distances[i-1];
        if distance_diff > 0.0 {
            let gradient = ((file_data.elevations[i] - file_data.elevations[i-1]) / distance_diff) * 100.0;
            
            if gradient.abs() > gradient_threshold {
                protected_indices.push(i-1);
                protected_indices.push(i);
            }
        }
    }
    
    // Apply smoothing but preserve protected points
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval);
    let mut processed_elevations = elevation_data.enhanced_altitude.clone();
    
    // Restore protected points
    for &idx in &protected_indices {
        if idx < processed_elevations.len() {
            processed_elevations[idx] = file_data.elevations[idx];
        }
    }
    
    // Calculate gain/loss from mixed elevations
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for i in 1..processed_elevations.len() {
        let delta = processed_elevations[i] - processed_elevations[i-1];
        if delta > 0.0 {
            gain += delta;
        } else {
            loss += -delta;
        }
    }
    
    (gain, loss)
}

fn process_two_pass(
    file_data: &GpxFileData,
    gain_interval: f64,
    loss_interval: f64
) -> (f64, f64) {
    // Pass 1: Calculate gain with tight smoothing
    let mut gain_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    gain_data.apply_custom_interval_processing(gain_interval);
    let gain = gain_data.get_total_elevation_gain();
    
    // Pass 2: Calculate loss with minimal smoothing
    let mut loss_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    loss_data.apply_custom_interval_processing(loss_interval);
    let loss = loss_data.get_total_elevation_loss();
    
    (gain, loss)
}

fn process_hybrid_selective(
    file_data: &GpxFileData,
    interval: f64,
    variance_threshold: f64
) -> (f64, f64) {
    let window_size = 10;
    let mut should_smooth = vec![true; file_data.elevations.len()];
    
    // Calculate local variance
    for i in 0..file_data.elevations.len() {
        let start = i.saturating_sub(window_size / 2);
        let end = (i + window_size / 2).min(file_data.elevations.len());
        
        if end > start {
            let window = &file_data.elevations[start..end];
            let mean = window.iter().sum::<f64>() / window.len() as f64;
            let variance = window.iter()
                .map(|&e| (e - mean).powi(2))
                .sum::<f64>() / window.len() as f64;
            
            // Don't smooth low-variance descending sections
            let is_descending = end > start + 1 && 
                file_data.elevations[end-1] < file_data.elevations[start];
            
            if variance < variance_threshold && is_descending {
                should_smooth[i] = false;
            }
        }
    }
    
    // Apply selective smoothing
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval);
    let smoothed = elevation_data.enhanced_altitude.clone();
    
    // Mix smoothed and raw based on should_smooth
    let mut final_elevations = vec![0.0; file_data.elevations.len()];
    for i in 0..file_data.elevations.len() {
        final_elevations[i] = if should_smooth[i] {
            smoothed[i]
        } else {
            file_data.elevations[i]
        };
    }
    
    // Calculate gain/loss
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for i in 1..final_elevations.len() {
        let delta = final_elevations[i] - final_elevations[i-1];
        if delta > 0.0 {
            gain += delta;
        } else {
            loss += -delta;
        }
    }
    
    (gain, loss)
}

fn calculate_raw_gain_loss(elevations: &[f64]) -> (u32, u32) {
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
    
    (gain.round() as u32, loss.round() as u32)
}

fn create_method_result(
    method: ProcessingMethod,
    params: &[f64],
    results: &[ProcessingResult]
) -> MethodResult {
    let method_name = match method {
        ProcessingMethod::Standard => "Standard Distance-Based",
        ProcessingMethod::AsymmetricInterval => "Asymmetric Intervals",
        ProcessingMethod::DirectionalDeadzone => "Directional Deadzone",
        ProcessingMethod::LossCompensation => "Loss Compensation",
        ProcessingMethod::GradientBased => "Gradient-Based Protection",
        ProcessingMethod::TwoPass => "Two-Pass Processing",
        ProcessingMethod::HybridSelective => "Hybrid Selective",
    };
    
    let parameters = match method {
        ProcessingMethod::Standard => format!("interval={}m", params[0]),
        ProcessingMethod::AsymmetricInterval => format!("gain={}m, loss={}m", params[0], params[1]),
        ProcessingMethod::DirectionalDeadzone => format!("gain_th={}m, loss_th={}m", params[0], params[1]),
        ProcessingMethod::LossCompensation => format!("interval={}m, factor={}", params[0], params[1]),
        ProcessingMethod::GradientBased => format!("interval={}m, gradient>{}%", params[0], params[1]),
        ProcessingMethod::TwoPass => format!("gain={}m, loss={}m", params[0], params[1]),
        ProcessingMethod::HybridSelective => format!("interval={}m, variance<{}", params[0], params[1]),
    };
    
    let accuracies: Vec<f32> = results.iter().map(|r| r.accuracy).collect();
    let gain_loss_ratios: Vec<f32> = results.iter().map(|r| r.gain_loss_ratio).collect();
    
    // Calculate accuracy bands
    let score_98_102 = accuracies.iter().filter(|&&acc| acc >= 98.0 && acc <= 102.0).count() as u32;
    let score_95_105 = accuracies.iter().filter(|&&acc| acc >= 95.0 && acc <= 105.0).count() as u32;
    let score_90_110 = accuracies.iter().filter(|&&acc| acc >= 90.0 && acc <= 110.0).count() as u32;
    let score_85_115 = accuracies.iter().filter(|&&acc| acc >= 85.0 && acc <= 115.0).count() as u32;
    let score_80_120 = accuracies.iter().filter(|&&acc| acc >= 80.0 && acc <= 120.0).count() as u32;
    let files_outside_80_120 = accuracies.iter().filter(|&&acc| acc < 80.0 || acc > 120.0).count() as u32;
    
    // Calculate gain/loss balance metrics
    let files_balanced_85_115 = gain_loss_ratios.iter()
        .filter(|&&ratio| ratio >= 85.0 && ratio <= 115.0)
        .count() as u32;
    let files_balanced_70_130 = gain_loss_ratios.iter()
        .filter(|&&ratio| ratio >= 70.0 && ratio <= 130.0)
        .count() as u32;
    
    let avg_gain_loss_ratio = gain_loss_ratios.iter().sum::<f32>() / gain_loss_ratios.len() as f32;
    
    let mut sorted_ratios = gain_loss_ratios.clone();
    sorted_ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_gain_loss_ratio = if sorted_ratios.len() % 2 == 0 {
        (sorted_ratios[sorted_ratios.len() / 2 - 1] + 
         sorted_ratios[sorted_ratios.len() / 2]) / 2.0
    } else {
        sorted_ratios[sorted_ratios.len() / 2]
    };
    
    // Scoring
    let weighted_accuracy_score = (score_98_102 as f32 * 10.0) +
                                 ((score_95_105 - score_98_102) as f32 * 6.0) +
                                 ((score_90_110 - score_95_105) as f32 * 3.0) +
                                 ((score_85_115 - score_90_110) as f32 * 1.5) +
                                 ((score_80_120 - score_85_115) as f32 * 1.0) -
                                 (files_outside_80_120 as f32 * 5.0);
    
    let total_files = results.len() as f32;
    let gain_loss_balance_score = (files_balanced_85_115 as f32 * 10.0) +
                                  ((files_balanced_70_130 - files_balanced_85_115) as f32 * 5.0) +
                                  ((median_gain_loss_ratio - 100.0).abs() * -2.0);
    
    // Statistics
    let average_accuracy = accuracies.iter().sum::<f32>() / accuracies.len() as f32;
    let mut sorted_accuracies = accuracies.clone();
    sorted_accuracies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let median_accuracy = if sorted_accuracies.len() % 2 == 0 {
        (sorted_accuracies[sorted_accuracies.len() / 2 - 1] + 
         sorted_accuracies[sorted_accuracies.len() / 2]) / 2.0
    } else {
        sorted_accuracies[sorted_accuracies.len() / 2]
    };
    
    let best_accuracy = accuracies.iter()
        .min_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied()
        .unwrap_or(100.0);
        
    let worst_accuracy = accuracies.iter()
        .max_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied()
        .unwrap_or(100.0);
    
    let variance = accuracies.iter()
        .map(|&acc| (acc - average_accuracy).powi(2))
        .sum::<f32>() / accuracies.len() as f32;
    let std_deviation = variance.sqrt();
    
    let success_rate = (score_90_110 as f32 / total_files) * 100.0;
    
    // Gain/loss metrics
    let avg_raw_gain = results.iter().map(|r| r.raw_gain).sum::<f32>() / total_files;
    let avg_raw_loss = results.iter().map(|r| r.raw_loss).sum::<f32>() / total_files;
    let avg_processed_gain = results.iter().map(|r| r.processed_gain).sum::<f32>() / total_files;
    let avg_processed_loss = results.iter().map(|r| r.processed_loss).sum::<f32>() / total_files;
    
    let total_raw_elevation_loss = results.iter().map(|r| r.raw_loss).sum::<f32>();
    
    let gain_reduction_percent = if avg_raw_gain > 0.0 {
        ((avg_raw_gain - avg_processed_gain) / avg_raw_gain) * 100.0
    } else {
        0.0
    };
    
    let loss_reduction_percent = if avg_raw_loss > 0.0 {
        ((avg_raw_loss - avg_processed_loss) / avg_raw_loss) * 100.0
    } else {
        0.0
    };
    
    let loss_preservation_score = 100.0 - (loss_reduction_percent - gain_reduction_percent).abs();
    
    // Combined score that heavily weights gain/loss balance
    let combined_score = (weighted_accuracy_score * 0.4) + 
                        (gain_loss_balance_score * 0.4) +
                        (loss_preservation_score * 0.2);
    
    MethodResult {
        method: method_name.to_string(),
        parameters,
        score_98_102,
        score_95_105,
        score_90_110,
        score_85_115,
        score_80_120,
        files_outside_80_120,
        weighted_accuracy_score,
        gain_loss_balance_score,
        files_balanced_85_115,
        files_balanced_70_130,
        avg_gain_loss_ratio,
        median_gain_loss_ratio,
        average_accuracy,
        median_accuracy,
        worst_accuracy,
        best_accuracy,
        std_deviation,
        success_rate,
        avg_raw_gain,
        avg_raw_loss,
        avg_processed_gain,
        avg_processed_loss,
        total_raw_elevation_loss,
        loss_reduction_percent,
        gain_reduction_percent,
        combined_score,
        loss_preservation_score,
        total_files: total_files as u32,
    }
}

fn write_comparison_results(results: &[MethodResult], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Method",
        "Parameters",
        "Combined Score",
        "Median Gain/Loss %",
        "Median Accuracy %",
        "Success Rate %",
        "Gain Reduction %",
        "Loss Reduction %",
        "Files Balanced 85-115%",
        "98-102%",
        "95-105%",
        "90-110%",
        "Accuracy Score",
        "Balance Score",
        "Preservation Score",
        "Total Files",
    ])?;
    
    // Sort by combined score
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    
    // Write data
    for result in sorted_results {
        wtr.write_record(&[
            &result.method,
            &result.parameters,
            &format!("{:.2}", result.combined_score),
            &format!("{:.1}", result.median_gain_loss_ratio),
            &format!("{:.2}", result.median_accuracy),
            &format!("{:.1}", result.success_rate),
            &format!("{:.1}", result.gain_reduction_percent),
            &format!("{:.1}", result.loss_reduction_percent),
            &result.files_balanced_85_115.to_string(),
            &result.score_98_102.to_string(),
            &result.score_95_105.to_string(),
            &result.score_90_110.to_string(),
            &format!("{:.2}", result.weighted_accuracy_score),
            &format!("{:.2}", result.gain_loss_balance_score),
            &format!("{:.2}", result.loss_preservation_score),
            &result.total_files.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    println!("\n✅ Results saved to: {}", output_path.display());
    Ok(())
}

fn print_comparison_summary(results: &[MethodResult]) {
    println!("\n📊 ASYMMETRIC METHODS COMPARISON SUMMARY");
    println!("========================================");
    
    // Find best overall
    let best = results.iter()
        .max_by(|a, b| a.combined_score.partial_cmp(&b.combined_score).unwrap())
        .unwrap();
    
    println!("\n🏆 BEST OVERALL METHOD:");
    println!("   Method: {}", best.method);
    println!("   Parameters: {}", best.parameters);
    println!("   Combined Score: {:.2}", best.combined_score);
    println!("   Median Gain/Loss Ratio: {:.1}%", best.median_gain_loss_ratio);
    println!("   Median Accuracy: {:.2}%", best.median_accuracy);
    println!("   Gain reduction: {:.1}%, Loss reduction: {:.1}%", 
             best.gain_reduction_percent, best.loss_reduction_percent);
    
    // Show top 5
    let mut sorted_by_score = results.to_vec();
    sorted_by_score.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    
    println!("\n🏅 TOP 5 METHODS:");
    println!("Rank | Method                  | Parameters              | Score  | Ratio% | Acc%  | Gain% | Loss%");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for (i, result) in sorted_by_score.iter().take(5).enumerate() {
        println!("{:4} | {:23} | {:23} | {:6.2} | {:6.1} | {:5.1} | {:5.1} | {:5.1}",
                 i + 1,
                 result.method,
                 result.parameters,
                 result.combined_score,
                 result.median_gain_loss_ratio,
                 result.median_accuracy,
                 result.gain_reduction_percent,
                 result.loss_reduction_percent);
    }
    
    // Compare best of each method type
    println!("\n📈 BEST OF EACH METHOD TYPE:");
    let method_types = [
        "Standard Distance-Based",
        "Asymmetric Intervals",
        "Directional Deadzone",
        "Loss Compensation",
        "Gradient-Based Protection",
        "Two-Pass Processing",
        "Hybrid Selective",
    ];
    
    for method_type in &method_types {
        if let Some(best_of_type) = results.iter()
            .filter(|r| r.method == *method_type)
            .max_by(|a, b| a.combined_score.partial_cmp(&b.combined_score).unwrap()) {
            
            println!("{}: ratio={:.1}%, acc={:.1}%, loss_red={:.1}%",
                     method_type,
                     best_of_type.median_gain_loss_ratio,
                     best_of_type.median_accuracy,
                     best_of_type.loss_reduction_percent);
        }
    }
    
    println!("\n💡 KEY FINDINGS:");
    
    // Find method with best gain/loss ratio
    let best_ratio = results.iter()
        .min_by_key(|r| ((r.median_gain_loss_ratio - 100.0).abs() * 100.0) as i32)
        .unwrap();
    
    println!("• Best gain/loss ratio: {} ({}) = {:.1}%",
             best_ratio.method, best_ratio.parameters, best_ratio.median_gain_loss_ratio);
    
    // Find method with best accuracy
    let best_accuracy = results.iter()
        .max_by(|a, b| a.median_accuracy.partial_cmp(&b.median_accuracy).unwrap())
        .unwrap();
    
    println!("• Best accuracy: {} ({}) = {:.2}%",
             best_accuracy.method, best_accuracy.parameters, best_accuracy.median_accuracy);
}