/// GARMIN-LIKE PROCESSOR WITH FIXED 23M INTERVAL + GPX FILE GENERATION
/// 
/// Processes GPX files using Garmin Connect-style approach with 23m interval
/// and saves the processed elevation data as new GPX files

use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use geo::{HaversineDistance, point};
use walkdir::WalkDir;
use std::fs;
use crate::tolerant_gpx_reader::read_gpx_tolerantly;
use gpx::{Gpx, Track, TrackSegment, Waypoint, write};
use std::io::BufWriter;

// Garmin-like processing parameters
const GARMIN_SMOOTHING_WINDOW: usize = 5;  // Light smoothing only
const MAX_REALISTIC_GRADIENT: f64 = 35.0;  // 35% max gradient
const SPIKE_THRESHOLD: f64 = 10.0;         // 10m sudden change is suspicious
const PROCESSING_INTERVAL: f64 = 23.0;     // Fixed 23m interval

#[derive(Debug, Clone)]
pub struct ProcessedResult {
    filename: String,
    total_points: u32,
    total_distance_km: f64,
    
    // Raw data
    raw_elevation_gain_m: f64,
    raw_elevation_loss_m: f64,
    raw_gain_loss_ratio: f64,
    
    // Processed data (23m interval)
    processed_elevation_gain_m: f64,
    processed_elevation_loss_m: f64,
    processed_gain_loss_ratio: f64,
    
    // Official comparison
    official_elevation_gain_m: u32,
    raw_accuracy_percent: f64,
    processed_accuracy_percent: f64,
    
    // Quality metrics
    noise_level: String,
    gradient_issues: u32,
    data_quality_score: u32,
    
    // Processing info
    points_before_processing: u32,
    points_after_processing: u32,
    processing_interval_m: f64,
}

#[derive(Debug, Serialize)]
pub struct ProcessingSummary {
    total_files_processed: u32,
    files_with_official_data: u32,
    files_successfully_saved: u32,
    processing_interval_m: f64,
    
    // Accuracy statistics
    avg_raw_accuracy: f64,
    avg_processed_accuracy: f64,
    files_improved_by_processing: u32,
    files_within_10_percent_raw: u32,
    files_within_10_percent_processed: u32,
    files_within_15_percent_raw: u32,
    files_within_15_percent_processed: u32,
}

