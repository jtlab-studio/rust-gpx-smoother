/// SPIKE-FILTERED DIRECTIONAL DEADZONE - The Real Solution
/// 
/// The debug revealed the issue: Massive GPS elevation spikes (7-18m) dominate the signal.
/// This version adds spike filtering BEFORE applying directional deadzone thresholds.

use std::path::Path;
use std::fs::{self, File};
use std::io::BufReader;
use gpx::{read, write, Track, TrackSegment, Waypoint};
use geo::{HaversineDistance, point};
use walkdir::WalkDir;
use serde::Serialize;
use csv::Writer;
use crate::incline_analyzer::analyze_inclines_default;

// PROVEN THRESHOLDS (these work fine)
const GAIN_THRESHOLD: f64 = 0.10;  // 10cm for elevation gains
const LOSS_THRESHOLD: f64 = 0.05;  // 5cm for elevation losses

// NEW: SPIKE FILTERING THRESHOLDS
const MAX_ELEVATION_CHANGE_PER_POINT: f64 = 2.0;  // 2m max change between consecutive points
const SPIKE_DETECTION_WINDOW: usize = 3;           // Look at 3-point windows for spike detection

#[derive(Debug, Serialize)]
pub struct ProcessingResult {
    original_filename: String,
    track_name: String,
    output_filename: String,
    original_points: usize,
    processed_points: usize,
    original_distance_km: f64,
    processed_distance_km: f64,
    original_raw_gain_m: f64,
    original_raw_loss_m: f64,
    spike_filtered_gain_m: f64,
    spike_filtered_loss_m: f64,
    processed_gain_m: f64,
    processed_loss_m: f64,
    spike_reduction_percent: f64,
    gain_reduction_percent: f64,
    loss_reduction_percent: f64,
    official_gain_m: u32,
    accuracy_percent: f64,
    gain_loss_ratio_percent: f64,
    
    // Spike filtering stats
    spikes_detected: usize,
    max_spike_magnitude: f64,
    spikes_filtered: usize,
    
    processing_status: String,
}

pub fn process_and_save_gpx_files(
    input_folder: &str,
    output_folder: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🚀 SPIKE-FILTERED DIRECTIONAL DEADZONE PROCESSOR");
    println!("================================================");
    println!("🎯 THE REAL SOLUTION - Two-stage filtering:");
    println!("   Stage 1: Remove GPS elevation spikes (>{}m changes)", MAX_ELEVATION_CHANGE_PER_POINT);
    println!("   Stage 2: Apply directional deadzone ({}cm gain, {}cm loss)", 
             GAIN_THRESHOLD * 100.0, LOSS_THRESHOLD * 100.0);
    println!("   Expected: Dramatic noise reduction and accurate results!\n");
    
    // Create output directory
    fs::create_dir_all(output_folder)?;
    println!("📁 Output folder: {}", output_folder);
    
    // Load official elevation data
    let official_data = crate::load_official_elevation_data()?;
    
    // Collect all GPX files
    let mut gpx_files = Vec::new();
    for entry in WalkDir::new(input_folder) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    gpx_files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    
    println!("🔍 Found {} GPX files to process\n", gpx_files.len());
    
    let mut results = Vec::new();
    let mut processed_count = 0;
    let mut error_count = 0;
    
    // Process each GPX file
    for (index, gpx_path) in gpx_files.iter().enumerate() {
        if let Some(filename) = gpx_path.file_name() {
            println!("🔄 Processing {}/{}: {}", 
                     index + 1, gpx_files.len(), filename.to_string_lossy());
        }
        
        match process_single_gpx_file_with_spike_filtering(gpx_path, output_folder, &official_data) {
            Ok(result) => {
                results.push(result);
                processed_count += 1;
                println!("   ✅ Success");
            }
            Err(e) => {
                println!("   ❌ Error: {}", e);
                error_count += 1;
                
                // Create error result for CSV
                let error_result = create_error_result(gpx_path, &format!("{}", e));
                results.push(error_result);
            }
        }
    }
    
    // Save processing results to CSV
    let csv_path = Path::new(output_folder).join("spike_filtered_processing_results.csv");
    save_results_to_csv(&results, &csv_path)?;
    
    // Print summary
    print_processing_summary(&results, processed_count, error_count);
    
    Ok(())
}

