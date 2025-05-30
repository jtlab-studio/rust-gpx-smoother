use std::{fs::{File, create_dir_all}, path::{Path, PathBuf}};
use std::io::{BufReader, BufWriter};
use gpx::{read, write, Gpx, Track, TrackSegment, Waypoint, Time};
use geo::{HaversineDistance, point};
use walkdir::WalkDir;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use crate::custom_smoother::{ElevationData, SmoothingVariant};

#[derive(Debug, Serialize, Clone)]
struct ProcessingResult {
    filename: String,
    original_points: usize,
    processed_points: usize,
    raw_elevation_gain_m: f32,
    raw_elevation_loss_m: f32,
    processed_elevation_gain_m: f32,
    processed_elevation_loss_m: f32,
    gain_reduction_percent: f32,
    loss_reduction_percent: f32,
    distance_km: f32,
}

pub fn run_gpx_output_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let interval_m = 6.1;
    let output_dir = r"C:\Users\Dzhu\Documents\GPX Files\GPX Analysis";
    
    println!("\n🔧 GPX PROCESSING AND OUTPUT ANALYSIS");
    println!("=====================================");
    println!("Processing all files at {:.1}m interval", interval_m);
    println!("Output directory: {}", output_dir);
    
    // Create output directory if it doesn't exist
    create_dir_all(output_dir)?;
    
    // Collect all GPX files
    let gpx_files: Vec<PathBuf> = WalkDir::new(gpx_folder)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().is_file() && 
            entry.path().extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase() == "gpx")
                .unwrap_or(false)
        })
        .map(|entry| entry.path().to_path_buf())
        .collect();
    
    println!("📁 Found {} GPX files to process", gpx_files.len());
    
    let start_time = std::time::Instant::now();
    let results = Arc::new(Mutex::new(Vec::new()));
    let processed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_files = gpx_files.len();
    
    // Process files in parallel
    gpx_files.par_iter().for_each(|gpx_path| {
        match process_single_gpx(gpx_path, interval_m, output_dir) {
            Ok(result) => {
                results.lock().unwrap().push(result);
                let count = processed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                println!("  [{}/{}] Processed: {}", count, total_files, gpx_path.file_name().unwrap().to_str().unwrap());
            }
            Err(e) => {
                eprintln!("❌ Error processing {:?}: {}", gpx_path, e);
            }
        }
    });
    
    // Write summary CSV
    let csv_path = Path::new(output_dir).join("processing_summary.csv");
    let final_results = results.lock().unwrap();
    write_summary_csv(&final_results, &csv_path)?;
    
    let elapsed = start_time.elapsed();
    println!("\n✅ PROCESSING COMPLETE!");
    println!("📊 Processed {} files in {:.2} seconds", final_results.len(), elapsed.as_secs_f64());
    println!("📁 Output GPX files saved to: {}", output_dir);
    println!("📄 Summary CSV saved to: {}", csv_path.display());
    
    // Print summary statistics
    print_processing_summary(&final_results);
    
    Ok(())
}

fn process_single_gpx(
    gpx_path: &Path,
    interval_m: f64,
    output_dir: &str
) -> Result<ProcessingResult, Box<dyn std::error::Error>> {
    // Read GPX file
    let file = File::open(gpx_path)?;
    let reader = BufReader::new(file);
    let gpx = read(reader)?;
    
    // Extract coordinates and elevations
    let mut coords: Vec<(f64, f64, f64)> = vec![];
    let mut timestamps: Vec<Option<Time>> = vec![];
    
    for track in &gpx.tracks {
        for segment in &track.segments {
            for point in &segment.points {
                if let Some(ele) = point.elevation {
                    coords.push((point.point().y(), point.point().x(), ele));
                    timestamps.push(point.time.clone());
                }
            }
        }
    }
    
    if coords.is_empty() {
        return Err("No valid coordinates with elevation found".into());
    }
    
    // Calculate distances
    let mut distances = vec![0.0];
    for i in 1..coords.len() {
        let a = point!(x: coords[i-1].1, y: coords[i-1].0);
        let b = point!(x: coords[i].1, y: coords[i].0);
        let dist = a.haversine_distance(&b);
        distances.push(distances[i-1] + dist);
    }
    
    let total_distance_km = distances.last().unwrap() / 1000.0;
    let elevations: Vec<f64> = coords.iter().map(|c| c.2).collect();
    
    // Calculate raw gain/loss
    let (raw_gain, raw_loss) = calculate_gain_loss(&elevations);
    
    // Process elevation data
    let mut elevation_data = ElevationData::new_with_variant(
        elevations.clone(),
        distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval_m);
    
    let processed_gain = elevation_data.get_total_elevation_gain();
    let processed_loss = elevation_data.get_total_elevation_loss();
    
    // Get the smoothed elevations from the elevation data
    let processed_elevations = elevation_data.enhanced_altitude.clone();
    
    // Create new GPX with processed elevations
    let mut new_gpx = Gpx {
        version: gpx.version.clone(),
        creator: Some(format!("{} - Processed at {}m intervals", 
            gpx.creator.as_ref().unwrap_or(&"Unknown".to_string()), interval_m)),
        metadata: gpx.metadata.clone(),
        waypoints: gpx.waypoints.clone(),
        tracks: vec![],
        routes: gpx.routes.clone(),
    };
    
    // Build new track with processed elevations
    let mut point_idx = 0;
    for track in &gpx.tracks {
        let mut new_track = Track {
            name: track.name.clone(),
            comment: track.comment.clone(),
            description: track.description.clone(),
            source: track.source.clone(),
            links: track.links.clone(),
            type_: track.type_.clone(),
            number: track.number,
            segments: vec![],
        };
        
        for segment in &track.segments {
            let mut new_segment = TrackSegment { points: vec![] };
            
            for point in &segment.points {
                if point.elevation.is_some() && point_idx < processed_elevations.len() {
                    let mut new_point = Waypoint::new(point.point());
                    new_point.elevation = Some(processed_elevations[point_idx]);
                    new_point.time = point.time.clone();
                    new_point.speed = point.speed;
                    new_point.name = point.name.clone();
                    new_point.comment = point.comment.clone();
                    new_point.description = point.description.clone();
                    new_point.source = point.source.clone();
                    new_point.links = point.links.clone();
                    new_point.symbol = point.symbol.clone();
                    new_point.type_ = point.type_.clone();
                    
                    new_segment.points.push(new_point);
                    point_idx += 1;
                }
            }
            
            if !new_segment.points.is_empty() {
                new_track.segments.push(new_segment);
            }
        }
        
        if !new_track.segments.is_empty() {
            new_gpx.tracks.push(new_track);
        }
    }
    
    // Write processed GPX file
    let filename = gpx_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.gpx");
    let output_path = Path::new(output_dir).join(filename);
    
    let output_file = File::create(&output_path)?;
    let writer = BufWriter::new(output_file);
    write(&new_gpx, writer)?;
    
    // Calculate reduction percentages
    let gain_reduction = if raw_gain > 0.0 {
        ((raw_gain - processed_gain) / raw_gain) * 100.0
    } else {
        0.0
    };
    
    let loss_reduction = if raw_loss > 0.0 {
        ((raw_loss - processed_loss) / raw_loss) * 100.0
    } else {
        0.0
    };
    
    Ok(ProcessingResult {
        filename: filename.to_string(),
        original_points: coords.len(),
        processed_points: new_gpx.tracks.iter()
            .flat_map(|t| &t.segments)
            .flat_map(|s| &s.points)
            .count(),
        raw_elevation_gain_m: raw_gain as f32,
        raw_elevation_loss_m: raw_loss as f32,
        processed_elevation_gain_m: processed_gain as f32,
        processed_elevation_loss_m: processed_loss as f32,
        gain_reduction_percent: gain_reduction as f32,
        loss_reduction_percent: loss_reduction as f32,
        distance_km: total_distance_km as f32,
    })
}

