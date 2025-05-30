use std::path::{Path, PathBuf};
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;
use crate::custom_smoother::{ElevationData, SmoothingVariant};

#[derive(Debug, Serialize, Clone)]
pub struct AnalysisResult {
    interval_m: f32,
    score_98_102: u32,
    score_95_105: u32,
    score_90_110: u32,
    score_85_115: u32,
    score_80_120: u32,
    files_outside_80_120: u32,
    weighted_score: f32,
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
    gain_loss_similarity: f32,
    gain_official_similarity: f32,
    // Raw data's total elevation loss (before any processing)
    total_raw_elevation_loss: f32,
    // Loss reduction percentage
    loss_reduction_percent: f32,
    gain_reduction_percent: f32,
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
    gain_loss_similarity: f32,
    gain_official_similarity: f32,
}

pub fn run_simplified_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🔬 FOCUSED DISTANCE-BASED ANALYSIS");
    println!("==================================");
    println!("Testing intervals: 5.90m to 7.00m in 0.01m increments (111 intervals)");
    println!("Approach: Pure distance-based smoothing only\n");
    
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
    
    // Process distance-based approach
    let processing_start = std::time::Instant::now();
    let results = process_distbased_range(&gpx_files_data, &files_with_elevation)?;
    println!("✅ Processing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write results
    let output_path = Path::new(gpx_folder).join("distbased_focused_5.9_to_7m.csv");
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

fn process_distbased_range(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<AnalysisResult>, Box<dyn std::error::Error>> {
    // Test intervals from 5.90m to 7.00m in 0.01m increments
    let intervals: Vec<f32> = (590..=700).map(|i| i as f32 * 0.01).collect();
    
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
                    let result = process_single_file(file_data, *interval);
                    
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
            results.push(create_analysis_result(interval, &interval_results));
        }
    }
    
    Ok(results)
}

fn process_single_file(file_data: &GpxFileData, interval: f32) -> ProcessingResult {
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
    results: &[&ProcessingResult]
) -> AnalysisResult {
    let accuracies: Vec<f32> = results.iter().map(|r| r.accuracy).collect();
    
    // Calculate accuracy bands
    let score_98_102 = accuracies.iter().filter(|&&acc| acc >= 98.0 && acc <= 102.0).count() as u32;
    let score_95_105 = accuracies.iter().filter(|&&acc| acc >= 95.0 && acc <= 105.0).count() as u32;
    let score_90_110 = accuracies.iter().filter(|&&acc| acc >= 90.0 && acc <= 110.0).count() as u32;
    let score_85_115 = accuracies.iter().filter(|&&acc| acc >= 85.0 && acc <= 115.0).count() as u32;
    let score_80_120 = accuracies.iter().filter(|&&acc| acc >= 80.0 && acc <= 120.0).count() as u32;
    let files_outside_80_120 = accuracies.iter().filter(|&&acc| acc < 80.0 || acc > 120.0).count() as u32;
    
    let weighted_score = (score_98_102 as f32 * 10.0) +
                        ((score_95_105 - score_98_102) as f32 * 6.0) +
                        ((score_90_110 - score_95_105) as f32 * 3.0) +
                        ((score_85_115 - score_90_110) as f32 * 1.5) +
                        ((score_80_120 - score_85_115) as f32 * 1.0) -
                        (files_outside_80_120 as f32 * 5.0);
    
    // Calculate statistics
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
    
    let total_files = results.len() as f32;
    let success_rate = (score_90_110 as f32 / total_files) * 100.0;
    
    // Calculate averages for gain/loss metrics
    let avg_raw_gain = results.iter().map(|r| r.raw_gain).sum::<f32>() / total_files;
    let avg_raw_loss = results.iter().map(|r| r.raw_loss).sum::<f32>() / total_files;
    let avg_processed_gain = results.iter().map(|r| r.processed_gain).sum::<f32>() / total_files;
    let avg_processed_loss = results.iter().map(|r| r.processed_loss).sum::<f32>() / total_files;
    let gain_loss_similarity = results.iter().map(|r| r.gain_loss_similarity).sum::<f32>() / total_files;
    let gain_official_similarity = results.iter().map(|r| r.gain_official_similarity).sum::<f32>() / total_files;
    
    // Calculate total raw elevation loss
    let total_raw_elevation_loss = results.iter().map(|r| r.raw_loss).sum::<f32>();
    
    // Calculate reduction percentages
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
    
    AnalysisResult {
        interval_m: interval,
        score_98_102,
        score_95_105,
        score_90_110,
        score_85_115,
        score_80_120,
        files_outside_80_120,
        weighted_score,
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
        gain_loss_similarity,
        gain_official_similarity,
        total_raw_elevation_loss,
        loss_reduction_percent,
        gain_reduction_percent,
        total_files: total_files as u32,
    }
}