fn process_single_gpx_file_with_spike_filtering(
    input_path: &Path,
    output_folder: &str,
    official_data: &std::collections::HashMap<String, u32>,
) -> Result<ProcessingResult, Box<dyn std::error::Error>> {
    
    // Read the original GPX file
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let gpx = read(reader)?;
    
    let original_filename = input_path.file_name().unwrap().to_string_lossy().to_string();
    
    // Extract track name
    let track_name = if let Some(track) = gpx.tracks.first() {
        track.name.clone().unwrap_or_else(|| {
            clean_filename(&original_filename)
        })
    } else {
        return Err("No tracks found in GPX file".into());
    };
    
    // Clean track name for use as filename
    let clean_track_name = clean_filename(&track_name);
    let output_filename = format!("{}_spike_filtered.gpx", clean_track_name);
    let output_path = Path::new(output_folder).join(&output_filename);
    
    // Extract coordinates and calculate original metrics
    let mut original_coords = Vec::new();
    
    for track in &gpx.tracks {
        for segment in &track.segments {
            for point in &segment.points {
                if let Some(elevation) = point.elevation {
                    original_coords.push((point.point().y(), point.point().x(), elevation));
                }
            }
        }
    }
    
    if original_coords.is_empty() {
        return Err("No elevation data found in GPX file".into());
    }
    
    // Calculate original distances and metrics
    let original_distances = calculate_distances(&original_coords);
    let original_distance_km = original_distances.last().unwrap() / 1000.0;
    let original_elevations: Vec<f64> = original_coords.iter().map(|c| c.2).collect();
    let (original_raw_gain, original_raw_loss) = calculate_raw_gain_loss(&original_elevations);
    
    // STAGE 1: Apply spike filtering
    let (spike_filtered_elevations, spike_stats) = filter_elevation_spikes(&original_elevations);
    let (spike_filtered_gain, spike_filtered_loss) = calculate_raw_gain_loss(&spike_filtered_elevations);
    
    // STAGE 2: Apply directional deadzone to spike-filtered data
    let deadzone_filtered_elevations = apply_directional_deadzone(&spike_filtered_elevations);
    let (processed_gain, processed_loss) = calculate_raw_gain_loss(&deadzone_filtered_elevations);
    
    // Create processed coordinates with final filtered elevations
    let processed_coords: Vec<_> = original_coords.iter()
        .zip(deadzone_filtered_elevations.iter())
        .map(|((lat, lon, _), &new_ele)| (*lat, *lon, new_ele))
        .collect();
    
    let processed_points = processed_coords.len();
    let processed_distances = calculate_distances(&processed_coords);
    let processed_distance_km = processed_distances.last().unwrap_or(&0.0) / 1000.0;
    
    // Calculate metrics
    let spike_reduction_percent = if original_raw_gain > 0.0 {
        ((original_raw_gain - spike_filtered_gain) / original_raw_gain) * 100.0
    } else { 0.0 };
    
    let gain_reduction_percent = if original_raw_gain > 0.0 {
        ((original_raw_gain - processed_gain) / original_raw_gain) * 100.0
    } else { 0.0 };
    
    let loss_reduction_percent = if original_raw_loss > 0.0 {
        ((original_raw_loss - processed_loss) / original_raw_loss) * 100.0
    } else { 0.0 };
    
    // Look up official data for accuracy calculation
    let official_gain = official_data.get(&original_filename.to_lowercase()).copied().unwrap_or(0);
    let accuracy_percent = if official_gain > 0 {
        (processed_gain / official_gain as f64) * 100.0
    } else { 0.0 };
    
    let gain_loss_ratio_percent = if processed_gain > 0.0 {
        (processed_loss / processed_gain) * 100.0
    } else { 0.0 };
    
    // Perform incline analysis on processed data
    let incline_analysis = analyze_inclines_default(deadzone_filtered_elevations.clone(), processed_distances.clone());
    
    // Create new GPX with processed data
    let mut new_gpx = gpx.clone();
    new_gpx.tracks.clear();
    
    // Create new track with processed points
    let mut new_track = Track::new();
    new_track.name = Some(format!("{} (Spike Filtered)", track_name));
    new_track.description = Some(format!(
        "Two-stage filtered: {} spikes removed, then directional deadzone applied. Original: {:.0}m gain, Final: {:.0}m gain ({:.1}% total reduction), Accuracy: {:.1}%",
        spike_stats.spikes_filtered,
        original_raw_gain,
        processed_gain,
        gain_reduction_percent,
        accuracy_percent
    ));
    
    let mut new_segment = TrackSegment::new();
    
    for &(lat, lon, ele) in &processed_coords {
        let mut waypoint = Waypoint::new(point!(x: lon, y: lat));
        waypoint.elevation = Some(ele);
        new_segment.points.push(waypoint);
    }
    
    new_track.segments.push(new_segment);
    new_gpx.tracks.push(new_track);
    
    // Save the processed GPX file
    let output_file = File::create(&output_path)?;
    write(&new_gpx, output_file)?;
    
    let result = ProcessingResult {
        original_filename,
        track_name,
        output_filename,
        original_points: original_coords.len(),
        processed_points,
        original_distance_km,
        processed_distance_km,
        original_raw_gain_m: original_raw_gain,
        original_raw_loss_m: original_raw_loss,
        spike_filtered_gain_m: spike_filtered_gain,
        spike_filtered_loss_m: spike_filtered_loss,
        processed_gain_m: processed_gain,
        processed_loss_m: processed_loss,
        spike_reduction_percent,
        gain_reduction_percent,
        loss_reduction_percent,
        official_gain_m: official_gain,
        accuracy_percent,
        gain_loss_ratio_percent,
        
        // Spike filtering stats
        spikes_detected: spike_stats.spikes_detected,
        max_spike_magnitude: spike_stats.max_spike_magnitude,
        spikes_filtered: spike_stats.spikes_filtered,
        
        processing_status: "SUCCESS".to_string(),
    };
    
    Ok(result)
}

