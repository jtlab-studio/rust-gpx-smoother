/// GARMIN-LIKE PROCESSOR WITH FOCUSED INTERVAL RANGE (7-25m)
/// 
/// Implements Garmin Connect-style elevation processing:
/// - Minimal smoothing (3-5 point moving average)
/// - Distance-based resampling from 7m to 25m in 0.25m increments
/// - No aggressive filtering or deadbands
/// - Preserves original data characteristics
/// - Comprehensive analysis for each interval

use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use geo::{HaversineDistance, point};
use walkdir::WalkDir;
use crate::tolerant_gpx_reader::read_gpx_tolerantly;

// Garmin-like processing parameters
const GARMIN_SMOOTHING_WINDOW: usize = 5;  // Light smoothing only
const MAX_REALISTIC_GRADIENT: f64 = 35.0;  // 35% max gradient
const SPIKE_THRESHOLD: f64 = 10.0;         // 10m sudden change is suspicious

// Updated interval range: 7m to 25m in 0.25m increments
const MIN_INTERVAL: f64 = 7.0;
const MAX_INTERVAL: f64 = 25.0;
const INTERVAL_STEP: f64 = 0.25;

#[derive(Debug, Clone)]
pub struct IntervalResult {
    interval_m: f64,
    gain_m: f64,
    loss_m: f64,
    ratio: f64,
    accuracy_percent: f64,
}

#[derive(Debug, Clone)]
pub struct GarminLikeResult {
    filename: String,
    total_points: u32,
    total_distance_km: f64,
    
    // Raw data analysis
    raw_elevation_gain_m: f64,
    raw_elevation_loss_m: f64,
    raw_gain_loss_ratio: f64,
    raw_accuracy_percent: f64,
    
    // Official data
    official_elevation_gain_m: u32,
    
    // Results for each interval (stored as HashMap for flexibility)
    interval_results: HashMap<String, IntervalResult>,
    
    // Best interval analysis
    best_interval_m: f64,
    best_accuracy_percent: f64,
    
    // Quality metrics
    noise_level: String,
    gradient_issues: u32,
    data_quality_score: u32,
}

#[derive(Debug, Serialize)]
pub struct GarminAnalysisSummary {
    total_files_processed: u32,
    files_with_official_data: u32,
    
    // Average accuracy for each interval (stored as HashMap)
    avg_accuracy_by_interval: HashMap<String, f64>,
    
    // Best interval distribution
    best_interval_distribution: HashMap<String, u32>,
    
    // Files within accuracy thresholds for each interval
    files_within_10_percent_by_interval: HashMap<String, u32>,
    files_within_15_percent_by_interval: HashMap<String, u32>,
    
    // Most common best interval
    most_common_best_interval: String,
    most_common_best_count: u32,
    
    // Summary statistics for the new range
    total_intervals_tested: u32,
    interval_range: String,
}

pub fn run_garmin_like_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏃 GARMIN-LIKE ELEVATION PROCESSING ANALYSIS");
    println!("===========================================");
    println!("Testing Garmin Connect-style processing approach:");
    println!("• Minimal smoothing (5-point moving average)");
    println!("• Distance-based resampling: 7m to 25m in 0.25m increments");
    
    // Calculate total intervals
    let total_intervals = ((MAX_INTERVAL - MIN_INTERVAL) / INTERVAL_STEP + 1.0) as u32;
    println!("• Total intervals tested: {}", total_intervals);
    
    println!("• No aggressive filtering or deadbands");
    println!("• Preserve original data characteristics");
    println!("• Compare with official elevation data");
    println!("• Track files within ±10% and ±15% accuracy\n");
    
    // Load official elevation data
    println!("📂 Loading official elevation data...");
    let official_data = crate::load_official_elevation_data()?;
    println!("✅ Loaded {} official elevation records", official_data.len());
    
    // Collect GPX files
    println!("📂 Scanning for GPX files...");
    let gpx_files = collect_gpx_files(gpx_folder)?;
    println!("🔍 Found {} GPX files to process\n", gpx_files.len());
    
    // Process each file
    let mut results = Vec::new();
    let mut errors = 0;
    
    for (index, gpx_path) in gpx_files.iter().enumerate() {
        let filename = gpx_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        println!("🔄 Processing {}/{}: {}", index + 1, gpx_files.len(), filename);
        
        match process_file_garmin_style(gpx_path, &filename, &official_data) {
            Ok(result) => {
                // Print summary for this file
                println!("   ✅ Success:");
                println!("      Raw: {:.1}m gain (ratio {:.2}, accuracy {:.1}%)", 
                         result.raw_elevation_gain_m, result.raw_gain_loss_ratio, result.raw_accuracy_percent);
                println!("      Best interval: {:.2}m ({:.1}% accuracy)", 
                         result.best_interval_m, result.best_accuracy_percent);
                
                results.push(result);
            }
            Err(e) => {
                println!("   ❌ Error: {}", e);
                errors += 1;
            }
        }
    }
    
    println!("\n✅ Processed {} files successfully, {} errors", results.len(), errors);
    
    // Calculate summary statistics
    let summary = calculate_summary(&results);
    
    // Write results to CSV
    let output_path = Path::new(gpx_folder).join("garmin_analysis_7-25m_detailed.csv");
    write_results_csv(&results, &output_path)?;
    
    let summary_path = Path::new(gpx_folder).join("garmin_analysis_7-25m_summary.csv");
    write_summary_csv(&summary, &summary_path)?;
    
    // Print analysis
    print_detailed_analysis(&results, &summary);
    
    println!("\n📁 Results saved to:");
    println!("   • {}", output_path.display());
    println!("   • {}", summary_path.display());
    
    Ok(())
}