fn write_results(results: &[AnalysisResult], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Interval (m)",
        "Weighted Score",
        "98-102%",
        "95-105%",
        "90-110%",
        "85-115%",
        "80-120%",
        "Outside 80-120%",
        "Success Rate %",
        "Average Accuracy %",
        "Median Accuracy %",
        "Best Accuracy %",
        "Worst Accuracy %",
        "Std Deviation",
        "Raw Gain (avg)",
        "Raw Loss (avg)",
        "Total Raw Loss",
        "Processed Gain (avg)",
        "Processed Loss (avg)",
        "Gain Reduction %",
        "Loss Reduction %",
        "Gain/Loss Similarity %",
        "Gain/Official Similarity %",
        "Total Files",
        "% in 98-102%",
        "% in 95-105%",
        "% in 90-110%",
    ])?;
    
    // Sort by weighted score for easier analysis
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.weighted_score.partial_cmp(&a.weighted_score).unwrap());
    
    // Write data
    for result in sorted_results {
        let total = result.total_files as f32;
        wtr.write_record(&[
            format!("{:.2}", result.interval_m),
            format!("{:.2}", result.weighted_score),
            result.score_98_102.to_string(),
            result.score_95_105.to_string(),
            result.score_90_110.to_string(),
            result.score_85_115.to_string(),
            result.score_80_120.to_string(),
            result.files_outside_80_120.to_string(),
            format!("{:.1}", result.success_rate),
            format!("{:.2}", result.average_accuracy),
            format!("{:.2}", result.median_accuracy),
            format!("{:.2}", result.best_accuracy),
            format!("{:.2}", result.worst_accuracy),
            format!("{:.2}", result.std_deviation),
            format!("{:.1}", result.avg_raw_gain),
            format!("{:.1}", result.avg_raw_loss),
            format!("{:.1}", result.total_raw_elevation_loss),
            format!("{:.1}", result.avg_processed_gain),
            format!("{:.1}", result.avg_processed_loss),
            format!("{:.1}", result.gain_reduction_percent),
            format!("{:.1}", result.loss_reduction_percent),
            format!("{:.1}", result.gain_loss_similarity),
            format!("{:.1}", result.gain_official_similarity),
            result.total_files.to_string(),
            format!("{:.1}", (result.score_98_102 as f32 / total) * 100.0),
            format!("{:.1}", (result.score_95_105 as f32 / total) * 100.0),
            format!("{:.1}", (result.score_90_110 as f32 / total) * 100.0),
        ])?;
    }
    
    wtr.flush()?;
    println!("\n✅ Results saved to: {}", output_path.display());
    Ok(())
}