pub fn run_garmin_23m_processing(
    input_folder: &str, 
    output_folder: &str
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏃 GARMIN-LIKE PROCESSING WITH 23M INTERVAL");
    println!("===========================================");
    println!("Processing approach:");
    println!("• Fixed 23m distance interval resampling");
    println!("• Minimal smoothing (5-point moving average)");
    println!("• Remove obvious GPS spikes only");
    println!("• Preserve natural elevation characteristics");
    println!("• Save processed GPX files with updated elevation data");
    println!("");
    
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_folder)?;
    println!("📁 Output folder: {}", output_folder);
    
    // Load official elevation data
    println!("📂 Loading official elevation data...");
    let official_data = crate::load_official_elevation_data()?;
    println!("✅ Loaded {} official elevation records", official_data.len());
    
    // Collect GPX files
    println!("📂 Scanning for GPX files...");
    let gpx_files = collect_gpx_files(input_folder)?;
    println!("🔍 Found {} GPX files to process\n", gpx_files.len());
    
    if gpx_files.is_empty() {
        println!("⚠️  No GPX files found in: {}", input_folder);
        println!("💡 Make sure the folder contains .gpx files");
        return Ok(());
    }
    
    // Process each file
    let mut results = Vec::new();
    let mut errors = 0;
    let mut saved_files = 0;
    
    for (index, gpx_path) in gpx_files.iter().enumerate() {
        let filename = gpx_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        println!("🔄 Processing {}/{}: {}", index + 1, gpx_files.len(), filename);
        
        match process_and_save_file(gpx_path, &filename, &official_data, output_folder) {
            Ok((result, saved)) => {
                // Print summary for this file
                println!("   ✅ Success:");
                println!("      Points: {} → {}", result.points_before_processing, result.points_after_processing);
                println!("      Raw gain: {:.1}m → Processed: {:.1}m", 
                         result.raw_elevation_gain_m, result.processed_elevation_gain_m);
                
                if result.official_elevation_gain_m > 0 {
                    println!("      Accuracy: {:.1}% → {:.1}% (official: {}m)", 
                             result.raw_accuracy_percent, result.processed_accuracy_percent, result.official_elevation_gain_m);
                } else {
                    println!("      No official data for accuracy comparison");
                }
                
                if saved {
                    println!("      💾 Saved processed GPX file");
                    saved_files += 1;
                } else {
                    println!("      ⚠️  Could not save GPX file");
                }
                
                results.push(result);
            }
            Err(e) => {
                println!("   ❌ Error: {}", e);
                errors += 1;
            }
        }
        println!(); // Add spacing
    }
    
    println!("✅ Processed {} files successfully, {} errors", results.len(), errors);
    println!("💾 Saved {} processed GPX files", saved_files);
    
    // Calculate and save summary
    let summary = calculate_processing_summary(&results, saved_files);
    
    // Write results to CSV
    let results_path = Path::new(output_folder).join("garmin_23m_processing_results.csv");
    write_results_csv(&results, &results_path)?;
    
    let summary_path = Path::new(output_folder).join("garmin_23m_processing_summary.csv");
    write_summary_csv(&summary, &summary_path)?;
    
    // Print analysis
    print_processing_analysis(&results, &summary);
    
    println!("\n📁 Files saved to:");
    println!("   • Processed GPX files: {}", output_folder);
    println!("   • Results CSV: {}", results_path.display());
    println!("   • Summary CSV: {}", summary_path.display());
    
    Ok(())
}

