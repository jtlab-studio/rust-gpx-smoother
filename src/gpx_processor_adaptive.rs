/// GPX PROCESSOR WITH ADAPTIVE QUALITY
/// 
/// Processes GPX files using the 1.9m symmetric method with adaptive quality
/// for files that need correction (ratio > 1.1). Saves processed files with
/// track name + "_Processed.gpx" suffix.

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use walkdir::WalkDir;
use gpx::{Gpx, Track, TrackSegment, Waypoint, write};
use geo::{HaversineDistance, point};
use crate::custom_smoother::{ElevationData, SmoothingVariant};
use crate::tolerant_gpx_reader::read_gpx_tolerantly;
use std::collections::HashMap;

pub fn process_and_save_gpx_files(
    input_folder: &str, 
    output_folder: &str,
    official_data: &HashMap<String, u32>
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🚀 PROCESSING GPX FILES WITH ADAPTIVE QUALITY");
    println!("===========================================");
    println!("📂 Input folder: {}", input_folder);
    println!("📂 Output folder: {}", output_folder);
    
    // Create output folder if it doesn't exist
    fs::create_dir_all(output_folder)?;
    
    // Collect all GPX files
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
    
    println!("🔍 Found {} GPX files to process", gpx_files.len());
    
    let mut success_count = 0;
    let mut error_count = 0;
    
    for (index, gpx_path) in gpx_files.iter().enumerate() {
        let filename = gpx_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        println!("\n🔄 Processing {}/{}: {}", index + 1, gpx_files.len(), filename);
        
        match process_single_gpx_file(gpx_path, output_folder, official_data) {
            Ok(output_path) => {
                println!("   ✅ Success! Saved as: {}", 
                         output_path.file_name().unwrap_or_default().to_str().unwrap_or("unknown"));
                success_count += 1;
            }
            Err(e) => {
                println!("   ❌ Error: {}", e);
                error_count += 1;
            }
        }
    }
    
    println!("\n📊 PROCESSING COMPLETE");
    println!("✅ Successfully processed: {} files", success_count);
    println!("❌ Errors: {} files", error_count);
    
    Ok(())
}

fn process_single_gpx_file(
    input_path: &Path,
    output_folder: &str,
    official_data: &HashMap<String, u32>
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Read GPX file with tolerant reader
    let mut gpx = read_gpx_tolerantly(input_path)?;
    
    // Get track name for output filename
    let track_name = get_track_name(&gpx);
    let output_filename = format!("{}_Processed.gpx", sanitize_filename(&track_name));
    let output_path = Path::new(output_folder).join(&output_filename);
    
    println!("   📄 Track name: {}", track_name);
    println!("   📄 Output file: {}", output_filename);
    
    // Process each track
    for track in &mut gpx.tracks {
        process_track(track)?;
    }
    
    // Save processed GPX
    save_gpx(&gpx, &output_path)?;
    
    // Report statistics
    if let Some(stats) = calculate_elevation_stats(&gpx) {
        println!("   📊 Processed elevation stats:");
        println!("      • Total gain: {:.1}m", stats.total_gain);
        println!("      • Total loss: {:.1}m", stats.total_loss);
        println!("      • Gain/Loss ratio: {:.3}", stats.gain_loss_ratio);
        
        // Check against official data if available
        let clean_filename = output_filename
            .replace("_Processed.gpx", ".gpx")
            .to_lowercase();
        
        if let Some(&official_gain) = official_data.get(&clean_filename) {
            let accuracy = (stats.total_gain / official_gain as f64) * 100.0;
            println!("      • Official gain: {}m", official_gain);
            println!("      • Accuracy: {:.1}%", accuracy);
        }
    }
    
    Ok(output_path)
}

