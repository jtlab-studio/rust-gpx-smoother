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
    // Current approach (baseline)
    current_score_98_102: u32,
    current_score_95_105: u32,
    current_score_90_110: u32,
    current_files_outside_80_120: u32,
    current_weighted_score: f32,
    current_median_accuracy: f32,
    current_worst_accuracy: f32,
    current_total_files: u32,
    
    // Current + Gap Detection
    current_gap_score_98_102: u32,
    current_gap_score_95_105: u32,
    current_gap_score_90_110: u32,
    current_gap_files_outside_80_120: u32,
    current_gap_weighted_score: f32,
    current_gap_median_accuracy: f32,
    current_gap_worst_accuracy: f32,
    
    // Current + Enhanced Two-Stage
    current_two_stage_score_98_102: u32,
    current_two_stage_score_95_105: u32,
    current_two_stage_score_90_110: u32,
    current_two_stage_files_outside_80_120: u32,
    current_two_stage_weighted_score: f32,
    current_two_stage_median_accuracy: f32,
    current_two_stage_worst_accuracy: f32,
    
    // Current + Quality Weighting
    current_quality_score_98_102: u32,
    current_quality_score_95_105: u32,
    current_quality_score_90_110: u32,
    current_quality_files_outside_80_120: u32,
    current_quality_weighted_score: f32,
    current_quality_median_accuracy: f32,
    current_quality_worst_accuracy: f32,
    
    // Current + Adaptive Intervals
    current_adaptive_score_98_102: u32,
    current_adaptive_score_95_105: u32,
    current_adaptive_score_90_110: u32,
    current_adaptive_files_outside_80_120: u32,
    current_adaptive_weighted_score: f32,
    current_adaptive_median_accuracy: f32,
    current_adaptive_worst_accuracy: f32,
    
    // Current + All Enhancements
    current_all_score_98_102: u32,
    current_all_score_95_105: u32,
    current_all_score_90_110: u32,
    current_all_files_outside_80_120: u32,
    current_all_weighted_score: f32,
    current_all_median_accuracy: f32,
    current_all_worst_accuracy: f32,
}

// Enhancement parameters
#[derive(Debug, Clone)]
struct EnhancementParams {
    // Gap detection
    gap_distance_threshold: f64,  // meters
    gap_time_threshold: f64,      // seconds
    gap_interpolation_interval: f64, // meters
    
    // Two-stage outlier
    gradient_iqr_multiplier: f64,
    rate_iqr_multiplier: f64,
    
    // Quality weighting
    quality_threshold_low: f64,
    quality_threshold_high: f64,
    
    // Adaptive intervals
    min_interval: f64,
    max_interval: f64,
    interval_quality_factor: f64,
}

