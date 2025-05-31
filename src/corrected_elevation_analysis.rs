/// CORRECTED ELEVATION ANALYSIS WITH PROPER SCORING
/// 
/// Proper Scoring Logic:
/// 1. PRIMARY: Gain accuracy vs official elevation gain benchmark
/// 2. SECONDARY: Loss should be close to gain value (what goes up, comes down)
/// 3. COMBINED: Best method = highest gain accuracy + gain/loss balance

use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;

#[derive(Debug, Serialize, Clone)]
pub struct CorrectedFileResult {
    filename: String,
    official_gain_m: u32,
    
    // Raw unprocessed data
    raw_gain_m: f32,
    raw_loss_m: f32,
    raw_gain_accuracy: f32,
    raw_gain_loss_ratio: f32,
    
    // Top 10 methods with corrected scoring
    method_1_name: String,
    method_1_gain_m: f32,
    method_1_loss_m: f32,
    method_1_gain_accuracy: f32,
    method_1_gain_loss_ratio: f32,
    method_1_combined_score: f32,
    
    method_2_name: String,
    method_2_gain_m: f32,
    method_2_loss_m: f32,
    method_2_gain_accuracy: f32,
    method_2_gain_loss_ratio: f32,
    method_2_combined_score: f32,
    
    method_3_name: String,
    method_3_gain_m: f32,
    method_3_loss_m: f32,
    method_3_gain_accuracy: f32,
    method_3_gain_loss_ratio: f32,
    method_3_combined_score: f32,
    
    method_4_name: String,
    method_4_gain_m: f32,
    method_4_loss_m: f32,
    method_4_gain_accuracy: f32,
    method_4_gain_loss_ratio: f32,
    method_4_combined_score: f32,
    
    method_5_name: String,
    method_5_gain_m: f32,
    method_5_loss_m: f32,
    method_5_gain_accuracy: f32,
    method_5_gain_loss_ratio: f32,
    method_5_combined_score: f32,
    
    // Current 3.0m baseline
    current_3m_gain_m: f32,
    current_3m_loss_m: f32,
    current_3m_gain_accuracy: f32,
    current_3m_gain_loss_ratio: f32,
    current_3m_combined_score: f32,
    
    // Analysis
    best_method_name: String,
    best_gain_accuracy: f32,
    best_gain_loss_balance: f32,
    improvement_vs_current: f32,
}

#[derive(Debug, Clone)]
struct MethodResult {
    name: String,
    gain: f32,
    loss: f32,
    gain_accuracy: f32,
    gain_loss_ratio: f32,
    combined_score: f32,
}

#[derive(Debug, Clone)]
struct GpxFileData {
    filename: String,
    elevations: Vec<f64>,
    distances: Vec<f64>,
    official_gain: u32,
}

pub fn run_corrected_elevation_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎯 CORRECTED ELEVATION ANALYSIS");
    println!("===============================");
    println!("PROPER SCORING LOGIC:");
    println!("1. PRIMARY: Gain accuracy vs official elevation gain");
    println!("2. SECONDARY: Gain/loss balance (should be close to 1.0 ratio)");
    println!("3. COMBINED: Best = highest gain accuracy + balanced gain/loss\n");
    
    let start_time = std::time::Instant::now();
    
    // Load data
    let (gpx_data, valid_files) = load_gpx_data(gpx_folder)?;
    let files_with_elevation: Vec<_> = valid_files.into_iter()
        .filter(|file| {
            if let Some(data) = gpx_data.get(file) {
                data.elevations.iter().any(|&e| (e - data.elevations[0]).abs() > 0.1) && data.official_gain > 0
            } else {
                false
            }
        })
        .collect();
    
    println!("📊 Processing {} files with official benchmarks", files_with_elevation.len());
    
    // Process with corrected methods
    let results = process_files_corrected_scoring(&gpx_data, &files_with_elevation)?;
    
    // Generate summary statistics
    let summary_stats = generate_summary_statistics(&results);
    
    // Write detailed results
    let detailed_output = Path::new(gpx_folder).join("corrected_elevation_analysis_detailed.csv");
    write_detailed_results(&results, &detailed_output)?;
    
    // Write summary
    let summary_output = Path::new(gpx_folder).join("corrected_elevation_analysis_summary.csv");
    write_summary_results(&summary_stats, &summary_output)?;
    
    // Print analysis
    print_corrected_analysis(&results, &summary_stats);
    
    println!("\n⏱️  Analysis completed in {:.1}s", start_time.elapsed().as_secs_f64());
    println!("📁 Results saved to:");
    println!("   • {}", detailed_output.display());
    println!("   • {}", summary_output.display());
    
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
        if entry.file_type().is_file() && 
           entry.path().extension().and_then(|s| s.to_str()) == Some("gpx") {
            
            let filename = entry.path().file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            
            if let Ok(file) = File::open(entry.path()) {
                let reader = BufReader::new(file);
                if let Ok(gpx) = read(reader) {
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
                            distances.push(distances[i-1] + a.haversine_distance(&b));
                        }
                        
                        let elevations: Vec<f64> = coords.iter().map(|c| c.2).collect();
                        let official_gain = official_data.get(&filename.to_lowercase()).copied().unwrap_or(0);
                        
                        if official_gain > 0 {
                            gpx_data.insert(filename.clone(), GpxFileData {
                                filename: filename.clone(),
                                elevations,
                                distances,
                                official_gain,
                            });
                            valid_files.push(filename);
                        }
                    }
                }
            }
        }
    }
    
    Ok((gpx_data, valid_files))
}