fn get_track_name(gpx: &Gpx) -> String {
    // Try to get track name from first track
    if let Some(track) = gpx.tracks.first() {
        if let Some(name) = &track.name {
            return name.clone();
        }
    }
    
    // Fallback to metadata name
    if let Some(metadata) = &gpx.metadata {
        if let Some(name) = &metadata.name {
            return name.clone();
        }
    }
    
    // Default fallback
    "Unnamed_Track".to_string()
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

fn process_track(track: &mut Track) -> Result<(), Box<dyn std::error::Error>> {
    for segment in &mut track.segments {
        process_segment(segment)?;
    }
    Ok(())
}

fn process_segment(segment: &mut TrackSegment) -> Result<(), Box<dyn std::error::Error>> {
    if segment.points.is_empty() {
        return Ok(());
    }
    
    // Extract coordinates and elevations
    let mut coords = Vec::new();
    let mut has_elevation = false;
    
    for point in &segment.points {
        let lat = point.point().y();
        let lon = point.point().x();
        let ele = point.elevation.unwrap_or(0.0);
        coords.push((lat, lon, ele));
        if point.elevation.is_some() {
            has_elevation = true;
        }
    }
    
    if !has_elevation {
        println!("      ⚠️  Segment has no elevation data, skipping processing");
        return Ok(());
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
    
    // Calculate raw gain/loss ratio
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&elevations);
    let raw_ratio = if raw_loss > 0.0 { raw_gain / raw_loss } else { f64::INFINITY };
    
    println!("      📊 Segment analysis:");
    println!("         • Points: {}", coords.len());
    println!("         • Distance: {:.1}km", distances.last().unwrap_or(&0.0) / 1000.0);
    println!("         • Raw gain/loss ratio: {:.3}", raw_ratio);
    
    // Process elevation data based on quality
    let processed_elevations = if raw_ratio <= 1.1 {
        // Good quality data - use standard 1.9m symmetric processing
        println!("         ✅ Good ratio - using standard 1.9m processing");
        
        let mut elevation_data = ElevationData::new_with_variant(
            elevations.clone(),
            distances.clone(),
            SmoothingVariant::SymmetricFixed
        );
        
        elevation_data.apply_custom_interval_processing_symmetric(1.9);
        elevation_data.enhanced_altitude
    } else {
        // Problematic data - use adaptive processing
        println!("         🔧 Poor ratio - using adaptive processing");
        
        let mut elevation_data = ElevationData::new_with_variant(
            elevations.clone(),
            distances.clone(),
            SmoothingVariant::AdaptiveQuality
        );
        
        elevation_data.process_elevation_data_adaptive();
        elevation_data.enhanced_altitude
    };
    
    // Update segment points with processed elevations
    for (i, point) in segment.points.iter_mut().enumerate() {
        if i < processed_elevations.len() {
            point.elevation = Some(processed_elevations[i]);
        }
    }
    
    // Report processing results
    let (proc_gain, proc_loss) = calculate_raw_gain_loss(&processed_elevations);
    let proc_ratio = if proc_loss > 0.0 { proc_gain / proc_loss } else { f64::INFINITY };
    println!("         • Processed gain/loss ratio: {:.3}", proc_ratio);
    
    Ok(())
}

fn calculate_raw_gain_loss(elevations: &[f64]) -> (f64, f64) {
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

fn save_gpx(gpx: &Gpx, output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(output_path)?;
    let writer = BufWriter::new(file);
    write(gpx, writer)?;
    Ok(())
}

struct ElevationStats {
    total_gain: f64,
    total_loss: f64,
    gain_loss_ratio: f64,
}

fn calculate_elevation_stats(gpx: &Gpx) -> Option<ElevationStats> {
    let mut total_gain = 0.0;
    let mut total_loss = 0.0;
    
    for track in &gpx.tracks {
        for segment in &track.segments {
            let elevations: Vec<f64> = segment.points
                .iter()
                .filter_map(|p| p.elevation)
                .collect();
            
            let (gain, loss) = calculate_raw_gain_loss(&elevations);
            total_gain += gain;
            total_loss += loss;
        }
    }
    
    if total_gain > 0.0 || total_loss > 0.0 {
        let gain_loss_ratio = if total_loss > 0.0 { 
            total_gain / total_loss 
        } else { 
            f64::INFINITY 
        };
        
        Some(ElevationStats {
            total_gain,
            total_loss,
            gain_loss_ratio,
        })
    } else {
        None
    }
}

pub fn run_gpx_processing_and_analysis(
    input_folder: &str,
    output_folder: &str
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎯 GPX PROCESSING WITH 1.9M ADAPTIVE METHOD");
    println!("==========================================");
    
    // Load official elevation data
    println!("📂 Loading official elevation data...");
    let official_data = crate::load_official_elevation_data()?;
    println!("✅ Loaded {} official elevation records", official_data.len());
    
    // Process all GPX files
    process_and_save_gpx_files(input_folder, output_folder, &official_data)?;
    
    println!("\n✅ All processing complete!");
    println!("📁 Processed files saved to: {}", output_folder);
    
    Ok(())
}