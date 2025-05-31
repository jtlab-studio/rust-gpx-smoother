/// FOCUSED SYMMETRIC ANALYSIS: 0.5m to 2.5m in 0.1m increments
/// 
/// High-resolution search for the optimal SymmetricFixed interval
/// Goal: Find the best balance between:
/// 1. Elevation gain accuracy (closest to 100%)
/// 2. Gain/loss balance (ratio closest to 1.0)
/// 3. Maximum files in 90-110% and 80-120% accuracy ranges

use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;

#[derive(Debug, Serialize, Clone)]
pub struct FocusedSymmetricResult {
    interval_m: f32,
    
    // Primary performance metrics
    files_in_90_110_percent: u32,
    files_in_80_120_percent: u32,
    files_in_95_105_percent: u32,
    files_in_98_102_percent: u32,
    
    // Accuracy statistics
    median_gain_accuracy: f32,
    mean_gain_accuracy: f32,
    best_gain_accuracy: f32,
    worst_gain_accuracy: f32,
    accuracy_std_deviation: f32,
    
    // Gain/loss balance metrics
    median_gain_loss_ratio: f32,
    mean_gain_loss_ratio: f32,
    files_balanced_08_12: u32,     // Ratio between 0.8-1.2
    files_excellent_09_11: u32,    // Ratio between 0.9-1.1
    files_perfect_095_105: u32,    // Ratio between 0.95-1.05
    ratio_std_deviation: f32,
    
    // Combined optimization score
    optimization_score: f32,       // Higher = better overall performance
    
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
    processed_gain: f32,
    processed_loss: f32,
    gain_accuracy: f32,
    gain_loss_ratio: f32,
}

