/// GARMIN-LIKE PROCESSOR
/// 
/// Implements Garmin Connect-style elevation processing:
/// - Minimal smoothing (3-5 point moving average)
/// - Distance-based resampling
/// - No aggressive filtering or deadbands
/// - Preserves original data characteristics
/// - Tests 10m, 25m, and 50m intervals

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

#[derive(Debug, Serialize, Clone)]
pub struct GarminLikeResult {
    filename: String,
    total_points: u32,
    total_distance_km: f64,
    
    // Raw data analysis
    raw_elevation_gain_m: f64,
    raw_elevation_loss_m: f64,
    raw_gain_loss_ratio: f64,
    
    // Results for each interval
    gain_10m: f64,
    loss_10m: f64,
    ratio_10m: f64,
    
    gain_25m: f64,
    loss_25m: f64,
    ratio_25m: f64,
    
    gain_50m: f64,
    loss_50m: f64,
    ratio_50m: f64,
    
    // Accuracy vs official data
    official_elevation_gain_m: u32,
    accuracy_raw: f64,
    accuracy_10m: f64,
    accuracy_25m: f64,
    accuracy_50m: f64,
    
    // Quality metrics
    noise_level: String,
    gradient_issues: u32,
    data_quality_score: u32,
}

#[derive(Debug, Serialize)]
pub struct GarminAnalysisSummary {
    total_files_processed: u32,
    files_with_official_data: u32,
    
    // Average accuracies for each method
    avg_accuracy_raw: f64,
    avg_accuracy_10m: f64,
    avg_accuracy_25m: f64,
    avg_accuracy_50m: f64,
    
    // Best performing interval stats
    best_interval_10m_count: u32,
    best_interval_25m_count: u32,
    best_interval_50m_count: u32,
    best_interval_raw_count: u32,
    
    // Files within accuracy thresholds
    files_within_10_percent_raw: u32,
    files_within_10_percent_10m: u32,
    files_within_10_percent_25m: u32,
    files_within_10_percent_50m: u32,
    
    // Average gain/loss ratios
    avg_ratio_raw: f64,
    avg_ratio_10m: f64,
    avg_ratio_25m: f64,
    avg_ratio_50m: f64,
}

