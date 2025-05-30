use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;
use crate::custom_smoother::{ElevationData, SmoothingVariant};

#[derive(Debug, Serialize, Clone)]
pub struct AnalysisResult {
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
    files_balanced_85_115: u32,  // Files where loss is 85-115% of gain
    files_balanced_70_130: u32,  // Files where loss is 70-130% of gain
    avg_gain_loss_ratio: f32,    // Average loss/gain ratio across files
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
    combined_score: f32,  // Combines accuracy and gain/loss balance
    loss_preservation_score: f32,  // How well loss is preserved relative to gain
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
    gain_loss_ratio: f32,  // processed_loss / processed_gain
    loss_preservation: f32,  // How much of original loss is preserved vs gain
}

pub fn run_simplified_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🔬 GAIN/LOSS BALANCE ANALYSIS");
    println!("=============================");
    println!("Testing intervals: 0.10m to 7.00m in 0.025m increments (276 intervals)");
    println!("Focus: Finding optimal balance between gain accuracy and loss preservation\n");
    
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
    let output_path = Path::new(gpx_folder).join("gain_loss_balance_analysis_0.1_to_7m.csv");
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
                    let result = process_single_file(file_data, *interval);
                    
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
    
    // Calculate gain/loss ratio
    let gain_loss_ratio = if processed_gain > 0.0 {
        (processed_loss / processed_gain) * 100.0
    } else {
        0.0
    };
    
    // Calculate loss preservation relative to gain preservation
    let gain_preservation = if raw_gain as f32 > 0.0 {
        processed_gain / raw_gain as f32
    } else {
        1.0
    };
    
    let loss_preservation = if raw_loss as f32 > 0.0 {
        processed_loss / raw_loss as f32
    } else {
        1.0
    };
    
    let relative_loss_preservation = if gain_preservation > 0.0 {
        (loss_preservation / gain_preservation) * 100.0
    } else {
        100.0
    };
    
    ProcessingResult {
        accuracy,
        raw_gain: raw_gain as f32,
        raw_loss: raw_loss as f32,
        processed_gain,
        processed_loss,
        gain_loss_ratio,
        loss_preservation: relative_loss_preservation,
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
    
    // Traditional accuracy metrics
    let weighted_accuracy_score = (score_98_102 as f32 * 10.0) +
                                 ((score_95_105 - score_98_102) as f32 * 6.0) +
                                 ((score_90_110 - score_95_105) as f32 * 3.0) +
                                 ((score_85_115 - score_90_110) as f32 * 1.5) +
                                 ((score_80_120 - score_85_115) as f32 * 1.0) -
                                 (files_outside_80_120 as f32 * 5.0);
    
    // Gain/loss balance score (higher when more files have balanced gain/loss)
    let total_files = results.len() as f32;
    let gain_loss_balance_score = (files_balanced_85_115 as f32 * 10.0) +
                                  ((files_balanced_70_130 - files_balanced_85_115) as f32 * 5.0) +
                                  ((median_gain_loss_ratio - 100.0).abs() * -2.0);
    
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
    
    let success_rate = (score_90_110 as f32 / total_files) * 100.0;
    
    // Calculate averages for gain/loss metrics
    let avg_raw_gain = results.iter().map(|r| r.raw_gain).sum::<f32>() / total_files;
    let avg_raw_loss = results.iter().map(|r| r.raw_loss).sum::<f32>() / total_files;
    let avg_processed_gain = results.iter().map(|r| r.processed_gain).sum::<f32>() / total_files;
    let avg_processed_loss = results.iter().map(|r| r.processed_loss).sum::<f32>() / total_files;
    
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
    
    // Loss preservation score (higher when loss reduction is similar to gain reduction)
    let loss_preservation_score = 100.0 - (loss_reduction_percent - gain_reduction_percent).abs();
    
    // Combined score that balances accuracy and gain/loss preservation
    let combined_score = (weighted_accuracy_score * 0.5) + 
                        (gain_loss_balance_score * 0.3) +
                        (loss_preservation_score * 0.2);
    
    AnalysisResult {
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
        total_files: total_files as u32,
    }
}

fn write_results(results: &[AnalysisResult], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
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
        "Total Files",
        "Files Outside 80-120%",
    ])?;
    
    // Sort by combined score for easier analysis
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
            result.total_files.to_string(),
            result.files_outside_80_120.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    println!("\n✅ Results saved to: {}", output_path.display());
    Ok(())
}

