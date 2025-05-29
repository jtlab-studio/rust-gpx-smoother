use std::{fs::File, path::Path};
use gpx::read;
use geo::HaversineDistance;
use geo::point;
use std::io::BufReader;
use walkdir::WalkDir;
use csv::{Writer, Reader};
use serde::{Serialize, Deserialize};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::collections::HashMap;

mod custom_smoother;
mod improved_scoring;
mod outlier_analysis;
mod enhanced_analysis;
mod simplified_analysis;

use custom_smoother::{ElevationData, SmoothingVariant};
use improved_scoring::run_improved_scoring_analysis;
use outlier_analysis::run_outlier_analysis;
use simplified_analysis::run_simplified_analysis;

#[derive(Debug, Deserialize)]
struct OfficialElevationRecord {
    filename: String,
    official_elevation_gain_m: u32,
    #[serde(default)]
    source: String,
    #[serde(default)]
    notes: String,
}

#[derive(Debug, Serialize, Clone)]
struct GpxAnalysis {
    filename: String,
    raw_distance_km: f32,
    raw_elevation_gain_m: u32,
    average_time_interval_seconds: u32,
    custom_original_elevation_gain_m: u32,
    custom_distbased_10m_elevation_gain_m: u32,
    official_elevation_gain_m: u32,
    original_accuracy_percent: f32,
    distbased_10m_accuracy_percent: f32,
}

// Separate struct for fine-grained analysis
#[derive(Debug, Clone)]
struct FineGrainedResult {
    filename: String,
    raw_distance_km: f32,
    raw_elevation_gain_m: u32,
    official_elevation_gain_m: u32,
    interval_gains: Vec<(f32, u32)>, // (interval_m, gain_m)
    interval_accuracies: Vec<(f32, f32)>, // (interval_m, accuracy_percent)
}

// Load official elevation data from CSV
pub fn load_official_elevation_data() -> Result<HashMap<String, u32>, Box<dyn std::error::Error>> {
    let mut official_data = HashMap::new();
    
    // Try to load from src folder first, then from current directory
    let csv_paths = vec![
        "src/official_elevation_data.csv",
        "official_elevation_data.csv",
    ];
    
    let mut csv_loaded = false;
    
    for csv_path in csv_paths {
        if Path::new(csv_path).exists() {
            println!("📄 Loading official elevation data from: {}", csv_path);
            
            let file = File::open(csv_path)?;
            let mut rdr = Reader::from_reader(file);
            
            for result in rdr.deserialize::<OfficialElevationRecord>() {
                match result {
                    Ok(record) => {
                        official_data.insert(record.filename.to_lowercase(), record.official_elevation_gain_m);
                    }
                    Err(e) => {
                        eprintln!("⚠️  Error parsing CSV record: {}", e);
                    }
                }
            }
            
            csv_loaded = true;
            println!("✅ Loaded {} official elevation records", official_data.len());
            break;
        }
    }
    
    if !csv_loaded {
        println!("⚠️  No official elevation data CSV found, using built-in defaults");
        
        // Fallback to built-in data
        let builtin_data = vec![
            ("berlin garmin.gpx", 73),
            ("bostonmarathon2024.gpx", 248),
            ("bostonmarathon2025.gpx", 248),
            ("cmt_46.gpx", 1700),
            ("newyork2024.gpx", 247),
            // ... (add more as needed)
        ];
        
        for (filename, gain) in builtin_data {
            official_data.insert(filename.to_lowercase(), gain);
        }
    }
    
    Ok(official_data)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();
    
    // Load official elevation data
    let official_elevation_data = load_official_elevation_data()?;
    
    // Set up Rayon thread pool to use all available cores
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_cpus::get())
        .build_global()
        .unwrap();
    
    let gpx_folder = r"C:\Users\Dzhu\Documents\GPX Files";
    let output_path = Path::new(gpx_folder).join("fine_grained_analysis_0.05_to_8m.csv");
    
    println!("\n🚀 PARALLELIZED FINE-GRAINED DISTANCE ANALYSIS");
    println!("==============================================");
    println!("💻 System: {} cores detected, 32GB RAM available", num_cpus::get());
    println!("📊 Analysis range: 0.05m to 8.0m in 0.05m increments");
    println!("📈 Total intervals to test: 160 intervals per file");
    println!("🎯 Output: {}", output_path.display());
    println!("==============================================\n");
    
    // Collect all GPX files first
    let gpx_files: Vec<_> = WalkDir::new(gpx_folder)
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
    
    let progress_counter = Arc::new(Mutex::new(0));
    let total_files = gpx_files.len();
    let official_data_arc = Arc::new(official_elevation_data);
    
    // Process files in parallel
    let results: Vec<_> = gpx_files
        .par_iter()
        .filter_map(|path| {
            let official_data = Arc::clone(&official_data_arc);
            let result = process_gpx_file_fine_grained(path, &official_data);
            
            // Update progress
            let mut counter = progress_counter.lock().unwrap();
            *counter += 1;
            let progress = *counter;
            println!("Progress: {}/{} files completed", progress, total_files);
            
            match result {
                Ok(analysis) => Some(analysis),
                Err(e) => {
                    eprintln!("❌ Error processing {}: {}", path.display(), e);
                    None
                }
            }
        })
        .collect();
    
    if results.is_empty() {
        println!("No GPX files processed successfully.");
        return Ok(());
    }
    
    // Write results to CSV
    write_fine_grained_csv(&results, &output_path)?;
    
    let elapsed = start_time.elapsed();
    println!("\n🎉 FINE-GRAINED ANALYSIS COMPLETE!");
    println!("📊 Results saved to: {}", output_path.display());
    println!("📁 Processed {} GPX files", results.len());
    println!("⏱️  Total time: {:.2} seconds", elapsed.as_secs_f64());
    println!("⚡ Average time per file: {:.2} seconds", elapsed.as_secs_f64() / results.len() as f64);
    
    // Print summary statistics
    print_fine_grained_summary(&results);
    
    // Run improved scoring analysis
    println!("\n🔄 Running improved scoring analysis...");
    if let Err(e) = run_improved_scoring_analysis(gpx_folder) {
        eprintln!("Error in scoring analysis: {}", e);
    }
    
    // Run outlier analysis
    println!("\n🔄 Running outlier analysis...");
    if let Err(e) = run_outlier_analysis(gpx_folder) {
        eprintln!("Error in outlier analysis: {}", e);
    }
    
    // Run simplified analysis
    println!("\n🔄 Running simplified DistBased vs TwoStage analysis...");
    if let Err(e) = run_simplified_analysis(gpx_folder) {
        eprintln!("Error in simplified analysis: {}", e);
    }
    
    Ok(())
}