fn process_files_corrected_scoring(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<CorrectedFileResult>, Box<dyn std::error::Error>> {
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    let results: Vec<CorrectedFileResult> = valid_files
        .par_iter()
        .filter_map(|filename| {
            if let Some(file_data) = gpx_data_arc.get(filename) {
                Some(process_single_file_corrected(file_data))
            } else {
                None
            }
        })
        .collect();
    
    Ok(results)
}

fn process_single_file_corrected(file_data: &GpxFileData) -> CorrectedFileResult {
    let official_gain = file_data.official_gain as f32;
    
    // Raw data
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&file_data.elevations);
    let raw_gain_accuracy = (raw_gain / official_gain) * 100.0;
    let raw_gain_loss_ratio = raw_gain / raw_loss.max(1.0); // Avoid division by zero
    
    // Test top methods from previous analysis
    let methods = vec![
        ("TwoPass-3m+2.5m", apply_two_pass_method(&file_data.elevations, &file_data.distances, 3.0, 2.5)),
        ("DistBased-2.1m", apply_distance_based(&file_data.elevations, &file_data.distances, 2.1)),
        ("DistBased-1.7m", apply_distance_based(&file_data.elevations, &file_data.distances, 1.7)),
        ("DistBased-1.9m", apply_distance_based(&file_data.elevations, &file_data.distances, 1.9)),
        ("DistBased-2.0m", apply_distance_based(&file_data.elevations, &file_data.distances, 2.0)),
    ];
    
    // Current 3.0m baseline
    let (current_gain, current_loss) = apply_distance_based(&file_data.elevations, &file_data.distances, 3.0);
    let current_gain_accuracy = (current_gain / official_gain) * 100.0;
    let current_gain_loss_ratio = current_gain / current_loss.max(1.0);
    let current_combined_score = calculate_combined_score(current_gain_accuracy, current_gain_loss_ratio);
    
    // Calculate scores for all methods
    let mut method_results: Vec<MethodResult> = methods.into_iter()
        .map(|(name, (gain, loss))| {
            let gain_accuracy = (gain / official_gain) * 100.0;
            let gain_loss_ratio = gain / loss.max(1.0);
            let combined_score = calculate_combined_score(gain_accuracy, gain_loss_ratio);
            
            MethodResult {
                name: name.to_string(),
                gain,
                loss,
                gain_accuracy,
                gain_loss_ratio,
                combined_score,
            }
        })
        .collect();
    
    // Sort by combined score (best first)
    method_results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    
    // Pad with empty methods if needed first
    while method_results.len() < 5 {
        method_results.push(MethodResult {
            name: "N/A".to_string(),
            gain: 0.0,
            loss: 0.0,
            gain_accuracy: 0.0,
            gain_loss_ratio: 0.0,
            combined_score: 0.0,
        });
    }
    
    // Find best method (after padding)
    let best_method = &method_results[0];
    let improvement_vs_current = best_method.combined_score - current_combined_score;
    
    CorrectedFileResult {
        filename: file_data.filename.clone(),
        official_gain_m: file_data.official_gain,
        
        raw_gain_m: raw_gain,
        raw_loss_m: raw_loss,
        raw_gain_accuracy: raw_gain_accuracy,
        raw_gain_loss_ratio: raw_gain_loss_ratio,
        
        method_1_name: method_results[0].name.clone(),
        method_1_gain_m: method_results[0].gain,
        method_1_loss_m: method_results[0].loss,
        method_1_gain_accuracy: method_results[0].gain_accuracy,
        method_1_gain_loss_ratio: method_results[0].gain_loss_ratio,
        method_1_combined_score: method_results[0].combined_score,
        
        method_2_name: method_results[1].name.clone(),
        method_2_gain_m: method_results[1].gain,
        method_2_loss_m: method_results[1].loss,
        method_2_gain_accuracy: method_results[1].gain_accuracy,
        method_2_gain_loss_ratio: method_results[1].gain_loss_ratio,
        method_2_combined_score: method_results[1].combined_score,
        
        method_3_name: method_results[2].name.clone(),
        method_3_gain_m: method_results[2].gain,
        method_3_loss_m: method_results[2].loss,
        method_3_gain_accuracy: method_results[2].gain_accuracy,
        method_3_gain_loss_ratio: method_results[2].gain_loss_ratio,
        method_3_combined_score: method_results[2].combined_score,
        
        method_4_name: method_results[3].name.clone(),
        method_4_gain_m: method_results[3].gain,
        method_4_loss_m: method_results[3].loss,
        method_4_gain_accuracy: method_results[3].gain_accuracy,
        method_4_gain_loss_ratio: method_results[3].gain_loss_ratio,
        method_4_combined_score: method_results[3].combined_score,
        
        method_5_name: method_results[4].name.clone(),
        method_5_gain_m: method_results[4].gain,
        method_5_loss_m: method_results[4].loss,
        method_5_gain_accuracy: method_results[4].gain_accuracy,
        method_5_gain_loss_ratio: method_results[4].gain_loss_ratio,
        method_5_combined_score: method_results[4].combined_score,
        
        current_3m_gain_m: current_gain,
        current_3m_loss_m: current_loss,
        current_3m_gain_accuracy: current_gain_accuracy,
        current_3m_gain_loss_ratio: current_gain_loss_ratio,
        current_3m_combined_score: current_combined_score,
        
        best_method_name: best_method.name.clone(),
        best_gain_accuracy: best_method.gain_accuracy,
        best_gain_loss_balance: best_method.gain_loss_ratio,
        improvement_vs_current: improvement_vs_current,
    }
}