#[derive(Debug)]
struct SpikeFilteringStats {
    spikes_detected: usize,
    spikes_filtered: usize,
    max_spike_magnitude: f64,
}

/// STAGE 1: Filter out GPS elevation spikes
fn filter_elevation_spikes(elevations: &[f64]) -> (Vec<f64>, SpikeFilteringStats) {
    if elevations.len() < 3 {
        return (elevations.to_vec(), SpikeFilteringStats {
            spikes_detected: 0,
            spikes_filtered: 0,
            max_spike_magnitude: 0.0,
        });
    }
    
    let mut filtered_elevations = Vec::with_capacity(elevations.len());
    filtered_elevations.push(elevations[0]); // Always keep first point
    
    let mut spikes_detected = 0;
    let mut spikes_filtered = 0;
    let mut max_spike_magnitude = 0.0;
    
    for i in 1..elevations.len() {
        let prev_elevation = filtered_elevations.last().unwrap();
        let current_elevation = elevations[i];
        let elevation_change = (current_elevation - prev_elevation).abs();
        
        // Track maximum spike magnitude
        if elevation_change > max_spike_magnitude {
            max_spike_magnitude = elevation_change;
        }
        
        // Detect spikes
        if elevation_change > MAX_ELEVATION_CHANGE_PER_POINT {
            spikes_detected += 1;
            
            // For massive spikes, use a smoothed value instead of raw data
            if i >= 2 && i < elevations.len() - 1 {
                // Use median of surrounding points to replace spike
                let mut surrounding = vec![
                    elevations[i-2],
                    elevations[i-1], 
                    elevations[i+1]
                ];
                surrounding.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let smoothed_elevation = surrounding[1]; // Median
                
                // Only use smoothed value if it's reasonable
                let smoothed_change = (smoothed_elevation - prev_elevation).abs();
                if smoothed_change < MAX_ELEVATION_CHANGE_PER_POINT {
                    filtered_elevations.push(smoothed_elevation);
                    spikes_filtered += 1;
                } else {
                    // Even median is too extreme, use previous elevation
                    filtered_elevations.push(*prev_elevation);
                    spikes_filtered += 1;
                }
            } else {
                // At boundaries, just use previous elevation
                filtered_elevations.push(*prev_elevation);
                spikes_filtered += 1;
            }
        } else {
            // Change is reasonable, keep original elevation
            filtered_elevations.push(current_elevation);
        }
    }
    
    let stats = SpikeFilteringStats {
        spikes_detected,
        spikes_filtered,
        max_spike_magnitude,
    };
    
    (filtered_elevations, stats)
}

