use std::{fs::File, path::Path};
use gpx::read;
use geo::HaversineDistance;
use geo::point;
use std::io::BufReader;
use walkdir::WalkDir;
use csv::Writer;
use serde::Serialize;
use gpx::Time;

mod custom_smoother;

use custom_smoother::{create_custom_original, create_custom_capping, create_custom_flat21, create_custom_postcap, create_custom_distbased_adaptive};

#[derive(Debug, Serialize)]
struct GpxAnalysis {
    filename: String,
    raw_distance_km: u32,
    raw_elevation_gain_m: u32,
    average_time_interval_seconds: u32,
    custom_original_elevation_gain_m: u32,
    custom_capping_elevation_gain_m: u32,
    custom_flat21_elevation_gain_m: u32,
    custom_postcap_elevation_gain_m: u32,
    custom_distbased_elevation_gain_m: u32,
    official_elevation_gain_m: u32,
    distbased_vs_official_diff_m: i32,
}

fn get_official_elevation_gain(filename: &str) -> u32 {
    // Official elevation gain lookup table based on your data
    match filename.to_lowercase().as_str() {
        "berlin garmin.gpx" => 73,
        "bostonmarathon2024.gpx" => 248,
        "bostonmarathon2025.gpx" => 248,
        "cmt_46.gpx" => 1700,
        "newyork2024.gpx" => 247,
        "nocnyjelen.gpx" => 2672,
        "nocny_jelen_76_ km_bez_klifów_001.gpx" => 2672,
        "o-see 50k.gpx" => 2300,
        "oravaman.gpx" => 1250,
        "tokyomarathon.gpx" => 40,
        "valencia2022.gpx" => 46,
        "xterra-o-see-ultra-trail-2024-50k.gpx" => 2300,
        _ => 0, // Unknown routes
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpx_folder = r"C:\Users\Dzhu\Documents\GPX Files";
    let output_path = Path::new(gpx_folder).join("adaptive_distbased_with_official.csv");
    
    println!("Analyzing GPX files with ADAPTIVE DISTBASED + OFFICIAL COMPARISON...");
    println!("1. Custom Original: Adaptive 83-point/5-point with conditional capping");
    println!("2. Custom Capping: 5-point smoothing + capping applied to ALL routes");
    println!("3. Custom Flat21: 21-point for flat routes + 5-point for hilly");
    println!("4. Custom PostCap: 5-point + capping + 83-point post-capping smoothing");
    println!("5. Custom DistBased: THREE-TIER ADAPTIVE distance-based processing");
    println!("   • Flat routes (<20m/km): 1.2m deadband + 120m Gaussian smoothing");
    println!("   • Hilly routes (20-40m/km): 2.0m deadband + 150m Gaussian smoothing");
    println!("   • Super Hilly routes (>40m/km): 1.5m deadband + 100m Gaussian smoothing");
    println!("6. Official Numbers: Direct comparison with known official values");
    println!("Output will be saved to: {}", output_path.display());
    
    let mut results = Vec::new();
    
    // Iterate through all GPX files in the directory
    for entry in WalkDir::new(gpx_folder) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    println!("Processing: {}", entry.path().display());
                    
                    match process_gpx_file(entry.path()) {
                        Ok(analysis) => {
                            results.push(analysis);
                            println!("  ✓ Completed successfully");
                        },
                        Err(e) => {
                            eprintln!("  ✗ Error processing {}: {}", entry.path().display(), e);
                        }
                    }
                }
            }
        }
    }
    
    if results.is_empty() {
        println!("No GPX files found or processed successfully.");
        return Ok(());
    }
    
    // Write results to CSV
    let mut wtr = Writer::from_path(&output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Filename",
        "Raw Distance (km)", 
        "Raw Elevation Gain (m)",
        "Average Time Interval (seconds)",
        "Custom Original Elevation Gain (m)",
        "Custom Capping Elevation Gain (m)",
        "Custom Flat21 Elevation Gain (m)",
        "Custom PostCap Elevation Gain (m)",
        "Custom DistBased Elevation Gain (m)",
        "Official Elevation Gain (m)",
        "DistBased vs Official Diff (m)",
    ])?;
    
    let file_count = results.len();
    
    // Write data - FIXED: Use &results to avoid moving
    for result in &results {
        wtr.write_record(&[
            &result.filename,
            &result.raw_distance_km.to_string(),
            &result.raw_elevation_gain_m.to_string(),
            &result.average_time_interval_seconds.to_string(),
            &result.custom_original_elevation_gain_m.to_string(),
            &result.custom_capping_elevation_gain_m.to_string(),
            &result.custom_flat21_elevation_gain_m.to_string(),
            &result.custom_postcap_elevation_gain_m.to_string(),
            &result.custom_distbased_elevation_gain_m.to_string(),
            &result.official_elevation_gain_m.to_string(),
            &result.distbased_vs_official_diff_m.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    
    println!("\nAnalysis complete! Results saved to: {}", output_path.display());
    println!("Processed {} GPX files", file_count);
    
    // Print summary of DistBased vs Official comparison - FIXED: Use &results
    println!("\n=== DistBased vs Official Comparison ===");
    for result in &results {
        if result.official_elevation_gain_m > 0 {
            let accuracy_pct = (result.custom_distbased_elevation_gain_m as f64 / result.official_elevation_gain_m as f64) * 100.0;
            println!("{}: DistBased {}m vs Official {}m ({:+}m, {:.1}% accuracy)", 
                     result.filename,
                     result.custom_distbased_elevation_gain_m,
                     result.official_elevation_gain_m,
                     result.distbased_vs_official_diff_m,
                     accuracy_pct);
        }
    }
    
    Ok(())
}

fn process_gpx_file(path: &Path) -> Result<GpxAnalysis, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let gpx = read(reader)?;
    
    let mut coords: Vec<(f64, f64, f64)> = vec![];
    let mut timestamps: Vec<Option<Time>> = vec![];
    
    // Extract coordinates and timestamps from GPX
    for track in gpx.tracks {
        for segment in track.segments {
            for pt in segment.points {
                if let Some(ele) = pt.elevation {
                    let lat = pt.point().y();
                    let lon = pt.point().x();
                    coords.push((lat, lon, ele));
                    timestamps.push(pt.time);
                }
            }
        }
    }
    
    if coords.is_empty() {
        return Err("No valid coordinates found in GPX file".into());
    }
    
    // Analyze timestamps to calculate average interval
    let average_time_interval = calculate_average_time_interval(&timestamps);
    
    println!("  → Average time interval: {} seconds", average_time_interval);
    
    // Compute cumulative distance
    let mut distances = vec![0.0];
    for i in 1..coords.len() {
        let a = point!(x: coords[i-1].1, y: coords[i-1].0);
        let b = point!(x: coords[i].1, y: coords[i].0);
        let dist = a.haversine_distance(&b);
        distances.push(distances[i-1] + dist);
    }
    
    let raw_elevations: Vec<f64> = coords.iter().map(|x| x.2).collect();
    let total_distance_km = distances.last().unwrap() / 1000.0;
    
    // Compute raw elevation gain
    let (raw_gain, _) = gain_loss(&raw_elevations);
    
    // Apply all five custom smoothing variants
    println!("  → Applying Custom Original (83-point/5-point adaptive)...");
    let custom_original = create_custom_original(raw_elevations.clone(), distances.clone());
    let custom_original_gain = custom_original.get_total_elevation_gain();
    
    println!("  → Applying Custom Capping (5-point + capping for ALL)...");
    let custom_capping = create_custom_capping(raw_elevations.clone(), distances.clone());
    let custom_capping_gain = custom_capping.get_total_elevation_gain();
    
    println!("  → Applying Custom Flat21 (21-point/5-point adaptive)...");
    let custom_flat21 = create_custom_flat21(raw_elevations.clone(), distances.clone());
    let custom_flat21_gain = custom_flat21.get_total_elevation_gain();
    
    println!("  → Applying Custom PostCap (5-point + capping + 83-point post-smoothing)...");
    let custom_postcap = create_custom_postcap(raw_elevations.clone(), distances.clone());
    let custom_postcap_gain = custom_postcap.get_total_elevation_gain();
    
    println!("  → Applying Custom DistBased ADAPTIVE (terrain-aware parameters)...");
    let custom_distbased = create_custom_distbased_adaptive(raw_elevations.clone(), distances.clone());
    let custom_distbased_gain = custom_distbased.get_total_elevation_gain();
    
    let filename = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    // Get official elevation gain for comparison
    let official_gain = get_official_elevation_gain(&filename);
    let distbased_vs_official_diff = custom_distbased_gain as i32 - official_gain as i32;
    
    if official_gain > 0 {
        println!("  → DistBased: {}m vs Official: {}m (Diff: {:+}m)", 
                 custom_distbased_gain.round() as u32, official_gain, distbased_vs_official_diff);
    }
    
    Ok(GpxAnalysis {
        filename,
        raw_distance_km: total_distance_km.round() as u32,
        raw_elevation_gain_m: raw_gain.round() as u32,
        average_time_interval_seconds: average_time_interval,
        custom_original_elevation_gain_m: custom_original_gain.round() as u32,
        custom_capping_elevation_gain_m: custom_capping_gain.round() as u32,
        custom_flat21_elevation_gain_m: custom_flat21_gain.round() as u32,
        custom_postcap_elevation_gain_m: custom_postcap_gain.round() as u32,
        custom_distbased_elevation_gain_m: custom_distbased_gain.round() as u32,
        official_elevation_gain_m: official_gain,
        distbased_vs_official_diff_m: distbased_vs_official_diff,
    })
}

fn calculate_average_time_interval(timestamps: &[Option<Time>]) -> u32 {
    // Count valid timestamps
    let valid_timestamps: Vec<&Time> = timestamps.iter()
        .filter_map(|ts| ts.as_ref())
        .collect();
    
    if valid_timestamps.len() < 2 {
        println!("  → No timestamps or insufficient timestamp data");
        return 0;
    }
    
    // Calculate intervals between consecutive timestamps
    let mut intervals = Vec::new();
    for i in 1..valid_timestamps.len() {
        // Convert GPX Time to chrono DateTime for calculation
        let time1_result = valid_timestamps[i-1].format();
        let time2_result = valid_timestamps[i].format();
        
        if let (Ok(time1_str), Ok(time2_str)) = (time1_result, time2_result) {
            // Parse the formatted time strings to chrono DateTime
            let dt1_result = time1_str.parse::<chrono::DateTime<chrono::Utc>>();
            let dt2_result = time2_str.parse::<chrono::DateTime<chrono::Utc>>();
            
            if let (Ok(dt1), Ok(dt2)) = (dt1_result, dt2_result) {
                let interval = dt2.signed_duration_since(dt1);
                let seconds = interval.num_seconds();
                
                // Only include reasonable intervals (between 0.1 and 3600 seconds)
                if seconds > 0 && seconds <= 3600 {
                    intervals.push(seconds as f64);
                }
            }
        }
    }
    
    if intervals.is_empty() {
        println!("  → No valid time intervals found");
        return 0;
    }
    
    // Calculate average interval
    let average_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
    
    // Calculate some statistics for debugging
    let min_interval = intervals.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_interval = intervals.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    println!("  → Timestamp analysis: {} valid intervals, avg={:.1}s, min={:.1}s, max={:.1}s", 
             intervals.len(), average_interval, min_interval, max_interval);
    
    // Return rounded average
    average_interval.round() as u32
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

