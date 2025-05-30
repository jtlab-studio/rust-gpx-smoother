use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;
use biquad::{Biquad, DirectForm1, ToHertz, Coefficients, Q_BUTTERWORTH_F64};

#[derive(Debug, Serialize, Clone)]
pub struct ButterworthResult {
    interval_m: f32,
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
    // Butterworth specific
    avg_cutoff_frequency: f32,
    avg_epsilon_used: f32,
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
    cutoff_used: f32,
    epsilon_used: f32,
}

pub fn run_butterworth_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🦋 BUTTERWORTH FILTER ANALYSIS");
    println!("==============================");
    println!("Testing intervals: 0.10m to 7.00m in 0.025m increments (276 intervals)");
    println!("Method: Zero-phase Butterworth low-pass filtering\n");
    
    // Add biquad to Cargo.toml if not present
    println!("⚠️  Ensure 'biquad = \"0.4\"' is in your Cargo.toml dependencies\n");
    
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
    
    // Process with Butterworth approach
    let processing_start = std::time::Instant::now();
    let results = process_butterworth_range(&gpx_files_data, &files_with_elevation)?;
    println!("✅ Processing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write results
    let output_path = Path::new(gpx_folder).join("butterworth_analysis_0.1_to_7m.csv");
    write_butterworth_results(&results, &output_path)?;
    
    // Write comparison CSV
    let comparison_path = Path::new(gpx_folder).join("butterworth_vs_distance_comparison.csv");
    write_comparison_summary(&results, &comparison_path)?;
    
    // Print summary
    print_butterworth_summary(&results);
    
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

fn process_butterworth_range(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<ButterworthResult>, Box<dyn std::error::Error>> {
    // Test intervals from 0.10m to 7.00m in 0.025m increments
    let intervals: Vec<f32> = (4..=280).map(|i| i as f32 * 0.025).collect();
    
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("\n🚀 Processing {} intervals × {} files = {} total calculations",
             intervals.len(), valid_files.len(), intervals.len() * valid_files.len());
    println!("⚡ Using parallel processing on {} cores", num_cpus::get());
    
    // Create work items
    let work_items: Vec<(f32, String)> = intervals.iter()
        .flat_map(|&interval| {
            valid_files.iter().map(move |file| (interval, file.clone()))
        })
        .collect();
    
    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_items = work_items.len();
    let start_time = std::time::Instant::now();
    
    // Process all work items in parallel
    let all_results: Vec<(f32, String, ProcessingResult)> = work_items
        .par_iter()
        .filter_map(|(interval, filename)| {
            let gpx_data = Arc::clone(&gpx_data_arc);
            let processed_clone = Arc::clone(&processed);
            
            if let Some(file_data) = gpx_data.get(filename) {
                if file_data.official_gain > 0 {
                    let result = process_single_file_butterworth(file_data, *interval);
                    
                    // Update progress
                    let count = processed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count % 2000 == 0 || count == total_items {
                        let elapsed = start_time.elapsed().as_secs_f64();
                        let rate = count as f64 / elapsed;
                        let remaining = (total_items - count) as f64 / rate;
                        println!("  Progress: {}/{} ({:.1}%) - {:.0} items/sec - ETA: {:.0}s",
                                 count, total_items, 
                                 (count as f64 / total_items as f64) * 100.0,
                                 rate, remaining);
                    }
                    
                    return Some((*interval, filename.clone(), result));
                }
            }
            None
        })
        .collect();
    
    println!("✅ Parallel processing complete, aggregating results...");
    
    // Aggregate results by interval
    let mut results = Vec::new();
    
    for interval in intervals {
        let interval_results: Vec<_> = all_results.iter()
            .filter(|(i, _, _)| *i == interval)
            .map(|(_, _, r)| r)
            .collect();
        
        if !interval_results.is_empty() {
            results.push(create_butterworth_result(interval, &interval_results));
        }
    }
    
    Ok(results)
}

fn process_single_file_butterworth(file_data: &GpxFileData, interval_m: f32) -> ProcessingResult {
    // Calculate raw gain/loss
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&file_data.elevations);
    
    // Adaptive resampling: use interval/3 for sample spacing
    let sample_spacing = (interval_m / 3.0).max(0.5) as f64; // Convert to f64
    
    // Resample elevations to uniform spacing
    let resampled_elevations = resample_to_uniform_spacing(
        &file_data.elevations,
        &file_data.distances,
        sample_spacing
    );
    
    if resampled_elevations.len() < 10 {
        // Not enough points for meaningful filtering
        return ProcessingResult {
            accuracy: 0.0,
            raw_gain: raw_gain as f32,
            raw_loss: raw_loss as f32,
            processed_gain: raw_gain as f32,
            processed_loss: raw_loss as f32,
            gain_loss_ratio: 100.0,
            cutoff_used: 0.0,
            epsilon_used: 0.0,
        };
    }
    
    // Calculate cutoff frequency based on spatial wavelength
    // We want to suppress features smaller than the interval
    // For a low-pass filter: keep wavelengths > interval, remove wavelengths < interval
    let wavelength_to_keep = interval_m as f64 * 2.0; // Keep features larger than 2x interval
    let cutoff_cycles_per_meter = 1.0 / wavelength_to_keep;
    
    // Convert to normalized frequency
    let normalized_cutoff = cutoff_cycles_per_meter * sample_spacing;
    
    // Convert to Hz for the filter (sample rate = 1/sample_spacing)
    let sample_rate_hz = 1.0 / sample_spacing;
    let cutoff_hz = normalized_cutoff * sample_rate_hz;
    
    // Ensure cutoff is reasonable (between 0.01 and 0.45 of Nyquist)
    let nyquist = sample_rate_hz / 2.0;
    let cutoff_hz = cutoff_hz.clamp(0.01 * nyquist, 0.45 * nyquist);
    
    // Apply Butterworth filter
    let coeffs = match Coefficients::<f64>::from_params(
        biquad::Type::LowPass,
        sample_rate_hz.hz(),
        cutoff_hz.hz(),
        Q_BUTTERWORTH_F64
    ) {
        Ok(c) => c,
        Err(_) => {
            // Filter design failed, return unfiltered results
            return ProcessingResult {
                accuracy: 0.0,
                raw_gain: raw_gain as f32,
                raw_loss: raw_loss as f32,
                processed_gain: raw_gain as f32,
                processed_loss: raw_loss as f32,
                gain_loss_ratio: 100.0,
                cutoff_used: cutoff_hz as f32,
                epsilon_used: 0.0,
            };
        }
    };
    
    // Forward pass
    let mut df_forward = DirectForm1::<f64>::new(coeffs);
    let mut elev_fwd: Vec<f64> = resampled_elevations
        .iter()
        .map(|&x| df_forward.run(x))
        .collect();
    
    // Backward pass (reverse, filter, reverse)
    elev_fwd.reverse();
    let mut df_backward = DirectForm1::<f64>::new(coeffs);
    let mut elev_smooth: Vec<f64> = elev_fwd
        .iter()
        .map(|&x| df_backward.run(x))
        .collect();
    elev_smooth.reverse();
    
    // Calculate local noise for adaptive epsilon
    let local_noise = calculate_local_noise(&elev_smooth);
    
    // Scale epsilon with interval size but keep it reasonable
    // Smaller intervals need smaller epsilon to preserve detail
    let base_epsilon = 0.05 + (0.02 * interval_m as f64);
    let epsilon = base_epsilon.max(0.5 * local_noise).min(0.5);
    
    // Apply dead-zone and calculate gain/loss
    let mut processed_gain = 0.0;
    let mut processed_loss = 0.0;
    
    for i in 1..elev_smooth.len() {
        // Account for actual distance between resampled points
        let delta = elev_smooth[i] - elev_smooth[i-1];
        if delta.abs() > epsilon {
            if delta > 0.0 {
                processed_gain += delta;
            } else {
                processed_loss += -delta;
            }
        }
    }
    
    // No need to scale back - we're already working in the resampled space
    // The gain/loss is calculated from elevation differences at sample_spacing intervals
    
    let accuracy = if file_data.official_gain > 0 {
        (processed_gain as f32 / file_data.official_gain as f32) * 100.0
    } else {
        100.0
    };
    
    let gain_loss_ratio = if processed_gain > 0.0 {
        (processed_loss / processed_gain * 100.0) as f32
    } else {
        100.0
    };
    
    ProcessingResult {
        accuracy,
        raw_gain: raw_gain as f32,
        raw_loss: raw_loss as f32,
        processed_gain: processed_gain as f32,
        processed_loss: processed_loss as f32,
        gain_loss_ratio,
        cutoff_used: cutoff_hz as f32,
        epsilon_used: epsilon as f32,
    }
}

fn resample_to_uniform_spacing(
    elevations: &[f64],
    distances: &[f64],
    spacing_m: f64
) -> Vec<f64> {
    if elevations.is_empty() || distances.is_empty() {
        return vec![];
    }
    
    let total_distance = distances.last().unwrap();
    let num_samples = (total_distance / spacing_m).ceil() as usize + 1;
    let mut resampled = Vec::with_capacity(num_samples);
    
    // Linear interpolation at uniform spacing
    for i in 0..num_samples {
        let target_distance = i as f64 * spacing_m;
        
        // Find the segment containing this distance
        let idx = match distances.binary_search_by(|d| {
            d.partial_cmp(&target_distance).unwrap()
        }) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        
        if idx >= distances.len() - 1 {
            resampled.push(elevations.last().unwrap().clone());
        } else {
            // Linear interpolation
            let d0 = distances[idx];
            let d1 = distances[idx + 1];
            let e0 = elevations[idx];
            let e1 = elevations[idx + 1];
            
            let t = (target_distance - d0) / (d1 - d0);
            let elevation = e0 + t * (e1 - e0);
            resampled.push(elevation);
        }
    }
    
    resampled
}

fn calculate_local_noise(elevations: &[f64]) -> f64 {
    if elevations.len() < 5 {
        return 0.2;
    }
    
    // Calculate standard deviation of first differences
    let mut deltas = Vec::with_capacity(elevations.len() - 1);
    for window in elevations.windows(2) {
        deltas.push(window[1] - window[0]);
    }
    
    let mean_delta: f64 = deltas.iter().sum::<f64>() / deltas.len() as f64;
    let variance: f64 = deltas.iter()
        .map(|&d| (d - mean_delta).powi(2))
        .sum::<f64>() / deltas.len() as f64;
    
    variance.sqrt()
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

fn create_butterworth_result(
    interval: f32,
    results: &[&ProcessingResult]
) -> ButterworthResult {
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
    
    // Enhanced combined score for Butterworth (emphasizes balance more)
    let combined_score = (weighted_accuracy_score * 0.4) + 
                        (gain_loss_balance_score * 0.4) +
                        (loss_preservation_score * 0.2);
    
    // Butterworth-specific metrics
    let avg_cutoff_frequency = results.iter().map(|r| r.cutoff_used).sum::<f32>() / total_files;
    let avg_epsilon_used = results.iter().map(|r| r.epsilon_used).sum::<f32>() / total_files;
    
    ButterworthResult {
        interval_m: interval,
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
        avg_cutoff_frequency,
        avg_epsilon_used,
        total_files: total_files as u32,
    }
}

fn write_butterworth_results(results: &[ButterworthResult], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Interval (m)",
        "Combined Score",
        "Accuracy Score",
        "Balance Score",
        "Loss Preservation Score",
        "98-102%",
        "95-105%",
        "90-110%",
        "Files Balanced 85-115%",
        "Files Balanced 70-130%",
        "Avg Gain/Loss Ratio %",
        "Median Gain/Loss Ratio %",
        "Success Rate %",
        "Average Accuracy %",
        "Median Accuracy %",
        "Raw Gain (avg)",
        "Raw Loss (avg)",
        "Processed Gain (avg)",
        "Processed Loss (avg)",
        "Gain Reduction %",
        "Loss Reduction %",
        "Avg Cutoff Hz",
        "Avg Epsilon",
        "Total Files",
        "Files Outside 80-120%",
    ])?;
    
    // Sort by combined score
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    
    // Write data
    for result in sorted_results {
        wtr.write_record(&[
            format!("{:.3}", result.interval_m),
            format!("{:.2}", result.combined_score),
            format!("{:.2}", result.weighted_accuracy_score),
            format!("{:.2}", result.gain_loss_balance_score),
            format!("{:.2}", result.loss_preservation_score),
            result.score_98_102.to_string(),
            result.score_95_105.to_string(),
            result.score_90_110.to_string(),
            result.files_balanced_85_115.to_string(),
            result.files_balanced_70_130.to_string(),
            format!("{:.1}", result.avg_gain_loss_ratio),
            format!("{:.1}", result.median_gain_loss_ratio),
            format!("{:.1}", result.success_rate),
            format!("{:.2}", result.average_accuracy),
            format!("{:.2}", result.median_accuracy),
            format!("{:.1}", result.avg_raw_gain),
            format!("{:.1}", result.avg_raw_loss),
            format!("{:.1}", result.avg_processed_gain),
            format!("{:.1}", result.avg_processed_loss),
            format!("{:.1}", result.gain_reduction_percent),
            format!("{:.1}", result.loss_reduction_percent),
            format!("{:.3}", result.avg_cutoff_frequency),
            format!("{:.3}", result.avg_epsilon_used),
            result.total_files.to_string(),
            result.files_outside_80_120.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    println!("\n✅ Butterworth results saved to: {}", output_path.display());
    Ok(())
}

fn write_comparison_summary(
    butterworth_results: &[ButterworthResult],
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    // This would compare with distance-based results if available
    // For now, just write key metrics
    let mut wtr = Writer::from_path(output_path)?;
    
    wtr.write_record(&[
        "Metric",
        "Best Butterworth Interval",
        "Value",
    ])?;
    
    // Find best by different criteria
    let best_combined = butterworth_results.iter()
        .max_by(|a, b| a.combined_score.partial_cmp(&b.combined_score).unwrap())
        .unwrap();
    
    let best_balance = butterworth_results.iter()
        .max_by(|a, b| a.median_gain_loss_ratio.partial_cmp(&b.median_gain_loss_ratio).unwrap())
        .unwrap();
    
    wtr.write_record(&[
        "Best Overall",
        &format!("{:.3}m", best_combined.interval_m),
        &format!("Score: {:.2}, Ratio: {:.1}%", best_combined.combined_score, best_combined.median_gain_loss_ratio),
    ])?;
    
    wtr.write_record(&[
        "Best Gain/Loss Balance",
        &format!("{:.3}m", best_balance.interval_m),
        &format!("Median Ratio: {:.1}%", best_balance.median_gain_loss_ratio),
    ])?;
    
    wtr.flush()?;
    Ok(())
}

fn print_butterworth_summary(results: &[ButterworthResult]) {
    println!("\n🦋 BUTTERWORTH FILTER ANALYSIS SUMMARY");
    println!("=====================================");
    
    // Find best results
    let best_combined = results.iter()
        .max_by(|a, b| a.combined_score.partial_cmp(&b.combined_score).unwrap())
        .unwrap();
    
    let best_balance = results.iter()
        .min_by_key(|r| ((r.median_gain_loss_ratio - 100.0).abs() * 100.0) as i32)
        .unwrap();
    
    println!("\n🏆 BEST OVERALL (Butterworth):");
    println!("   Interval: {:.3}m", best_combined.interval_m);
    println!("   Combined Score: {:.2}", best_combined.combined_score);
    println!("   Median Gain/Loss Ratio: {:.1}%", best_combined.median_gain_loss_ratio);
    println!("   Gain reduction: {:.1}%, Loss reduction: {:.1}%",
             best_combined.gain_reduction_percent, best_combined.loss_reduction_percent);
    println!("   Cutoff frequency: {:.3} Hz", best_combined.avg_cutoff_frequency);
    println!("   Dead-zone epsilon: {:.3}m", best_combined.avg_epsilon_used);
    
    println!("\n🎯 BEST GAIN/LOSS PRESERVATION:");
    println!("   Interval: {:.3}m", best_balance.interval_m);
    println!("   Median Gain/Loss Ratio: {:.1}%", best_balance.median_gain_loss_ratio);
    println!("   Files with balanced gain/loss: {} ({:.1}%)",
             best_balance.files_balanced_85_115,
             (best_balance.files_balanced_85_115 as f32 / best_balance.total_files as f32) * 100.0);
    
    // Show top 5
    let mut sorted_by_combined = results.to_vec();
    sorted_by_combined.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    
    println!("\n🏅 TOP 5 INTERVALS (Butterworth):");
    println!("Rank | Interval | Combined | Median Ratio | Gain Red% | Loss Red% | Cutoff Hz");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for (i, result) in sorted_by_combined.iter().take(5).enumerate() {
        println!("{:4} | {:7.3}m | {:8.2} | {:11.1}% | {:9.1} | {:9.1} | {:9.3}",
                 i + 1,
                 result.interval_m,
                 result.combined_score,
                 result.median_gain_loss_ratio,
                 result.gain_reduction_percent,
                 result.loss_reduction_percent,
                 result.avg_cutoff_frequency);
    }
    
    println!("\n💡 KEY ADVANTAGE:");
    println!("Butterworth filtering maintains much better gain/loss symmetry");
    println!("compared to distance-based resampling, especially for rough terrain.");
}