fn collect_gpx_files(gpx_folder: &str) -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error>> {
    let mut gpx_files = Vec::new();
    
    for entry in WalkDir::new(gpx_folder).max_depth(1) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    gpx_files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    
    gpx_files.sort();
    Ok(gpx_files)
}

fn process_file_garmin_style(
    gpx_path: &Path,
    filename: &str,
    official_data: &HashMap<String, u32>
) -> Result<GarminLikeResult, Box<dyn std::error::Error>> {
    // Read GPX file
    let gpx = read_gpx_tolerantly(gpx_path)?;
    
    // Extract coordinates with elevation
    let mut coords: Vec<(f64, f64, f64)> = Vec::new();
    
    for track in &gpx.tracks {
        for segment in &track.segments {
            for point in &segment.points {
                if let Some(elevation) = point.elevation {
                    let lat = point.point().y();
                    let lon = point.point().x();
                    coords.push((lat, lon, elevation));
                }
            }
        }
    }
    
    if coords.is_empty() {
        return Err("No elevation data found".into());
    }
    
    // Calculate distances
    let mut distances = vec![0.0];
    for i in 1..coords.len() {
        let a = point!(x: coords[i-1].1, y: coords[i-1].0);
        let b = point!(x: coords[i].1, y: coords[i].0);
        let dist = a.haversine_distance(&b);
        distances.push(distances[i-1] + dist);
    }
    
    let elevations: Vec<f64> = coords.iter().map(|c| c.2).collect();
    let total_distance_km = distances.last().unwrap_or(&0.0) / 1000.0;
    
    // Calculate raw metrics
    let (raw_gain, raw_loss) = calculate_gain_loss(&elevations);
    let raw_ratio = if raw_loss > 0.0 { raw_gain / raw_loss } else { f64::INFINITY };
    
    // Get official data
    let clean_filename = filename
        .replace("_Processed.gpx", ".gpx")
        .replace("_Cleaned.gpx", ".gpx")
        .replace("_Fixed.gpx", ".gpx")
        .replace("cleaned_", "")
        .to_lowercase();
    
    let official_gain = official_data
        .get(&clean_filename)
        .copied()
        .unwrap_or(0);
    
    let raw_accuracy = if official_gain > 0 {
        (raw_gain / official_gain as f64) * 100.0
    } else {
        0.0
    };
    
    // Process with all intervals in the new range
    let mut interval_results = HashMap::new();
    let mut best_interval = IntervalResult {
        interval_m: 0.0,
        gain_m: 0.0,
        loss_m: 0.0,
        ratio: 0.0,
        accuracy_percent: 0.0,
    };
    let mut best_accuracy_diff = f64::INFINITY;
    
    // Generate intervals from 7m to 25m in 0.25m increments
    let mut interval = MIN_INTERVAL;
    while interval <= MAX_INTERVAL + 0.001 { // Add small epsilon to handle floating point precision
        let processed = process_with_garmin_method(&elevations, &distances, interval);
        
        let accuracy = if official_gain > 0 {
            (processed.0 / official_gain as f64) * 100.0
        } else {
            0.0
        };
        
        let result = IntervalResult {
            interval_m: interval,
            gain_m: processed.0,
            loss_m: processed.1,
            ratio: processed.2,
            accuracy_percent: accuracy,
        };
        
        // Check if this is the best interval
        if official_gain > 0 {
            let accuracy_diff = (accuracy - 100.0).abs();
            if accuracy_diff < best_accuracy_diff {
                best_accuracy_diff = accuracy_diff;
                best_interval = result.clone();
            }
        }
        
        interval_results.insert(format!("{:.2}m", interval), result);
        
        interval += INTERVAL_STEP;
    }
    
    // Calculate quality metrics
    let (noise_level, gradient_issues) = analyze_data_quality(&elevations, &distances);
    let data_quality_score = calculate_quality_score(noise_level.as_str(), gradient_issues, raw_ratio);
    
    // Build result struct
    Ok(GarminLikeResult {
        filename: filename.to_string(),
        total_points: coords.len() as u32,
        total_distance_km,
        raw_elevation_gain_m: raw_gain,
        raw_elevation_loss_m: raw_loss,
        raw_gain_loss_ratio: raw_ratio,
        raw_accuracy_percent: raw_accuracy,
        official_elevation_gain_m: official_gain,
        interval_results,
        best_interval_m: best_interval.interval_m,
        best_accuracy_percent: best_interval.accuracy_percent,
        noise_level,
        gradient_issues,
        data_quality_score,
    })
}