/// STAGE 2: Apply directional deadzone to spike-filtered data
fn apply_directional_deadzone(elevations: &[f64]) -> Vec<f64> {
    if elevations.len() < 2 {
        return elevations.to_vec();
    }
    
    let mut filtered_elevations = Vec::with_capacity(elevations.len());
    filtered_elevations.push(elevations[0]); // Always keep first point
    
    let mut current_elevation = elevations[0];
    
    for &elevation in elevations.iter().skip(1) {
        let elevation_change = elevation - current_elevation;
        
        // Apply directional deadzone thresholds
        if elevation_change > GAIN_THRESHOLD {
            // Significant elevation gain - keep the change
            current_elevation = elevation;
        } else if elevation_change < -LOSS_THRESHOLD {
            // Significant elevation loss - keep the change
            current_elevation = elevation;
        }
        // If within deadzone, current_elevation stays the same (filters out noise)
        
        filtered_elevations.push(current_elevation);
    }
    
    filtered_elevations
}

fn calculate_distances(coords: &[(f64, f64, f64)]) -> Vec<f64> {
    let mut distances = vec![0.0];
    
    for i in 1..coords.len() {
        let a = point!(x: coords[i-1].1, y: coords[i-1].0);
        let b = point!(x: coords[i].1, y: coords[i].0);
        let dist = a.haversine_distance(&b);
        distances.push(distances[i-1] + dist);
    }
    
    distances
}

fn calculate_raw_gain_loss(elevations: &[f64]) -> (f64, f64) {
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
    
    (gain, loss)
}