fn print_summary(results: &[AnalysisResult]) {
    println!("\n📊 GAIN/LOSS BALANCE ANALYSIS SUMMARY");
    println!("=====================================");
    
    // Find best by different criteria
    let best_combined = results.iter()
        .max_by(|a, b| a.combined_score.partial_cmp(&b.combined_score).unwrap())
        .unwrap();
    
    let best_accuracy = results.iter()
        .max_by(|a, b| a.weighted_accuracy_score.partial_cmp(&b.weighted_accuracy_score).unwrap())
        .unwrap();
    
    let best_balance = results.iter()
        .max_by(|a, b| a.gain_loss_balance_score.partial_cmp(&b.gain_loss_balance_score).unwrap())
        .unwrap();
    
    let best_preservation = results.iter()
        .max_by(|a, b| a.loss_preservation_score.partial_cmp(&b.loss_preservation_score).unwrap())
        .unwrap();
    
    println!("\n🏆 BEST INTERVALS BY CRITERIA:");
    
    println!("\n1️⃣ BEST OVERALL (Combined Score):");
    println!("   Interval: {:.3}m", best_combined.interval_m);
    println!("   Combined Score: {:.2}", best_combined.combined_score);
    println!("   Success Rate: {:.1}% ({}/{} within ±10%)", 
             best_combined.success_rate, best_combined.score_90_110, best_combined.total_files);
    println!("   Median Gain/Loss Ratio: {:.1}%", best_combined.median_gain_loss_ratio);
    println!("   Files with balanced gain/loss (85-115%): {} ({:.1}%)", 
             best_combined.files_balanced_85_115,
             (best_combined.files_balanced_85_115 as f32 / best_combined.total_files as f32) * 100.0);
    println!("   Gain reduction: {:.1}%, Loss reduction: {:.1}%",
             best_combined.gain_reduction_percent, best_combined.loss_reduction_percent);
    
    println!("\n2️⃣ BEST ACCURACY (Traditional scoring):");
    println!("   Interval: {:.3}m", best_accuracy.interval_m);
    println!("   Accuracy Score: {:.2}", best_accuracy.weighted_accuracy_score);
    println!("   Median accuracy: {:.2}%", best_accuracy.median_accuracy);
    println!("   BUT: Gain/Loss ratio: {:.1}%, Loss reduction: {:.1}%",
             best_accuracy.median_gain_loss_ratio, best_accuracy.loss_reduction_percent);
    
    println!("\n3️⃣ BEST GAIN/LOSS BALANCE:");
    println!("   Interval: {:.3}m", best_balance.interval_m);
    println!("   Balance Score: {:.2}", best_balance.gain_loss_balance_score);
    println!("   Median Gain/Loss Ratio: {:.1}%", best_balance.median_gain_loss_ratio);
    println!("   Files balanced (85-115%): {} ({:.1}%)", 
             best_balance.files_balanced_85_115,
             (best_balance.files_balanced_85_115 as f32 / best_balance.total_files as f32) * 100.0);
    
    println!("\n4️⃣ BEST LOSS PRESERVATION:");
    println!("   Interval: {:.3}m", best_preservation.interval_m);
    println!("   Preservation Score: {:.2}", best_preservation.loss_preservation_score);
    println!("   Gain reduction: {:.1}%, Loss reduction: {:.1}%",
             best_preservation.gain_reduction_percent, best_preservation.loss_reduction_percent);
    
    // Show top 5 by combined score
    let mut sorted_by_combined = results.to_vec();
    sorted_by_combined.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    
    println!("\n🏅 TOP 5 INTERVALS (Combined Score):");
    println!("Rank | Interval | Combined | Accuracy | Balance | Median Ratio | Balanced Files | Gain Red% | Loss Red%");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for (i, result) in sorted_by_combined.iter().take(5).enumerate() {
        println!("{:4} | {:7.3}m | {:8.2} | {:8.2} | {:7.2} | {:11.1}% | {:14} | {:9.1} | {:9.1}",
                 i + 1,
                 result.interval_m,
                 result.combined_score,
                 result.weighted_accuracy_score,
                 result.gain_loss_balance_score,
                 result.median_gain_loss_ratio,
                 result.files_balanced_85_115,
                 result.gain_reduction_percent,
                 result.loss_reduction_percent);
    }
    
    println!("\n💡 KEY INSIGHTS:");
    
    // Analyze the trade-off
    let small_intervals: Vec<&AnalysisResult> = results.iter()
        .filter(|r| r.interval_m <= 2.0)
        .collect();
    let large_intervals: Vec<&AnalysisResult> = results.iter()
        .filter(|r| r.interval_m >= 5.0)
        .collect();
    
    if !small_intervals.is_empty() && !large_intervals.is_empty() {
        let avg_small_ratio = small_intervals.iter()
            .map(|r| r.median_gain_loss_ratio)
            .sum::<f32>() / small_intervals.len() as f32;
        let avg_large_ratio = large_intervals.iter()
            .map(|r| r.median_gain_loss_ratio)
            .sum::<f32>() / large_intervals.len() as f32;
        
        println!("• Small intervals (<2m): Better gain/loss balance (avg ratio: {:.1}%)", avg_small_ratio);
        println!("• Large intervals (>5m): Better accuracy but poor loss preservation (avg ratio: {:.1}%)", avg_large_ratio);
    }
    
    println!("\n🎯 RECOMMENDATION:");
    println!("Use {:.3}m intervals for the best balance between:", best_combined.interval_m);
    println!("  • Elevation gain accuracy ({:.1}% median accuracy)", best_combined.median_accuracy);
    println!("  • Natural gain/loss preservation ({:.1}% median ratio)", best_combined.median_gain_loss_ratio);
    println!("  • Reasonable reductions (Gain: {:.1}%, Loss: {:.1}%)",
             best_combined.gain_reduction_percent, best_combined.loss_reduction_percent);
}