fn calculate_combined_score(gain_accuracy: f32, gain_loss_ratio: f32) -> f32 {
    // PRIMARY: Gain accuracy (weight: 80%)
    let gain_score = 100.0 - (gain_accuracy - 100.0).abs();
    
    // SECONDARY: Gain/loss balance (weight: 20%)
    // Ideal ratio is 1.0 (gain = loss), penalize deviations
    let ratio_penalty = (gain_loss_ratio - 1.0).abs() * 10.0; // 10% penalty per 0.1 deviation
    let balance_score = (100.0 - ratio_penalty).max(0.0);
    
    // Combined score (0-100 scale)
    (gain_score * 0.8 + balance_score * 0.2).max(0.0)
}

fn calculate_raw_gain_loss(elevations: &[f64]) -> (f32, f32) {
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

fn apply_distance_based(elevations: &[f64], distances: &[f64], interval: f64) -> (f32, f32) {
    // Use your existing distance-based processing
    use crate::custom_smoother::{ElevationData, SmoothingVariant};
    
    let mut elevation_data = ElevationData::new_with_variant(
        elevations.to_vec(),
        distances.to_vec(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval);
    let gain = elevation_data.get_total_elevation_gain() as f32;
    let loss = elevation_data.get_total_elevation_loss() as f32;
    
    (gain, loss)
}

fn apply_two_pass_method(elevations: &[f64], distances: &[f64], gain_interval: f64, loss_interval: f64) -> (f32, f32) {
    // Two-pass: different intervals for gain and loss
    let (gain, _) = apply_distance_based(elevations, distances, gain_interval);
    let (_, loss) = apply_distance_based(elevations, distances, loss_interval);
    (gain, loss)
}

#[derive(Debug, Serialize)]
struct SummaryStats {
    method_name: String,
    avg_gain_accuracy: f32,
    median_gain_accuracy: f32,
    avg_gain_loss_ratio: f32,
    median_gain_loss_ratio: f32,
    avg_combined_score: f32,
    files_within_5_percent_gain: u32,
    files_within_10_percent_gain: u32,
    files_with_balanced_ratio: u32, // gain/loss ratio between 0.8-1.2
    total_files: u32,
}

fn generate_summary_statistics(results: &[CorrectedFileResult]) -> Vec<SummaryStats> {
    let methods = vec![
        ("Current-3.0m", extract_current_stats(results)),
        ("Best-Per-File", extract_best_stats(results)),
        ("TwoPass-3m+2.5m", extract_method_stats(results, 1)),
        ("DistBased-2.1m", extract_method_stats(results, 2)),
        ("DistBased-1.7m", extract_method_stats(results, 3)),
        ("DistBased-1.9m", extract_method_stats(results, 4)),
        ("DistBased-2.0m", extract_method_stats(results, 5)),
    ];
    
    methods.into_iter().map(|(name, stats)| {
        SummaryStats {
            method_name: name.to_string(),
            avg_gain_accuracy: stats.0,
            median_gain_accuracy: stats.1,
            avg_gain_loss_ratio: stats.2,
            median_gain_loss_ratio: stats.3,
            avg_combined_score: stats.4,
            files_within_5_percent_gain: stats.5,
            files_within_10_percent_gain: stats.6,
            files_with_balanced_ratio: stats.7,
            total_files: results.len() as u32,
        }
    }).collect()
}

fn extract_current_stats(results: &[CorrectedFileResult]) -> (f32, f32, f32, f32, f32, u32, u32, u32) {
    let gain_accs: Vec<f32> = results.iter().map(|r| r.current_3m_gain_accuracy).collect();
    let ratios: Vec<f32> = results.iter().map(|r| r.current_3m_gain_loss_ratio).collect();
    let scores: Vec<f32> = results.iter().map(|r| r.current_3m_combined_score).collect();
    
    calculate_stats(&gain_accs, &ratios, &scores)
}

fn extract_best_stats(results: &[CorrectedFileResult]) -> (f32, f32, f32, f32, f32, u32, u32, u32) {
    let gain_accs: Vec<f32> = results.iter().map(|r| r.best_gain_accuracy).collect();
    let ratios: Vec<f32> = results.iter().map(|r| r.best_gain_loss_balance).collect();
    let scores: Vec<f32> = results.iter().map(|r| r.method_1_combined_score).collect();
    
    calculate_stats(&gain_accs, &ratios, &scores)
}

fn extract_method_stats(results: &[CorrectedFileResult], method_num: usize) -> (f32, f32, f32, f32, f32, u32, u32, u32) {
    let (gain_accs, ratios, scores): (Vec<f32>, Vec<f32>, Vec<f32>) = match method_num {
        1 => (
            results.iter().map(|r| r.method_1_gain_accuracy).collect(),
            results.iter().map(|r| r.method_1_gain_loss_ratio).collect(),
            results.iter().map(|r| r.method_1_combined_score).collect(),
        ),
        2 => (
            results.iter().map(|r| r.method_2_gain_accuracy).collect(),
            results.iter().map(|r| r.method_2_gain_loss_ratio).collect(),
            results.iter().map(|r| r.method_2_combined_score).collect(),
        ),
        3 => (
            results.iter().map(|r| r.method_3_gain_accuracy).collect(),
            results.iter().map(|r| r.method_3_gain_loss_ratio).collect(),
            results.iter().map(|r| r.method_3_combined_score).collect(),
        ),
        4 => (
            results.iter().map(|r| r.method_4_gain_accuracy).collect(),
            results.iter().map(|r| r.method_4_gain_loss_ratio).collect(),
            results.iter().map(|r| r.method_4_combined_score).collect(),
        ),
        _ => (
            results.iter().map(|r| r.method_5_gain_accuracy).collect(),
            results.iter().map(|r| r.method_5_gain_loss_ratio).collect(),
            results.iter().map(|r| r.method_5_combined_score).collect(),
        ),
    };
    
    calculate_stats(&gain_accs, &ratios, &scores)
}

fn calculate_stats(gain_accs: &[f32], ratios: &[f32], scores: &[f32]) -> (f32, f32, f32, f32, f32, u32, u32, u32) {
    let avg_gain_acc = gain_accs.iter().sum::<f32>() / gain_accs.len() as f32;
    let avg_ratio = ratios.iter().sum::<f32>() / ratios.len() as f32;
    let avg_score = scores.iter().sum::<f32>() / scores.len() as f32;
    
    let mut sorted_gains = gain_accs.to_vec();
    sorted_gains.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_gain_acc = sorted_gains[sorted_gains.len() / 2];
    
    let mut sorted_ratios = ratios.to_vec();
    sorted_ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_ratio = sorted_ratios[sorted_ratios.len() / 2];
    
    let within_5_pct = gain_accs.iter().filter(|&&acc| (acc - 100.0).abs() <= 5.0).count() as u32;
    let within_10_pct = gain_accs.iter().filter(|&&acc| (acc - 100.0).abs() <= 10.0).count() as u32;
    let balanced_ratio = ratios.iter().filter(|&&r| r >= 0.8 && r <= 1.2).count() as u32;
    
    (avg_gain_acc, median_gain_acc, avg_ratio, median_ratio, avg_score, within_5_pct, within_10_pct, balanced_ratio)
}

fn write_detailed_results(results: &[CorrectedFileResult], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Filename", "Official_Gain_m",
        "Raw_Gain_m", "Raw_Loss_m", "Raw_Gain_Acc_%", "Raw_Gain_Loss_Ratio",
        "Best_Method", "Best_Gain_m", "Best_Loss_m", "Best_Gain_Acc_%", "Best_Ratio", "Best_Score",
        "Method2", "M2_Gain_m", "M2_Loss_m", "M2_Gain_Acc_%", "M2_Ratio", "M2_Score",
        "Method3", "M3_Gain_m", "M3_Loss_m", "M3_Gain_Acc_%", "M3_Ratio", "M3_Score",
        "Current_3m_Gain_m", "Current_3m_Loss_m", "Current_3m_Gain_Acc_%", "Current_3m_Ratio", "Current_3m_Score",
        "Improvement_vs_Current"
    ])?;
    
    // Write data
    for result in results {
        wtr.write_record(&[
            &result.filename,
            &result.official_gain_m.to_string(),
            &format!("{:.1}", result.raw_gain_m),
            &format!("{:.1}", result.raw_loss_m),
            &format!("{:.1}", result.raw_gain_accuracy),
            &format!("{:.2}", result.raw_gain_loss_ratio),
            &result.method_1_name,
            &format!("{:.1}", result.method_1_gain_m),
            &format!("{:.1}", result.method_1_loss_m),
            &format!("{:.1}", result.method_1_gain_accuracy),
            &format!("{:.2}", result.method_1_gain_loss_ratio),
            &format!("{:.1}", result.method_1_combined_score),
            &result.method_2_name,
            &format!("{:.1}", result.method_2_gain_m),
            &format!("{:.1}", result.method_2_loss_m),
            &format!("{:.1}", result.method_2_gain_accuracy),
            &format!("{:.2}", result.method_2_gain_loss_ratio),
            &format!("{:.1}", result.method_2_combined_score),
            &result.method_3_name,
            &format!("{:.1}", result.method_3_gain_m),
            &format!("{:.1}", result.method_3_loss_m),
            &format!("{:.1}", result.method_3_gain_accuracy),
            &format!("{:.2}", result.method_3_gain_loss_ratio),
            &format!("{:.1}", result.method_3_combined_score),
            &format!("{:.1}", result.current_3m_gain_m),
            &format!("{:.1}", result.current_3m_loss_m),
            &format!("{:.1}", result.current_3m_gain_accuracy),
            &format!("{:.2}", result.current_3m_gain_loss_ratio),
            &format!("{:.1}", result.current_3m_combined_score),
            &format!("{:.1}", result.improvement_vs_current),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn write_summary_results(summary_stats: &[SummaryStats], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    wtr.write_record(&[
        "Method", "Avg_Gain_Acc_%", "Median_Gain_Acc_%", "Avg_Gain_Loss_Ratio", "Median_Ratio",
        "Avg_Combined_Score", "Files_Within_5%_Gain", "Files_Within_10%_Gain", "Files_Balanced_Ratio", "Total_Files"
    ])?;
    
    for stats in summary_stats {
        wtr.write_record(&[
            &stats.method_name,
            &format!("{:.1}", stats.avg_gain_accuracy),
            &format!("{:.1}", stats.median_gain_accuracy),
            &format!("{:.2}", stats.avg_gain_loss_ratio),
            &format!("{:.2}", stats.median_gain_loss_ratio),
            &format!("{:.1}", stats.avg_combined_score),
            &stats.files_within_5_percent_gain.to_string(),
            &stats.files_within_10_percent_gain.to_string(),
            &stats.files_with_balanced_ratio.to_string(),
            &stats.total_files.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_corrected_analysis(results: &[CorrectedFileResult], summary_stats: &[SummaryStats]) {
    println!("\n🎯 CORRECTED ELEVATION ANALYSIS RESULTS");
    println!("======================================");
    
    // Overall statistics
    let total_files = results.len();
    let significant_improvements = results.iter()
        .filter(|r| r.improvement_vs_current > 5.0)
        .count();
    
    println!("\n📊 OVERALL STATISTICS:");
    println!("Total files analyzed: {}", total_files);
    println!("Files with significant improvement (>5 points): {}", significant_improvements);
    println!("Improvement rate: {:.1}%", (significant_improvements as f32 / total_files as f32) * 100.0);
    
    // Method comparison
    println!("\n🏆 METHOD COMPARISON (Corrected Scoring):");
    println!("Method                | Avg Gain Acc% | Within ±5% | Within ±10% | Balanced Ratio | Avg Score");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for stats in summary_stats {
        println!("{:20} | {:12.1} | {:9} | {:10} | {:13} | {:8.1}",
                 stats.method_name,
                 stats.avg_gain_accuracy,
                 stats.files_within_5_percent_gain,
                 stats.files_within_10_percent_gain,
                 stats.files_with_balanced_ratio,
                 stats.avg_combined_score);
    }
    
    // Best method identification
    if let Some(best_method) = summary_stats.iter().max_by(|a, b| a.avg_combined_score.partial_cmp(&b.avg_combined_score).unwrap()) {
        println!("\n🏅 OVERALL WINNER: {}", best_method.method_name);
        println!("• Average gain accuracy: {:.1}%", best_method.avg_gain_accuracy);
        println!("• Files within ±5% gain accuracy: {}/{}", best_method.files_within_5_percent_gain, best_method.total_files);
        println!("• Files with balanced gain/loss: {}/{}", best_method.files_with_balanced_ratio, best_method.total_files);
        println!("• Combined score: {:.1}", best_method.avg_combined_score);
    }
    
    // Key insights
    println!("\n💡 KEY INSIGHTS:");
    if let Some(current_stats) = summary_stats.iter().find(|s| s.method_name.contains("Current")) {
        if let Some(best_stats) = summary_stats.iter().max_by(|a, b| a.avg_combined_score.partial_cmp(&b.avg_combined_score).unwrap()) {
            let gain_improvement = best_stats.avg_gain_accuracy - current_stats.avg_gain_accuracy;
            let score_improvement = best_stats.avg_combined_score - current_stats.avg_combined_score;
            
            println!("• Gain accuracy improvement: {:.1} percentage points", gain_improvement);
            println!("• Combined score improvement: {:.1} points", score_improvement);
            println!("• Additional files within ±5%: {}", best_stats.files_within_5_percent_gain.saturating_sub(current_stats.files_within_5_percent_gain));
        }
    }
    
    // Validation notes
    println!("\n✅ VALIDATION NOTES:");
    println!("• Scoring now focuses primarily on gain accuracy vs official benchmarks");
    println!("• Gain/loss balance used as secondary metric (penalizes unrealistic ratios)");
    println!("• Combined score: 80% gain accuracy + 20% gain/loss balance");
    println!("• All elevation numbers are actual calculated values, not percentages");
}