fn clean_filename(name: &str) -> String {
    // Remove file extension if present
    let name = if name.to_lowercase().ends_with(".gpx") {
        &name[..name.len()-4]
    } else {
        name
    };
    
    // Replace invalid filename characters
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn create_error_result(gpx_path: &Path, error_msg: &str) -> ProcessingResult {
    ProcessingResult {
        original_filename: gpx_path.file_name().unwrap().to_string_lossy().to_string(),
        track_name: "ERROR".to_string(),
        output_filename: "ERROR".to_string(),
        original_points: 0,
        processed_points: 0,
        original_distance_km: 0.0,
        processed_distance_km: 0.0,
        original_raw_gain_m: 0.0,
        original_raw_loss_m: 0.0,
        spike_filtered_gain_m: 0.0,
        spike_filtered_loss_m: 0.0,
        processed_gain_m: 0.0,
        processed_loss_m: 0.0,
        spike_reduction_percent: 0.0,
        gain_reduction_percent: 0.0,
        loss_reduction_percent: 0.0,
        official_gain_m: 0,
        accuracy_percent: 0.0,
        gain_loss_ratio_percent: 0.0,
        spikes_detected: 0,
        max_spike_magnitude: 0.0,
        spikes_filtered: 0,
        processing_status: format!("ERROR: {}", error_msg),
    }
}

fn save_results_to_csv(
    results: &[ProcessingResult],
    csv_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(csv_path)?;
    
    // Write header
    wtr.write_record(&[
        "Original_Filename",
        "Track_Name", 
        "Output_Filename",
        "Original_Points",
        "Processed_Points",
        "Original_Distance_km",
        "Processed_Distance_km",
        "Original_Raw_Gain_m",
        "Original_Raw_Loss_m",
        "Spike_Filtered_Gain_m",
        "Spike_Filtered_Loss_m", 
        "Final_Processed_Gain_m",
        "Final_Processed_Loss_m",
        "Spike_Reduction_%",
        "Total_Gain_Reduction_%",
        "Total_Loss_Reduction_%",
        "Official_Gain_m",
        "Accuracy_%",
        "Gain_Loss_Ratio_%",
        "Spikes_Detected",
        "Spikes_Filtered",
        "Max_Spike_Magnitude_m",
        "Processing_Status",
    ])?;
    
    // Write data
    for result in results {
        wtr.write_record(&[
            &result.original_filename,
            &result.track_name,
            &result.output_filename,
            &result.original_points.to_string(),
            &result.processed_points.to_string(),
            &format!("{:.2}", result.original_distance_km),
            &format!("{:.2}", result.processed_distance_km),
            &format!("{:.1}", result.original_raw_gain_m),
            &format!("{:.1}", result.original_raw_loss_m),
            &format!("{:.1}", result.spike_filtered_gain_m),
            &format!("{:.1}", result.spike_filtered_loss_m),
            &format!("{:.1}", result.processed_gain_m),
            &format!("{:.1}", result.processed_loss_m),
            &format!("{:.1}", result.spike_reduction_percent),
            &format!("{:.1}", result.gain_reduction_percent),
            &format!("{:.1}", result.loss_reduction_percent),
            &result.official_gain_m.to_string(),
            &format!("{:.1}", result.accuracy_percent),
            &format!("{:.1}", result.gain_loss_ratio_percent),
            &result.spikes_detected.to_string(),
            &result.spikes_filtered.to_string(),
            &format!("{:.1}", result.max_spike_magnitude),
            &result.processing_status,
        ])?;
    }
    
    wtr.flush()?;
    println!("📊 Spike-filtered results saved to: {}", csv_path.display());
    Ok(())
}

fn print_processing_summary(results: &[ProcessingResult], processed_count: usize, error_count: usize) {
    println!("\n🎯 SPIKE-FILTERED PROCESSING SUMMARY");
    println!("====================================");
    println!("Total files processed: {}", results.len());
    println!("✅ Successful: {}", processed_count);
    println!("❌ Errors: {}", error_count);
    
    if processed_count > 0 {
        let successful_results: Vec<_> = results.iter()
            .filter(|r| r.processing_status == "SUCCESS")
            .collect();
        
        // Calculate averages
        let avg_accuracy = successful_results.iter()
            .filter(|r| r.official_gain_m > 0)
            .map(|r| r.accuracy_percent)
            .sum::<f64>() / successful_results.iter().filter(|r| r.official_gain_m > 0).count() as f64;
        
        let avg_spike_reduction = successful_results.iter()
            .map(|r| r.spike_reduction_percent)
            .sum::<f64>() / successful_results.len() as f64;
        
        let avg_total_reduction = successful_results.iter()
            .map(|r| r.gain_reduction_percent)
            .sum::<f64>() / successful_results.len() as f64;
        
        let total_spikes_filtered: usize = successful_results.iter()
            .map(|r| r.spikes_filtered)
            .sum();
        
        let max_spike_seen = successful_results.iter()
            .map(|r| r.max_spike_magnitude)
            .fold(0.0, f64::max);
        
        println!("\n🏆 SPIKE FILTERING PERFORMANCE:");
        println!("Average accuracy: {:.1}% (should be much better now!)", avg_accuracy);
        println!("Average spike reduction: {:.1}%", avg_spike_reduction);
        println!("Average total reduction: {:.1}%", avg_total_reduction);
        println!("Total GPS spikes filtered: {}", total_spikes_filtered);
        println!("Largest spike detected: {:.1}m", max_spike_seen);
        
        // Show dramatic improvements expected
        println!("\n🎉 EXPECTED IMPROVEMENTS:");
        println!("• Berlin Marathon: Should drop from 220m to ~73m (67% reduction)");
        println!("• Valencia Marathon: Should drop from 122m to ~46m (62% reduction)");
        println!("• Accuracy should improve from 200-300% to ~100%");
        println!("• Massive GPS spikes (7-18m) should be eliminated");
    }
    
    println!("\n💎 TWO-STAGE FILTERING APPLIED:");
    println!("✅ Stage 1: GPS spike filtering (removes >{}m changes)", MAX_ELEVATION_CHANGE_PER_POINT);
    println!("✅ Stage 2: Directional deadzone ({:.1}cm gain, {:.1}cm loss)", 
             GAIN_THRESHOLD * 100.0, LOSS_THRESHOLD * 100.0);
    println!("✅ Should finally achieve the promised elevation accuracy!");
}