fn process_gpx_file_fine_grained(
    path: &Path, 
    official_data: &HashMap<String, u32>
) -> Result<FineGrainedResult, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let gpx = read(reader)?;
    
    let mut coords: Vec<(f64, f64, f64)> = vec![];
    
    for track in gpx.tracks {
        for segment in track.segments {
            for pt in segment.points {
                if let Some(ele) = pt.elevation {
                    let lat = pt.point().y();
                    let lon = pt.point().x();
                    coords.push((lat, lon, ele));
                }
            }
        }
    }
    
    if coords.is_empty() {
        return Err("No valid coordinates found in GPX file".into());
    }
    
    // Calculate distances
    let mut distances = vec![0.0];
    for i in 1..coords.len() {
        let a = point!(x: coords[i-1].1, y: coords[i-1].0);
        let b = point!(x: coords[i].1, y: coords[i].0);
        let dist = a.haversine_distance(&b);
        distances.push(distances[i-1] + dist);
    }
    
    let raw_elevations: Vec<f64> = coords.iter().map(|x| x.2).collect();
    let total_distance_km = distances.last().unwrap() / 1000.0;
    let (raw_gain, _) = gain_loss(&raw_elevations);
    
    let filename = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    // Look up official gain from CSV data
    let official_gain = official_data
        .get(&filename.to_lowercase())
        .copied()
        .unwrap_or(0);
    
    if official_gain == 0 {
        println!("⚠️  No official data for: {}", filename);
    }
    
    println!("🔄 Processing: {} ({:.1}km, official: {}m)", filename, total_distance_km, official_gain);
    
    // Generate intervals from 0.05m to 8.0m in 0.05m increments
    let intervals: Vec<f64> = (1..=160).map(|i| i as f64 * 0.05).collect();
    
    // Process all intervals in parallel for this file
    let interval_results: Vec<(f32, u32, f32)> = intervals
        .par_iter()
        .map(|&interval| {
            let gain = distbased_with_interval(&raw_elevations, &distances, interval);
            let gain_u32 = gain.round() as u32;
            let accuracy = if official_gain > 0 {
                (gain_u32 as f32 / official_gain as f32) * 100.0
            } else {
                0.0
            };
            (interval as f32, gain_u32, accuracy)
        })
        .collect();
    
    let mut interval_gains = Vec::new();
    let mut interval_accuracies = Vec::new();
    
    for (interval, gain, accuracy) in interval_results {
        interval_gains.push((interval, gain));
        interval_accuracies.push((interval, accuracy));
    }
    
    Ok(FineGrainedResult {
        filename,
        raw_distance_km: total_distance_km as f32,
        raw_elevation_gain_m: raw_gain.round() as u32,
        official_elevation_gain_m: official_gain,
        interval_gains,
        interval_accuracies,
    })
}