fn calculate_gain_loss(elevations: &[f64]) -> (f64, f64) {
    if elevations.len() < 2 {
        return (0.0, 0.0);
    }
    
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for window in elevations.windows(2) {
        let change = window[1] - window[0];
        if change > 0.0 {
            gain += change;
        } else if change < 0.0 {
            loss += -change;
        }
    }
    
    (gain, loss)
}

fn process_with_garmin_method(
    elevations: &[f64],
    distances: &[f64],
    interval_meters: f64
) -> (f64, f64, f64) {
    // Step 1: Resample to uniform distance intervals
    let resampled = resample_to_distance_interval(elevations, distances, interval_meters);
    
    if resampled.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    
    // Step 2: Apply light smoothing (Garmin-style)
    let smoothed = apply_light_smoothing(&resampled, GARMIN_SMOOTHING_WINDOW);
    
    // Step 3: Remove obvious spikes (but keep legitimate steep sections)
    let cleaned = remove_obvious_spikes(&smoothed);
    
    // Step 4: Calculate gain/loss
    let (gain, loss) = calculate_gain_loss(&cleaned);
    let ratio = if loss > 0.0 { gain / loss } else { f64::INFINITY };
    
    (gain, loss, ratio)
}

fn resample_to_distance_interval(
    elevations: &[f64],
    distances: &[f64],
    interval_meters: f64
) -> Vec<f64> {
    if elevations.is_empty() || distances.is_empty() {
        return vec![];
    }
    
    let total_distance = distances.last().unwrap();
    let num_points = (total_distance / interval_meters).ceil() as usize + 1;
    
    // Prevent excessive memory usage
    if num_points > 100_000 {
        return vec![];
    }
    
    let mut resampled = Vec::with_capacity(num_points);
    
    for i in 0..num_points {
        let target_distance = i as f64 * interval_meters;
        if target_distance > *total_distance {
            break;
        }
        
        // Find interpolation points
        let elevation = interpolate_elevation_at_distance(
            elevations,
            distances,
            target_distance
        );
        resampled.push(elevation);
    }
    
    resampled
}

fn interpolate_elevation_at_distance(
    elevations: &[f64],
    distances: &[f64],
    target_distance: f64
) -> f64 {
    if target_distance <= 0.0 {
        return elevations.first().copied().unwrap_or(0.0);
    }
    
    if target_distance >= *distances.last().unwrap() {
        return elevations.last().copied().unwrap_or(0.0);
    }
    
    // Binary search would be more efficient, but linear search is fine for now
    for i in 1..distances.len() {
        if distances[i] >= target_distance {
            let d1 = distances[i - 1];
            let d2 = distances[i];
            let e1 = elevations[i - 1];
            let e2 = elevations[i];
            
            if (d2 - d1).abs() < 1e-10 {
                return e1;
            }
            
            let t = (target_distance - d1) / (d2 - d1);
            return e1 + t * (e2 - e1);
        }
    }
    
    elevations.last().copied().unwrap_or(0.0)
}