pub fn run_focused_symmetric_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🎯 FOCUSED SYMMETRIC ANALYSIS: 0.5m to 2.5m");
    println!("============================================");
    println!("High-resolution optimization of SymmetricFixed method:");
    println!("• Testing intervals: 0.5m to 2.5m in 0.1m increments (21 intervals)");
    println!("• Focus: Find optimal balance between accuracy and gain/loss ratio");
    println!("• Target metrics:");
    println!("  - Maximum files in 90-110% accuracy range");
    println!("  - Best gain/loss ratio balance (closest to 1.0)");
    println!("  - Highest combined optimization score\n");
    
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
    
    println!("📊 Processing {} files with valid elevation data", files_with_elevation.len());
    
    // Generate focused interval range: 0.5m to 2.5m in 0.1m increments
    let intervals: Vec<f32> = (5..=25).map(|i| i as f32 * 0.1).collect();
    println!("🔬 Testing {} intervals: {:.1}m to {:.1}m", 
             intervals.len(), intervals[0], intervals[intervals.len()-1]);
    
    // Process all intervals
    let processing_start = std::time::Instant::now();
    let results = process_all_symmetric_intervals(&gpx_files_data, &files_with_elevation, &intervals)?;
    println!("✅ Processing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write detailed results
    let output_path = Path::new(gpx_folder).join("focused_symmetric_analysis_0.5_to_2.5m.csv");
    write_focused_results(&results, &output_path)?;
    
    // Print comprehensive analysis
    print_focused_analysis(&results);
    
    let total_time = total_start.elapsed();
    println!("\n⏱️  TOTAL EXECUTION TIME: {:.1} seconds", total_time.as_secs_f64());
    println!("📁 Results saved to: {}", output_path.display());
    
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
                                        
                                        if official_gain > 0 {
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

fn process_all_symmetric_intervals(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String],
    intervals: &[f32]
) -> Result<Vec<FocusedSymmetricResult>, Box<dyn std::error::Error>> {
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("🚀 Processing {} intervals × {} files = {} total calculations",
             intervals.len(), valid_files.len(), 
             intervals.len() * valid_files.len());
    
    let results: Vec<FocusedSymmetricResult> = intervals
        .par_iter()
        .map(|&interval| {
            let file_results: Vec<SingleFileResult> = valid_files
                .iter()
                .filter_map(|filename| {
                    if let Some(file_data) = gpx_data_arc.get(filename) {
                        Some(process_single_file_symmetric(file_data, interval))
                    } else {
                        None
                    }
                })
                .collect();
            
            calculate_focused_metrics(interval, &file_results)
        })
        .collect();
    
    Ok(results)
}

fn process_single_file_symmetric(
    file_data: &GpxFileData,
    interval: f32
) -> SingleFileResult {
    // Apply SymmetricFixed processing with the specified interval
    let (gain, loss) = apply_symmetric_fixed_method(&file_data.elevations, &file_data.distances, interval as f64);
    
    let official_gain = file_data.official_gain as f32;
    let gain_accuracy = (gain / official_gain) * 100.0;
    let gain_loss_ratio = gain / loss.max(1.0); // Avoid division by zero
    
    SingleFileResult {
        filename: file_data.filename.clone(),
        official_gain: file_data.official_gain,
        processed_gain: gain,
        processed_loss: loss,
        gain_accuracy,
        gain_loss_ratio,
    }
}

fn apply_symmetric_fixed_method(
    elevations: &[f64],
    distances: &[f64],
    interval: f64
) -> (f32, f32) {
    // Use the new symmetric deadband filtering from custom_smoother
    use crate::custom_smoother::{ElevationData, SmoothingVariant};
    
    let mut elevation_data = ElevationData::new_with_variant(
        elevations.to_vec(),
        distances.to_vec(),
        SmoothingVariant::SymmetricFixed  // Use the fixed symmetric variant
    );
    
    // Apply custom interval processing with symmetric deadband
    elevation_data.apply_custom_interval_processing_symmetric(interval);
    
    let gain = elevation_data.get_total_elevation_gain() as f32;
    let loss = elevation_data.get_total_elevation_loss() as f32;
    
    (gain, loss)
}

fn calculate_focused_metrics(
    interval: f32,
    file_results: &[SingleFileResult]
) -> FocusedSymmetricResult {
    if file_results.is_empty() {
        return FocusedSymmetricResult {
            interval_m: interval,
            files_in_90_110_percent: 0,
            files_in_80_120_percent: 0,
            files_in_95_105_percent: 0,
            files_in_98_102_percent: 0,
            median_gain_accuracy: 0.0,
            mean_gain_accuracy: 0.0,
            best_gain_accuracy: 0.0,
            worst_gain_accuracy: 0.0,
            accuracy_std_deviation: 0.0,
            median_gain_loss_ratio: 0.0,
            mean_gain_loss_ratio: 0.0,
            files_balanced_08_12: 0,
            files_excellent_09_11: 0,
            files_perfect_095_105: 0,
            ratio_std_deviation: 0.0,
            optimization_score: 0.0,
            total_files: 0,
        };
    }
    
    let total_files = file_results.len() as u32;
    
    // Extract accuracy and ratio vectors
    let accuracies: Vec<f32> = file_results.iter().map(|r| r.gain_accuracy).collect();
    let ratios: Vec<f32> = file_results.iter().map(|r| r.gain_loss_ratio).collect();
    
    // Count files in accuracy ranges
    let files_in_90_110_percent = accuracies.iter().filter(|&&acc| acc >= 90.0 && acc <= 110.0).count() as u32;
    let files_in_80_120_percent = accuracies.iter().filter(|&&acc| acc >= 80.0 && acc <= 120.0).count() as u32;
    let files_in_95_105_percent = accuracies.iter().filter(|&&acc| acc >= 95.0 && acc <= 105.0).count() as u32;
    let files_in_98_102_percent = accuracies.iter().filter(|&&acc| acc >= 98.0 && acc <= 102.0).count() as u32;
    
    // Count files in ratio ranges
    let files_balanced_08_12 = ratios.iter().filter(|&&r| r >= 0.8 && r <= 1.2).count() as u32;
    let files_excellent_09_11 = ratios.iter().filter(|&&r| r >= 0.9 && r <= 1.1).count() as u32;
    let files_perfect_095_105 = ratios.iter().filter(|&&r| r >= 0.95 && r <= 1.05).count() as u32;
    
    // Calculate accuracy statistics
    let mean_gain_accuracy = accuracies.iter().sum::<f32>() / total_files as f32;
    let median_gain_accuracy = calculate_median(&accuracies);
    let best_gain_accuracy = accuracies.iter()
        .min_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied().unwrap_or(100.0);
    let worst_gain_accuracy = accuracies.iter()
        .max_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied().unwrap_or(100.0);
    let accuracy_std_deviation = calculate_std_deviation(&accuracies);
    
    // Calculate ratio statistics
    let mean_gain_loss_ratio = ratios.iter().sum::<f32>() / total_files as f32;
    let median_gain_loss_ratio = calculate_median(&ratios);
    let ratio_std_deviation = calculate_std_deviation(&ratios);
    
    // Calculate comprehensive optimization score
    let optimization_score = calculate_optimization_score(
        files_in_90_110_percent,
        files_in_80_120_percent,
        files_in_95_105_percent,
        files_in_98_102_percent,
        files_balanced_08_12,
        files_excellent_09_11,
        files_perfect_095_105,
        mean_gain_accuracy,
        median_gain_loss_ratio,
        accuracy_std_deviation,
        ratio_std_deviation,
        total_files
    );
    
    FocusedSymmetricResult {
        interval_m: interval,
        files_in_90_110_percent,
        files_in_80_120_percent,
        files_in_95_105_percent,
        files_in_98_102_percent,
        median_gain_accuracy,
        mean_gain_accuracy,
        best_gain_accuracy,
        worst_gain_accuracy,
        accuracy_std_deviation,
        median_gain_loss_ratio,
        mean_gain_loss_ratio,
        files_balanced_08_12,
        files_excellent_09_11,
        files_perfect_095_105,
        ratio_std_deviation,
        optimization_score,
        total_files,
    }
}

fn calculate_optimization_score(
    files_90_110: u32,
    _files_80_120: u32,
    _files_95_105: u32,
    files_98_102: u32,
    _files_balanced: u32,
    files_excellent: u32,
    _files_perfect: u32,
    mean_accuracy: f32,
    median_ratio: f32,
    accuracy_std: f32,
    ratio_std: f32,
    total_files: u32
) -> f32 {
    let total_f = total_files as f32;
    
    // Primary metrics (70% of score)
    let accuracy_coverage = (files_90_110 as f32 / total_f) * 100.0 * 0.30; // 30% weight on 90-110% coverage
    let precision_coverage = (files_98_102 as f32 / total_f) * 100.0 * 0.20; // 20% weight on 98-102% precision
    let balance_quality = (files_excellent as f32 / total_f) * 100.0 * 0.20; // 20% weight on excellent balance
    
    // Accuracy quality (20% of score)
    let accuracy_quality = (100.0 - (mean_accuracy - 100.0).abs()) * 0.10; // 10% weight on mean accuracy
    let ratio_quality = (100.0 - (median_ratio - 1.0).abs() * 50.0).max(0.0) * 0.10; // 10% weight on ratio balance
    
    // Consistency bonus (10% of score)
    let consistency_bonus = ((20.0 - accuracy_std.min(20.0)) / 20.0 * 50.0 + 
                            (2.0 - ratio_std.min(2.0)) / 2.0 * 50.0) * 0.10;
    
    accuracy_coverage + precision_coverage + balance_quality + accuracy_quality + ratio_quality + consistency_bonus
}

fn calculate_median(values: &[f32]) -> f32 {
    if values.is_empty() { return 0.0; }
    
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    }
}

