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
    pub quality_score: f64, // 0-100, higher is better
}

#[derive(Debug, Serialize, Clone)]
pub struct ComparativeAnalysisResult {
    interval_m: f32,
    // Baseline (current approach)
    baseline_score_98_102: u32,
    baseline_score_95_105: u32,
    baseline_score_90_110: u32,
    baseline_files_outside_80_120: u32,
    baseline_weighted_score: f32,
    baseline_median_accuracy: f32,
    baseline_worst_accuracy: f32,
    baseline_total_files: u32,
    
    // With GPS Quality Processing
    gps_quality_score_98_102: u32,
    gps_quality_score_95_105: u32,
    gps_quality_score_90_110: u32,
    gps_quality_files_outside_80_120: u32,
    gps_quality_weighted_score: f32,
    gps_quality_median_accuracy: f32,
    gps_quality_worst_accuracy: f32,
    gps_quality_total_files: u32,
    
    // With GPS Quality + Statistical Outlier Removal
    combined_score_98_102: u32,
    combined_score_95_105: u32,
    combined_score_90_110: u32,
    combined_files_outside_80_120: u32,
    combined_weighted_score: f32,
    combined_median_accuracy: f32,
    combined_worst_accuracy: f32,
    combined_total_files: u32,
}

pub fn run_enhanced_comparative_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔬 ENHANCED COMPARATIVE ANALYSIS");
    println!("================================");
    println!("Comparing three approaches:");
    println!("1. Baseline (current distance-based processing)");
    println!("2. GPS Quality-Based Processing");
    println!("3. GPS Quality + Statistical Outlier Removal");
    
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
                // Check if file has any elevation variation
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
    
    // Process with three different approaches
    let processing_start = std::time::Instant::now();
    let results = process_all_approaches_optimized(&gpx_files_data, &files_with_elevation)?;
    println!("✅ Processing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write results
    write_comparative_results(&results, Path::new(gpx_folder).join("comparative_analysis_results.csv"))?;
    
    // Print summary
    print_comparative_summary(&results);
    
    Ok(())
}

#[derive(Debug, Clone)]
struct GpxFileData {
    filename: String,
    elevations: Vec<f64>,
    distances: Vec<f64>,
    timestamps: Vec<f64>, // seconds since start
    official_gain: u32,
}