fn apply_light_smoothing(data: &[f64], window: usize) -> Vec<f64> {
    if data.is_empty() || window == 0 {
        return data.to_vec();
    }
    
    let mut smoothed = Vec::with_capacity(data.len());
    let half_window = window / 2;
    
    for i in 0..data.len() {
        let start = if i >= half_window { i - half_window } else { 0 };
        let end = std::cmp::min(i + half_window + 1, data.len());
        
        let sum: f64 = data[start..end].iter().sum();
        let count = end - start;
        
        smoothed.push(sum / count as f64);
    }
    
    smoothed
}

fn remove_obvious_spikes(elevations: &[f64]) -> Vec<f64> {
    if elevations.len() < 3 {
        return elevations.to_vec();
    }
    
    let mut cleaned = elevations.to_vec();
    
    // Only remove really obvious spikes (single point jumps)
    for i in 1..(cleaned.len() - 1) {
        let prev = cleaned[i - 1];
        let curr = cleaned[i];
        let next = cleaned[i + 1];
        
        // Check if this is a spike (up then down or down then up)
        let jump_up = curr - prev;
        let jump_down = next - curr;
        
        if jump_up.abs() > SPIKE_THRESHOLD && jump_down.abs() > SPIKE_THRESHOLD {
            if jump_up.signum() != jump_down.signum() {
                // This is likely a spike - interpolate
                cleaned[i] = (prev + next) / 2.0;
            }
        }
    }
    
    cleaned
}

fn analyze_data_quality(elevations: &[f64], distances: &[f64]) -> (String, u32) {
    // Calculate elevation changes
    let mut changes = Vec::new();
    for i in 1..elevations.len() {
        changes.push(elevations[i] - elevations[i - 1]);
    }
    
    // Calculate noise level (standard deviation of changes)
    let mean_change = changes.iter().sum::<f64>() / changes.len() as f64;
    let variance = changes.iter()
        .map(|&x| (x - mean_change).powi(2))
        .sum::<f64>() / changes.len() as f64;
    let std_dev = variance.sqrt();
    
    let noise_level = if std_dev < 1.0 {
        "Low"
    } else if std_dev < 3.0 {
        "Medium"
    } else {
        "High"
    }.to_string();
    
    // Count gradient issues
    let mut gradient_issues = 0;
    for i in 1..elevations.len() {
        if distances[i] > distances[i - 1] {
            let gradient = ((elevations[i] - elevations[i - 1]) / 
                          (distances[i] - distances[i - 1])) * 100.0;
            
            if gradient.abs() > MAX_REALISTIC_GRADIENT {
                gradient_issues += 1;
            }
        }
    }
    
    (noise_level, gradient_issues)
}

fn calculate_quality_score(noise_level: &str, gradient_issues: u32, ratio: f64) -> u32 {
    let mut score: u32 = 100;
    
    // Deduct for noise
    match noise_level {
        "Medium" => score = score.saturating_sub(10),
        "High" => score = score.saturating_sub(25),
        _ => {}
    }
    
    // Deduct for gradient issues
    score = score.saturating_sub(gradient_issues.min(30));
    
    // Deduct for bad gain/loss ratio
    if ratio > 1.2 || ratio < 0.8 {
        score = score.saturating_sub(15);
    }
    
    score
}