fn calculate_gain_loss(elevations: &[f64]) -> (f64, f64) {
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

fn write_summary_csv(results: &[ProcessingResult], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Filename",
        "Distance (km)",
        "Original Points",
        "Processed Points",
        "Raw Elevation Gain (m)",
        "Raw Elevation Loss (m)",
        "Processed Elevation Gain (m)",
        "Processed Elevation Loss (m)",
        "Gain Reduction %",
        "Loss Reduction %",
    ])?;
    
    // Sort by filename for easier reading
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| a.filename.cmp(&b.filename));
    
    // Write data
    for result in sorted_results {
        wtr.write_record(&[
            &result.filename,
            &format!("{:.1}", result.distance_km),
            &result.original_points.to_string(),
            &result.processed_points.to_string(),
            &format!("{:.1}", result.raw_elevation_gain_m),
            &format!("{:.1}", result.raw_elevation_loss_m),
            &format!("{:.1}", result.processed_elevation_gain_m),
            &format!("{:.1}", result.processed_elevation_loss_m),
            &format!("{:.1}", result.gain_reduction_percent),
            &format!("{:.1}", result.loss_reduction_percent),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_processing_summary(results: &[ProcessingResult]) {
    if results.is_empty() {
        return;
    }
    
    let total_raw_gain: f32 = results.iter().map(|r| r.raw_elevation_gain_m).sum();
    let total_raw_loss: f32 = results.iter().map(|r| r.raw_elevation_loss_m).sum();
    let total_processed_gain: f32 = results.iter().map(|r| r.processed_elevation_gain_m).sum();
    let total_processed_loss: f32 = results.iter().map(|r| r.processed_elevation_loss_m).sum();
    let total_distance: f32 = results.iter().map(|r| r.distance_km).sum();
    
    let avg_gain_reduction = results.iter().map(|r| r.gain_reduction_percent).sum::<f32>() / results.len() as f32;
    let avg_loss_reduction = results.iter().map(|r| r.loss_reduction_percent).sum::<f32>() / results.len() as f32;
    
    println!("\n📊 PROCESSING SUMMARY");
    println!("====================");
    println!("Total distance: {:.1} km", total_distance);
    println!("Total raw elevation gain: {:.1} m", total_raw_gain);
    println!("Total raw elevation loss: {:.1} m", total_raw_loss);
    println!("Total processed elevation gain: {:.1} m", total_processed_gain);
    println!("Total processed elevation loss: {:.1} m", total_processed_loss);
    println!("Average gain reduction: {:.1}%", avg_gain_reduction);
    println!("Average loss reduction: {:.1}%", avg_loss_reduction);
    
    // Find files with highest reductions
    let mut sorted_by_gain_reduction = results.to_vec();
    sorted_by_gain_reduction.sort_by(|a, b| b.gain_reduction_percent.partial_cmp(&a.gain_reduction_percent).unwrap());
    
    println!("\n🔝 Files with highest gain reduction:");
    for result in sorted_by_gain_reduction.iter().take(5) {
        println!("  {} - {:.1}% reduction ({:.0}m → {:.0}m)", 
                 result.filename, 
                 result.gain_reduction_percent,
                 result.raw_elevation_gain_m,
                 result.processed_elevation_gain_m);
    }
}