pub fn run_garmin_like_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏃 GARMIN-LIKE ELEVATION PROCESSING ANALYSIS");
    println!("===========================================");
    println!("Testing Garmin Connect-style processing approach:");
    println!("• Minimal smoothing (5-point moving average)");
    println!("• Distance-based resampling: 10m, 25m, 50m");
    println!("• No aggressive filtering or deadbands");
    println!("• Preserve original data characteristics");
    println!("• Compare with official elevation data\n");
    
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
                         result.raw_elevation_gain_m, result.raw_gain_loss_ratio, result.accuracy_raw);
                println!("      10m: {:.1}m gain (ratio {:.2}, accuracy {:.1}%)", 
                         result.gain_10m, result.ratio_10m, result.accuracy_10m);
                println!("      25m: {:.1}m gain (ratio {:.2}, accuracy {:.1}%)", 
                         result.gain_25m, result.ratio_25m, result.accuracy_25m);
                println!("      50m: {:.1}m gain (ratio {:.2}, accuracy {:.1}%)", 
                         result.gain_50m, result.ratio_50m, result.accuracy_50m);
                
                if result.official_elevation_gain_m > 0 {
                    // Find best accuracy
                    let accuracies = vec![
                        ("Raw", result.accuracy_raw),
                        ("10m", result.accuracy_10m),
                        ("25m", result.accuracy_25m),
                        ("50m", result.accuracy_50m),
                    ];
                    let best = accuracies.iter()
                        .min_by_key(|(_, acc)| ((acc - 100.0).abs() * 100.0) as i32)
                        .unwrap();
                    println!("      🎯 Best: {} interval ({:.1}% accuracy)", best.0, best.1);
                }
                
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
    let output_path = Path::new(gpx_folder).join("garmin_like_analysis_results.csv");
    write_results_csv(&results, &output_path)?;
    
    let summary_path = Path::new(gpx_folder).join("garmin_like_analysis_summary.csv");
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
    
    // Apply Garmin-like processing for each interval
    let processed_10m = process_with_garmin_method(&elevations, &distances, 10.0);
    let processed_25m = process_with_garmin_method(&elevations, &distances, 25.0);
    let processed_50m = process_with_garmin_method(&elevations, &distances, 50.0);
    
    // Calculate quality metrics
    let (noise_level, gradient_issues) = analyze_data_quality(&elevations, &distances);
    let data_quality_score = calculate_quality_score(noise_level.as_str(), gradient_issues, raw_ratio);
    
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
    
    // Calculate accuracies
    let accuracy_raw = if official_gain > 0 {
        (raw_gain / official_gain as f64) * 100.0
    } else {
        0.0
    };
    
    let accuracy_10m = if official_gain > 0 {
        (processed_10m.0 / official_gain as f64) * 100.0
    } else {
        0.0
    };
    
    let accuracy_25m = if official_gain > 0 {
        (processed_25m.0 / official_gain as f64) * 100.0
    } else {
        0.0
    };
    
    let accuracy_50m = if official_gain > 0 {
        (processed_50m.0 / official_gain as f64) * 100.0
    } else {
        0.0
    };
    
    Ok(GarminLikeResult {
        filename: filename.to_string(),
        total_points: coords.len() as u32,
        total_distance_km,
        raw_elevation_gain_m: raw_gain,
        raw_elevation_loss_m: raw_loss,
        raw_gain_loss_ratio: raw_ratio,
        gain_10m: processed_10m.0,
        loss_10m: processed_10m.1,
        ratio_10m: processed_10m.2,
        gain_25m: processed_25m.0,
        loss_25m: processed_25m.1,
        ratio_25m: processed_25m.2,
        gain_50m: processed_50m.0,
        loss_50m: processed_50m.1,
        ratio_50m: processed_50m.2,
        official_elevation_gain_m: official_gain,
        accuracy_raw,
        accuracy_10m,
        accuracy_25m,
        accuracy_50m,
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
    
    if files_with_official.is_empty() {
        return GarminAnalysisSummary {
            total_files_processed: total_files,
            files_with_official_data: 0,
            avg_accuracy_raw: 0.0,
            avg_accuracy_10m: 0.0,
            avg_accuracy_25m: 0.0,
            avg_accuracy_50m: 0.0,
            best_interval_10m_count: 0,
            best_interval_25m_count: 0,
            best_interval_50m_count: 0,
            best_interval_raw_count: 0,
            files_within_10_percent_raw: 0,
            files_within_10_percent_10m: 0,
            files_within_10_percent_25m: 0,
            files_within_10_percent_50m: 0,
            avg_ratio_raw: 0.0,
            avg_ratio_10m: 0.0,
            avg_ratio_25m: 0.0,
            avg_ratio_50m: 0.0,
        };
    }
    
    // Calculate average accuracies
    let avg_accuracy_raw = files_with_official.iter()
        .map(|r| r.accuracy_raw)
        .sum::<f64>() / files_with_official.len() as f64;
    
    let avg_accuracy_10m = files_with_official.iter()
        .map(|r| r.accuracy_10m)
        .sum::<f64>() / files_with_official.len() as f64;
    
    let avg_accuracy_25m = files_with_official.iter()
        .map(|r| r.accuracy_25m)
        .sum::<f64>() / files_with_official.len() as f64;
    
    let avg_accuracy_50m = files_with_official.iter()
        .map(|r| r.accuracy_50m)
        .sum::<f64>() / files_with_official.len() as f64;
    
    // Count best performing interval for each file
    let mut best_raw = 0;
    let mut best_10m = 0;
    let mut best_25m = 0;
    let mut best_50m = 0;
    
    for result in &files_with_official {
        let accuracies = vec![
            ("raw", (result.accuracy_raw - 100.0).abs()),
            ("10m", (result.accuracy_10m - 100.0).abs()),
            ("25m", (result.accuracy_25m - 100.0).abs()),
            ("50m", (result.accuracy_50m - 100.0).abs()),
        ];
        
        let best = accuracies.iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        
        match best.0 {
            "raw" => best_raw += 1,
            "10m" => best_10m += 1,
            "25m" => best_25m += 1,
            "50m" => best_50m += 1,
            _ => {}
        }
    }
    
    // Count files within 10% accuracy
    let within_10_raw = files_with_official.iter()
        .filter(|r| r.accuracy_raw >= 90.0 && r.accuracy_raw <= 110.0)
        .count() as u32;
    
    let within_10_10m = files_with_official.iter()
        .filter(|r| r.accuracy_10m >= 90.0 && r.accuracy_10m <= 110.0)
        .count() as u32;
    
    let within_10_25m = files_with_official.iter()
        .filter(|r| r.accuracy_25m >= 90.0 && r.accuracy_25m <= 110.0)
        .count() as u32;
    
    let within_10_50m = files_with_official.iter()
        .filter(|r| r.accuracy_50m >= 90.0 && r.accuracy_50m <= 110.0)
        .count() as u32;
    
    // Calculate average ratios
    let valid_ratios: Vec<_> = results.iter()
        .filter(|r| r.raw_gain_loss_ratio.is_finite())
        .collect();
    
    let avg_ratio_raw = if !valid_ratios.is_empty() {
        valid_ratios.iter().map(|r| r.raw_gain_loss_ratio).sum::<f64>() / valid_ratios.len() as f64
    } else {
        0.0
    };
    
    let avg_ratio_10m = if !valid_ratios.is_empty() {
        valid_ratios.iter().map(|r| r.ratio_10m).filter(|r| r.is_finite()).sum::<f64>() / 
        valid_ratios.iter().filter(|r| r.ratio_10m.is_finite()).count() as f64
    } else {
        0.0
    };
    
    let avg_ratio_25m = if !valid_ratios.is_empty() {
        valid_ratios.iter().map(|r| r.ratio_25m).filter(|r| r.is_finite()).sum::<f64>() / 
        valid_ratios.iter().filter(|r| r.ratio_25m.is_finite()).count() as f64
    } else {
        0.0
    };
    
    let avg_ratio_50m = if !valid_ratios.is_empty() {
        valid_ratios.iter().map(|r| r.ratio_50m).filter(|r| r.is_finite()).sum::<f64>() / 
        valid_ratios.iter().filter(|r| r.ratio_50m.is_finite()).count() as f64
    } else {
        0.0
    };
    
    GarminAnalysisSummary {
        total_files_processed: total_files,
        files_with_official_data: files_with_official.len() as u32,
        avg_accuracy_raw,
        avg_accuracy_10m,
        avg_accuracy_25m,
        avg_accuracy_50m,
        best_interval_10m_count: best_10m,
        best_interval_25m_count: best_25m,
        best_interval_50m_count: best_50m,
        best_interval_raw_count: best_raw,
        files_within_10_percent_raw: within_10_raw,
        files_within_10_percent_10m: within_10_10m,
        files_within_10_percent_25m: within_10_25m,
        files_within_10_percent_50m: within_10_50m,
        avg_ratio_raw,
        avg_ratio_10m,
        avg_ratio_25m,
        avg_ratio_50m,
    }
}

fn write_results_csv(
    results: &[GarminLikeResult],
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write results for each file
    for result in results {
        wtr.serialize(result)?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn write_summary_csv(
    summary: &GarminAnalysisSummary,
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write as key-value pairs
    wtr.write_record(&["Metric", "Value"])?;
    wtr.write_record(&["Total Files Processed", &summary.total_files_processed.to_string()])?;
    wtr.write_record(&["Files with Official Data", &summary.files_with_official_data.to_string()])?;
    wtr.write_record(&["Average Accuracy Raw", &format!("{:.2}%", summary.avg_accuracy_raw)])?;
    wtr.write_record(&["Average Accuracy 10m", &format!("{:.2}%", summary.avg_accuracy_10m)])?;
    wtr.write_record(&["Average Accuracy 25m", &format!("{:.2}%", summary.avg_accuracy_25m)])?;
    wtr.write_record(&["Average Accuracy 50m", &format!("{:.2}%", summary.avg_accuracy_50m)])?;
    wtr.write_record(&["Best Interval: Raw", &summary.best_interval_raw_count.to_string()])?;
    wtr.write_record(&["Best Interval: 10m", &summary.best_interval_10m_count.to_string()])?;
    wtr.write_record(&["Best Interval: 25m", &summary.best_interval_25m_count.to_string()])?;
    wtr.write_record(&["Best Interval: 50m", &summary.best_interval_50m_count.to_string()])?;
    wtr.write_record(&["Files within ±10% (Raw)", &summary.files_within_10_percent_raw.to_string()])?;
    wtr.write_record(&["Files within ±10% (10m)", &summary.files_within_10_percent_10m.to_string()])?;
    wtr.write_record(&["Files within ±10% (25m)", &summary.files_within_10_percent_25m.to_string()])?;
    wtr.write_record(&["Files within ±10% (50m)", &summary.files_within_10_percent_50m.to_string()])?;
    wtr.write_record(&["Average Ratio Raw", &format!("{:.3}", summary.avg_ratio_raw)])?;
    wtr.write_record(&["Average Ratio 10m", &format!("{:.3}", summary.avg_ratio_10m)])?;
    wtr.write_record(&["Average Ratio 25m", &format!("{:.3}", summary.avg_ratio_25m)])?;
    wtr.write_record(&["Average Ratio 50m", &format!("{:.3}", summary.avg_ratio_50m)])?;
    
    wtr.flush()?;
    Ok(())
}

fn print_detailed_analysis(results: &[GarminLikeResult], summary: &GarminAnalysisSummary) {
    println!("\n📊 GARMIN-LIKE PROCESSING RESULTS");
    println!("=================================");
    
    println!("\n📈 OVERALL STATISTICS:");
    println!("• Total files processed: {}", summary.total_files_processed);
    println!("• Files with official data: {}", summary.files_with_official_data);
    
    if summary.files_with_official_data > 0 {
        println!("\n🎯 AVERAGE ACCURACY BY METHOD:");
        println!("• Raw (unprocessed): {:.1}%", summary.avg_accuracy_raw);
        println!("• 10m intervals: {:.1}%", summary.avg_accuracy_10m);
        println!("• 25m intervals: {:.1}%", summary.avg_accuracy_25m);
        println!("• 50m intervals: {:.1}%", summary.avg_accuracy_50m);
        
        println!("\n🏆 BEST PERFORMING INTERVAL:");
        println!("• Raw performed best: {} files", summary.best_interval_raw_count);
        println!("• 10m performed best: {} files", summary.best_interval_10m_count);
        println!("• 25m performed best: {} files", summary.best_interval_25m_count);
        println!("• 50m performed best: {} files", summary.best_interval_50m_count);
        
        println!("\n✅ FILES WITHIN ±10% ACCURACY:");
        let total = summary.files_with_official_data as f64;
        println!("• Raw: {}/{} ({:.1}%)", 
                 summary.files_within_10_percent_raw, 
                 summary.files_with_official_data,
                 (summary.files_within_10_percent_raw as f64 / total) * 100.0);
        println!("• 10m: {}/{} ({:.1}%)", 
                 summary.files_within_10_percent_10m, 
                 summary.files_with_official_data,
                 (summary.files_within_10_percent_10m as f64 / total) * 100.0);
        println!("• 25m: {}/{} ({:.1}%)", 
                 summary.files_within_10_percent_25m, 
                 summary.files_with_official_data,
                 (summary.files_within_10_percent_25m as f64 / total) * 100.0);
        println!("• 50m: {}/{} ({:.1}%)", 
                 summary.files_within_10_percent_50m, 
                 summary.files_with_official_data,
                 (summary.files_within_10_percent_50m as f64 / total) * 100.0);
        
        println!("\n⚖️  AVERAGE GAIN/LOSS RATIOS:");
        println!("• Raw: {:.3}", summary.avg_ratio_raw);
        println!("• 10m: {:.3}", summary.avg_ratio_10m);
        println!("• 25m: {:.3}", summary.avg_ratio_25m);
        println!("• 50m: {:.3}", summary.avg_ratio_50m);
        
        // Find and display best examples
        println!("\n🌟 BEST ACCURACY EXAMPLES:");
        let mut best_files: Vec<_> = results.iter()
            .filter(|r| r.official_elevation_gain_m > 0)
            .collect();
        
        best_files.sort_by(|a, b| {
            let a_best = vec![a.accuracy_raw, a.accuracy_10m, a.accuracy_25m, a.accuracy_50m]
                .into_iter()
                .map(|acc| (acc - 100.0).abs())
                .fold(f64::INFINITY, f64::min);
            let b_best = vec![b.accuracy_raw, b.accuracy_10m, b.accuracy_25m, b.accuracy_50m]
                .into_iter()
                .map(|acc| (acc - 100.0).abs())
                .fold(f64::INFINITY, f64::min);
            a_best.partial_cmp(&b_best).unwrap()
        });
        
        for result in best_files.iter().take(5) {
            println!("\n   📁 {}", result.filename);
            println!("      Official: {}m", result.official_elevation_gain_m);
            println!("      Raw: {:.0}m ({:.1}%)", result.raw_elevation_gain_m, result.accuracy_raw);
            println!("      10m: {:.0}m ({:.1}%)", result.gain_10m, result.accuracy_10m);
            println!("      25m: {:.0}m ({:.1}%)", result.gain_25m, result.accuracy_25m);
            println!("      50m: {:.0}m ({:.1}%)", result.gain_50m, result.accuracy_50m);
            
            // Show which is best
            let accuracies = vec![
                ("Raw", (result.accuracy_raw - 100.0).abs()),
                ("10m", (result.accuracy_10m - 100.0).abs()),
                ("25m", (result.accuracy_25m - 100.0).abs()),
                ("50m", (result.accuracy_50m - 100.0).abs()),
            ];
            let best = accuracies.iter()
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .unwrap();
            println!("      🎯 Best: {} interval", best.0);
        }
    }
    
    println!("\n💡 KEY INSIGHTS:");
    if summary.avg_accuracy_raw > 105.0 {
        println!("• Raw data tends to overestimate elevation gain");
        println!("• Distance-based resampling helps reduce noise");
    }
    
    if summary.avg_ratio_10m < summary.avg_ratio_raw {
        println!("• 10m intervals improve gain/loss balance");
    }
    
    let best_overall = vec![
        ("Raw", summary.avg_accuracy_raw),
        ("10m", summary.avg_accuracy_10m),
        ("25m", summary.avg_accuracy_25m),
        ("50m", summary.avg_accuracy_50m),
    ].into_iter()
    .min_by(|a, b| (a.1 - 100.0).abs().partial_cmp(&(b.1 - 100.0).abs()).unwrap())
    .unwrap();
    
    println!("• {} intervals provide best overall accuracy ({:.1}%)", best_overall.0, best_overall.1);
    
    println!("\n🔍 COMPARED TO OUR CURRENT APPROACH:");
    println!("• Garmin-like processing is much simpler");
    println!("• No complex adaptive thresholds needed");
    println!("• Light smoothing preserves ride characteristics");
    println!("• Distance-based resampling is effective for noise reduction");
}