// Optimized distance-based processing with custom intervals
fn distbased_with_interval(raw_elevations: &[f64], distances: &[f64], interval_meters: f64) -> f64 {
    let mut elevation_data = ElevationData::new_with_variant(
        raw_elevations.to_vec(), 
        distances.to_vec(), 
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval_meters);
    elevation_data.get_total_elevation_gain()
}

fn gain_loss(elevs: &[f64]) -> (f64, f64) {
    let mut gain = 0.0;
    let mut loss = 0.0;
    for w in elevs.windows(2) {
        let delta = w[1] - w[0];
        if delta > 0.0 {
            gain += delta;
        } else {
            loss += -delta;
        }
    }
    (gain, loss)
}

fn write_fine_grained_csv(results: &[FineGrainedResult], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Build header
    let mut header = vec![
        "Filename".to_string(),
        "Raw Distance (km)".to_string(),
        "Raw Elevation Gain (m)".to_string(),
        "Official Elevation Gain (m)".to_string(),
    ];
    
    // Add columns for each interval
    for i in 1..=160 {
        let interval = i as f32 * 0.05;
        header.push(format!("{:.2}m Gain", interval));
        header.push(format!("{:.2}m Accuracy %", interval));
    }
    
    wtr.write_record(&header)?;
    
    // Write data rows
    for result in results {
        let mut row = vec![
            result.filename.clone(),
            result.raw_distance_km.to_string(),
            result.raw_elevation_gain_m.to_string(),
            result.official_elevation_gain_m.to_string(),
        ];
        
        // Add interval data
        for i in 0..result.interval_gains.len() {
            row.push(result.interval_gains[i].1.to_string());
            row.push(format!("{:.1}", result.interval_accuracies[i].1));
        }
        
        wtr.write_record(&row)?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_fine_grained_summary(results: &[FineGrainedResult]) {
    println!("\n📊 FINE-GRAINED ANALYSIS SUMMARY");
    println!("================================");
    
    // Find best interval for each file
    let mut best_intervals: Vec<f32> = Vec::new();
    
    for result in results {
        if result.official_elevation_gain_m > 0 {
            // Find interval with accuracy closest to 100%
            let best_idx = result.interval_accuracies
                .iter()
                .enumerate()
                .min_by_key(|(_, (_, acc))| ((acc - 100.0).abs() * 100.0) as i32)
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            
            if best_idx < result.interval_gains.len() {
                best_intervals.push(result.interval_gains[best_idx].0);
            }
        }
    }
    
    if !best_intervals.is_empty() {
        best_intervals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_interval = best_intervals[best_intervals.len() / 2];
        let avg_interval = best_intervals.iter().sum::<f32>() / best_intervals.len() as f32;
        
        println!("🎯 Optimal interval statistics:");
        println!("  Average optimal interval: {:.2}m", avg_interval);
        println!("  Median optimal interval: {:.2}m", median_interval);
        println!("  Min optimal interval: {:.2}m", best_intervals.first().unwrap());
        println!("  Max optimal interval: {:.2}m", best_intervals.last().unwrap());
        
        // Count distribution
        println!("\n📈 Optimal interval distribution:");
        let mut distribution: HashMap<i32, i32> = HashMap::new();
        for &interval in &best_intervals {
            // Convert to integer key (multiply by 10 to preserve one decimal place)
            let bucket_key = ((interval / 0.5).round() * 5.0) as i32;
            *distribution.entry(bucket_key).or_insert(0) += 1;
        }
        
        let mut buckets: Vec<_> = distribution.into_iter().collect();
        buckets.sort_by_key(|&(k, _)| k);
        
        for (bucket_key, count) in buckets {
            let bucket_value = bucket_key as f32 / 10.0;
            println!("  {:.1}m ± 0.25m: {} files", bucket_value, count);
        }
    }
}