impl Default for EnhancementParams {
    fn default() -> Self {
        Self {
            gap_distance_threshold: 200.0,
            gap_time_threshold: 30.0,
            gap_interpolation_interval: 50.0,
            gradient_iqr_multiplier: 2.0,
            rate_iqr_multiplier: 3.0,
            quality_threshold_low: 0.3,
            quality_threshold_high: 0.7,
            min_interval: 1.5,
            max_interval: 6.0,
            interval_quality_factor: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
struct TerrainMetrics {
    gain_per_km: f64,
    elevation_variance: f64,
    gradient_std_dev: f64,
    terrain_type: TerrainType,
}

#[derive(Debug, Clone)]
enum TerrainType {
    Flat,         // < 20m/km
    Rolling,      // 20-40m/km  
    Hilly,        // 40-80m/km
    Mountainous,  // > 80m/km
}

fn classify_terrain(gain_per_km: f64) -> TerrainType {
    if gain_per_km < 20.0 {
        TerrainType::Flat
    } else if gain_per_km < 40.0 {
        TerrainType::Rolling
    } else if gain_per_km < 80.0 {
        TerrainType::Hilly
    } else {
        TerrainType::Mountainous
    }
}

pub fn run_enhanced_comparative_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🔬 COMPLEMENTARY ENHANCEMENTS ANALYSIS");
    println!("======================================");
    println!("Testing approaches that BUILD ON the current method:");
    println!("1. Current (Baseline) - 77.7% success");
    println!("2. Current + Gap Detection");
    println!("3. Current + Enhanced Two-Stage");
    println!("4. Current + Quality Weighting");
    println!("5. Current + Adaptive Intervals");
    println!("6. Current + All Enhancements");
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
    write_comprehensive_results(&results, Path::new(gpx_folder).join("complementary_enhancements_analysis.csv"))?;
    
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
    
    println!("\n🚀 Processing {} intervals × {} files × 6 approaches = {} total calculations",
             intervals.len(), valid_files.len(), intervals.len() * valid_files.len() * 6);
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
                    
                    // 1. Current approach (baseline)
                    let current_gain = calculate_current_approach(file_data, *interval);
                    accuracies.push((current_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 2. Current + Gap Detection
                    let current_gap_gain = calculate_current_plus_gap_detection(file_data, *interval);
                    accuracies.push((current_gap_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 3. Current + Enhanced Two-Stage
                    let current_two_stage_gain = calculate_current_plus_enhanced_two_stage(file_data, *interval);
                    accuracies.push((current_two_stage_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 4. Current + Quality Weighting
                    let current_quality_gain = calculate_current_plus_quality_weighting(file_data, *interval);
                    accuracies.push((current_quality_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 5. Current + Adaptive Intervals
                    let current_adaptive_gain = calculate_current_plus_adaptive_intervals(file_data, *interval);
                    accuracies.push((current_adaptive_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
                    // 6. Current + All Enhancements
                    let current_all_gain = calculate_current_plus_all_enhancements(file_data, *interval);
                    accuracies.push((current_all_gain as f32 / file_data.official_gain as f32) * 100.0);
                    
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
            
            let approach_accuracies: Vec<Vec<f32>> = (0..6).map(|i| {
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
                // Current + Gap
                current_gap_score_98_102: metrics[1].0,
                current_gap_score_95_105: metrics[1].1,
                current_gap_score_90_110: metrics[1].2,
                current_gap_files_outside_80_120: metrics[1].3,
                current_gap_weighted_score: metrics[1].4,
                current_gap_median_accuracy: metrics[1].5,
                current_gap_worst_accuracy: metrics[1].6,
                // Current + Two-Stage
                current_two_stage_score_98_102: metrics[2].0,
                current_two_stage_score_95_105: metrics[2].1,
                current_two_stage_score_90_110: metrics[2].2,
                current_two_stage_files_outside_80_120: metrics[2].3,
                current_two_stage_weighted_score: metrics[2].4,
                current_two_stage_median_accuracy: metrics[2].5,
                current_two_stage_worst_accuracy: metrics[2].6,
                // Current + Quality
                current_quality_score_98_102: metrics[3].0,
                current_quality_score_95_105: metrics[3].1,
                current_quality_score_90_110: metrics[3].2,
                current_quality_files_outside_80_120: metrics[3].3,
                current_quality_weighted_score: metrics[3].4,
                current_quality_median_accuracy: metrics[3].5,
                current_quality_worst_accuracy: metrics[3].6,
                // Current + Adaptive
                current_adaptive_score_98_102: metrics[4].0,
                current_adaptive_score_95_105: metrics[4].1,
                current_adaptive_score_90_110: metrics[4].2,
                current_adaptive_files_outside_80_120: metrics[4].3,
                current_adaptive_weighted_score: metrics[4].4,
                current_adaptive_median_accuracy: metrics[4].5,
                current_adaptive_worst_accuracy: metrics[4].6,
                // Current + All
                current_all_score_98_102: metrics[5].0,
                current_all_score_95_105: metrics[5].1,
                current_all_score_90_110: metrics[5].2,
                current_all_files_outside_80_120: metrics[5].3,
                current_all_weighted_score: metrics[5].4,
                current_all_median_accuracy: metrics[5].5,
                current_all_worst_accuracy: metrics[5].6,
            }
        })
        .collect();
    
    Ok(results)
}

// Current approach (baseline)
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

// Complementary approaches that build on Current

fn calculate_current_plus_gap_detection(file_data: &GpxFileData, interval: f32) -> u32 {
    let params = EnhancementParams::default();
    
    // First: Fill gaps in the raw data
    let filled_data = apply_gap_detection_filling(file_data, &params);
    
    // Then: Apply Current's proven outlier removal
    let cleaned_elevations = remove_statistical_outliers(&filled_data.elevations, &filled_data.distances);
    
    let enhanced_data = GpxFileData {
        filename: filled_data.filename.clone(),
        elevations: cleaned_elevations,
        distances: filled_data.distances.clone(),
        timestamps: filled_data.timestamps.clone(),
        official_gain: filled_data.official_gain,
    };
    
    // Finally: Standard baseline calculation
    calculate_baseline_gain(&enhanced_data, interval)
}

fn calculate_current_plus_enhanced_two_stage(file_data: &GpxFileData, interval: f32) -> u32 {
    let params = EnhancementParams::default();
    
    // Start with Current's outlier removal
    let cleaned_elevations = remove_statistical_outliers(&file_data.elevations, &file_data.distances);
    
    let cleaned_data = GpxFileData {
        filename: file_data.filename.clone(),
        elevations: cleaned_elevations,
        distances: file_data.distances.clone(),
        timestamps: file_data.timestamps.clone(),
        official_gain: file_data.official_gain,
    };
    
    // Add the second stage (elevation gain rate outlier removal)
    let enhanced_data = apply_second_stage_outlier_removal(&cleaned_data, &params);
    
    calculate_baseline_gain(&enhanced_data, interval)
}

fn calculate_current_plus_quality_weighting(file_data: &GpxFileData, interval: f32) -> u32 {
    let params = EnhancementParams::default();
    
    // First: Current's outlier removal
    let cleaned_elevations = remove_statistical_outliers(&file_data.elevations, &file_data.distances);
    
    let cleaned_data = GpxFileData {
        filename: file_data.filename.clone(),
        elevations: cleaned_elevations,
        distances: file_data.distances.clone(),
        timestamps: file_data.timestamps.clone(),
        official_gain: file_data.official_gain,
    };
    
    // Calculate quality scores for each point
    let quality_scores = calculate_point_quality_scores(&cleaned_data);
    
    // Apply quality-weighted processing
    let enhanced_data = apply_quality_weighted_processing(&cleaned_data, &quality_scores, &params);
    
    calculate_baseline_gain(&enhanced_data, interval)
}

fn calculate_current_plus_adaptive_intervals(file_data: &GpxFileData, base_interval: f32) -> u32 {
    let params = EnhancementParams::default();
    
    // First: Current's outlier removal
    let cleaned_elevations = remove_statistical_outliers(&file_data.elevations, &file_data.distances);
    
    let cleaned_data = GpxFileData {
        filename: file_data.filename.clone(),
        elevations: cleaned_elevations,
        distances: file_data.distances.clone(),
        timestamps: file_data.timestamps.clone(),
        official_gain: file_data.official_gain,
    };
    
    // Calculate adaptive intervals based on local quality
    let intervals = calculate_adaptive_intervals(&cleaned_data, base_interval, &params);
    
    // Process with adaptive intervals
    calculate_with_adaptive_intervals(&cleaned_data, &intervals)
}

fn calculate_current_plus_all_enhancements(file_data: &GpxFileData, base_interval: f32) -> u32 {
    let params = EnhancementParams::default();
    
    // Step 1: Gap detection on raw data
    let filled_data = apply_gap_detection_filling(file_data, &params);
    
    // Step 2: Current's outlier removal
    let cleaned_elevations = remove_statistical_outliers(&filled_data.elevations, &filled_data.distances);
    
    let cleaned_data = GpxFileData {
        filename: filled_data.filename.clone(),
        elevations: cleaned_elevations,
        distances: filled_data.distances.clone(),
        timestamps: filled_data.timestamps.clone(),
        official_gain: filled_data.official_gain,
    };
    
    // Step 3: Second stage outlier removal
    let two_stage_data = apply_second_stage_outlier_removal(&cleaned_data, &params);
    
    // Step 4: Quality weighting
    let quality_scores = calculate_point_quality_scores(&two_stage_data);
    let quality_weighted_data = apply_quality_weighted_processing(&two_stage_data, &quality_scores, &params);
    
    // Step 5: Adaptive intervals
    let intervals = calculate_adaptive_intervals(&quality_weighted_data, base_interval, &params);
    
    // Final calculation
    calculate_with_adaptive_intervals(&quality_weighted_data, &intervals)
}

// Enhancement implementations

fn apply_gap_detection_filling(file_data: &GpxFileData, params: &EnhancementParams) -> GpxFileData {
    let mut filled_elevations = file_data.elevations.clone();
    let mut filled_distances = file_data.distances.clone();
    let mut filled_timestamps = file_data.timestamps.clone();
    
    // Detect gaps
    let mut gaps = Vec::new();
    for i in 1..file_data.distances.len() {
        let dist_gap = file_data.distances[i] - file_data.distances[i-1];
        let time_gap = file_data.timestamps[i] - file_data.timestamps[i-1];
        
        if dist_gap > params.gap_distance_threshold || time_gap > params.gap_time_threshold {
            gaps.push((i-1, i));
        }
    }
    
    // Fill gaps with interpolated points
    for (start, end) in gaps.iter().rev() {
        let points_to_insert = ((file_data.distances[*end] - file_data.distances[*start]) / params.gap_interpolation_interval) as usize;
        
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

fn apply_second_stage_outlier_removal(file_data: &GpxFileData, params: &EnhancementParams) -> GpxFileData {
    let mut cleaned_elevations = file_data.elevations.clone();
    
    // Calculate elevation gain rates (m/min)
    let mut gain_rates = Vec::new();
    for i in 1..file_data.elevations.len() {
        let elev_change = file_data.elevations[i] - file_data.elevations[i-1];
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
        
        let upper_bound = q3 + params.rate_iqr_multiplier * iqr;
        
        // Smooth outlier gain rates
        let mut rate_idx = 0;
        for i in 1..cleaned_elevations.len() {
            if file_data.timestamps[i] - file_data.timestamps[i-1] > 0.0 {
                let elev_change = cleaned_elevations[i] - cleaned_elevations[i-1];
                let time_change = file_data.timestamps[i] - file_data.timestamps[i-1];
                let rate = (elev_change / time_change) * 60.0;
                
                if elev_change > 0.0 && rate > upper_bound && rate_idx < gain_rates.len() {
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

fn calculate_point_quality_scores(file_data: &GpxFileData) -> Vec<f64> {
    let mut quality_scores = vec![1.0; file_data.elevations.len()];
    let window_size = 10;
    
    for i in 0..file_data.elevations.len() {
        let start_idx = if i >= window_size / 2 { i - window_size / 2 } else { 0 };
        let end_idx = if i + window_size / 2 < file_data.elevations.len() { 
            i + window_size / 2 
        } else { 
            file_data.elevations.len() - 1 
        };
        
        let mut quality: f64 = 1.0;
        
        // Check local point density
        if end_idx > start_idx {
            let distance_span = file_data.distances[end_idx] - file_data.distances[start_idx];
            let point_count = end_idx - start_idx + 1;
            let avg_spacing = distance_span / point_count as f64;
            
            if avg_spacing > 50.0 {
                quality *= 0.5; // Sparse data
            } else if avg_spacing < 2.0 {
                quality *= 0.8; // Very dense (might be noisy)
            }
        }
        
        // Check gradient consistency
        if i > 0 && i < file_data.elevations.len() - 1 {
            let grad1 = (file_data.elevations[i] - file_data.elevations[i-1]) / 
                       (file_data.distances[i] - file_data.distances[i-1] + 0.001);
            let grad2 = (file_data.elevations[i+1] - file_data.elevations[i]) / 
                       (file_data.distances[i+1] - file_data.distances[i] + 0.001);
            
            let grad_change = (grad1 - grad2).abs();
            if grad_change > 0.5 {
                quality *= 0.7; // Large gradient change
            }
        }
        
        quality_scores[i] = quality.max(0.1).min(1.0);
    }
    
    quality_scores
}

fn apply_quality_weighted_processing(
    file_data: &GpxFileData, 
    quality_scores: &[f64],
    params: &EnhancementParams
) -> GpxFileData {
    let mut processed_elevations = file_data.elevations.clone();
    
    // Apply quality-weighted smoothing
    for i in 1..processed_elevations.len() - 1 {
        if quality_scores[i] < params.quality_threshold_low {
            // Low quality point - use heavy smoothing
            let weight = 0.2;
            processed_elevations[i] = processed_elevations[i] * weight + 
                                     (processed_elevations[i-1] + processed_elevations[i+1]) * 0.5 * (1.0 - weight);
        } else if quality_scores[i] < params.quality_threshold_high {
            // Medium quality - light smoothing
            let weight = 0.7;
            processed_elevations[i] = processed_elevations[i] * weight + 
                                     (processed_elevations[i-1] + processed_elevations[i+1]) * 0.5 * (1.0 - weight);
        }
        // High quality points remain unchanged
    }
    
    GpxFileData {
        filename: file_data.filename.clone(),
        elevations: processed_elevations,
        distances: file_data.distances.clone(),
        timestamps: file_data.timestamps.clone(),
        official_gain: file_data.official_gain,
    }
}

fn calculate_adaptive_intervals(
    file_data: &GpxFileData,
    base_interval: f32,
    params: &EnhancementParams
) -> Vec<f64> {
    let mut intervals = Vec::new();
    let window_size = 20;
    
    for i in 0..file_data.distances.len() {
        let start_idx = if i >= window_size / 2 { i - window_size / 2 } else { 0 };
        let end_idx = if i + window_size / 2 < file_data.distances.len() { 
            i + window_size / 2 
        } else { 
            file_data.distances.len() - 1 
        };
        
        // Calculate local quality
        let local_quality = calculate_local_quality(file_data, start_idx, end_idx);
        
        // High quality -> smaller intervals, Low quality -> larger intervals
        let quality_factor = 1.0 + params.interval_quality_factor * (1.0 - local_quality);
        let interval = (base_interval as f64 * quality_factor)
            .max(params.min_interval)
            .min(params.max_interval);
        
        intervals.push(interval);
    }
    
    intervals
}

fn calculate_local_quality(file_data: &GpxFileData, start: usize, end: usize) -> f64 {
    if end <= start {
        return 0.5;
    }
    
    let mut quality_score: f64 = 1.0;
    
    // Check point density
    let distance_span = file_data.distances[end] - file_data.distances[start];
    let point_count = end - start + 1;
    let avg_spacing = distance_span / point_count as f64;
    
    if avg_spacing > 50.0 {
        quality_score *= 0.5; // Sparse data
    } else if avg_spacing < 5.0 {
        quality_score *= 0.9; // Very dense (might be noisy)
    }
    
    // Check gradient consistency
    let mut gradient_changes = Vec::new();
    for i in (start + 1)..end {
        let dist_diff = file_data.distances[i] - file_data.distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (file_data.elevations[i] - file_data.elevations[i-1]) / dist_diff * 100.0;
            gradient_changes.push(gradient);
        }
    }
    
    if !gradient_changes.is_empty() {
        let gradient_variance = calculate_variance(&gradient_changes);
        if gradient_variance > 100.0 {
            quality_score *= 0.7; // High gradient variability
        }
    }
    
    quality_score.max(0.1).min(1.0)
}

fn calculate_with_adaptive_intervals(file_data: &GpxFileData, intervals: &[f64]) -> u32 {
    if intervals.is_empty() {
        return 0;
    }
    
    let mut processed_elevations = Vec::new();
    let mut processed_distances = Vec::new();
    
    let mut current_idx = 0;
    let mut interval_idx = 0;
    
    while current_idx < file_data.elevations.len() {
        processed_elevations.push(file_data.elevations[current_idx]);
        processed_distances.push(file_data.distances[current_idx]);
        
        // Get interval for current position
        let interval = intervals.get(interval_idx).unwrap_or(&3.7);
        interval_idx = (interval_idx + 1).min(intervals.len() - 1);
        
        // Find next point based on adaptive interval
        let target_dist = file_data.distances[current_idx] + interval;
        let mut next_idx = current_idx + 1;
        
        while next_idx < file_data.distances.len() && file_data.distances[next_idx] < target_dist {
            next_idx += 1;
        }
        
        current_idx = next_idx;
        
        if current_idx >= file_data.distances.len() {
            // Add last point if not already included
            let last_idx = file_data.distances.len() - 1;
            if processed_distances.last() != Some(&file_data.distances[last_idx]) {
                processed_elevations.push(file_data.elevations[last_idx]);
                processed_distances.push(file_data.distances[last_idx]);
            }
            break;
        }
    }
    
    // Calculate elevation gain using the processed data
    let elevation_data = ElevationData::new_with_variant(
        processed_elevations,
        processed_distances,
        SmoothingVariant::DistBased
    );
    
    elevation_data.get_total_elevation_gain().round() as u32
}

// Helper functions

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

fn calculate_variance(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>() / values.len() as f64
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
        // Current + Gap
        "C+Gap Score", "C+Gap 98-102%", "C+Gap 90-110%", "C+Gap Outside", "C+Gap Success%",
        // Current + Two-Stage
        "C+2Stage Score", "C+2Stage 98-102%", "C+2Stage 90-110%", "C+2Stage Outside", "C+2Stage Success%",
        // Current + Quality
        "C+Quality Score", "C+Quality 98-102%", "C+Quality 90-110%", "C+Quality Outside", "C+Quality Success%",
        // Current + Adaptive
        "C+Adaptive Score", "C+Adaptive 98-102%", "C+Adaptive 90-110%", "C+Adaptive Outside", "C+Adaptive Success%",
        // Current + All
        "C+All Score", "C+All 98-102%", "C+All 90-110%", "C+All Outside", "C+All Success%",
    ])?;
    
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.current_all_weighted_score.partial_cmp(&a.current_all_weighted_score).unwrap());
    
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
            // Current + Gap
            format!("{:.0}", result.current_gap_weighted_score),
            result.current_gap_score_98_102.to_string(),
            result.current_gap_score_90_110.to_string(),
            result.current_gap_files_outside_80_120.to_string(),
            format!("{:.1}", (result.current_gap_score_90_110 as f32 / total_files) * 100.0),
            // Current + Two-Stage
            format!("{:.0}", result.current_two_stage_weighted_score),
            result.current_two_stage_score_98_102.to_string(),
            result.current_two_stage_score_90_110.to_string(),
            result.current_two_stage_files_outside_80_120.to_string(),
            format!("{:.1}", (result.current_two_stage_score_90_110 as f32 / total_files) * 100.0),
            // Current + Quality
            format!("{:.0}", result.current_quality_weighted_score),
            result.current_quality_score_98_102.to_string(),
            result.current_quality_score_90_110.to_string(),
            result.current_quality_files_outside_80_120.to_string(),
            format!("{:.1}", (result.current_quality_score_90_110 as f32 / total_files) * 100.0),
            // Current + Adaptive
            format!("{:.0}", result.current_adaptive_weighted_score),
            result.current_adaptive_score_98_102.to_string(),
            result.current_adaptive_score_90_110.to_string(),
            result.current_adaptive_files_outside_80_120.to_string(),
            format!("{:.1}", (result.current_adaptive_score_90_110 as f32 / total_files) * 100.0),
            // Current + All
            format!("{:.0}", result.current_all_weighted_score),
            result.current_all_score_98_102.to_string(),
            result.current_all_score_90_110.to_string(),
            result.current_all_files_outside_80_120.to_string(),
            format!("{:.1}", (result.current_all_score_90_110 as f32 / total_files) * 100.0),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_comprehensive_summary(results: &[ComparativeAnalysisResult]) {
    println!("\n📊 COMPLEMENTARY ENHANCEMENTS ANALYSIS SUMMARY");
    println!("==============================================");
    
    // Find best interval for each approach using boxed trait objects
    let approaches: Vec<(&str, Box<dyn Fn(&ComparativeAnalysisResult) -> f32>)> = vec![
        ("Current", Box::new(|r| r.current_weighted_score)),
        ("Current + Gap Detection", Box::new(|r| r.current_gap_weighted_score)),
        ("Current + Enhanced Two-Stage", Box::new(|r| r.current_two_stage_weighted_score)),
        ("Current + Quality Weighting", Box::new(|r| r.current_quality_weighted_score)),
        ("Current + Adaptive Intervals", Box::new(|r| r.current_adaptive_weighted_score)),
        ("Current + All Enhancements", Box::new(|r| r.current_all_weighted_score)),
    ];
    
    let mut best_results = Vec::new();
    
    for (name, score_fn) in &approaches {
        let best = results.iter()
            .max_by(|a, b| score_fn(a).partial_cmp(&score_fn(b)).unwrap())
            .unwrap();
        best_results.push((*name, best));
    }
    
    println!("\n🏆 OPTIMAL INTERVALS BY APPROACH:");
    println!("Approach                       | Interval | Score  | 98-102% | 90-110% | Outside | Success Rate");
    println!("-------------------------------|----------|--------|---------|---------|---------|-------------");
    
    for (name, result) in &best_results {
        let (score, s98, s90, outside, total) = match *name {
            "Current" => (result.current_weighted_score, result.current_score_98_102, 
                         result.current_score_90_110, result.current_files_outside_80_120,
                         result.current_total_files),
            "Current + Gap Detection" => (result.current_gap_weighted_score, result.current_gap_score_98_102,
                                         result.current_gap_score_90_110, result.current_gap_files_outside_80_120,
                                         result.current_total_files),
            "Current + Enhanced Two-Stage" => (result.current_two_stage_weighted_score, result.current_two_stage_score_98_102,
                                              result.current_two_stage_score_90_110, result.current_two_stage_files_outside_80_120,
                                              result.current_total_files),
            "Current + Quality Weighting" => (result.current_quality_weighted_score, result.current_quality_score_98_102,
                                             result.current_quality_score_90_110, result.current_quality_files_outside_80_120,
                                             result.current_total_files),
            "Current + Adaptive Intervals" => (result.current_adaptive_weighted_score, result.current_adaptive_score_98_102,
                                              result.current_adaptive_score_90_110, result.current_adaptive_files_outside_80_120,
                                              result.current_total_files),
            "Current + All Enhancements" => (result.current_all_weighted_score, result.current_all_score_98_102,
                                            result.current_all_score_90_110, result.current_all_files_outside_80_120,
                                            result.current_total_files),
            _ => (0.0, 0, 0, 0, 1),
        };
        
        let success_rate = (s90 as f32 / total as f32) * 100.0;
        
        println!("{:30} | {:7.1}m | {:6.0} | {:7} | {:7} | {:7} | {:10.1}%",
                 name, result.interval_m, score, s98, s90, outside, success_rate);
    }
    
    // Compare improvements
    let current_best = best_results[0].1;
    let best_enhanced = best_results.iter()
        .skip(1)
        .max_by(|a, b| {
            let score_a = match a.0 {
                "Current + Gap Detection" => a.1.current_gap_weighted_score,
                "Current + Enhanced Two-Stage" => a.1.current_two_stage_weighted_score,
                "Current + Quality Weighting" => a.1.current_quality_weighted_score,
                "Current + Adaptive Intervals" => a.1.current_adaptive_weighted_score,
                "Current + All Enhancements" => a.1.current_all_weighted_score,
                _ => 0.0,
            };
            let score_b = match b.0 {
                "Current + Gap Detection" => b.1.current_gap_weighted_score,
                "Current + Enhanced Two-Stage" => b.1.current_two_stage_weighted_score,
                "Current + Quality Weighting" => b.1.current_quality_weighted_score,
                "Current + Adaptive Intervals" => b.1.current_adaptive_weighted_score,
                "Current + All Enhancements" => b.1.current_all_weighted_score,
                _ => 0.0,
            };
            score_a.partial_cmp(&score_b).unwrap()
        })
        .unwrap();
    
    let current_success = (current_best.current_score_90_110 as f32 / current_best.current_total_files as f32) * 100.0;
    
    let (best_enhanced_success, best_enhanced_name) = match best_enhanced.0 {
        "Current + Gap Detection" => (
            (best_enhanced.1.current_gap_score_90_110 as f32 / current_best.current_total_files as f32) * 100.0,
            "Current + Gap Detection"
        ),
        "Current + Enhanced Two-Stage" => (
            (best_enhanced.1.current_two_stage_score_90_110 as f32 / current_best.current_total_files as f32) * 100.0,
            "Current + Enhanced Two-Stage"
        ),
        "Current + Quality Weighting" => (
            (best_enhanced.1.current_quality_score_90_110 as f32 / current_best.current_total_files as f32) * 100.0,
            "Current + Quality Weighting"
        ),
        "Current + Adaptive Intervals" => (
            (best_enhanced.1.current_adaptive_score_90_110 as f32 / current_best.current_total_files as f32) * 100.0,
            "Current + Adaptive Intervals"
        ),
        "Current + All Enhancements" => (
            (best_enhanced.1.current_all_score_90_110 as f32 / current_best.current_total_files as f32) * 100.0,
            "Current + All Enhancements"
        ),
        _ => (0.0, "Unknown"),
    };
    
    println!("\n🎯 OVERALL IMPROVEMENT:");
    println!("Current approach:    {:.1}% success rate", current_success);
    println!("Best enhancement ({}): {:.1}% success rate ({:+.1}% improvement)", 
             best_enhanced_name, best_enhanced_success, best_enhanced_success - current_success);
    
    // Show improvements for each enhancement
    println!("\n📈 ENHANCEMENT CONTRIBUTIONS:");
    let improvements = vec![
        ("Gap Detection", best_results[1].1.current_gap_score_90_110 as i32 - current_best.current_score_90_110 as i32),
        ("Enhanced Two-Stage", best_results[2].1.current_two_stage_score_90_110 as i32 - current_best.current_score_90_110 as i32),
        ("Quality Weighting", best_results[3].1.current_quality_score_90_110 as i32 - current_best.current_score_90_110 as i32),
        ("Adaptive Intervals", best_results[4].1.current_adaptive_score_90_110 as i32 - current_best.current_score_90_110 as i32),
        ("All Enhancements", best_results[5].1.current_all_score_90_110 as i32 - current_best.current_score_90_110 as i32),
    ];
    
    for (name, improvement) in improvements {
        if improvement >= 0 {
            println!("  + {} : {:+} files moved into 90-110% band", name, improvement);
        } else {
            println!("  - {} : {} files moved out of 90-110% band", name, -improvement);
        }
    }
    
    // Recommendations
    println!("\n💡 RECOMMENDATIONS:");
    if best_enhanced_success > current_success + 2.0 {
        println!("✅ The {} approach shows significant improvement!", best_enhanced_name);
        println!("   Consider adopting this as your new standard approach.");
    } else if best_enhanced_success > current_success {
        println!("📊 Modest improvements found. The {} approach is slightly better.", best_enhanced_name);
        println!("   May be worth implementing for critical applications.");
    } else {
        println!("⚠️  No enhancements improved upon the current approach.");
        println!("   The current method is already well-optimized for your data.");
    }
}