fn calculate_std_deviation(values: &[f32]) -> f32 {
    if values.is_empty() { return 0.0; }
    
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let variance = values.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f32>() / values.len() as f32;
    variance.sqrt()
}

fn write_focused_results(
    results: &[FocusedSymmetricResult],
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Interval_m", "Optimization_Score",
        "Files_90-110%", "Files_80-120%", "Files_95-105%", "Files_98-102%",
        "Mean_Accuracy_%", "Median_Accuracy_%", "Best_Accuracy_%", "Worst_Accuracy_%", "Accuracy_StdDev",
        "Mean_Ratio", "Median_Ratio", "Files_Balanced_0.8-1.2", "Files_Excellent_0.9-1.1", 
        "Files_Perfect_0.95-1.05", "Ratio_StdDev", "Total_Files"
    ])?;
    
    // Sort by optimization score (highest first)
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.optimization_score.partial_cmp(&a.optimization_score).unwrap());
    
    // Write data
    for result in sorted_results {
        wtr.write_record(&[
            &format!("{:.1}", result.interval_m),
            &format!("{:.2}", result.optimization_score),
            &result.files_in_90_110_percent.to_string(),
            &result.files_in_80_120_percent.to_string(),
            &result.files_in_95_105_percent.to_string(),
            &result.files_in_98_102_percent.to_string(),
            &format!("{:.2}", result.mean_gain_accuracy),
            &format!("{:.2}", result.median_gain_accuracy),
            &format!("{:.2}", result.best_gain_accuracy),
            &format!("{:.2}", result.worst_gain_accuracy),
            &format!("{:.2}", result.accuracy_std_deviation),
            &format!("{:.3}", result.mean_gain_loss_ratio),
            &format!("{:.3}", result.median_gain_loss_ratio),
            &result.files_balanced_08_12.to_string(),
            &result.files_excellent_09_11.to_string(),
            &result.files_perfect_095_105.to_string(),
            &format!("{:.3}", result.ratio_std_deviation),
            &result.total_files.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_focused_analysis(results: &[FocusedSymmetricResult]) {
    println!("\n🎯 FOCUSED SYMMETRIC ANALYSIS RESULTS");
    println!("====================================");
    
    // Sort by optimization score
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.optimization_score.partial_cmp(&a.optimization_score).unwrap());
    
    let best_result = &sorted_results[0];
    let total_files = best_result.total_files;
    
    println!("\n🏆 OPTIMAL INTERVAL FOUND:");
    println!("Interval: {:.1}m", best_result.interval_m);
    println!("Optimization Score: {:.2}", best_result.optimization_score);
    
    println!("\n📊 ACCURACY PERFORMANCE:");
    println!("• Files in 90-110% range: {}/{} ({:.1}%)", 
             best_result.files_in_90_110_percent, total_files,
             (best_result.files_in_90_110_percent as f32 / total_files as f32) * 100.0);
    println!("• Files in 80-120% range: {}/{} ({:.1}%)", 
             best_result.files_in_80_120_percent, total_files,
             (best_result.files_in_80_120_percent as f32 / total_files as f32) * 100.0);
    println!("• Files in 95-105% range: {}/{} ({:.1}%)", 
             best_result.files_in_95_105_percent, total_files,
             (best_result.files_in_95_105_percent as f32 / total_files as f32) * 100.0);
    println!("• Files in 98-102% range: {}/{} ({:.1}%)", 
             best_result.files_in_98_102_percent, total_files,
             (best_result.files_in_98_102_percent as f32 / total_files as f32) * 100.0);
    
    println!("\n⚖️  GAIN/LOSS BALANCE:");
    println!("• Median gain/loss ratio: {:.3} (ideal: 1.000)", best_result.median_gain_loss_ratio);
    println!("• Files with balanced ratios (0.8-1.2): {}/{} ({:.1}%)", 
             best_result.files_balanced_08_12, total_files,
             (best_result.files_balanced_08_12 as f32 / total_files as f32) * 100.0);
    println!("• Files with excellent ratios (0.9-1.1): {}/{} ({:.1}%)", 
             best_result.files_excellent_09_11, total_files,
             (best_result.files_excellent_09_11 as f32 / total_files as f32) * 100.0);
    println!("• Files with perfect ratios (0.95-1.05): {}/{} ({:.1}%)", 
             best_result.files_perfect_095_105, total_files,
             (best_result.files_perfect_095_105 as f32 / total_files as f32) * 100.0);
    
    println!("\n📈 STATISTICAL QUALITY:");
    println!("• Mean accuracy: {:.2}%", best_result.mean_gain_accuracy);
    println!("• Median accuracy: {:.2}%", best_result.median_gain_accuracy);
    println!("• Best accuracy: {:.2}%", best_result.best_gain_accuracy);
    println!("• Worst accuracy: {:.2}%", best_result.worst_gain_accuracy);
    println!("• Accuracy std deviation: {:.2}%", best_result.accuracy_std_deviation);
    println!("• Ratio std deviation: {:.3}", best_result.ratio_std_deviation);
    
    // Show top 10 intervals
    println!("\n🔝 TOP 10 INTERVALS:");
    println!("Rank | Interval | Score  | 90-110% | Balanced | Median Acc% | Median Ratio");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for (i, result) in sorted_results.iter().take(10).enumerate() {
        println!("{:4} | {:7.1}m | {:6.2} | {:7} | {:8} | {:10.2} | {:11.3}",
                 i + 1,
                 result.interval_m,
                 result.optimization_score,
                 result.files_in_90_110_percent,
                 result.files_balanced_08_12,
                 result.median_gain_accuracy,
                 result.median_gain_loss_ratio);
    }
    
    // Performance trends analysis
    println!("\n📊 PERFORMANCE TRENDS:");
    
    // Find the interval with most files in 90-110%
    let best_coverage = sorted_results.iter()
        .max_by_key(|r| r.files_in_90_110_percent)
        .unwrap();
    
    // Find the interval with best balance
    let best_balance = sorted_results.iter()
        .min_by_key(|r| ((r.median_gain_loss_ratio - 1.0).abs() * 1000.0) as i32)
        .unwrap();
    
    // Find the interval with best precision
    let best_precision = sorted_results.iter()
        .max_by_key(|r| r.files_in_98_102_percent)
        .unwrap();
    
    println!("• Best coverage (90-110%): {:.1}m with {}/{} files ({:.1}%)",
             best_coverage.interval_m, best_coverage.files_in_90_110_percent, total_files,
             (best_coverage.files_in_90_110_percent as f32 / total_files as f32) * 100.0);
    
    println!("• Best balance: {:.1}m with {:.3} median ratio",
             best_balance.interval_m, best_balance.median_gain_loss_ratio);
    
    println!("• Best precision (98-102%): {:.1}m with {}/{} files ({:.1}%)",
             best_precision.interval_m, best_precision.files_in_98_102_percent, total_files,
             (best_precision.files_in_98_102_percent as f32 / total_files as f32) * 100.0);
    
    // Interval range analysis
    println!("\n🔍 INTERVAL RANGE INSIGHTS:");
    let low_intervals = sorted_results.iter().filter(|r| r.interval_m <= 1.0);
    let mid_intervals = sorted_results.iter().filter(|r| r.interval_m > 1.0 && r.interval_m <= 2.0);
    let high_intervals = sorted_results.iter().filter(|r| r.interval_m > 2.0);
    
    if let Some(best_low) = low_intervals.max_by(|a, b| a.optimization_score.partial_cmp(&b.optimization_score).unwrap()) {
        println!("• Best low interval (≤1.0m): {:.1}m (score: {:.2})", best_low.interval_m, best_low.optimization_score);
    }
    
    if let Some(best_mid) = mid_intervals.max_by(|a, b| a.optimization_score.partial_cmp(&b.optimization_score).unwrap()) {
        println!("• Best mid interval (1.0-2.0m): {:.1}m (score: {:.2})", best_mid.interval_m, best_mid.optimization_score);
    }
    
    if let Some(best_high) = high_intervals.max_by(|a, b| a.optimization_score.partial_cmp(&b.optimization_score).unwrap()) {
        println!("• Best high interval (>2.0m): {:.1}m (score: {:.2})", best_high.interval_m, best_high.optimization_score);
    }
    
    println!("\n🚀 RECOMMENDATION:");
    println!("Use SymmetricFixed with {:.1}m interval for optimal performance!", best_result.interval_m);
    println!("This achieves the best balance of accuracy and gain/loss ratio.");
}