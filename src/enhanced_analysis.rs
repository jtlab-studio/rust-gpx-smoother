use std::path::{Path, PathBuf};
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;
use crate::custom_smoother::{ElevationData, SmoothingVariant};

#[derive(Debug, Clone)]
pub struct GpsQualityMetrics {
    pub average_point_spacing_m: f64,
    pub elevation_noise_ratio: f64,
    pub sampling_frequency_hz: f64,
    pub elevation_change_consistency: f64,
    pub signal_gaps_count: u32,
    pub quality_score: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ComparativeAnalysisResult {
    interval_m: f32,
    // Current approach
    current_score_98_102: u32,
    current_score_95_105: u32,
    current_score_90_110: u32,
    current_files_outside_80_120: u32,
    current_weighted_score: f32,
    current_median_accuracy: f32,
    current_worst_accuracy: f32,
    current_total_files: u32,
    
    // Gap Detection only
    gap_detection_score_98_102: u32,
    gap_detection_score_95_105: u32,
    gap_detection_score_90_110: u32,
    gap_detection_files_outside_80_120: u32,
    gap_detection_weighted_score: f32,
    gap_detection_median_accuracy: f32,
    gap_detection_worst_accuracy: f32,
    
    // Two-Stage Outlier only
    two_stage_score_98_102: u32,
    two_stage_score_95_105: u32,
    two_stage_score_90_110: u32,
    two_stage_files_outside_80_120: u32,
    two_stage_weighted_score: f32,
    two_stage_median_accuracy: f32,
    two_stage_worst_accuracy: f32,
    
    // Segment QA only
    segment_qa_score_98_102: u32,
    segment_qa_score_95_105: u32,
    segment_qa_score_90_110: u32,
    segment_qa_files_outside_80_120: u32,
    segment_qa_weighted_score: f32,
    segment_qa_median_accuracy: f32,
    segment_qa_worst_accuracy: f32,
    
    // Hybrid Interval only
    hybrid_score_98_102: u32,
    hybrid_score_95_105: u32,
    hybrid_score_90_110: u32,
    hybrid_files_outside_80_120: u32,
    hybrid_weighted_score: f32,
    hybrid_median_accuracy: f32,
    hybrid_worst_accuracy: f32,
    
    // Gap + Two-Stage
    gap_two_stage_score_98_102: u32,
    gap_two_stage_score_95_105: u32,
    gap_two_stage_score_90_110: u32,
    gap_two_stage_files_outside_80_120: u32,
    gap_two_stage_weighted_score: f32,
    gap_two_stage_median_accuracy: f32,
    gap_two_stage_worst_accuracy: f32,
    
    // All Combined
    all_combined_score_98_102: u32,
    all_combined_score_95_105: u32,
    all_combined_score_90_110: u32,
    all_combined_files_outside_80_120: u32,
    all_combined_weighted_score: f32,
    all_combined_median_accuracy: f32,
    all_combined_worst_accuracy: f32,
}

pub fn run_enhanced_comparative_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🔬 COMPREHENSIVE ENHANCED COMPARATIVE ANALYSIS");
    println!("=============================================");
    println!("Testing 7 different approaches:");
    println!("1. Current (Statistical Outlier Removal)");
    println!("2. Gap Detection & Filling");
    println!("3. Two-Stage Outlier Removal");
    println!("4. Segment-Based Quality Analysis");
    println!("5. Hybrid Interval Processing");
    println!("6. Gap Detection + Two-Stage Outlier");
    println!("7. All Four Enhancements Combined");
    println!("\nInterval range: 0.5m to 4.0m (0.1m increments)");
    println!("Total intervals: 36");
    
    let input_csv = Path::new(gpx_folder).join("fine_grained_analysis_0.05_to_8m.csv");
    
    if !input_csv.exists() {
        eprintln!("Error: Fine-grained analysis CSV not found. Run the main analysis first.");
        return Ok(());
    }
    
    // Load GPX files and their data
    println!("\n📂 Loading GPX files...");
    let start = std::time::Instant::now();
    let (gpx_files_data, valid_files) = load_gpx_data(gpx_folder)?;
    println!("✅ Loaded {} files in {:.2}s", valid_files.len(), start.elapsed().as_secs_f64());
    
    // Filter out files with 0% elevation data
    let files_with_elevation: Vec<_> = valid_files.into_iter()
        .filter(|file| {
            if let Some(data) = gpx_files_data.get(file) {
                let has_elevation = data.elevations.iter()
                    .any(|&e| (e - data.elevations[0]).abs() > 0.1);
                if !has_elevation {
                    println!("⚠️  Excluding {} - no elevation variation detected", file);
                }
                has_elevation
            } else {
                false
            }
        })
        .collect();
    
