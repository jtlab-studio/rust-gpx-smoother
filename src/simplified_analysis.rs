use std::path::{Path, PathBuf};
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;
use crate::custom_smoother::{ElevationData, SmoothingVariant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApproachType {
    DistBased,              // Pure distance-based approach
    DistBasedTwoStage,      // Distance-based + gradient & climb rate outlier removal
}

#[derive(Debug, Serialize, Clone)]
pub struct AnalysisResult {
    interval_m: f32,
    // DistBased results
    distbased_score_98_102: u32,
    distbased_score_95_105: u32,
    distbased_score_90_110: u32,
    distbased_files_outside_80_120: u32,
    distbased_weighted_score: f32,
    distbased_median_accuracy: f32,
    distbased_worst_accuracy: f32,
    distbased_success_rate: f32,
    // DistBased raw gain/loss
    distbased_avg_raw_gain: f32,
    distbased_avg_raw_loss: f32,
    distbased_avg_processed_gain: f32,
    distbased_avg_processed_loss: f32,
    distbased_gain_loss_similarity: f32,  // How similar processed gain is to processed loss
    distbased_gain_official_similarity: f32,  // How similar processed gain is to official gain
    
    // DistBasedTwoStage results
    twostage_score_98_102: u32,
    twostage_score_95_105: u32,
    twostage_score_90_110: u32,
    twostage_files_outside_80_120: u32,
    twostage_weighted_score: f32,
    twostage_median_accuracy: f32,
    twostage_worst_accuracy: f32,
    twostage_success_rate: f32,
    // TwoStage raw gain/loss
    twostage_avg_raw_gain: f32,
    twostage_avg_raw_loss: f32,
    twostage_avg_processed_gain: f32,
    twostage_avg_processed_loss: f32,
    twostage_gain_loss_similarity: f32,
    twostage_gain_official_similarity: f32,
    
    total_files: u32,
}

#[derive(Debug, Clone)]
struct GpxFileData {
    filename: String,
    elevations: Vec<f64>,
    distances: Vec<f64>,
    timestamps: Vec<f64>,
    official_gain: u32,
}

#[derive(Debug, Clone)]
struct ProcessingResult {
    accuracy: f32,
    raw_gain: f32,
    raw_loss: f32,
    processed_gain: f32,
    processed_loss: f32,
    gain_loss_similarity: f32,
    gain_official_similarity: f32,
}

pub fn run_simplified_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🔬 SIMPLIFIED ANALYSIS: DistBased vs DistBased+TwoStage");
    println!("========================================================");
    println!("Testing intervals: 1.0m to 8.0m in 0.1m increments (71 intervals)");
    println!("Tracking elevation loss and gain/loss similarity\n");
    
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
                if !has_elevation {
                    println!("⚠️  Excluding {} - no elevation variation", file);
                }
                has_elevation
            } else {
                false
            }
        })
        .collect();
    
    println!("📊 Processing {} files with valid elevation data", files_with_elevation.len());
    
    // Process both approaches
    let processing_start = std::time::Instant::now();
    let results = process_both_approaches(&gpx_files_data, &files_with_elevation)?;
    println!("✅ Processing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write results
    let output_path = Path::new(gpx_folder).join("distbased_vs_twostage_analysis.csv");
    write_results(&results, &output_path)?;
    
    // Print summary
    print_summary(&results);
    
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
                                    let mut timestamps = vec![];
                                    
                                    for track in gpx.tracks {
                                        for segment in track.segments {
                                            for pt in segment.points {
                                                if let Some(ele) = pt.elevation {
                                                    coords.push((pt.point().y(), pt.point().x(), ele));
                                                    
                                                    if let Some(time) = pt.time {
                                                        if let Ok(time_str) = time.format() {
                                                            timestamps.push(time_str);
                                                        }
                                                    }
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
                                        
                                        let mut time_seconds = vec![0.0];
                                        if timestamps.len() >= 2 {
                                            for i in 1..timestamps.len().min(coords.len()) {
                                                time_seconds.push(i as f64);
                                            }
                                        }
                                        while time_seconds.len() < coords.len() {
                                            time_seconds.push(time_seconds.len() as f64);
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
                                            timestamps: time_seconds,
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

fn process_both_approaches(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<AnalysisResult>, Box<dyn std::error::Error>> {
    // Test intervals from 1.0m to 8.0m in 0.1m increments
    let intervals: Vec<f32> = (10..=80).map(|i| i as f32 * 0.1).collect();
    
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("\n🚀 Processing {} intervals × {} files × 2 approaches = {} total calculations",
             intervals.len(), valid_files.len(), intervals.len() * valid_files.len() * 2);
    println!("⚡ Using parallel processing on {} cores", num_cpus::get());
    
    // Create work items
    let work_items: Vec<(f32, String, ApproachType)> = intervals.iter()
        .flat_map(|&interval| {
            valid_files.iter().flat_map(move |file| {
                vec![
                    (interval, file.clone(), ApproachType::DistBased),
                    (interval, file.clone(), ApproachType::DistBasedTwoStage),
                ]
            })
        })
        .collect();
    
    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_items = work_items.len();
    let start_time = std::time::Instant::now();
    
    // Process all work items in parallel
    let all_results: Vec<(f32, String, ApproachType, ProcessingResult)> = work_items
        .par_iter()
        .filter_map(|(interval, filename, approach)| {
            let gpx_data = Arc::clone(&gpx_data_arc);
            let processed_clone = Arc::clone(&processed);
            
            if let Some(file_data) = gpx_data.get(filename) {
                if file_data.official_gain > 0 {
                    let result = match approach {
                        ApproachType::DistBased => process_distbased(file_data, *interval),
                        ApproachType::DistBasedTwoStage => process_distbased_twostage(file_data, *interval),
                    };
                    
                    // Update progress
                    let count = processed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count % 1000 == 0 || count == total_items {
                        let elapsed = start_time.elapsed().as_secs_f64();
                        let rate = count as f64 / elapsed;
                        let remaining = (total_items - count) as f64 / rate;
                        println!("  Progress: {}/{} ({:.1}%) - {:.0} items/sec - ETA: {:.0}s",
                                 count, total_items, 
                                 (count as f64 / total_items as f64) * 100.0,
                                 rate, remaining);
                    }
                    
                    return Some((*interval, filename.clone(), *approach, result));
                }
            }
            None
        })
        .collect();
    
    println!("✅ Parallel processing complete, aggregating results...");
    
    // Aggregate results by interval
    let mut results = Vec::new();
    
    for interval in intervals {
        // Collect results for this interval
        let distbased_results: Vec<_> = all_results.iter()
            .filter(|(i, _, a, _)| *i == interval && *a == ApproachType::DistBased)
            .map(|(_, _, _, r)| r)
            .collect();
            
        let twostage_results: Vec<_> = all_results.iter()
            .filter(|(i, _, a, _)| *i == interval && *a == ApproachType::DistBasedTwoStage)
            .map(|(_, _, _, r)| r)
            .collect();
        
        if !distbased_results.is_empty() && !twostage_results.is_empty() {
            results.push(create_analysis_result(interval, &distbased_results, &twostage_results));
        }
    }
    
    Ok(results)
}

fn process_distbased(file_data: &GpxFileData, interval: f32) -> ProcessingResult {
    // Calculate raw gain/loss
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&file_data.elevations);
    
    // Process with distance-based approach
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval as f64);
    
    let processed_gain = elevation_data.get_total_elevation_gain() as f32;
    let processed_loss = elevation_data.get_total_elevation_loss() as f32;
    
    let accuracy = (processed_gain / file_data.official_gain as f32) * 100.0;
    
    // Calculate similarities
    let gain_loss_similarity = if processed_gain > 0.0 && processed_loss > 0.0 {
        100.0 - ((processed_gain - processed_loss).abs() / processed_gain.max(processed_loss)) * 100.0
    } else {
        0.0
    };
    
    let gain_official_similarity = if file_data.official_gain > 0 {
        100.0 - ((processed_gain - file_data.official_gain as f32).abs() / file_data.official_gain as f32) * 100.0
    } else {
        0.0
    };
    
    ProcessingResult {
        accuracy,
        raw_gain: raw_gain as f32,
        raw_loss: raw_loss as f32,
        processed_gain,
        processed_loss,
        gain_loss_similarity,
        gain_official_similarity,
    }
}

fn process_distbased_twostage(file_data: &GpxFileData, interval: f32) -> ProcessingResult {
    // Calculate raw gain/loss
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&file_data.elevations);
    
    // Apply two-stage outlier removal first
    let cleaned_data = apply_two_stage_outlier_removal(file_data);
    
    // Process with distance-based approach
    let mut elevation_data = ElevationData::new_with_variant(
        cleaned_data.elevations,
        cleaned_data.distances,
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval as f64);
    
    let processed_gain = elevation_data.get_total_elevation_gain() as f32;
    let processed_loss = elevation_data.get_total_elevation_loss() as f32;
    
    let accuracy = (processed_gain / file_data.official_gain as f32) * 100.0;
    
    // Calculate similarities
    let gain_loss_similarity = if processed_gain > 0.0 && processed_loss > 0.0 {
        100.0 - ((processed_gain - processed_loss).abs() / processed_gain.max(processed_loss)) * 100.0
    } else {
        0.0
    };
    
    let gain_official_similarity = if file_data.official_gain > 0 {
        100.0 - ((processed_gain - file_data.official_gain as f32).abs() / file_data.official_gain as f32) * 100.0
    } else {
        0.0
    };
    
    ProcessingResult {
        accuracy,
        raw_gain: raw_gain as f32,
        raw_loss: raw_loss as f32,
        processed_gain,
        processed_loss,
        gain_loss_similarity,
        gain_official_similarity,
    }
}

fn apply_two_stage_outlier_removal(file_data: &GpxFileData) -> GpxFileData {
    let mut cleaned_data = file_data.clone();
    
    // Stage 1: Gradient-based outlier removal (IQR method)
    let mut gradients = Vec::new();
    for i in 1..file_data.elevations.len() {
        let dist_diff = file_data.distances[i] - file_data.distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (file_data.elevations[i] - file_data.elevations[i-1]) / dist_diff * 100.0;
            gradients.push(gradient);
        }
    }
    
    if gradients.len() > 4 {
        let mut sorted_gradients = gradients.clone();
        sorted_gradients.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let q1 = sorted_gradients[sorted_gradients.len() / 4];
        let q3 = sorted_gradients[(sorted_gradients.len() * 3) / 4];
        let iqr = q3 - q1;
        
        let lower_bound = q1 - 2.0 * iqr;
        let upper_bound = q3 + 2.0 * iqr;
        
        // Apply gradient-based cleaning
        for i in 1..file_data.elevations.len() - 1 {
            if i <= gradients.len() {
                let gradient = gradients[i-1];
                if gradient < lower_bound || gradient > upper_bound {
                    cleaned_data.elevations[i] = (cleaned_data.elevations[i-1] + cleaned_data.elevations[i+1]) / 2.0;
                }
            }
        }
    }
    
    // Stage 2: Climb rate outlier removal
    let mut climb_rates = Vec::new();
    for i in 1..cleaned_data.elevations.len() {
        let elev_change = cleaned_data.elevations[i] - cleaned_data.elevations[i-1];
        let time_change = cleaned_data.timestamps[i] - cleaned_data.timestamps[i-1];
        
        if time_change > 0.0 && elev_change > 0.0 {
            let rate = (elev_change / time_change) * 3600.0; // m/hour
            climb_rates.push((i, rate));
        }
    }
    
    if climb_rates.len() > 4 {
        let mut sorted_rates: Vec<f64> = climb_rates.iter().map(|(_, r)| *r).collect();
        sorted_rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let q1 = sorted_rates[sorted_rates.len() / 4];
        let q3 = sorted_rates[(sorted_rates.len() * 3) / 4];
        let iqr = q3 - q1;
        let upper_bound = (q3 + 3.0 * iqr).min(1200.0); // Max 1200 m/hour
        
        for (idx, rate) in climb_rates {
            if rate > upper_bound {
                let time_change = cleaned_data.timestamps[idx] - cleaned_data.timestamps[idx-1];
                let max_change = (upper_bound / 3600.0) * time_change;
                cleaned_data.elevations[idx] = cleaned_data.elevations[idx-1] + max_change;
            }
        }
    }
    
    cleaned_data
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

fn create_analysis_result(
    interval: f32,
    distbased_results: &[&ProcessingResult],
    twostage_results: &[&ProcessingResult]
) -> AnalysisResult {
    // Calculate metrics for DistBased
    let db_accuracies: Vec<f32> = distbased_results.iter().map(|r| r.accuracy).collect();
    let db_metrics = calculate_accuracy_metrics(&db_accuracies);
    
    let db_total_files = distbased_results.len() as f32;
    let db_success_rate = (db_metrics.2 as f32 / db_total_files) * 100.0;
    
    let db_avg_raw_gain = distbased_results.iter().map(|r| r.raw_gain).sum::<f32>() / db_total_files;
    let db_avg_raw_loss = distbased_results.iter().map(|r| r.raw_loss).sum::<f32>() / db_total_files;
    let db_avg_proc_gain = distbased_results.iter().map(|r| r.processed_gain).sum::<f32>() / db_total_files;
    let db_avg_proc_loss = distbased_results.iter().map(|r| r.processed_loss).sum::<f32>() / db_total_files;
    let db_avg_gl_sim = distbased_results.iter().map(|r| r.gain_loss_similarity).sum::<f32>() / db_total_files;
    let db_avg_go_sim = distbased_results.iter().map(|r| r.gain_official_similarity).sum::<f32>() / db_total_files;
    
    // Calculate metrics for TwoStage
    let ts_accuracies: Vec<f32> = twostage_results.iter().map(|r| r.accuracy).collect();
    let ts_metrics = calculate_accuracy_metrics(&ts_accuracies);
    
    let ts_total_files = twostage_results.len() as f32;
    let ts_success_rate = (ts_metrics.2 as f32 / ts_total_files) * 100.0;
    
    let ts_avg_raw_gain = twostage_results.iter().map(|r| r.raw_gain).sum::<f32>() / ts_total_files;
    let ts_avg_raw_loss = twostage_results.iter().map(|r| r.raw_loss).sum::<f32>() / ts_total_files;
    let ts_avg_proc_gain = twostage_results.iter().map(|r| r.processed_gain).sum::<f32>() / ts_total_files;
    let ts_avg_proc_loss = twostage_results.iter().map(|r| r.processed_loss).sum::<f32>() / ts_total_files;
    let ts_avg_gl_sim = twostage_results.iter().map(|r| r.gain_loss_similarity).sum::<f32>() / ts_total_files;
    let ts_avg_go_sim = twostage_results.iter().map(|r| r.gain_official_similarity).sum::<f32>() / ts_total_files;
    
    AnalysisResult {
        interval_m: interval,
        // DistBased
        distbased_score_98_102: db_metrics.0,
        distbased_score_95_105: db_metrics.1,
        distbased_score_90_110: db_metrics.2,
        distbased_files_outside_80_120: db_metrics.3,
        distbased_weighted_score: db_metrics.4,
        distbased_median_accuracy: db_metrics.5,
        distbased_worst_accuracy: db_metrics.6,
        distbased_success_rate: db_success_rate,
        distbased_avg_raw_gain: db_avg_raw_gain,
        distbased_avg_raw_loss: db_avg_raw_loss,
        distbased_avg_processed_gain: db_avg_proc_gain,
        distbased_avg_processed_loss: db_avg_proc_loss,
        distbased_gain_loss_similarity: db_avg_gl_sim,
        distbased_gain_official_similarity: db_avg_go_sim,
        // TwoStage
        twostage_score_98_102: ts_metrics.0,
        twostage_score_95_105: ts_metrics.1,
        twostage_score_90_110: ts_metrics.2,
        twostage_files_outside_80_120: ts_metrics.3,
        twostage_weighted_score: ts_metrics.4,
        twostage_median_accuracy: ts_metrics.5,
        twostage_worst_accuracy: ts_metrics.6,
        twostage_success_rate: ts_success_rate,
        twostage_avg_raw_gain: ts_avg_raw_gain,
        twostage_avg_raw_loss: ts_avg_raw_loss,
        twostage_avg_processed_gain: ts_avg_proc_gain,
        twostage_avg_processed_loss: ts_avg_proc_loss,
        twostage_gain_loss_similarity: ts_avg_gl_sim,
        twostage_gain_official_similarity: ts_avg_go_sim,
        
        total_files: db_total_files as u32,
    }
}

fn calculate_accuracy_metrics(accuracies: &[f32]) -> (u32, u32, u32, u32, f32, f32, f32) {
    if accuracies.is_empty() {
        return (0, 0, 0, 0, 0.0, 0.0, 0.0);
    }
    
    let score_98_102 = accuracies.iter().filter(|&&acc| acc >= 98.0 && acc <= 102.0).count() as u32;
    let score_95_105 = accuracies.iter().filter(|&&acc| acc >= 95.0 && acc <= 105.0).count() as u32;
    let score_90_110 = accuracies.iter().filter(|&&acc| acc >= 90.0 && acc <= 110.0).count() as u32;
    let files_outside_80_120 = accuracies.iter().filter(|&&acc| acc < 80.0 || acc > 120.0).count() as u32;
    
    let weighted_score = (score_98_102 as f32 * 10.0) +
                        ((score_95_105 - score_98_102) as f32 * 6.0) +
                        ((score_90_110 - score_95_105) as f32 * 3.0) -
                        (files_outside_80_120 as f32 * 5.0);
    
    let mut sorted_accuracies = accuracies.to_vec();
    sorted_accuracies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let median_accuracy = if sorted_accuracies.len() % 2 == 0 {
        (sorted_accuracies[sorted_accuracies.len() / 2 - 1] + 
         sorted_accuracies[sorted_accuracies.len() / 2]) / 2.0
    } else {
        sorted_accuracies[sorted_accuracies.len() / 2]
    };
    
    let worst_accuracy = accuracies.iter()
        .max_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied()
        .unwrap_or(100.0);
    
    (score_98_102, score_95_105, score_90_110, files_outside_80_120, 
     weighted_score, median_accuracy, worst_accuracy)
}

fn write_results(results: &[AnalysisResult], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Interval (m)",
        // DistBased columns
        "DB Score", "DB 98-102%", "DB 95-105%", "DB 90-110%", "DB Outside 80-120%", 
        "DB Success%", "DB Median Acc%", "DB Worst Acc%",
        "DB Raw Gain", "DB Raw Loss", "DB Proc Gain", "DB Proc Loss",
        "DB Gain/Loss Sim%", "DB Gain/Official Sim%",
        // TwoStage columns
        "TS Score", "TS 98-102%", "TS 95-105%", "TS 90-110%", "TS Outside 80-120%",
        "TS Success%", "TS Median Acc%", "TS Worst Acc%",
        "TS Raw Gain", "TS Raw Loss", "TS Proc Gain", "TS Proc Loss",
        "TS Gain/Loss Sim%", "TS Gain/Official Sim%",
        // General
        "Total Files", "Score Difference"
    ])?;
    
    // Sort by DistBased score for easier analysis
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.distbased_weighted_score.partial_cmp(&a.distbased_weighted_score).unwrap());
    
    // Write data
    for result in sorted_results {
        wtr.write_record(&[
            format!("{:.1}", result.interval_m),
            // DistBased
            format!("{:.0}", result.distbased_weighted_score),
            result.distbased_score_98_102.to_string(),
            result.distbased_score_95_105.to_string(),
            result.distbased_score_90_110.to_string(),
            result.distbased_files_outside_80_120.to_string(),
            format!("{:.1}", result.distbased_success_rate),
            format!("{:.1}", result.distbased_median_accuracy),
            format!("{:.1}", result.distbased_worst_accuracy),
            format!("{:.0}", result.distbased_avg_raw_gain),
            format!("{:.0}", result.distbased_avg_raw_loss),
            format!("{:.0}", result.distbased_avg_processed_gain),
            format!("{:.0}", result.distbased_avg_processed_loss),
            format!("{:.1}", result.distbased_gain_loss_similarity),
            format!("{:.1}", result.distbased_gain_official_similarity),
            // TwoStage
            format!("{:.0}", result.twostage_weighted_score),
            result.twostage_score_98_102.to_string(),
            result.twostage_score_95_105.to_string(),
            result.twostage_score_90_110.to_string(),
            result.twostage_files_outside_80_120.to_string(),
            format!("{:.1}", result.twostage_success_rate),
            format!("{:.1}", result.twostage_median_accuracy),
            format!("{:.1}", result.twostage_worst_accuracy),
            format!("{:.0}", result.twostage_avg_raw_gain),
            format!("{:.0}", result.twostage_avg_raw_loss),
            format!("{:.0}", result.twostage_avg_processed_gain),
            format!("{:.0}", result.twostage_avg_processed_loss),
            format!("{:.1}", result.twostage_gain_loss_similarity),
            format!("{:.1}", result.twostage_gain_official_similarity),
            // General
            result.total_files.to_string(),
            format!("{:.0}", result.distbased_weighted_score - result.twostage_weighted_score),
        ])?;
    }
    
    wtr.flush()?;
    println!("\n✅ Results saved to: {}", output_path.display());
    Ok(())
}

fn print_summary(results: &[AnalysisResult]) {
    println!("\n📊 ANALYSIS SUMMARY: DistBased vs DistBased+TwoStage");
    println!("====================================================");
    
    // Find best intervals for each approach
    let best_distbased = results.iter()
        .max_by(|a, b| a.distbased_weighted_score.partial_cmp(&b.distbased_weighted_score).unwrap())
        .unwrap();
        
    let best_twostage = results.iter()
        .max_by(|a, b| a.twostage_weighted_score.partial_cmp(&b.twostage_weighted_score).unwrap())
        .unwrap();
    
    println!("\n🏆 BEST INTERVALS:");
    println!("\nDistBased (Pure):");
    println!("  Interval: {:.1}m", best_distbased.interval_m);
    println!("  Score: {:.0}", best_distbased.distbased_weighted_score);
    println!("  Success Rate: {:.1}% ({}/{})", best_distbased.distbased_success_rate, 
             best_distbased.distbased_score_90_110, best_distbased.total_files);
    println!("  Raw Gain/Loss: {:.0}m / {:.0}m", 
             best_distbased.distbased_avg_raw_gain, best_distbased.distbased_avg_raw_loss);
    println!("  Processed Gain/Loss: {:.0}m / {:.0}m", 
             best_distbased.distbased_avg_processed_gain, best_distbased.distbased_avg_processed_loss);
    println!("  Gain/Loss Similarity: {:.1}%", best_distbased.distbased_gain_loss_similarity);
    println!("  Gain/Official Similarity: {:.1}%", best_distbased.distbased_gain_official_similarity);
    
    println!("\nDistBased + TwoStage:");
    println!("  Interval: {:.1}m", best_twostage.interval_m);
    println!("  Score: {:.0}", best_twostage.twostage_weighted_score);
    println!("  Success Rate: {:.1}% ({}/{})", best_twostage.twostage_success_rate,
             best_twostage.twostage_score_90_110, best_twostage.total_files);
    println!("  Raw Gain/Loss: {:.0}m / {:.0}m", 
             best_twostage.twostage_avg_raw_gain, best_twostage.twostage_avg_raw_loss);
    println!("  Processed Gain/Loss: {:.0}m / {:.0}m", 
             best_twostage.twostage_avg_processed_gain, best_twostage.twostage_avg_processed_loss);
    println!("  Gain/Loss Similarity: {:.1}%", best_twostage.twostage_gain_loss_similarity);
    println!("  Gain/Official Similarity: {:.1}%", best_twostage.twostage_gain_official_similarity);
    
    // Analysis of gain/loss reduction
    println!("\n📈 ELEVATION PROCESSING IMPACT:");
    println!("\nDistBased at {:.1}m:", best_distbased.interval_m);
    let db_gain_reduction = ((best_distbased.distbased_avg_raw_gain - best_distbased.distbased_avg_processed_gain) 
                            / best_distbased.distbased_avg_raw_gain) * 100.0;
    let db_loss_reduction = ((best_distbased.distbased_avg_raw_loss - best_distbased.distbased_avg_processed_loss) 
                            / best_distbased.distbased_avg_raw_loss) * 100.0;
    println!("  Gain reduction: {:.1}%", db_gain_reduction);
    println!("  Loss reduction: {:.1}%", db_loss_reduction);
    
    println!("\nTwoStage at {:.1}m:", best_twostage.interval_m);
    let ts_gain_reduction = ((best_twostage.twostage_avg_raw_gain - best_twostage.twostage_avg_processed_gain) 
                            / best_twostage.twostage_avg_raw_gain) * 100.0;
    let ts_loss_reduction = ((best_twostage.twostage_avg_raw_loss - best_twostage.twostage_avg_processed_loss) 
                            / best_twostage.twostage_avg_raw_loss) * 100.0;
    println!("  Gain reduction: {:.1}%", ts_gain_reduction);
    println!("  Loss reduction: {:.1}%", ts_loss_reduction);
    
    // Winner
    println!("\n🎯 RECOMMENDATION:");
    if best_distbased.distbased_weighted_score > best_twostage.twostage_weighted_score {
        println!("✅ Use pure DistBased approach at {:.1}m intervals", best_distbased.interval_m);
        println!("   Score advantage: +{:.0} points", 
                 best_distbased.distbased_weighted_score - best_twostage.twostage_weighted_score);
    } else {
        println!("✅ Use DistBased + TwoStage approach at {:.1}m intervals", best_twostage.interval_m);
        println!("   Score advantage: +{:.0} points", 
                 best_twostage.twostage_weighted_score - best_distbased.distbased_weighted_score);
    }
    
    // Show gain/loss similarity insights
    println!("\n💡 GAIN/LOSS SIMILARITY INSIGHTS:");
    let high_similarity_count = results.iter()
        .filter(|r| r.distbased_gain_loss_similarity > 90.0)
        .count();
    println!("  Intervals with >90% gain/loss similarity: {} of {}", high_similarity_count, results.len());
    println!("  Average gain/loss similarity: {:.1}%", 
             results.iter().map(|r| r.distbased_gain_loss_similarity).sum::<f32>() / results.len() as f32);
    println!("  Note: Low similarity is normal - most routes don't have equal gain/loss");
}