fn calculate_summary(results: &[GarminLikeResult]) -> GarminAnalysisSummary {
    let total_files = results.len() as u32;
    let files_with_official: Vec<_> = results.iter()
        .filter(|r| r.official_elevation_gain_m > 0)
        .collect();
    
    // Initialize maps
    let mut avg_accuracy_by_interval = HashMap::new();
    let mut best_interval_distribution = HashMap::new();
    let mut files_within_10_percent = HashMap::new();
    let mut files_within_15_percent = HashMap::new();
    
    // Calculate averages for each interval
    let mut interval = MIN_INTERVAL;
    while interval <= MAX_INTERVAL + 0.001 {
        let interval_key = format!("{:.2}m", interval);
        
        if !files_with_official.is_empty() {
            let mut sum = 0.0;
            let mut count = 0;
            
            for result in &files_with_official {
                if let Some(interval_result) = result.interval_results.get(&interval_key) {
                    sum += interval_result.accuracy_percent;
                    count += 1;
                    
                    // Check accuracy thresholds
                    if interval_result.accuracy_percent >= 90.0 && interval_result.accuracy_percent <= 110.0 {
                        *files_within_10_percent.entry(interval_key.clone()).or_insert(0) += 1;
                    }
                    if interval_result.accuracy_percent >= 85.0 && interval_result.accuracy_percent <= 115.0 {
                        *files_within_15_percent.entry(interval_key.clone()).or_insert(0) += 1;
                    }
                }
            }
            
            if count > 0 {
                avg_accuracy_by_interval.insert(interval_key, sum / count as f64);
            }
        }
        
        interval += INTERVAL_STEP;
    }
    
    // Track best intervals
    for result in &files_with_official {
        let best_key = format!("{:.2}m", result.best_interval_m);
        *best_interval_distribution.entry(best_key).or_insert(0) += 1;
    }
    
    // Find most common best interval
    let (most_common_interval, most_common_count) = best_interval_distribution
        .iter()
        .max_by_key(|&(_, count)| count)
        .map(|(k, v)| (k.clone(), *v))
        .unwrap_or(("None".to_string(), 0));
    
    // Calculate total intervals tested
    let total_intervals_tested = ((MAX_INTERVAL - MIN_INTERVAL) / INTERVAL_STEP + 1.0) as u32;
    
    GarminAnalysisSummary {
        total_files_processed: total_files,
        files_with_official_data: files_with_official.len() as u32,
        avg_accuracy_by_interval,
        best_interval_distribution,
        files_within_10_percent_by_interval: files_within_10_percent,
        files_within_15_percent_by_interval: files_within_15_percent,
        most_common_best_interval: most_common_interval,
        most_common_best_count: most_common_count,
        total_intervals_tested,
        interval_range: format!("{}m to {}m in {}m increments", MIN_INTERVAL, MAX_INTERVAL, INTERVAL_STEP),
    }
}