fn print_summary(results: &[AnalysisResult]) {
    println!("\n📊 FOCUSED DISTANCE-BASED ANALYSIS SUMMARY");
    println!("==========================================");
    
    // Find best interval
    let best = results.iter()
        .max_by(|a, b| a.weighted_score.partial_cmp(&b.weighted_score).unwrap())
        .unwrap();
    
    println!("\n🏆 BEST INTERVAL:");
    println!("  Interval: {:.2}m", best.interval_m);
    println!("  Weighted Score: {:.2}", best.weighted_score);
    println!("  Success Rate: {:.1}% ({}/{} files within ±10%)", 
             best.success_rate, best.score_90_110, best.total_files);
    
    println!("\n📈 ACCURACY DISTRIBUTION:");
    println!("  98-102% accuracy: {} files ({:.1}%)", 
             best.score_98_102, (best.score_98_102 as f32 / best.total_files as f32) * 100.0);
    println!("  95-105% accuracy: {} files ({:.1}%)", 
             best.score_95_105, (best.score_95_105 as f32 / best.total_files as f32) * 100.0);
    println!("  90-110% accuracy: {} files ({:.1}%)", 
             best.score_90_110, (best.score_90_110 as f32 / best.total_files as f32) * 100.0);
    println!("  Outside 80-120%: {} files ({:.1}%)", 
             best.files_outside_80_120, (best.files_outside_80_120 as f32 / best.total_files as f32) * 100.0);
    
    println!("\n📊 STATISTICS:");
    println!("  Average accuracy: {:.2}%", best.average_accuracy);
    println!("  Median accuracy: {:.2}%", best.median_accuracy);
    println!("  Best accuracy: {:.2}%", best.best_accuracy);
    println!("  Worst accuracy: {:.2}%", best.worst_accuracy);
    println!("  Standard deviation: {:.2}", best.std_deviation);
    
    println!("\n⛰️ ELEVATION PROCESSING:");
    println!("  Average raw gain: {:.1}m", best.avg_raw_gain);
    println!("  Average processed gain: {:.1}m", best.avg_processed_gain);
    println!("  Gain reduction: {:.1}%", best.gain_reduction_percent);
    println!("  Average raw loss: {:.1}m", best.avg_raw_loss);
    println!("  Average processed loss: {:.1}m", best.avg_processed_loss);
    println!("  Loss reduction: {:.1}%", best.loss_reduction_percent);
    println!("  Total raw elevation loss: {:.1}m", best.total_raw_elevation_loss);
    
    println!("\n🔍 SIMILARITY METRICS:");
    println!("  Gain/Loss similarity: {:.1}%", best.gain_loss_similarity);
    println!("  Gain/Official similarity: {:.1}%", best.gain_official_similarity);
    
    // Find top 5 intervals
    let mut sorted_by_score = results.to_vec();
    sorted_by_score.sort_by(|a, b| b.weighted_score.partial_cmp(&a.weighted_score).unwrap());
    
    println!("\n🏅 TOP 5 INTERVALS:");
    println!("Rank | Interval | Score  | 98-102% | 95-105% | 90-110% | Median % | Success %");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for (i, result) in sorted_by_score.iter().take(5).enumerate() {
        println!("{:4} | {:7.2}m | {:6.2} | {:7} | {:7} | {:7} | {:7.2} | {:8.1}",
                 i + 1,
                 result.interval_m,
                 result.weighted_score,
                 result.score_98_102,
                 result.score_95_105,
                 result.score_90_110,
                 result.median_accuracy,
                 result.success_rate);
    }
    
    // Analyze the range
    let min_score = results.iter().map(|r| r.weighted_score).fold(f32::INFINITY, f32::min);
    let max_score = results.iter().map(|r| r.weighted_score).fold(f32::NEG_INFINITY, f32::max);
    
    println!("\n📉 SCORE RANGE ANALYSIS:");
    println!("  Minimum score: {:.2} at {:.2}m", 
             min_score, 
             results.iter().find(|r| r.weighted_score == min_score).unwrap().interval_m);
    println!("  Maximum score: {:.2} at {:.2}m", 
             max_score, 
             results.iter().find(|r| r.weighted_score == max_score).unwrap().interval_m);
    println!("  Score variation: {:.2} points", max_score - min_score);
    
    // Find intervals with best specific metrics
    let best_tight = results.iter().max_by_key(|r| r.score_98_102).unwrap();
    let best_median = results.iter()
        .min_by_key(|r| ((r.median_accuracy - 100.0).abs() * 100.0) as i32)
        .unwrap();
    let best_consistency = results.iter()
        .min_by(|a, b| a.std_deviation.partial_cmp(&b.std_deviation).unwrap())
        .unwrap();
    
    println!("\n💡 SPECIALIZED BESTS:");
    println!("  Most files in 98-102%: {:.2}m ({} files, {:.1}%)", 
             best_tight.interval_m, 
             best_tight.score_98_102,
             (best_tight.score_98_102 as f32 / best_tight.total_files as f32) * 100.0);
    println!("  Best median accuracy: {:.2}m ({:.2}%)", 
             best_median.interval_m, 
             best_median.median_accuracy);
    println!("  Most consistent (lowest σ): {:.2}m (σ = {:.2})", 
             best_consistency.interval_m, 
             best_consistency.std_deviation);
    
    println!("\n🎯 RECOMMENDATION:");
    println!("Use {:.2}m distance intervals for optimal elevation gain accuracy.", best.interval_m);
    println!("This achieves {:.1}% success rate with {:.2}% median accuracy across {} files.",
             best.success_rate, best.median_accuracy, best.total_files);
}