fn load_gpx_data(gpx_folder: &str) -> Result<(HashMap<String, GpxFileData>, Vec<String>), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::BufReader;
    use gpx::read;
    use geo::{HaversineDistance, point};
    use walkdir::WalkDir;
    
    let mut gpx_data = HashMap::new();
    let mut valid_files = Vec::new();
    
    // Load official elevation data
    let official_data = crate::load_official_elevation_data()?;
    
    // Process each GPX file
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
                    
                    // Try to load the GPX file
                    match File::open(path) {
                        Ok(file) => {
                            let reader = BufReader::new(file);
                            match read(reader) {
                                Ok(gpx) => {
                                    // Extract data from GPX
                                    let mut coords: Vec<(f64, f64, f64)> = vec![];
                                    let mut timestamps = vec![];
                                    
                                    for track in gpx.tracks {
                                        for segment in track.segments {
                                            for pt in segment.points {
                                                if let Some(ele) = pt.elevation {
                                                    coords.push((pt.point().y(), pt.point().x(), ele));
                                                    
                                                    // Extract timestamp if available
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
                                        // Calculate distances
                                        let mut distances = vec![0.0];
                                        for i in 1..coords.len() {
                                            let a = point!(x: coords[i-1].1, y: coords[i-1].0);
                                            let b = point!(x: coords[i].1, y: coords[i].0);
                                            let dist = a.haversine_distance(&b);
                                            distances.push(distances[i-1] + dist);
                                        }
                                        
                                        // Convert timestamps to seconds
                                        let mut time_seconds = vec![0.0];
                                        if timestamps.len() >= 2 {
                                            for i in 1..timestamps.len().min(coords.len()) {
                                                // Simple incrementing for now
                                                time_seconds.push(i as f64);
                                            }
                                        }
                                        // Pad with estimated times if needed
                                        while time_seconds.len() < coords.len() {
                                            time_seconds.push(time_seconds.len() as f64);
                                        }
                                        
                                        let elevations: Vec<f64> = coords.iter().map(|c| c.2).collect();
                                        
                                        // Get official elevation gain
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

fn process_all_approaches_optimized(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String]
) -> Result<Vec<ComparativeAnalysisResult>, Box<dyn std::error::Error>> {
    // Test intervals from 0.05m to 8.0m
    let intervals: Vec<f32> = (1..=160).map(|i| i as f32 * 0.05).collect();
    
    // Create Arc for shared data
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("\n🚀 Processing {} intervals × {} files × 3 approaches = {} total calculations",
             intervals.len(), valid_files.len(), intervals.len() * valid_files.len() * 3);
    println!("⚡ Using parallel processing on {} cores", num_cpus::get());
    
    // Create all work items (interval, file) pairs
    let work_items: Vec<(f32, String)> = intervals.iter()
        .flat_map(|&interval| {
            valid_files.iter().map(move |file| (interval, file.clone()))
        })
        .collect();
    
    println!("📊 Creating {} work items for parallel processing...", work_items.len());
    
    // Add progress tracking
    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_items = work_items.len();
    let start_time = std::time::Instant::now();
    
    // Process all work items in parallel
    let all_results: Vec<(f32, String, f32, f32, f32)> = work_items
        .par_iter()
        .filter_map(|(interval, filename)| {
            let gpx_data = Arc::clone(&gpx_data_arc);
            let processed_clone = Arc::clone(&processed);
            
            if let Some(file_data) = gpx_data.get(filename) {
                if file_data.official_gain > 0 {
                    // Approach 1: Baseline
                    let baseline_gain = calculate_baseline_gain(file_data, *interval);
                    let baseline_accuracy = (baseline_gain as f32 / file_data.official_gain as f32) * 100.0;
                    
                    // Approach 2: GPS Quality-Based
                    let gps_metrics = calculate_gps_quality_metrics(file_data);
                    let gps_quality_gain = calculate_gps_quality_adjusted_gain(file_data, *interval, &gps_metrics);
                    let gps_quality_accuracy = (gps_quality_gain as f32 / file_data.official_gain as f32) * 100.0;
                    
                    // Approach 3: Combined (GPS Quality + Statistical Outlier Removal)
                    let combined_gain = calculate_combined_approach_gain(file_data, *interval, &gps_metrics);
                    let combined_accuracy = (combined_gain as f32 / file_data.official_gain as f32) * 100.0;
                    
                    // Update progress
                    let count = processed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count % 1000 == 0 {
                        let elapsed = start_time.elapsed().as_secs_f64();
                        let rate = count as f64 / elapsed;
                        let remaining = (total_items - count) as f64 / rate;
                        println!("  Progress: {}/{} ({:.1}%) - {:.0} items/sec - ETA: {:.0}s",
                                 count, total_items, 
                                 (count as f64 / total_items as f64) * 100.0,
                                 rate, remaining);
                    }
                    
                    return Some((*interval, filename.clone(), baseline_accuracy, gps_quality_accuracy, combined_accuracy));
                }
            }
            None
        })
        .collect();
    
    println!("✅ Parallel processing complete, aggregating results...");
    
    // Group results by interval
    let mut interval_results: HashMap<i32, Vec<(f32, f32, f32)>> = HashMap::new();
    
    for (interval, _filename, baseline, gps_quality, combined) in all_results {
        let key = (interval * 100.0) as i32;
        interval_results.entry(key)
            .or_insert_with(Vec::new)
            .push((baseline, gps_quality, combined));
    }
    
    // Convert to final results
    let results: Vec<ComparativeAnalysisResult> = intervals
        .iter()
        .map(|&interval| {
            let key = (interval * 100.0) as i32;
            let accuracies = interval_results.get(&key).cloned().unwrap_or_default();
            
            let baseline_accuracies: Vec<f32> = accuracies.iter().map(|a| a.0).collect();
            let gps_quality_accuracies: Vec<f32> = accuracies.iter().map(|a| a.1).collect();
            let combined_accuracies: Vec<f32> = accuracies.iter().map(|a| a.2).collect();
            
            // Calculate metrics for each approach
            let baseline_metrics = calculate_accuracy_metrics(&baseline_accuracies);
            let gps_quality_metrics = calculate_accuracy_metrics(&gps_quality_accuracies);
            let combined_metrics = calculate_accuracy_metrics(&combined_accuracies);
            
            ComparativeAnalysisResult {
                interval_m: interval,
                // Baseline
                baseline_score_98_102: baseline_metrics.0,
                baseline_score_95_105: baseline_metrics.1,
                baseline_score_90_110: baseline_metrics.2,
                baseline_files_outside_80_120: baseline_metrics.3,
                baseline_weighted_score: baseline_metrics.4,
                baseline_median_accuracy: baseline_metrics.5,
                baseline_worst_accuracy: baseline_metrics.6,
                baseline_total_files: baseline_accuracies.len() as u32,
                // GPS Quality
                gps_quality_score_98_102: gps_quality_metrics.0,
                gps_quality_score_95_105: gps_quality_metrics.1,
                gps_quality_score_90_110: gps_quality_metrics.2,
                gps_quality_files_outside_80_120: gps_quality_metrics.3,
                gps_quality_weighted_score: gps_quality_metrics.4,
                gps_quality_median_accuracy: gps_quality_metrics.5,
                gps_quality_worst_accuracy: gps_quality_metrics.6,
                gps_quality_total_files: gps_quality_accuracies.len() as u32,
                // Combined
                combined_score_98_102: combined_metrics.0,
                combined_score_95_105: combined_metrics.1,
                combined_score_90_110: combined_metrics.2,
                combined_files_outside_80_120: combined_metrics.3,
                combined_weighted_score: combined_metrics.4,
                combined_median_accuracy: combined_metrics.5,
                combined_worst_accuracy: combined_metrics.6,
                combined_total_files: combined_accuracies.len() as u32,
            }
        })
        .collect();
    
    Ok(results)
}

fn calculate_baseline_gain(file_data: &GpxFileData, interval: f32) -> u32 {
    // Use existing distance-based processing
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval as f64);
    elevation_data.get_total_elevation_gain().round() as u32
}

fn calculate_gps_quality_metrics(file_data: &GpxFileData) -> GpsQualityMetrics {
    let n = file_data.elevations.len();
    if n < 2 {
        return GpsQualityMetrics {
            average_point_spacing_m: 0.0,
            elevation_noise_ratio: 1.0,
            sampling_frequency_hz: 0.0,
            elevation_change_consistency: 0.0,
            signal_gaps_count: 0,
            quality_score: 0.0,
        };
    }
    
    // Calculate average point spacing
    let total_distance = file_data.distances.last().unwrap_or(&0.0);
    let average_spacing = total_distance / (n - 1) as f64;
    
    // Calculate sampling frequency
    let total_time = file_data.timestamps.last().unwrap_or(&0.0);
    let avg_sampling_freq = if *total_time > 0.0 {
        (n - 1) as f64 / total_time
    } else {
        0.0
    };
    
    // Calculate elevation noise ratio
    let mut elevation_changes = Vec::new();
    for i in 1..n {
        elevation_changes.push(file_data.elevations[i] - file_data.elevations[i-1]);
    }
    
    // Count sign changes (indicates noise)
    let mut sign_changes = 0;
    for i in 1..elevation_changes.len() {
        if elevation_changes[i] * elevation_changes[i-1] < 0.0 {
            sign_changes += 1;
        }
    }
    let noise_ratio = sign_changes as f64 / elevation_changes.len() as f64;
    
    // Detect signal gaps (>10 seconds between points)
    let mut gaps = 0;
    for i in 1..file_data.timestamps.len() {
        if file_data.timestamps[i] - file_data.timestamps[i-1] > 10.0 {
            gaps += 1;
        }
    }
    
    // Calculate consistency of elevation changes
    let elevation_variance = calculate_variance(&elevation_changes);
    let consistency = 1.0 / (1.0 + elevation_variance.sqrt());
    
    // Calculate overall quality score (0-100)
    let spacing_score = (1.0 - (average_spacing - 10.0).abs() / 50.0).max(0.0) * 25.0;
    let frequency_score = (avg_sampling_freq.min(1.0)) * 25.0;
    let noise_score = (1.0 - noise_ratio) * 25.0;
    let consistency_score = consistency * 25.0;
    
    let quality_score = spacing_score + frequency_score + noise_score + consistency_score;
    
    GpsQualityMetrics {
        average_point_spacing_m: average_spacing,
        elevation_noise_ratio: noise_ratio,
        sampling_frequency_hz: avg_sampling_freq,
        elevation_change_consistency: consistency,
        signal_gaps_count: gaps,
        quality_score,
    }
}

fn calculate_gps_quality_adjusted_gain(
    file_data: &GpxFileData,
    interval: f32,
    gps_metrics: &GpsQualityMetrics
) -> u32 {
    // Start with baseline calculation
    let baseline_gain = calculate_baseline_gain(file_data, interval);
    
    // Apply corrections based on GPS quality
    let _quality_factor = gps_metrics.quality_score / 100.0;
    
    // For poor quality GPS (< 50 score), apply gain recovery
    if gps_metrics.quality_score < 50.0 {
        // Estimate lost elevation due to poor sampling
        let sampling_correction = if gps_metrics.sampling_frequency_hz < 0.5 {
            1.2 // 20% gain recovery for very low sampling
        } else if gps_metrics.sampling_frequency_hz < 1.0 {
            1.1 // 10% gain recovery for low sampling
        } else {
            1.0
        };
        
        // Noise correction - high noise often masks real elevation changes
        let noise_correction = if gps_metrics.elevation_noise_ratio > 0.5 {
            1.15 // 15% gain recovery for very noisy data
        } else if gps_metrics.elevation_noise_ratio > 0.3 {
            1.08 // 8% gain recovery for noisy data
        } else {
            1.0
        };
        
        let corrected_gain = baseline_gain as f64 * sampling_correction * noise_correction;
        corrected_gain.round() as u32
    } else {
        baseline_gain
    }
}

fn calculate_combined_approach_gain(
    file_data: &GpxFileData,
    interval: f32,
    gps_metrics: &GpsQualityMetrics
) -> u32 {
    // First apply statistical outlier removal to elevations
    let cleaned_elevations = remove_statistical_outliers(&file_data.elevations, &file_data.distances);
    
    // Create new file data with cleaned elevations
    let cleaned_file_data = GpxFileData {
        filename: file_data.filename.clone(),
        elevations: cleaned_elevations,
        distances: file_data.distances.clone(),
        timestamps: file_data.timestamps.clone(),
        official_gain: file_data.official_gain,
    };
    
    // Then apply GPS quality-based processing on cleaned data
    calculate_gps_quality_adjusted_gain(&cleaned_file_data, interval, gps_metrics)
}

fn remove_statistical_outliers(elevations: &[f64], distances: &[f64]) -> Vec<f64> {
    if elevations.len() < 10 {
        return elevations.to_vec();
    }
    
    let mut cleaned = elevations.to_vec();
    
    // Calculate gradients
    let mut gradients = Vec::new();
    for i in 1..elevations.len() {
        let dist_diff = distances[i] - distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (elevations[i] - elevations[i-1]) / dist_diff * 100.0;
            gradients.push(gradient);
        }
    }
    
    // Calculate IQR for gradients
    let mut sorted_gradients = gradients.clone();
    sorted_gradients.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let q1_idx = sorted_gradients.len() / 4;
    let q3_idx = (sorted_gradients.len() * 3) / 4;
    let q1 = sorted_gradients[q1_idx];
    let q3 = sorted_gradients[q3_idx];
    let iqr = q3 - q1;
    
    // Define outlier thresholds (more conservative for hills)
    let lower_bound = q1 - 2.0 * iqr;
    let upper_bound = q3 + 2.0 * iqr;
    
    // Smooth outliers
    for i in 1..elevations.len() - 1 {
        if i < gradients.len() {
            let gradient = gradients[i-1];
            
            // If gradient is an outlier, interpolate
            if gradient < lower_bound || gradient > upper_bound {
                // Use linear interpolation from surrounding points
                let prev_valid = find_previous_valid_point(i, &gradients, lower_bound, upper_bound);
                let next_valid = find_next_valid_point(i, &gradients, lower_bound, upper_bound);
                
                if let (Some(prev), Some(next)) = (prev_valid, next_valid) {
                    // Interpolate elevation
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
    Some(0) // Default to start
}

fn find_next_valid_point(
    start: usize,
    gradients: &[f64],
    lower_bound: f64,
    upper_bound: f64
) -> Option<usize> {
    for i in start..gradients.len() {
        if gradients[i] >= lower_bound && gradients[i] <= upper_bound {
            return Some(i + 1); // +1 because gradients array is offset
        }
    }
    Some(gradients.len()) // Default to end
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

fn write_comparative_results(
    results: &[ComparativeAnalysisResult],
    output_path: PathBuf
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write headers
    wtr.write_record(&[
        "Interval (m)",
        // Baseline
        "Baseline Score",
        "Baseline 98-102%",
        "Baseline 95-105%",
        "Baseline 90-110%",
        "Baseline Outside 80-120%",
        "Baseline Median %",
        "Baseline Worst %",
        // GPS Quality
        "GPS Quality Score",
        "GPS Quality 98-102%",
        "GPS Quality 95-105%",
        "GPS Quality 90-110%",
        "GPS Quality Outside 80-120%",
        "GPS Quality Median %",
        "GPS Quality Worst %",
        // Combined
        "Combined Score",
        "Combined 98-102%",
        "Combined 95-105%",
        "Combined 90-110%",
        "Combined Outside 80-120%",
        "Combined Median %",
        "Combined Worst %",
        // Improvements
        "GPS vs Baseline Score Δ",
        "Combined vs Baseline Score Δ",
    ])?;
    
    // Sort by combined score (best approach)
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.combined_weighted_score.partial_cmp(&a.combined_weighted_score).unwrap());
    
    for result in sorted_results {
        wtr.write_record(&[
            format!("{:.2}", result.interval_m),
            // Baseline
            format!("{:.1}", result.baseline_weighted_score),
            result.baseline_score_98_102.to_string(),
            result.baseline_score_95_105.to_string(),
            result.baseline_score_90_110.to_string(),
            result.baseline_files_outside_80_120.to_string(),
            format!("{:.1}", result.baseline_median_accuracy),
            format!("{:.1}", result.baseline_worst_accuracy),
            // GPS Quality
            format!("{:.1}", result.gps_quality_weighted_score),
            result.gps_quality_score_98_102.to_string(),
            result.gps_quality_score_95_105.to_string(),
            result.gps_quality_score_90_110.to_string(),
            result.gps_quality_files_outside_80_120.to_string(),
            format!("{:.1}", result.gps_quality_median_accuracy),
            format!("{:.1}", result.gps_quality_worst_accuracy),
            // Combined
            format!("{:.1}", result.combined_weighted_score),
            result.combined_score_98_102.to_string(),
            result.combined_score_95_105.to_string(),
            result.combined_score_90_110.to_string(),
            result.combined_files_outside_80_120.to_string(),
            format!("{:.1}", result.combined_median_accuracy),
            format!("{:.1}", result.combined_worst_accuracy),
            // Improvements
            format!("{:+.1}", result.gps_quality_weighted_score - result.baseline_weighted_score),
            format!("{:+.1}", result.combined_weighted_score - result.baseline_weighted_score),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_comparative_summary(results: &[ComparativeAnalysisResult]) {
    println!("\n📊 COMPARATIVE ANALYSIS SUMMARY");
    println!("===============================");
    
    // Find best interval for each approach
    let best_baseline = results.iter()
        .max_by(|a, b| a.baseline_weighted_score.partial_cmp(&b.baseline_weighted_score).unwrap())
        .unwrap();
    
    let best_gps_quality = results.iter()
        .max_by(|a, b| a.gps_quality_weighted_score.partial_cmp(&b.gps_quality_weighted_score).unwrap())
        .unwrap();
    
    let best_combined = results.iter()
        .max_by(|a, b| a.combined_weighted_score.partial_cmp(&b.combined_weighted_score).unwrap())
        .unwrap();
    
    println!("\n🏆 OPTIMAL INTERVALS:");
    println!("Baseline:        {:.2}m (Score: {:.1})", best_baseline.interval_m, best_baseline.baseline_weighted_score);
    println!("GPS Quality:     {:.2}m (Score: {:.1})", best_gps_quality.interval_m, best_gps_quality.gps_quality_weighted_score);
    println!("Combined:        {:.2}m (Score: {:.1})", best_combined.interval_m, best_combined.combined_weighted_score);
    
    println!("\n📈 IMPROVEMENTS AT OPTIMAL INTERVALS:");
    
    // Compare at the combined optimal interval
    let comparison = results.iter()
        .find(|r| (r.interval_m - best_combined.interval_m).abs() < 0.01)
        .unwrap();
    
    println!("\nAt {:.2}m interval:", comparison.interval_m);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Metric              | Baseline | GPS Quality | Combined");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Files in 98-102%    |    {:3}   |     {:3}     |    {:3}", 
             comparison.baseline_score_98_102,
             comparison.gps_quality_score_98_102,
             comparison.combined_score_98_102);
    println!("Files in 90-110%    |    {:3}   |     {:3}     |    {:3}", 
             comparison.baseline_score_90_110,
             comparison.gps_quality_score_90_110,
             comparison.combined_score_90_110);
    println!("Outside 80-120%     |    {:3}   |     {:3}     |    {:3}", 
             comparison.baseline_files_outside_80_120,
             comparison.gps_quality_files_outside_80_120,
             comparison.combined_files_outside_80_120);
    println!("Median Accuracy %   |  {:5.1}   |   {:5.1}    |  {:5.1}", 
             comparison.baseline_median_accuracy,
             comparison.gps_quality_median_accuracy,
             comparison.combined_median_accuracy);
    println!("Worst Accuracy %    |  {:5.1}   |   {:5.1}    |  {:5.1}", 
             comparison.baseline_worst_accuracy,
             comparison.gps_quality_worst_accuracy,
             comparison.combined_worst_accuracy);
    
    // Calculate improvements
    let gps_improvement_98_102 = comparison.gps_quality_score_98_102 as i32 - comparison.baseline_score_98_102 as i32;
    let combined_improvement_98_102 = comparison.combined_score_98_102 as i32 - comparison.baseline_score_98_102 as i32;
    
    let gps_improvement_outliers = comparison.baseline_files_outside_80_120 as i32 - comparison.gps_quality_files_outside_80_120 as i32;
    let combined_improvement_outliers = comparison.baseline_files_outside_80_120 as i32 - comparison.combined_files_outside_80_120 as i32;
    
    println!("\n🎯 KEY IMPROVEMENTS:");
    println!("GPS Quality Processing:");
    println!("  • {:+} files moved into 98-102% band", gps_improvement_98_102);
    println!("  • {:+} files moved out of outlier range", gps_improvement_outliers);
    println!("  • Score improvement: {:+.1}", comparison.gps_quality_weighted_score - comparison.baseline_weighted_score);
    
    println!("\nCombined Approach:");
    println!("  • {:+} files moved into 98-102% band", combined_improvement_98_102);
    println!("  • {:+} files moved out of outlier range", combined_improvement_outliers);
    println!("  • Score improvement: {:+.1}", comparison.combined_weighted_score - comparison.baseline_weighted_score);
    
    // Success rate analysis
    let baseline_success_rate = comparison.baseline_score_90_110 as f32 / comparison.baseline_total_files as f32 * 100.0;
    let combined_success_rate = comparison.combined_score_90_110 as f32 / comparison.combined_total_files as f32 * 100.0;
    
    println!("\n✅ SUCCESS RATE (90-110% accuracy):");
    println!("Baseline:  {:.1}%", baseline_success_rate);
    println!("Combined:  {:.1}% ({:+.1}% improvement)", combined_success_rate, combined_success_rate - baseline_success_rate);
}