    println!("📊 Processing {} files with valid elevation data", files_with_elevation.len());
    
    // Process with all approaches
    let processing_start = std::time::Instant::now();
    let results = process_all_approaches_comprehensive(&gpx_files_data, &files_with_elevation)?;
    println!("✅ Processing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write results
    write_comprehensive_results(&results, Path::new(gpx_folder).join("comprehensive_enhancement_analysis.csv"))?;
    
    // Print summary
    print_comprehensive_summary(&results);
    
    let total_time = total_start.elapsed();
    println!("\n⏱️  TOTAL EXECUTION TIME: {} minutes {:.1} seconds", 
             total_time.as_secs() / 60, 
             total_time.as_secs_f64() % 60.0);
    
    Ok(())
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
struct SegmentQuality {
    start_idx: usize,
    end_idx: usize,
    quality_score: f64,
    has_gaps: bool,
    noise_level: f64,
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

fn process_all_approaches_comprehensive(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<ComparativeAnalysisResult>, Box<dyn std::error::Error>> {
    // Test intervals from 0.5m to 4.0m in 0.1m increments
    let intervals: Vec<f32> = (5..=40).map(|i| i as f32 * 0.1).collect();
    
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("\n🚀 Processing {} intervals × {} files × 7 approaches = {} total calculations",
             intervals.len(), valid_files.len(), intervals.len() * valid_files.len() * 7);
    println!("⚡ Using parallel processing on {} cores", num_cpus::get());
    
    let work_items: Vec<(f32, String)> = intervals.iter()
        .flat_map(|&interval| {
            valid_files.iter().map(move |file| (interval, file.clone()))
        })
        .collect();
    
    println!("📊 Creating {} work items for parallel processing...", work_items.len());
    
    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_items = work_items.len();
    let start_time = std::time::Instant::now();
    
    // Process all work items in parallel
    let all_results: Vec<(f32, String, Vec<f32>)> = work_items
        .par_iter()
        .filter_map(|(interval, filename)| {
            let gpx_data = Arc::clone(&gpx_data_arc);
            let processed_clone = Arc::clone(&processed);
            
            if let Some(file_data) = gpx_data.get(filename) {
                if file_data.official_gain > 0 {
                    let mut accuracies = Vec::new();
                    
                    // 1. Current approach (Statistical Outlier Removal)
                    let current_gain = calculate_current_approach(file_data, *interval);
                    accuracies.push((current_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 2. Gap Detection only
                    let gap_detection_gain = calculate_gap_detection_only(file_data, *interval);
                    accuracies.push((gap_detection_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 3. Two-Stage Outlier only
                    let two_stage_gain = calculate_two_stage_outlier_only(file_data, *interval);
                    accuracies.push((two_stage_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 4. Segment QA only
                    let segment_qa_gain = calculate_segment_qa_only(file_data, *interval);
                    accuracies.push((segment_qa_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 5. Hybrid Interval only
                    let hybrid_gain = calculate_hybrid_interval_only(file_data, *interval);
                    accuracies.push((hybrid_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 6. Gap + Two-Stage
                    let gap_two_stage_gain = calculate_gap_plus_two_stage(file_data, *interval);
                    accuracies.push((gap_two_stage_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 7. All Combined
                    let all_combined_gain = calculate_all_combined(file_data, *interval);
                    accuracies.push((all_combined_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // Update progress
                    let count = processed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count % 500 == 0 {
                        let elapsed = start_time.elapsed().as_secs_f64();
                        let rate = count as f64 / elapsed;
                        let remaining = (total_items - count) as f64 / rate;
                        println!("  Progress: {}/{} ({:.1}%) - {:.0} items/sec - ETA: {:.0}s",
                                 count, total_items, 
                                 (count as f64 / total_items as f64) * 100.0,
                                 rate, remaining);
                    }
                    
                    return Some((*interval, filename.clone(), accuracies));
                }
            }
            None
        })
        .collect();
    
    println!("✅ Parallel processing complete, aggregating results...");
    
    // Group results by interval
    let mut interval_results: HashMap<i32, Vec<Vec<f32>>> = HashMap::new();
    
    for (interval, _filename, accuracies) in all_results {
        let key = (interval * 10.0) as i32;
        interval_results.entry(key)
            .or_insert_with(Vec::new)
            .push(accuracies);
    }
    
    // Convert to final results
    let results: Vec<ComparativeAnalysisResult> = intervals
        .iter()
        .map(|&interval| {
            let key = (interval * 10.0) as i32;
            let all_accuracies = interval_results.get(&key).cloned().unwrap_or_default();
            
            let approach_accuracies: Vec<Vec<f32>> = (0..7).map(|i| {
                all_accuracies.iter().map(|acc| acc.get(i).copied().unwrap_or(100.0)).collect()
            }).collect();
            
            let metrics: Vec<_> = approach_accuracies.iter()
                .map(|accs| calculate_accuracy_metrics(accs))
                .collect();
            
            ComparativeAnalysisResult {
                interval_m: interval,
                // Current
                current_score_98_102: metrics[0].0,
                current_score_95_105: metrics[0].1,
                current_score_90_110: metrics[0].2,
                current_files_outside_80_120: metrics[0].3,
                current_weighted_score: metrics[0].4,
                current_median_accuracy: metrics[0].5,
                current_worst_accuracy: metrics[0].6,
                current_total_files: approach_accuracies[0].len() as u32,
                // Gap Detection
                gap_detection_score_98_102: metrics[1].0,
                gap_detection_score_95_105: metrics[1].1,
                gap_detection_score_90_110: metrics[1].2,
                gap_detection_files_outside_80_120: metrics[1].3,
                gap_detection_weighted_score: metrics[1].4,
                gap_detection_median_accuracy: metrics[1].5,
                gap_detection_worst_accuracy: metrics[1].6,
                // Two-Stage
                two_stage_score_98_102: metrics[2].0,
                two_stage_score_95_105: metrics[2].1,
                two_stage_score_90_110: metrics[2].2,
                two_stage_files_outside_80_120: metrics[2].3,
                two_stage_weighted_score: metrics[2].4,
                two_stage_median_accuracy: metrics[2].5,
                two_stage_worst_accuracy: metrics[2].6,
                // Segment QA
                segment_qa_score_98_102: metrics[3].0,
                segment_qa_score_95_105: metrics[3].1,
                segment_qa_score_90_110: metrics[3].2,
                segment_qa_files_outside_80_120: metrics[3].3,
                segment_qa_weighted_score: metrics[3].4,
                segment_qa_median_accuracy: metrics[3].5,
                segment_qa_worst_accuracy: metrics[3].6,
                // Hybrid
                hybrid_score_98_102: metrics[4].0,
                hybrid_score_95_105: metrics[4].1,
                hybrid_score_90_110: metrics[4].2,
                hybrid_files_outside_80_120: metrics[4].3,
                hybrid_weighted_score: metrics[4].4,
                hybrid_median_accuracy: metrics[4].5,
                hybrid_worst_accuracy: metrics[4].6,
                // Gap + Two-Stage
                gap_two_stage_score_98_102: metrics[5].0,
                gap_two_stage_score_95_105: metrics[5].1,
                gap_two_stage_score_90_110: metrics[5].2,
                gap_two_stage_files_outside_80_120: metrics[5].3,
                gap_two_stage_weighted_score: metrics[5].4,
                gap_two_stage_median_accuracy: metrics[5].5,
                gap_two_stage_worst_accuracy: metrics[5].6,
                // All Combined
                all_combined_score_98_102: metrics[6].0,
                all_combined_score_95_105: metrics[6].1,
                all_combined_score_90_110: metrics[6].2,
                all_combined_files_outside_80_120: metrics[6].3,
                all_combined_weighted_score: metrics[6].4,
                all_combined_median_accuracy: metrics[6].5,
                all_combined_worst_accuracy: metrics[6].6,
            }
        })
        .collect();
    
    Ok(results)
}

// Calculate functions for each approach

fn calculate_current_approach(file_data: &GpxFileData, interval: f32) -> u32 {
    let cleaned_elevations = remove_statistical_outliers(&file_data.elevations, &file_data.distances);
    
    let cleaned_file_data = GpxFileData {
        filename: file_data.filename.clone(),
        elevations: cleaned_elevations,
        distances: file_data.distances.clone(),
        timestamps: file_data.timestamps.clone(),
        official_gain: file_data.official_gain,
    };
    
    calculate_baseline_gain(&cleaned_file_data, interval)
}

fn calculate_gap_detection_only(file_data: &GpxFileData, interval: f32) -> u32 {
    let filled_data = apply_gap_detection_filling(file_data);
    calculate_baseline_gain(&filled_data, interval)
}

fn calculate_two_stage_outlier_only(file_data: &GpxFileData, interval: f32) -> u32 {
    let cleaned_data = apply_two_stage_outlier_removal(file_data);
    calculate_baseline_gain(&cleaned_data, interval)
}

fn calculate_segment_qa_only(file_data: &GpxFileData, interval: f32) -> u32 {
    let segments = analyze_segment_quality(file_data);
    calculate_segment_based_gain(file_data, interval, &segments)
}

fn calculate_hybrid_interval_only(file_data: &GpxFileData, interval: f32) -> u32 {
    calculate_hybrid_interval_gain(file_data, interval)
}

fn calculate_gap_plus_two_stage(file_data: &GpxFileData, interval: f32) -> u32 {
    let filled_data = apply_gap_detection_filling(file_data);
    let cleaned_data = apply_two_stage_outlier_removal(&filled_data);
    calculate_baseline_gain(&cleaned_data, interval)
}

fn calculate_all_combined(file_data: &GpxFileData, interval: f32) -> u32 {
    // Apply in order: Gap Detection -> Two-Stage Outlier -> Segment QA -> Hybrid
    let filled_data = apply_gap_detection_filling(file_data);
    let cleaned_data = apply_two_stage_outlier_removal(&filled_data);
    let segments = analyze_segment_quality(&cleaned_data);
    calculate_segment_based_hybrid_gain(&cleaned_data, interval, &segments)
}

// Enhancement implementations

fn apply_gap_detection_filling(file_data: &GpxFileData) -> GpxFileData {
    let mut filled_elevations = file_data.elevations.clone();
    let mut filled_distances = file_data.distances.clone();
    let mut filled_timestamps = file_data.timestamps.clone();
    
    // Detect gaps (>200m or >30s)
    let mut gaps = Vec::new();
    for i in 1..file_data.distances.len() {
        let dist_gap = file_data.distances[i] - file_data.distances[i-1];
        let time_gap = file_data.timestamps[i] - file_data.timestamps[i-1];
        
        if dist_gap > 200.0 || time_gap > 30.0 {
            gaps.push((i-1, i));
        }
    }
    
    // Fill gaps with interpolated points
    for (start, end) in gaps.iter().rev() {
        let points_to_insert = ((file_data.distances[*end] - file_data.distances[*start]) / 50.0) as usize;
        
        for j in 1..=points_to_insert {
            let t = j as f64 / (points_to_insert + 1) as f64;
            let interp_dist = file_data.distances[*start] * (1.0 - t) + file_data.distances[*end] * t;
            let interp_elev = file_data.elevations[*start] * (1.0 - t) + file_data.elevations[*end] * t;
            let interp_time = file_data.timestamps[*start] * (1.0 - t) + file_data.timestamps[*end] * t;
            
            filled_distances.insert(*start + j, interp_dist);
            filled_elevations.insert(*start + j, interp_elev);
            filled_timestamps.insert(*start + j, interp_time);
        }
    }
    
    GpxFileData {
        filename: file_data.filename.clone(),
        elevations: filled_elevations,
        distances: filled_distances,
        timestamps: filled_timestamps,
        official_gain: file_data.official_gain,
    }
}

fn apply_two_stage_outlier_removal(file_data: &GpxFileData) -> GpxFileData {
    // Stage 1: Gradient outlier removal (existing)
    let stage1_elevations = remove_statistical_outliers(&file_data.elevations, &file_data.distances);
    
    // Stage 2: Elevation gain rate outlier removal
    let mut cleaned_elevations = stage1_elevations.clone();
    
    // Calculate elevation gain rates (m/min)
    let mut gain_rates = Vec::new();
    for i in 1..stage1_elevations.len() {
        let elev_change = stage1_elevations[i] - stage1_elevations[i-1];
        let time_change = file_data.timestamps[i] - file_data.timestamps[i-1];
        
        if time_change > 0.0 && elev_change > 0.0 {
            let rate = (elev_change / time_change) * 60.0; // m/min
            gain_rates.push(rate);
        }
    }
    
    if gain_rates.len() > 4 {
        // Calculate IQR for gain rates
        let mut sorted_rates = gain_rates.clone();
        sorted_rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let q1 = sorted_rates[sorted_rates.len() / 4];
        let q3 = sorted_rates[(sorted_rates.len() * 3) / 4];
        let iqr = q3 - q1;
        
        // More conservative threshold for gain rates
        let upper_bound = q3 + 3.0 * iqr;
        
        // Smooth outlier gain rates
        let mut rate_idx = 0;
        for i in 1..cleaned_elevations.len() {
            if file_data.timestamps[i] - file_data.timestamps[i-1] > 0.0 {
                let elev_change = cleaned_elevations[i] - cleaned_elevations[i-1];
                let time_change = file_data.timestamps[i] - file_data.timestamps[i-1];
                let rate = (elev_change / time_change) * 60.0;
                
                if elev_change > 0.0 && rate > upper_bound && rate_idx < gain_rates.len() {
                    // Cap the elevation change
                    let max_rate = upper_bound;
                    let max_change = (max_rate / 60.0) * time_change;
                    cleaned_elevations[i] = cleaned_elevations[i-1] + max_change;
                }
                rate_idx += 1;
            }
        }
    }
    
    GpxFileData {
        filename: file_data.filename.clone(),
        elevations: cleaned_elevations,
        distances: file_data.distances.clone(),
        timestamps: file_data.timestamps.clone(),
        official_gain: file_data.official_gain,
    }
}

fn analyze_segment_quality(file_data: &GpxFileData) -> Vec<SegmentQuality> {
    let mut segments = Vec::new();
    let segment_length = 1000.0; // 1km segments
    
    let total_distance = file_data.distances.last().unwrap_or(&0.0);
    let num_segments = (total_distance / segment_length).ceil() as usize;
    
    for seg in 0..num_segments {
        let start_dist = seg as f64 * segment_length;
        let end_dist = ((seg + 1) as f64 * segment_length).min(*total_distance);
        
        // Find indices for this segment
        let start_idx = file_data.distances.iter()
            .position(|&d| d >= start_dist)
            .unwrap_or(0);
        let end_idx = file_data.distances.iter()
            .position(|&d| d >= end_dist)
            .unwrap_or(file_data.distances.len() - 1);
        
        if end_idx > start_idx {
            // Calculate quality metrics for segment
            let segment_elevations = &file_data.elevations[start_idx..=end_idx];
            let segment_distances = &file_data.distances[start_idx..=end_idx];
            
            // Check for gaps
            let mut has_gaps = false;
            for i in 1..segment_distances.len() {
                if segment_distances[i] - segment_distances[i-1] > 100.0 {
                    has_gaps = true;
                    break;
                }
            }
            
            // Calculate noise level
            let mut elevation_changes = Vec::new();
            for i in 1..segment_elevations.len() {
                elevation_changes.push((segment_elevations[i] - segment_elevations[i-1]).abs());
            }
            
            let avg_change = if !elevation_changes.is_empty() {
                elevation_changes.iter().sum::<f64>() / elevation_changes.len() as f64
            } else {
                0.0
            };
            
            let noise_level = if avg_change > 0.0 {
                elevation_changes.iter()
                    .map(|&c| (c - avg_change).powi(2))
                    .sum::<f64>() / elevation_changes.len() as f64
            } else {
                0.0
            }.sqrt();
            
            // Calculate quality score (0-100)
            let gap_penalty = if has_gaps { 20.0 } else { 0.0 };
            let noise_penalty = (noise_level / 10.0).min(30.0);
            let quality_score = (100.0 - gap_penalty - noise_penalty).max(0.0);
            
            segments.push(SegmentQuality {
                start_idx,
                end_idx,
                quality_score,
                has_gaps,
                noise_level,
            });
        }
    }
    
    segments
}

fn calculate_segment_based_gain(
    file_data: &GpxFileData,
    base_interval: f32,
    segments: &[SegmentQuality]
) -> u32 {
    let mut total_gain = 0.0;
    
    for segment in segments {
        // Use larger interval for low-quality segments
        let interval = if segment.quality_score < 50.0 {
            base_interval * 2.0
        } else if segment.quality_score < 70.0 {
            base_interval * 1.5
        } else {
            base_interval
        };
        
        let segment_elevations = file_data.elevations[segment.start_idx..=segment.end_idx].to_vec();
        let segment_distances = file_data.distances[segment.start_idx..=segment.end_idx].to_vec();
        
        // Adjust distances to start from 0
        let base_dist = segment_distances[0];
        let adjusted_distances: Vec<f64> = segment_distances.iter()
            .map(|&d| d - base_dist)
            .collect();
        
        let segment_data = GpxFileData {
            filename: file_data.filename.clone(),
            elevations: segment_elevations,
            distances: adjusted_distances,
            timestamps: vec![0.0; segment.end_idx - segment.start_idx + 1],
            official_gain: 0,
        };
        
        let segment_gain = calculate_baseline_gain(&segment_data, interval);
        total_gain += segment_gain as f64;
    }
    
    total_gain.round() as u32
}

fn calculate_hybrid_interval_gain(file_data: &GpxFileData, base_interval: f32) -> u32 {
    // Analyze local terrain complexity
    let mut gains = Vec::new();
    let window_size = 20; // 20 points
    
    for i in window_size..file_data.elevations.len() {
        let window_elevations = &file_data.elevations[i-window_size..i];
        let window_distances = &file_data.distances[i-window_size..i];
        
        let mut window_gain = 0.0;
        for j in 1..window_elevations.len() {
            let elev_change = window_elevations[j] - window_elevations[j-1];
            if elev_change > 0.0 {
                window_gain += elev_change;
            }
        }
        
        let window_distance = window_distances.last().unwrap() - window_distances[0];
        let gain_per_km = if window_distance > 0.0 {
            (window_gain / window_distance) * 1000.0
        } else {
            0.0
        };
        
        gains.push(gain_per_km);
    }
    
    // Create variable intervals based on local complexity
    let mut processed_elevations = Vec::new();
    let mut processed_distances = Vec::new();
    
    let mut current_idx = 0;
    while current_idx < file_data.elevations.len() {
        let local_gain = if current_idx >= window_size && current_idx - window_size < gains.len() {
            gains[current_idx - window_size]
        } else {
            20.0 // Default
        };
        
        // Adaptive interval based on local terrain
        let local_interval = if local_gain < 10.0 {
            base_interval * 2.0  // Flat
        } else if local_gain < 30.0 {
            base_interval        // Rolling
        } else if local_gain < 60.0 {
            base_interval * 0.75 // Hilly
        } else {
            base_interval * 0.5  // Steep
        };
        
        processed_elevations.push(file_data.elevations[current_idx]);
        processed_distances.push(file_data.distances[current_idx]);
        
        // Find next point based on interval
        let target_dist = file_data.distances[current_idx] + local_interval as f64;
        current_idx = file_data.distances.iter()
            .position(|&d| d >= target_dist)
            .unwrap_or(file_data.distances.len());
        
        if current_idx >= file_data.distances.len() {
            break;
        }
    }
    
    let hybrid_data = GpxFileData {
        filename: file_data.filename.clone(),
        elevations: processed_elevations,
        distances: processed_distances,
        timestamps: vec![0.0; processed_elevations.len()],
        official_gain: file_data.official_gain,
    };
    
    let mut elevation_data = ElevationData::new_with_variant(
        hybrid_data.elevations.clone(),
        hybrid_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.get_total_elevation_gain().round() as u32
}

fn calculate_segment_based_hybrid_gain(
    file_data: &GpxFileData,
    base_interval: f32,
    segments: &[SegmentQuality]
) -> u32 {
    let mut total_gain = 0.0;
    
    for segment in segments {
        let segment_elevations = file_data.elevations[segment.start_idx..=segment.end_idx].to_vec();
        let segment_distances = file_data.distances[segment.start_idx..=segment.end_idx].to_vec();
        let segment_timestamps = file_data.timestamps[segment.start_idx..=segment.end_idx].to_vec();
        
        let base_dist = segment_distances[0];
        let adjusted_distances: Vec<f64> = segment_distances.iter()
            .map(|&d| d - base_dist)
            .collect();
        
        let segment_data = GpxFileData {
            filename: file_data.filename.clone(),
            elevations: segment_elevations,
            distances: adjusted_distances,
            timestamps: segment_timestamps,
            official_gain: 0,
        };
        
        // Use quality-based interval
        let quality_adjusted_interval = if segment.quality_score < 50.0 {
            base_interval * 2.0
        } else if segment.quality_score < 70.0 {
            base_interval * 1.5
        } else {
            base_interval
        };
        
        let segment_gain = calculate_hybrid_interval_gain(&segment_data, quality_adjusted_interval);
        total_gain += segment_gain as f64;
    }
    
    total_gain.round() as u32
}

fn calculate_baseline_gain(file_data: &GpxFileData, interval: f32) -> u32 {
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval as f64);
    elevation_data.get_total_elevation_gain().round() as u32
}

fn remove_statistical_outliers(elevations: &[f64], distances: &[f64]) -> Vec<f64> {
    if elevations.len() < 10 {
        return elevations.to_vec();
    }
    
    let mut cleaned = elevations.to_vec();
    
    let mut gradients = Vec::new();
    for i in 1..elevations.len() {
        let dist_diff = distances[i] - distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (elevations[i] - elevations[i-1]) / dist_diff * 100.0;
            gradients.push(gradient);
        }
    }
    
    let mut sorted_gradients = gradients.clone();
    sorted_gradients.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let q1_idx = sorted_gradients.len() / 4;
    let q3_idx = (sorted_gradients.len() * 3) / 4;
    let q1 = sorted_gradients[q1_idx];
    let q3 = sorted_gradients[q3_idx];
    let iqr = q3 - q1;
    
    let lower_bound = q1 - 2.0 * iqr;
    let upper_bound = q3 + 2.0 * iqr;
    
    for i in 1..elevations.len() - 1 {
        if i < gradients.len() {
            let gradient = gradients[i-1];
            
            if gradient < lower_bound || gradient > upper_bound {
                let prev_valid = find_previous_valid_point(i, &gradients, lower_bound, upper_bound);
                let next_valid = find_next_valid_point(i, &gradients, lower_bound, upper_bound);
                
                if let (Some(prev), Some(next)) = (prev_valid, next_valid) {
                    let weight = (i - prev) as f64 / (next - prev) as f64;
                    cleaned[i] = cleaned[prev] * (1.0 - weight) + cleaned[next] * weight;
                }
            }
        }
    }
    
    cleaned
}

fn find_previous_valid_point(
    start: usize,
    gradients: &[f64],
    lower_bound: f64,
    upper_bound: f64
) -> Option<usize> {
    for i in (0..start).rev() {
        if i < gradients.len() && gradients[i] >= lower_bound && gradients[i] <= upper_bound {
            return Some(i);
        }
    }
    Some(0)
}

fn find_next_valid_point(
    start: usize,
    gradients: &[f64],
    lower_bound: f64,
    upper_bound: f64
) -> Option<usize> {
    for i in start..gradients.len() {
        if gradients[i] >= lower_bound && gradients[i] <= upper_bound {
            return Some(i + 1);
        }
    }
    Some(gradients.len())
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

fn write_comprehensive_results(
    results: &[ComparativeAnalysisResult],
    output_path: PathBuf
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    wtr.write_record(&[
        "Interval (m)",
        // Current
        "Current Score", "Current 98-102%", "Current 90-110%", "Current Outside", "Current Success%",
        // Gap Detection
        "Gap Score", "Gap 98-102%", "Gap 90-110%", "Gap Outside", "Gap Success%",
        // Two-Stage
        "TwoStage Score", "TwoStage 98-102%", "TwoStage 90-110%", "TwoStage Outside", "TwoStage Success%",
        // Segment QA
        "SegQA Score", "SegQA 98-102%", "SegQA 90-110%", "SegQA Outside", "SegQA Success%",
        // Hybrid
        "Hybrid Score", "Hybrid 98-102%", "Hybrid 90-110%", "Hybrid Outside", "Hybrid Success%",
        // Gap+TwoStage
        "Gap+TS Score", "Gap+TS 98-102%", "Gap+TS 90-110%", "Gap+TS Outside", "Gap+TS Success%",
        // All Combined
        "AllComb Score", "AllComb 98-102%", "AllComb 90-110%", "AllComb Outside", "AllComb Success%",
    ])?;
    
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.all_combined_weighted_score.partial_cmp(&a.all_combined_weighted_score).unwrap());
    
    for result in sorted_results {
        let total_files = result.current_total_files as f32;
        wtr.write_record(&[
            format!("{:.1}", result.interval_m),
            // Current
            format!("{:.0}", result.current_weighted_score),
            result.current_score_98_102.to_string(),
            result.current_score_90_110.to_string(),
            result.current_files_outside_80_120.to_string(),
            format!("{:.1}", (result.current_score_90_110 as f32 / total_files) * 100.0),
            // Gap Detection
            format!("{:.0}", result.gap_detection_weighted_score),
            result.gap_detection_score_98_102.to_string(),
            result.gap_detection_score_90_110.to_string(),
            result.gap_detection_files_outside_80_120.to_string(),
            format!("{:.1}", (result.gap_detection_score_90_110 as f32 / total_files) * 100.0),
            // Two-Stage
            format!("{:.0}", result.two_stage_weighted_score),
            result.two_stage_score_98_102.to_string(),
            result.two_stage_score_90_110.to_string(),
            result.two_stage_files_outside_80_120.to_string(),
            format!("{:.1}", (result.two_stage_score_90_110 as f32 / total_files) * 100.0),
            // Segment QA
            format!("{:.0}", result.segment_qa_weighted_score),
            result.segment_qa_score_98_102.to_string(),
            result.segment_qa_score_90_110.to_string(),
            result.segment_qa_files_outside_80_120.to_string(),
            format!("{:.1}", (result.segment_qa_score_90_110 as f32 / total_files) * 100.0),
            // Hybrid
            format!("{:.0}", result.hybrid_weighted_score),
            result.hybrid_score_98_102.to_string(),
            result.hybrid_score_90_110.to_string(),
            result.hybrid_files_outside_80_120.to_string(),
            format!("{:.1}", (result.hybrid_score_90_110 as f32 / total_files) * 100.0),
            // Gap+TwoStage
            format!("{:.0}", result.gap_two_stage_weighted_score),
            result.gap_two_stage_score_98_102.to_string(),
            result.gap_two_stage_score_90_110.to_string(),
            result.gap_two_stage_files_outside_80_120.to_string(),
            format!("{:.1}", (result.gap_two_stage_score_90_110 as f32 / total_files) * 100.0),
            // All Combined
            format!("{:.0}", result.all_combined_weighted_score),
            result.all_combined_score_98_102.to_string(),
            result.all_combined_score_90_110.to_string(),
            result.all_combined_files_outside_80_120.to_string(),
            format!("{:.1}", (result.all_combined_score_90_110 as f32 / total_files) * 100.0),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_comprehensive_summary(results: &[ComparativeAnalysisResult]) {
    println!("\n📊 COMPREHENSIVE ENHANCEMENT ANALYSIS SUMMARY");
    println!("============================================");
    
    // Find best interval for each approach
    let approaches = vec![
        ("Current", |r: &ComparativeAnalysisResult| r.current_weighted_score),
        ("Gap Detection", |r: &ComparativeAnalysisResult| r.gap_detection_weighted_score),
        ("Two-Stage Outlier", |r: &ComparativeAnalysisResult| r.two_stage_weighted_score),
        ("Segment QA", |r: &ComparativeAnalysisResult| r.segment_qa_weighted_score),
        ("Hybrid Interval", |r: &ComparativeAnalysisResult| r.hybrid_weighted_score),
        ("Gap + Two-Stage", |r: &ComparativeAnalysisResult| r.gap_two_stage_weighted_score),
        ("All Combined", |r: &ComparativeAnalysisResult| r.all_combined_weighted_score),
    ];
    
    let mut best_results = Vec::new();
    
    for (name, score_fn) in &approaches {
        let best = results.iter()
            .max_by(|a, b| score_fn(a).partial_cmp(&score_fn(b)).unwrap())
            .unwrap();
        best_results.push((*name, best));
    }
    
    println!("\n🏆 OPTIMAL INTERVALS BY APPROACH:");
    println!("Approach            | Interval | Score  | 98-102% | 90-110% | Outside | Success Rate");
    println!("-------------------|----------|--------|---------|---------|---------|-------------");
    
    for (name, result) in &best_results {
        let (score, s98, s90, outside, total) = match *name {
            "Current" => (result.current_weighted_score, result.current_score_98_102, 
                         result.current_score_90_110, result.current_files_outside_80_120,
                         result.current_total_files),
            "Gap Detection" => (result.gap_detection_weighted_score, result.gap_detection_score_98_102,
                               result.gap_detection_score_90_110, result.gap_detection_files_outside_80_120,
                               result.current_total_files),
            "Two-Stage Outlier" => (result.two_stage_weighted_score, result.two_stage_score_98_102,
                                   result.two_stage_score_90_110, result.two_stage_files_outside_80_120,
                                   result.current_total_files),
            "Segment QA" => (result.segment_qa_weighted_score, result.segment_qa_score_98_102,
                            result.segment_qa_score_90_110, result.segment_qa_files_outside_80_120,
                            result.current_total_files),
            "Hybrid Interval" => (result.hybrid_weighted_score, result.hybrid_score_98_102,
                                 result.hybrid_score_90_110, result.hybrid_files_outside_80_120,
                                 result.current_total_files),
            "Gap + Two-Stage" => (result.gap_two_stage_weighted_score, result.gap_two_stage_score_98_102,
                                 result.gap_two_stage_score_90_110, result.gap_two_stage_files_outside_80_120,
                                 result.current_total_files),
            "All Combined" => (result.all_combined_weighted_score, result.all_combined_score_98_102,
                              result.all_combined_score_90_110, result.all_combined_files_outside_80_120,
                              result.current_total_files),
            _ => (0.0, 0, 0, 0, 1),
        };
        
        let success_rate = (s90 as f32 / total as f32) * 100.0;
        
        println!("{:18} | {:7.1}m | {:6.0} | {:7} | {:7} | {:7} | {:10.1}%",
                 name, result.interval_m, score, s98, s90, outside, success_rate);
    }
    
    // Compare improvements
    let current_best = best_results[0].1;
    let combined_best = best_results[6].1;
    
    let current_success = (current_best.current_score_90_110 as f32 / current_best.current_total_files as f32) * 100.0;
    let combined_success = (combined_best.all_combined_score_90_110 as f32 / combined_best.current_total_files as f32) * 100.0;
    
    println!("\n🎯 OVERALL IMPROVEMENT:");
    println!("Current approach:    {:.1}% success rate", current_success);
    println!("All Combined:        {:.1}% success rate ({:+.1}% improvement)", 
             combined_success, combined_success - current_success);
    
    println!("\n📈 FILES MOVED INTO 90-110% BAND:");
    let improvements = vec![
        ("Gap Detection", best_results[1].1.gap_detection_score_90_110 - current_best.current_score_90_110),
        ("Two-Stage Outlier", best_results[2].1.two_stage_score_90_110 - current_best.current_score_90_110),
        ("Segment QA", best_results[3].1.segment_qa_score_90_110 - current_best.current_score_90_110),
        ("Hybrid Interval", best_results[4].1.hybrid_score_90_110 - current_best.current_score_90_110),
        ("Gap + Two-Stage", best_results[5].1.gap_two_stage_score_90_110 - current_best.current_score_90_110),
        ("All Combined", best_results[6].1.all_combined_score_90_110 - current_best.current_score_90_110),
    ];
    
    for (name, improvement) in improvements {
        if improvement != 0 {
            println!("{:18} {:+} files", name, improvement);
        }
    }
}