fn collect_gpx_files(input_folder: &str) -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error>> {
    let mut gpx_files = Vec::new();
    
    for entry in WalkDir::new(input_folder).max_depth(1) {
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

fn process_and_save_file(
    gpx_path: &Path,
    filename: &str,
    official_data: &HashMap<String, u32>,
    output_folder: &str
) -> Result<(ProcessedResult, bool), Box<dyn std::error::Error>> {
    // Read GPX file
    let mut gpx = read_gpx_tolerantly(gpx_path)?;
    
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
    
    let points_before = coords.len() as u32;
    
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
    
    // Process with 23m interval
    let (processed_elevations, processed_coords) = process_with_23m_interval(&elevations, &distances, &coords);
    let (processed_gain, processed_loss) = calculate_gain_loss(&processed_elevations);
    let processed_ratio = if processed_loss > 0.0 { processed_gain / processed_loss } else { f64::INFINITY };
    
    let points_after = processed_coords.len() as u32;
    
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
    
    let processed_accuracy = if official_gain > 0 {
        (processed_gain / official_gain as f64) * 100.0
    } else {
        0.0
    };
    
    // Calculate quality metrics
    let (noise_level, gradient_issues) = analyze_data_quality(&elevations, &distances);
    let data_quality_score = calculate_quality_score(noise_level.as_str(), gradient_issues, raw_ratio);
    
    // Create processed GPX with updated elevation data
    let saved = save_processed_gpx(&mut gpx, &processed_coords, filename, output_folder)?;
    
    let result = ProcessedResult {
        filename: filename.to_string(),
        total_points: points_before,
        total_distance_km,
        raw_elevation_gain_m: raw_gain,
        raw_elevation_loss_m: raw_loss,
        raw_gain_loss_ratio: raw_ratio,
        processed_elevation_gain_m: processed_gain,
        processed_elevation_loss_m: processed_loss,
        processed_gain_loss_ratio: processed_ratio,
        official_elevation_gain_m: official_gain,
        raw_accuracy_percent: raw_accuracy,
        processed_accuracy_percent: processed_accuracy,
        noise_level,
        gradient_issues,
        data_quality_score,
        points_before_processing: points_before,
        points_after_processing: points_after,
        processing_interval_m: PROCESSING_INTERVAL,
    };
    
    Ok((result, saved))
}

fn process_with_23m_interval(
    elevations: &[f64],
    distances: &[f64],
    coords: &[(f64, f64, f64)]
) -> (Vec<f64>, Vec<(f64, f64, f64)>) {
    // Step 1: Resample to 23m intervals
    let (resampled_coords, resampled_elevations) = resample_to_distance_interval(coords, distances, PROCESSING_INTERVAL);
    
    if resampled_elevations.is_empty() {
        return (elevations.to_vec(), coords.to_vec());
    }
    
    // Step 2: Apply light Garmin-style smoothing
    let smoothed = apply_light_smoothing(&resampled_elevations, GARMIN_SMOOTHING_WINDOW);
    
    // Step 3: Remove obvious spikes
    let cleaned = remove_obvious_spikes(&smoothed);
    
    // Update coordinates with cleaned elevations
    let mut updated_coords = resampled_coords;
    for (i, &elevation) in cleaned.iter().enumerate() {
        if i < updated_coords.len() {
            updated_coords[i].2 = elevation;
        }
    }
    
    (cleaned, updated_coords)
}

fn resample_to_distance_interval(
    coords: &[(f64, f64, f64)],
    distances: &[f64],
    interval_meters: f64
) -> (Vec<(f64, f64, f64)>, Vec<f64>) {
    if coords.is_empty() || distances.is_empty() {
        return (vec![], vec![]);
    }
    
    let total_distance = distances.last().unwrap();
    let num_points = (total_distance / interval_meters).ceil() as usize + 1;
    
    // Prevent excessive memory usage
    if num_points > 100_000 {
        return (coords.to_vec(), coords.iter().map(|c| c.2).collect());
    }
    
    let mut resampled_coords = Vec::with_capacity(num_points);
    let mut resampled_elevations = Vec::with_capacity(num_points);
    
    for i in 0..num_points {
        let target_distance = i as f64 * interval_meters;
        if target_distance > *total_distance {
            break;
        }
        
        // Interpolate position and elevation at target distance
        let (lat, lon, elevation) = interpolate_position_at_distance(coords, distances, target_distance);
        
        resampled_coords.push((lat, lon, elevation));
        resampled_elevations.push(elevation);
    }
    
    (resampled_coords, resampled_elevations)
}

fn interpolate_position_at_distance(
    coords: &[(f64, f64, f64)],
    distances: &[f64],
    target_distance: f64
) -> (f64, f64, f64) {
    if target_distance <= 0.0 {
        return coords.first().copied().unwrap_or((0.0, 0.0, 0.0));
    }
    
    if target_distance >= *distances.last().unwrap() {
        return coords.last().copied().unwrap_or((0.0, 0.0, 0.0));
    }
    
    // Find interpolation points
    for i in 1..distances.len() {
        if distances[i] >= target_distance {
            let d1 = distances[i - 1];
            let d2 = distances[i];
            let c1 = coords[i - 1];
            let c2 = coords[i];
            
            if (d2 - d1).abs() < 1e-10 {
                return c1;
            }
            
            let t = (target_distance - d1) / (d2 - d1);
            let lat = c1.0 + t * (c2.0 - c1.0);
            let lon = c1.1 + t * (c2.1 - c1.1);
            let elevation = c1.2 + t * (c2.2 - c1.2);
            
            return (lat, lon, elevation);
        }
    }
    
    coords.last().copied().unwrap_or((0.0, 0.0, 0.0))
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

fn save_processed_gpx(
    gpx: &mut Gpx,
    processed_coords: &[(f64, f64, f64)],
    filename: &str,
    output_folder: &str
) -> Result<bool, Box<dyn std::error::Error>> {
    // Create new track with processed data
    let mut new_track = Track::new();
    new_track.name = Some(format!("Processed - {}", filename.replace(".gpx", "")));
    new_track.description = Some(format!("Processed with 23m Garmin-like algorithm"));
    
    let mut new_segment = TrackSegment::new();
    
    // Add processed points
    for &(lat, lon, elevation) in processed_coords {
        let mut waypoint = Waypoint::new(geo::Point::new(lon, lat));
        waypoint.elevation = Some(elevation);
        new_segment.points.push(waypoint);
    }
    
    new_track.segments.push(new_segment);
    
    // Replace tracks in GPX
    gpx.tracks = vec![new_track];
    
    // Generate output filename
    let output_filename = if filename.ends_with(".gpx") {
        format!("{}_Processed_23m.gpx", filename.trim_end_matches(".gpx"))
    } else {
        format!("{}_Processed_23m.gpx", filename)
    };
    
    let output_path = Path::new(output_folder).join(output_filename);
    
    // Write GPX file
    match std::fs::File::create(&output_path) {
        Ok(file) => {
            let mut writer = BufWriter::new(file);
            match write(gpx, &mut writer) {
                Ok(_) => Ok(true),
                Err(e) => {
                    eprintln!("Failed to write GPX data: {}", e);
                    Ok(false)
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to create output file: {}", e);
            Ok(false)
        }
    }
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
        if i < distances.len() && distances[i] > distances[i - 1] {
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

fn calculate_processing_summary(results: &[ProcessedResult], saved_files: u32) -> ProcessingSummary {
    let total_files = results.len() as u32;
    let files_with_official: Vec<_> = results.iter()
        .filter(|r| r.official_elevation_gain_m > 0)
        .collect();
    
    let files_with_official_count = files_with_official.len() as u32;
    
    // Calculate accuracy statistics
    let avg_raw_accuracy = if !files_with_official.is_empty() {
        files_with_official.iter().map(|r| r.raw_accuracy_percent).sum::<f64>() / files_with_official.len() as f64
    } else {
        0.0
    };
    
    let avg_processed_accuracy = if !files_with_official.is_empty() {
        files_with_official.iter().map(|r| r.processed_accuracy_percent).sum::<f64>() / files_with_official.len() as f64
    } else {
        0.0
    };
    
    // Count files improved by processing
    let files_improved = files_with_official.iter()
        .filter(|r| (r.processed_accuracy_percent - 100.0).abs() < (r.raw_accuracy_percent - 100.0).abs())
        .count() as u32;
    
    // Count files within accuracy thresholds
    let files_within_10_percent_raw = files_with_official.iter()
        .filter(|r| r.raw_accuracy_percent >= 90.0 && r.raw_accuracy_percent <= 110.0)
        .count() as u32;
    
    let files_within_10_percent_processed = files_with_official.iter()
        .filter(|r| r.processed_accuracy_percent >= 90.0 && r.processed_accuracy_percent <= 110.0)
        .count() as u32;
    
    let files_within_15_percent_raw = files_with_official.iter()
        .filter(|r| r.raw_accuracy_percent >= 85.0 && r.raw_accuracy_percent <= 115.0)
        .count() as u32;
    
    let files_within_15_percent_processed = files_with_official.iter()
        .filter(|r| r.processed_accuracy_percent >= 85.0 && r.processed_accuracy_percent <= 115.0)
        .count() as u32;
    
    ProcessingSummary {
        total_files_processed: total_files,
        files_with_official_data: files_with_official_count,
        files_successfully_saved: saved_files,
        processing_interval_m: PROCESSING_INTERVAL,
        avg_raw_accuracy,
        avg_processed_accuracy,
        files_improved_by_processing: files_improved,
        files_within_10_percent_raw,
        files_within_10_percent_processed,
        files_within_15_percent_raw,
        files_within_15_percent_processed,
    }
}

fn write_results_csv(
    results: &[ProcessedResult],
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "filename",
        "total_distance_km",
        "points_before_processing",
        "points_after_processing",
        "processing_interval_m",
        "raw_elevation_gain_m",
        "raw_elevation_loss_m",
        "raw_gain_loss_ratio",
        "processed_elevation_gain_m",
        "processed_elevation_loss_m",
        "processed_gain_loss_ratio",
        "official_elevation_gain_m",
        "raw_accuracy_percent",
        "processed_accuracy_percent",
        "accuracy_improvement",
        "noise_level",
        "gradient_issues",
        "data_quality_score",
    ])?;
    
    // Write data rows
    for result in results {
        let accuracy_improvement = if result.official_elevation_gain_m > 0 {
            (result.processed_accuracy_percent - 100.0).abs() - (result.raw_accuracy_percent - 100.0).abs()
        } else {
            0.0
        };
        
        wtr.write_record(&[
            &result.filename,
            &format!("{:.2}", result.total_distance_km),
            &result.points_before_processing.to_string(),
            &result.points_after_processing.to_string(),
            &format!("{:.1}", result.processing_interval_m),
            &format!("{:.1}", result.raw_elevation_gain_m),
            &format!("{:.1}", result.raw_elevation_loss_m),
            &format!("{:.3}", result.raw_gain_loss_ratio),
            &format!("{:.1}", result.processed_elevation_gain_m),
            &format!("{:.1}", result.processed_elevation_loss_m),
            &format!("{:.3}", result.processed_gain_loss_ratio),
            &result.official_elevation_gain_m.to_string(),
            &format!("{:.1}", result.raw_accuracy_percent),
            &format!("{:.1}", result.processed_accuracy_percent),
            &format!("{:.1}", accuracy_improvement),
            &result.noise_level,
            &result.gradient_issues.to_string(),
            &result.data_quality_score.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn write_summary_csv(
    summary: &ProcessingSummary,
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write summary statistics
    wtr.write_record(&["Metric", "Value", "Notes"])?;
    wtr.write_record(&["Total Files Processed", &summary.total_files_processed.to_string(), ""])?;
    wtr.write_record(&["Files with Official Data", &summary.files_with_official_data.to_string(), ""])?;
    wtr.write_record(&["Files Successfully Saved", &summary.files_successfully_saved.to_string(), "GPX files created"])?;
    wtr.write_record(&["Processing Interval", &format!("{}m", summary.processing_interval_m), "Fixed interval"])?;
    
    wtr.write_record(&["", "", ""])?; // Empty row
    wtr.write_record(&["Accuracy Comparison", "", ""])?;
    wtr.write_record(&["Average Raw Accuracy", &format!("{:.1}%", summary.avg_raw_accuracy), "Before processing"])?;
    wtr.write_record(&["Average Processed Accuracy", &format!("{:.1}%", summary.avg_processed_accuracy), "After processing"])?;
    wtr.write_record(&["Files Improved by Processing", &summary.files_improved_by_processing.to_string(), "Closer to 100%"])?;
    
    wtr.write_record(&["", "", ""])?; // Empty row
    wtr.write_record(&["Files Within ±10% Accuracy", "", ""])?;
    wtr.write_record(&["Raw Data", &summary.files_within_10_percent_raw.to_string(), "Before processing"])?;
    wtr.write_record(&["Processed Data", &summary.files_within_10_percent_processed.to_string(), "After processing"])?;
    
    wtr.write_record(&["", "", ""])?; // Empty row
    wtr.write_record(&["Files Within ±15% Accuracy", "", ""])?;
    wtr.write_record(&["Raw Data", &summary.files_within_15_percent_raw.to_string(), "Before processing"])?;
    wtr.write_record(&["Processed Data", &summary.files_within_15_percent_processed.to_string(), "After processing"])?;
    
    wtr.flush()?;
    Ok(())
}

fn print_processing_analysis(results: &[ProcessedResult], summary: &ProcessingSummary) {
    println!("\n📊 GARMIN-LIKE 23M PROCESSING RESULTS");
    println!("====================================");
    
    println!("\n📈 OVERALL STATISTICS:");
    println!("• Total files processed: {}", summary.total_files_processed);
    println!("• Files with official data: {}", summary.files_with_official_data);
    println!("• GPX files successfully saved: {}", summary.files_successfully_saved);
    println!("• Processing interval: {}m", summary.processing_interval_m);
    
    if summary.files_with_official_data > 0 {
        println!("\n🎯 ACCURACY COMPARISON:");
        println!("• Average raw accuracy: {:.1}%", summary.avg_raw_accuracy);
        println!("• Average processed accuracy: {:.1}%", summary.avg_processed_accuracy);
        
        let accuracy_change = summary.avg_processed_accuracy - summary.avg_raw_accuracy;
        if accuracy_change > 0.0 {
            println!("• Improvement: +{:.1}% ✅", accuracy_change);
        } else if accuracy_change < 0.0 {
            println!("• Change: {:.1}% ⚠️", accuracy_change);
        } else {
            println!("• No significant change");
        }
        
        println!("• Files improved by processing: {}/{} ({:.1}%)", 
                 summary.files_improved_by_processing, 
                 summary.files_with_official_data,
                 (summary.files_improved_by_processing as f64 / summary.files_with_official_data as f64) * 100.0);
        
        println!("\n✅ ACCURACY THRESHOLDS:");
        println!("• Within ±10%:");
        println!("  - Raw: {}/{} ({:.1}%)", 
                 summary.files_within_10_percent_raw, summary.files_with_official_data,
                 (summary.files_within_10_percent_raw as f64 / summary.files_with_official_data as f64) * 100.0);
        println!("  - Processed: {}/{} ({:.1}%)", 
                 summary.files_within_10_percent_processed, summary.files_with_official_data,
                 (summary.files_within_10_percent_processed as f64 / summary.files_with_official_data as f64) * 100.0);
        
        println!("• Within ±15%:");
        println!("  - Raw: {}/{} ({:.1}%)", 
                 summary.files_within_15_percent_raw, summary.files_with_official_data,
                 (summary.files_within_15_percent_raw as f64 / summary.files_with_official_data as f64) * 100.0);
        println!("  - Processed: {}/{} ({:.1}%)", 
                 summary.files_within_15_percent_processed, summary.files_with_official_data,
                 (summary.files_within_15_percent_processed as f64 / summary.files_with_official_data as f64) * 100.0);
        
        println!("\n🌟 TOP PERFORMING PROCESSED FILES:");
        let mut best_files: Vec<_> = results.iter()
            .filter(|r| r.official_elevation_gain_m > 0)
            .collect();
        
        best_files.sort_by(|a, b| {
            let a_error = (a.processed_accuracy_percent - 100.0).abs();
            let b_error = (b.processed_accuracy_percent - 100.0).abs();
            a_error.partial_cmp(&b_error).unwrap()
        });
        
        for (i, result) in best_files.iter().take(5).enumerate() {
            println!("\n{}. {} (Official: {}m)", i + 1, result.filename, result.official_elevation_gain_m);
            println!("   Raw: {:.1}m ({:.1}%) → Processed: {:.1}m ({:.1}%)", 
                     result.raw_elevation_gain_m, result.raw_accuracy_percent,
                     result.processed_elevation_gain_m, result.processed_accuracy_percent);
            println!("   Points: {} → {}", result.points_before_processing, result.points_after_processing);
        }
        
        println!("\n💡 PROCESSING INSIGHTS:");
        println!("• 23m interval provides good balance of smoothing and detail preservation");
        println!("• Garmin-like approach preserves natural terrain characteristics");
        println!("• Processed GPX files can be imported into GPS devices or mapping software");
        println!("• Light smoothing removes GPS noise while maintaining elevation profile accuracy");
    }
}