fn write_results_csv(
    results: &[GarminLikeResult],
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Build header
    let mut header = vec![
        "filename".to_string(),
        "total_points".to_string(),
        "total_distance_km".to_string(),
        "raw_elevation_gain_m".to_string(),
        "raw_elevation_loss_m".to_string(),
        "raw_gain_loss_ratio".to_string(),
        "raw_accuracy_percent".to_string(),
        "official_elevation_gain_m".to_string(),
    ];
    
    // Add columns for each interval
    let mut interval = MIN_INTERVAL;
    while interval <= MAX_INTERVAL + 0.001 {
        let prefix = format!("{:.2}", interval);
        header.push(format!("gain_{}m", prefix));
        header.push(format!("loss_{}m", prefix));
        header.push(format!("ratio_{}m", prefix));
        header.push(format!("accuracy_{}m", prefix));
        interval += INTERVAL_STEP;
    }
    
    // Add best interval and quality columns
    header.push("best_interval_m".to_string());
    header.push("best_accuracy_percent".to_string());
    header.push("noise_level".to_string());
    header.push("gradient_issues".to_string());
    header.push("data_quality_score".to_string());
    
    wtr.write_record(&header)?;
    
    // Write data rows
    for result in results {
        let mut row = vec![
            result.filename.clone(),
            result.total_points.to_string(),
            format!("{:.2}", result.total_distance_km),
            format!("{:.1}", result.raw_elevation_gain_m),
            format!("{:.1}", result.raw_elevation_loss_m),
            format!("{:.3}", result.raw_gain_loss_ratio),
            format!("{:.1}", result.raw_accuracy_percent),
            result.official_elevation_gain_m.to_string(),
        ];
        
        // Add interval data
        let mut interval = MIN_INTERVAL;
        while interval <= MAX_INTERVAL + 0.001 {
            let interval_key = format!("{:.2}m", interval);
            if let Some(interval_result) = result.interval_results.get(&interval_key) {
                row.push(format!("{:.1}", interval_result.gain_m));
                row.push(format!("{:.1}", interval_result.loss_m));
                row.push(format!("{:.3}", interval_result.ratio));
                row.push(format!("{:.1}", interval_result.accuracy_percent));
            } else {
                row.push("0.0".to_string());
                row.push("0.0".to_string());
                row.push("0.0".to_string());
                row.push("0.0".to_string());
            }
            interval += INTERVAL_STEP;
        }
        
        // Add best interval and quality data
        row.push(format!("{:.2}", result.best_interval_m));
        row.push(format!("{:.1}", result.best_accuracy_percent));
        row.push(result.noise_level.clone());
        row.push(result.gradient_issues.to_string());
        row.push(result.data_quality_score.to_string());
        
        wtr.write_record(&row)?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn write_summary_csv(
    summary: &GarminAnalysisSummary,
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write summary statistics - use 3 columns consistently
    wtr.write_record(&["Metric", "Value", "Notes"])?;
    wtr.write_record(&["Total Files Processed", &summary.total_files_processed.to_string(), ""])?;
    wtr.write_record(&["Files with Official Data", &summary.files_with_official_data.to_string(), ""])?;
    wtr.write_record(&["Total Intervals Tested", &summary.total_intervals_tested.to_string(), ""])?;
    wtr.write_record(&["Interval Range", &summary.interval_range, ""])?;
    
    // Average accuracies
    wtr.write_record(&["", "", ""])?; // Empty row
    wtr.write_record(&["Average Accuracies by Interval", "Accuracy %", ""])?;
    let mut interval = MIN_INTERVAL;
    while interval <= MAX_INTERVAL + 0.001 {
        let interval_key = format!("{:.2}m", interval);
        let avg = summary.avg_accuracy_by_interval
            .get(&interval_key)
            .unwrap_or(&0.0);
        wtr.write_record(&[&interval_key, &format!("{:.2}%", avg), ""])?;
        interval += INTERVAL_STEP;
    }
    
    // Best interval distribution
    wtr.write_record(&["", "", ""])?;
    wtr.write_record(&["Best Interval Distribution", "Count", ""])?;
    let mut sorted_intervals: Vec<_> = summary.best_interval_distribution.iter().collect();
    sorted_intervals.sort_by(|a, b| {
        let a_num: f64 = a.0.trim_end_matches('m').parse().unwrap_or(0.0);
        let b_num: f64 = b.0.trim_end_matches('m').parse().unwrap_or(0.0);
        a_num.partial_cmp(&b_num).unwrap()
    });
    for (interval, count) in sorted_intervals {
        wtr.write_record(&[interval, &count.to_string(), ""])?;
    }
    
    // Files within ±10% accuracy
    wtr.write_record(&["", "", ""])?; // Empty row with 3 columns
    wtr.write_record(&["Files Within ±10% Accuracy", "Count", "Percentage"])?;
    interval = MIN_INTERVAL;
    while interval <= MAX_INTERVAL + 0.001 {
        let interval_key = format!("{:.2}m", interval);
        let count = summary.files_within_10_percent_by_interval
            .get(&interval_key)
            .unwrap_or(&0);
        let percentage = if summary.files_with_official_data > 0 {
            (*count as f64 / summary.files_with_official_data as f64) * 100.0
        } else {
            0.0
        };
        wtr.write_record(&[&interval_key, &count.to_string(), &format!("{:.1}%", percentage)])?;
        interval += INTERVAL_STEP;
    }
    
    // Files within ±15% accuracy
    wtr.write_record(&["", "", ""])?; // Empty row with 3 columns
    wtr.write_record(&["Files Within ±15% Accuracy", "Count", "Percentage"])?;
    interval = MIN_INTERVAL;
    while interval <= MAX_INTERVAL + 0.001 {
        let interval_key = format!("{:.2}m", interval);
        let count = summary.files_within_15_percent_by_interval
            .get(&interval_key)
            .unwrap_or(&0);
        let percentage = if summary.files_with_official_data > 0 {
            (*count as f64 / summary.files_with_official_data as f64) * 100.0
        } else {
            0.0
        };
        wtr.write_record(&[&interval_key, &count.to_string(), &format!("{:.1}%", percentage)])?;
        interval += INTERVAL_STEP;
    }
    
    wtr.write_record(&["", "", ""])?; // Empty row with 3 columns
    wtr.write_record(&["Most Common Best Interval", &summary.most_common_best_interval, ""])?;
    wtr.write_record(&["Files with this Best Interval", &summary.most_common_best_count.to_string(), ""])?;
    
    wtr.flush()?;
    Ok(())
}

fn print_detailed_analysis(results: &[GarminLikeResult], summary: &GarminAnalysisSummary) {
    println!("\n📊 GARMIN-LIKE PROCESSING RESULTS (7-25m in 0.25m increments)");
    println!("==============================================================");
    
    println!("\n📈 OVERALL STATISTICS:");
    println!("• Total files processed: {}", summary.total_files_processed);
    println!("• Files with official data: {}", summary.files_with_official_data);
    println!("• Total intervals tested: {}", summary.total_intervals_tested);
    println!("• Interval range: {}", summary.interval_range);
    
    if summary.files_with_official_data > 0 {
        println!("\n🎯 BEST AVERAGE ACCURACIES:");
        let mut accuracies: Vec<_> = summary.avg_accuracy_by_interval.iter().collect();
        accuracies.sort_by(|a, b| {
            let a_diff = (a.1 - 100.0).abs();
            let b_diff = (b.1 - 100.0).abs();
            a_diff.partial_cmp(&b_diff).unwrap()
        });
        
        // Show top 10 intervals
        for (interval, acc) in accuracies.iter().take(10) {
            println!("• {}: {:.1}%", interval, acc);
        }
        
        if let Some((best_interval, best_acc)) = accuracies.first() {
            println!("\n🏆 BEST INTERVAL: {} with {:.1}% average accuracy", best_interval, best_acc);
        }
        
        println!("\n🏆 MOST COMMON BEST INTERVALS:");
        let mut sorted_best: Vec<_> = summary.best_interval_distribution.iter().collect();
        sorted_best.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
        
        for (interval, count) in sorted_best.iter().take(10) {
            let percentage = (**count as f64 / summary.files_with_official_data as f64) * 100.0;
            println!("• {}: {} files ({:.1}%)", interval, count, percentage);
        }
        
        println!("\n✅ BEST PERFORMING INTERVALS (±10% accuracy):");
        let mut within_10: Vec<_> = summary.files_within_10_percent_by_interval.iter().collect();
        within_10.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
        
        for (interval, count) in within_10.iter().take(10) {
            let percentage = (**count as f64 / summary.files_with_official_data as f64) * 100.0;
            println!("• {}: {}/{} ({:.1}%)", 
                     interval, count, summary.files_with_official_data, percentage);
        }
        
        println!("\n🎯 EXCELLENT PERFORMING INTERVALS (±15% accuracy):");
        let mut within_15: Vec<_> = summary.files_within_15_percent_by_interval.iter().collect();
        within_15.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
        
        for (interval, count) in within_15.iter().take(10) {
            let percentage = (**count as f64 / summary.files_with_official_data as f64) * 100.0;
            println!("• {}: {}/{} ({:.1}%)", 
                     interval, count, summary.files_with_official_data, percentage);
        }
        
        println!("\n🌟 TOP PERFORMING FILES:");
        let mut best_files: Vec<_> = results.iter()
            .filter(|r| r.official_elevation_gain_m > 0)
            .collect();
        
        best_files.sort_by(|a, b| {
            let a_error = (a.best_accuracy_percent - 100.0).abs();
            let b_error = (b.best_accuracy_percent - 100.0).abs();
            a_error.partial_cmp(&b_error).unwrap()
        });
        
        for (i, result) in best_files.iter().take(5).enumerate() {
            println!("\n{}. {} (Official: {}m)", i + 1, result.filename, result.official_elevation_gain_m);
            println!("   Best: {:.2}m interval ({:.1}% accuracy)", 
                     result.best_interval_m, result.best_accuracy_percent);
            println!("   Quality: {} noise, {} gradient issues", 
                     result.noise_level, result.gradient_issues);
        }
        
        println!("\n💡 KEY INSIGHTS:");
        println!("• Most common best interval: {} ({} files)", 
                 summary.most_common_best_interval, summary.most_common_best_count);
        
        // Find the best performing interval overall
        if let Some((best_overall, best_count)) = within_10.first() {
            let best_percentage = (**best_count as f64 / summary.files_with_official_data as f64) * 100.0;
            println!("• Best overall interval for ±10% accuracy: {} ({:.1}% of files)", 
                     best_overall, best_percentage);
        }
        
        println!("\n🔍 FOCUSED RANGE ANALYSIS (7-25m):");
        println!("• Focused on the most promising interval range");
        println!("• Fine-grained 0.25m increments for precision");
        println!("• {} intervals tested vs previous 43 intervals", summary.total_intervals_tested);
        println!("• Better resolution in the optimal range");
        println!("• Tracks both ±10% and ±15% accuracy thresholds");
    }
}