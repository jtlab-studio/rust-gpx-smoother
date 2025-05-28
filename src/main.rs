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
mod simple_smoother;
mod smart_spike_removal;

use custom_smoother::{create_custom_original, create_custom_distbased_adaptive, ElevationData, SmoothingVariant};
use smart_spike_removal::GpsQualityMetrics;

#[derive(Debug, Serialize)]
struct GpxAnalysis {
    filename: String,
    raw_distance_km: u32,
    raw_elevation_gain_m: u32,
    average_time_interval_seconds: u32,
    custom_original_elevation_gain_m: u32,
    custom_distbased_10m_elevation_gain_m: u32,
    distbased_1m_interval_elevation_gain_m: u32,
    distbased_1_5m_interval_elevation_gain_m: u32,
    distbased_2m_interval_elevation_gain_m: u32,
    distbased_2_5m_interval_elevation_gain_m: u32,
    distbased_3m_interval_elevation_gain_m: u32,
    distbased_3_5m_interval_elevation_gain_m: u32,
    distbased_4m_interval_elevation_gain_m: u32,
    distbased_4_5m_interval_elevation_gain_m: u32,
    distbased_5m_interval_elevation_gain_m: u32,
    distbased_5_5m_interval_elevation_gain_m: u32,
    distbased_6m_interval_elevation_gain_m: u32,
    distbased_6_5m_interval_elevation_gain_m: u32,
    distbased_7m_interval_elevation_gain_m: u32,
    distbased_12m_interval_elevation_gain_m: u32,
    distbased_15m_interval_elevation_gain_m: u32,
    distbased_20m_interval_elevation_gain_m: u32,
    distbased_30m_interval_elevation_gain_m: u32,
    distbased_50m_interval_elevation_gain_m: u32,
    official_elevation_gain_m: u32,
    original_accuracy_percent: f32,
    distbased_10m_accuracy_percent: f32,
    distbased_1m_accuracy_percent: f32,
    distbased_1_5m_accuracy_percent: f32,
    distbased_2m_accuracy_percent: f32,
    distbased_2_5m_accuracy_percent: f32,
    distbased_3m_accuracy_percent: f32,
    distbased_3_5m_accuracy_percent: f32,
    distbased_4m_accuracy_percent: f32,
    distbased_4_5m_accuracy_percent: f32,
    distbased_5m_accuracy_percent: f32,
    distbased_5_5m_accuracy_percent: f32,
    distbased_6m_accuracy_percent: f32,
    distbased_6_5m_accuracy_percent: f32,
    distbased_7m_accuracy_percent: f32,
    distbased_12m_accuracy_percent: f32,
    distbased_15m_accuracy_percent: f32,
    distbased_20m_accuracy_percent: f32,
    distbased_30m_accuracy_percent: f32,
    distbased_50m_accuracy_percent: f32,
    gps_quality_score: f32,
}

fn get_official_elevation_gain(filename: &str) -> u32 {
    match filename.to_lowercase().as_str() {
        // Original races
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
        
        // Processed races from your data
        "cdh_2024_868f768a27.gpx" => 6400,
        "exp_2024_v1_d870334997.gpx" => 2100,
        "pda_2024_b0233ba7ee.gpx" => 3300,
        "sky_2024_de336280ae.gpx" => 800,
        "vda_2024_5ab5a38e62.gpx" => 10000,
        "kodiak_ultra_marathons_by_utmb_100k.gpx" => 2350,
        "kodiak_ultra_marathons_by_utmb_100_mile.gpx" => 4100,
        "trans_int_160.gpx" => 8980,
        "tokyo-grand-trail-2025-110km.gpx" => 7789,
        "trail-de-haute-provence-2025-thp120.gpx" => 5860,
        "volvic-volcanic-experience-2025.gpx" => 3381,
        
        // New races to be processed
        "chedi_10.gpx" => 300,
        "suthep_20.gpx" => 1190,
        "mut_25_km_2025.gpx" => 850,
        "mut_lite_2025.gpx" => 260,
        "mut_marathon_2025.gpx" => 2300,
        "mut_60.gpx" => 3050,
        "mut_100_m.gpx" => 8100,
        "mut_100_km.gpx" => 4850,
        "mozart100_mozart_100.gpx" => 5800,
        "mozart100_city.gpx" => 300,
        "mozart100_half.gpx" => 1000,
        "mozart100_light.gpx" => 1600,
        "grindstone_utmb_100_mile.gpx" => 6400,
        "wserupdatedaug2024.gpx" => 4960,
        "kat_100miles.gpx" => 9900,
        "mrw_utmb_100_m.gpx" => 8400,
        "eiger250.gpx" => 18000,
        "x-alpine.gpx" => 9300,
        "arc_100.gpx" => 4900,
        "arc_12.gpx" => 500,
        "tarawera_ultra_trail_160km.gpx" => 3700,
        "tarawera_ultra_trail_21km.gpx" => 400,
        "utcc_120_k.gpx" => 5200,
        "cwr_10_k.gpx" => 400,
        "cht_20_k.gpx" => 800,
        "istria_100.gpx" => 7437,
        "istria_21.gpx" => 157,
        "istria_42.gpx" => 1153,
        "x-plore.gpx" => 1700,
        "x-marathon.gpx" => 3000,
        "x-traverse.gpx" => 5300,
        "kat_easy_trail.gpx" => 250,
        "kat100_speed_trail.gpx" => 1650,
        "the-arctic-triple-lofoten-ultra-trail-100-miles.gpx" => 7000,
        "the-arctic-triple-lofoten-ultra-trail-50-miles.gpx" => 3500,
        "k130-1.gpx" => 9500,
        "k31.gpx" => 1816,
        
        // Additional races for completeness
        "mainova-frankfurt-marathon 2023.gpx" => 28,
        "pilolcura.gpx" => 3500,
        "oncol.gpx" => 1600,
        "uka pain 50km.gpx" => 1500,
        
        _ => 0,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpx_folder = r"C:\Users\Dzhu\Documents\GPX Files";
    let output_path = Path::new(gpx_folder).join("comprehensive_granularity_analysis.csv");
    
    println!("🔧 COMPREHENSIVE DISTANCE GRANULARITY ANALYSIS");
    println!("==============================================");
    println!("📊 COMPLETE DISTANCE-BASED PROCESSING INTERVALS:");
    println!("1. Custom Original: Proven adaptive method");
    println!("2. DistBased 10m: Current baseline (terrain-aware)");
    println!("3. 🔬 Ultra-Fine: 1m, 1.5m, 2m, 2.5m");
    println!("4. 🔬 Fine: 3m, 3.5m, 4m, 4.5m");
    println!("5. 🔬 Medium: 5m, 5.5m, 6m, 6.5m, 7m");
    println!("6. 🔬 Coarse: 12m, 15m, 20m, 30m, 50m");
    println!();
    println!("🎯 GOAL: Find absolute optimal granularity across full spectrum");
    println!("📈 Output: {}", output_path.display());
    println!("==============================================");
    
    let mut results = Vec::new();
    
    for entry in WalkDir::new(gpx_folder) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    println!("\n🔄 Processing: {}", entry.path().display());
                    
                    match process_gpx_file(entry.path()) {
                        Ok(analysis) => {
                            results.push(analysis);
                            println!("  ✅ Completed successfully");
                        },
                        Err(e) => {
                            eprintln!("  ❌ Error processing {}: {}", entry.path().display(), e);
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
    
    let mut wtr = Writer::from_path(&output_path)?;
    
    wtr.write_record(&[
        "Filename",
        "Raw Distance (km)", 
        "Raw Elevation Gain (m)",
        "Average Time Interval (seconds)",
        "Custom Original Elevation Gain (m)",
        "Custom DistBased 10m Elevation Gain (m)",
        "DistBased 1m Interval Elevation Gain (m)",
        "DistBased 1.5m Interval Elevation Gain (m)",
        "DistBased 2m Interval Elevation Gain (m)",
        "DistBased 2.5m Interval Elevation Gain (m)",
        "DistBased 3m Interval Elevation Gain (m)",
        "DistBased 3.5m Interval Elevation Gain (m)",
        "DistBased 4m Interval Elevation Gain (m)",
        "DistBased 4.5m Interval Elevation Gain (m)",
        "DistBased 5m Interval Elevation Gain (m)",
        "DistBased 5.5m Interval Elevation Gain (m)",
        "DistBased 6m Interval Elevation Gain (m)",
        "DistBased 6.5m Interval Elevation Gain (m)",
        "DistBased 7m Interval Elevation Gain (m)",
        "DistBased 12m Interval Elevation Gain (m)",
        "DistBased 15m Interval Elevation Gain (m)",
        "DistBased 20m Interval Elevation Gain (m)",
        "DistBased 30m Interval Elevation Gain (m)",
        "DistBased 50m Interval Elevation Gain (m)",
        "Official Elevation Gain (m)",
        "Original Accuracy %",
        "DistBased 10m Accuracy %",
        "DistBased 1m Accuracy %",
        "DistBased 1.5m Accuracy %",
        "DistBased 2m Accuracy %",
        "DistBased 2.5m Accuracy %",
        "DistBased 3m Accuracy %",
        "DistBased 3.5m Accuracy %",
        "DistBased 4m Accuracy %",
        "DistBased 4.5m Accuracy %",
        "DistBased 5m Accuracy %",
        "DistBased 5.5m Accuracy %",
        "DistBased 6m Accuracy %",
        "DistBased 6.5m Accuracy %",
        "DistBased 7m Accuracy %",
        "DistBased 12m Accuracy %",
        "DistBased 15m Accuracy %",
        "DistBased 20m Accuracy %",
        "DistBased 30m Accuracy %",
        "DistBased 50m Accuracy %",
        "GPS Quality Score",
    ])?;
    
    for result in &results {
        wtr.write_record(&[
            &result.filename,
            &result.raw_distance_km.to_string(),
            &result.raw_elevation_gain_m.to_string(),
            &result.average_time_interval_seconds.to_string(),
            &result.custom_original_elevation_gain_m.to_string(),
            &result.custom_distbased_10m_elevation_gain_m.to_string(),
            &result.distbased_1m_interval_elevation_gain_m.to_string(),
            &result.distbased_1_5m_interval_elevation_gain_m.to_string(),
            &result.distbased_2m_interval_elevation_gain_m.to_string(),
            &result.distbased_2_5m_interval_elevation_gain_m.to_string(),
            &result.distbased_3m_interval_elevation_gain_m.to_string(),
            &result.distbased_3_5m_interval_elevation_gain_m.to_string(),
            &result.distbased_4m_interval_elevation_gain_m.to_string(),
            &result.distbased_4_5m_interval_elevation_gain_m.to_string(),
            &result.distbased_5m_interval_elevation_gain_m.to_string(),
            &result.distbased_5_5m_interval_elevation_gain_m.to_string(),
            &result.distbased_6m_interval_elevation_gain_m.to_string(),
            &result.distbased_6_5m_interval_elevation_gain_m.to_string(),
            &result.distbased_7m_interval_elevation_gain_m.to_string(),
            &result.distbased_12m_interval_elevation_gain_m.to_string(),
            &result.distbased_15m_interval_elevation_gain_m.to_string(),
            &result.distbased_20m_interval_elevation_gain_m.to_string(),
            &result.distbased_30m_interval_elevation_gain_m.to_string(),
            &result.distbased_50m_interval_elevation_gain_m.to_string(),
            &result.official_elevation_gain_m.to_string(),
            &format!("{:.1}", result.original_accuracy_percent),
            &format!("{:.1}", result.distbased_10m_accuracy_percent),
            &format!("{:.1}", result.distbased_1m_accuracy_percent),
            &format!("{:.1}", result.distbased_1_5m_accuracy_percent),
            &format!("{:.1}", result.distbased_2m_accuracy_percent),
            &format!("{:.1}", result.distbased_2_5m_accuracy_percent),
            &format!("{:.1}", result.distbased_3m_accuracy_percent),
            &format!("{:.1}", result.distbased_3_5m_accuracy_percent),
            &format!("{:.1}", result.distbased_4m_accuracy_percent),
            &format!("{:.1}", result.distbased_4_5m_accuracy_percent),
            &format!("{:.1}", result.distbased_5m_accuracy_percent),
            &format!("{:.1}", result.distbased_5_5m_accuracy_percent),
            &format!("{:.1}", result.distbased_6m_accuracy_percent),
            &format!("{:.1}", result.distbased_6_5m_accuracy_percent),
            &format!("{:.1}", result.distbased_7m_accuracy_percent),
            &format!("{:.1}", result.distbased_12m_accuracy_percent),
            &format!("{:.1}", result.distbased_15m_accuracy_percent),
            &format!("{:.1}", result.distbased_20m_accuracy_percent),
            &format!("{:.1}", result.distbased_30m_accuracy_percent),
            &format!("{:.1}", result.distbased_50m_accuracy_percent),
            &result.gps_quality_score.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    
    println!("\n🎉 COMPREHENSIVE GRANULARITY ANALYSIS COMPLETE!");
    println!("📊 Results saved to: {}", output_path.display());
    println!("📁 Processed {} GPX files", results.len());
    
    // Print comprehensive granularity comparison summaries
    println!("\n🏆 COMPREHENSIVE DISTANCE GRANULARITY ACCURACY COMPARISON:");
    let mut interval_winners = [0; 20]; // Track winners for all intervals
    let interval_names = ["1m", "1.5m", "2m", "2.5m", "3m", "3.5m", "4m", "4.5m", "5m", "5.5m", 
                         "6m", "6.5m", "7m", "10m", "12m", "15m", "20m", "30m", "50m", "Original"];
    
    for result in &results {
        if result.official_elevation_gain_m > 0 {
            let accuracies = [
                result.distbased_1m_accuracy_percent,
                result.distbased_1_5m_accuracy_percent,
                result.distbased_2m_accuracy_percent,
                result.distbased_2_5m_accuracy_percent,
                result.distbased_3m_accuracy_percent,
                result.distbased_3_5m_accuracy_percent,
                result.distbased_4m_accuracy_percent,
                result.distbased_4_5m_accuracy_percent,
                result.distbased_5m_accuracy_percent,
                result.distbased_5_5m_accuracy_percent,
                result.distbased_6m_accuracy_percent,
                result.distbased_6_5m_accuracy_percent,
                result.distbased_7m_accuracy_percent,
                result.distbased_10m_accuracy_percent,
                result.distbased_12m_accuracy_percent,
                result.distbased_15m_accuracy_percent,
                result.distbased_20m_accuracy_percent,
                result.distbased_30m_accuracy_percent,
                result.distbased_50m_accuracy_percent,
                result.original_accuracy_percent,
            ];
            
            let deviations: Vec<f32> = accuracies.iter()
                .map(|&acc| (acc - 100.0).abs())
                .collect();
            
            let min_deviation = deviations.iter().cloned().fold(f32::INFINITY, f32::min);
            let best_index = deviations.iter().position(|&d| (d - min_deviation).abs() < 0.01).unwrap();
            interval_winners[best_index] += 1;
            
            println!("{}: Official {}m | Winner: {} ({:.1}%)", 
                     result.filename.replace(".gpx", "").chars().take(30).collect::<String>(),
                     result.official_elevation_gain_m,
                     interval_names[best_index],
                     accuracies[best_index]);
        }
    }
    
    println!("\n📈 COMPREHENSIVE GRANULARITY PERFORMANCE SUMMARY:");
    for (i, &wins) in interval_winners.iter().enumerate() {
        if wins > 0 {
            println!("{} interval most accurate: {} routes", interval_names[i], wins);
        }
    }
    
    Ok(())
}

// Distance-based processing with custom intervals
fn distbased_with_interval(raw_elevations: &[f64], distances: &[f64], interval_meters: f64) -> f64 {
    println!("  🔬 Applying DistBased processing with {:.1}m intervals...", interval_meters);
    
    // Create a custom ElevationData with modified interval
    let mut elevation_data = ElevationData::new_with_variant(
        raw_elevations.to_vec(), 
        distances.to_vec(), 
        SmoothingVariant::DistBased
    );
    
    // Override the distance-based processing with custom interval
    elevation_data.apply_custom_interval_processing(interval_meters);
    
    elevation_data.get_total_elevation_gain()
}

fn calculate_accuracy_percent(predicted: u32, official: u32) -> f32 {
    if official == 0 {
        0.0
    } else {
        (predicted as f32 / official as f32) * 100.0
    }
}

fn process_gpx_file(path: &Path) -> Result<GpxAnalysis, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let gpx = read(reader)?;
    
    let mut coords: Vec<(f64, f64, f64)> = vec![];
    let mut timestamps: Vec<Option<Time>> = vec![];
    
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
    
    let average_time_interval = calculate_average_time_interval(&timestamps);
    
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
    
    println!("  📍 Distance: {:.1}km, Raw elevation gain: {:.0}m", total_distance_km, raw_gain);
    
    // Convert timestamps for quality analysis
    let timestamp_seconds: Option<Vec<f64>> = if timestamps.iter().any(|t| t.is_some()) {
        Some(convert_timestamps_to_seconds(&timestamps))
    } else {
        None
    };
    
    // Apply all processing methods
    println!("  🔄 Applying comprehensive distance-based processing methods...");
    
    let custom_original = create_custom_original(raw_elevations.clone(), distances.clone());
    let custom_original_gain = custom_original.get_total_elevation_gain();
    
    let custom_distbased_10m = create_custom_distbased_adaptive(raw_elevations.clone(), distances.clone());
    let custom_distbased_10m_gain = custom_distbased_10m.get_total_elevation_gain();
    
    // Apply all distance interval methods - comprehensive set
    let distbased_1m_gain = distbased_with_interval(&raw_elevations, &distances, 1.0);
    let distbased_1_5m_gain = distbased_with_interval(&raw_elevations, &distances, 1.5);
    let distbased_2m_gain = distbased_with_interval(&raw_elevations, &distances, 2.0);
    let distbased_2_5m_gain = distbased_with_interval(&raw_elevations, &distances, 2.5);
    let distbased_3m_gain = distbased_with_interval(&raw_elevations, &distances, 3.0);
    let distbased_3_5m_gain = distbased_with_interval(&raw_elevations, &distances, 3.5);
    let distbased_4m_gain = distbased_with_interval(&raw_elevations, &distances, 4.0);
    let distbased_4_5m_gain = distbased_with_interval(&raw_elevations, &distances, 4.5);
    let distbased_5m_gain = distbased_with_interval(&raw_elevations, &distances, 5.0);
    let distbased_5_5m_gain = distbased_with_interval(&raw_elevations, &distances, 5.5);
    let distbased_6m_gain = distbased_with_interval(&raw_elevations, &distances, 6.0);
    let distbased_6_5m_gain = distbased_with_interval(&raw_elevations, &distances, 6.5);
    let distbased_7m_gain = distbased_with_interval(&raw_elevations, &distances, 7.0);
    let distbased_12m_gain = distbased_with_interval(&raw_elevations, &distances, 12.0);
    let distbased_15m_gain = distbased_with_interval(&raw_elevations, &distances, 15.0);
    let distbased_20m_gain = distbased_with_interval(&raw_elevations, &distances, 20.0);
    let distbased_30m_gain = distbased_with_interval(&raw_elevations, &distances, 30.0);
    let distbased_50m_gain = distbased_with_interval(&raw_elevations, &distances, 50.0);
    
    // Calculate GPS quality score for reporting
    let quality_metrics = GpsQualityMetrics::analyze_gps_quality(
        &raw_elevations, &distances, timestamp_seconds.as_deref()
    );
    
    let filename = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    let official_gain = get_official_elevation_gain(&filename);
    
    // Calculate accuracy percentages for all methods
    let original_accuracy = calculate_accuracy_percent(custom_original_gain.round() as u32, official_gain);
    let distbased_10m_accuracy = calculate_accuracy_percent(custom_distbased_10m_gain.round() as u32, official_gain);
    let distbased_1m_accuracy = calculate_accuracy_percent(distbased_1m_gain.round() as u32, official_gain);
    let distbased_1_5m_accuracy = calculate_accuracy_percent(distbased_1_5m_gain.round() as u32, official_gain);
    let distbased_2m_accuracy = calculate_accuracy_percent(distbased_2m_gain.round() as u32, official_gain);
    let distbased_2_5m_accuracy = calculate_accuracy_percent(distbased_2_5m_gain.round() as u32, official_gain);
    let distbased_3m_accuracy = calculate_accuracy_percent(distbased_3m_gain.round() as u32, official_gain);
    let distbased_3_5m_accuracy = calculate_accuracy_percent(distbased_3_5m_gain.round() as u32, official_gain);
    let distbased_4m_accuracy = calculate_accuracy_percent(distbased_4m_gain.round() as u32, official_gain);
    let distbased_4_5m_accuracy = calculate_accuracy_percent(distbased_4_5m_gain.round() as u32, official_gain);
    let distbased_5m_accuracy = calculate_accuracy_percent(distbased_5m_gain.round() as u32, official_gain);
    let distbased_5_5m_accuracy = calculate_accuracy_percent(distbased_5_5m_gain.round() as u32, official_gain);
    let distbased_6m_accuracy = calculate_accuracy_percent(distbased_6m_gain.round() as u32, official_gain);
    let distbased_6_5m_accuracy = calculate_accuracy_percent(distbased_6_5m_gain.round() as u32, official_gain);
    let distbased_7m_accuracy = calculate_accuracy_percent(distbased_7m_gain.round() as u32, official_gain);
    let distbased_12m_accuracy = calculate_accuracy_percent(distbased_12m_gain.round() as u32, official_gain);
    let distbased_15m_accuracy = calculate_accuracy_percent(distbased_15m_gain.round() as u32, official_gain);
    let distbased_20m_accuracy = calculate_accuracy_percent(distbased_20m_gain.round() as u32, official_gain);
    let distbased_30m_accuracy = calculate_accuracy_percent(distbased_30m_gain.round() as u32, official_gain);
    let distbased_50m_accuracy = calculate_accuracy_percent(distbased_50m_gain.round() as u32, official_gain);
    
    if official_gain > 0 {
        println!("  📊 Sample Accuracies vs Official {}m:", official_gain);
        println!("    1m: {:.1}% | 3m: {:.1}% | 6m: {:.1}% | 10m: {:.1}% | 20m: {:.1}% | 50m: {:.1}%", 
                 distbased_1m_accuracy, distbased_3m_accuracy, distbased_6m_accuracy, 
                 distbased_10m_accuracy, distbased_20m_accuracy, distbased_50m_accuracy);
        println!("  🎯 GPS Quality Score: {:.2}", quality_metrics.quality_score);
    }
    
    Ok(GpxAnalysis {
        filename,
        raw_distance_km: total_distance_km.round() as u32,
        raw_elevation_gain_m: raw_gain.round() as u32,
        average_time_interval_seconds: average_time_interval,
        custom_original_elevation_gain_m: custom_original_gain.round() as u32,
        custom_distbased_10m_elevation_gain_m: custom_distbased_10m_gain.round() as u32,
        distbased_1m_interval_elevation_gain_m: distbased_1m_gain.round() as u32,
        distbased_1_5m_interval_elevation_gain_m: distbased_1_5m_gain.round() as u32,
        distbased_2m_interval_elevation_gain_m: distbased_2m_gain.round() as u32,
        distbased_2_5m_interval_elevation_gain_m: distbased_2_5m_gain.round() as u32,
        distbased_3m_interval_elevation_gain_m: distbased_3m_gain.round() as u32,
        distbased_3_5m_interval_elevation_gain_m: distbased_3_5m_gain.round() as u32,
        distbased_4m_interval_elevation_gain_m: distbased_4m_gain.round() as u32,
        distbased_4_5m_interval_elevation_gain_m: distbased_4_5m_gain.round() as u32,
        distbased_5m_interval_elevation_gain_m: distbased_5m_gain.round() as u32,
        distbased_5_5m_interval_elevation_gain_m: distbased_5_5m_gain.round() as u32,
        distbased_6m_interval_elevation_gain_m: distbased_6m_gain.round() as u32,
        distbased_6_5m_interval_elevation_gain_m: distbased_6_5m_gain.round() as u32,
        distbased_7m_interval_elevation_gain_m: distbased_7m_gain.round() as u32,
        distbased_12m_interval_elevation_gain_m: distbased_12m_gain.round() as u32,
        distbased_15m_interval_elevation_gain_m: distbased_15m_gain.round() as u32,
        distbased_20m_interval_elevation_gain_m: distbased_20m_gain.round() as u32,
        distbased_30m_interval_elevation_gain_m: distbased_30m_gain.round() as u32,
        distbased_50m_interval_elevation_gain_m: distbased_50m_gain.round() as u32,
        official_elevation_gain_m: official_gain,
        original_accuracy_percent: original_accuracy,
        distbased_10m_accuracy_percent: distbased_10m_accuracy,
        distbased_1m_accuracy_percent: distbased_1m_accuracy,
        distbased_1_5m_accuracy_percent: distbased_1_5m_accuracy,
        distbased_2m_accuracy_percent: distbased_2m_accuracy,
        distbased_2_5m_accuracy_percent: distbased_2_5m_accuracy,
        distbased_3m_accuracy_percent: distbased_3m_accuracy,
        distbased_3_5m_accuracy_percent: distbased_3_5m_accuracy,
        distbased_4m_accuracy_percent: distbased_4m_accuracy,
        distbased_4_5m_accuracy_percent: distbased_4_5m_accuracy,
        distbased_5m_accuracy_percent: distbased_5m_accuracy,
        distbased_5_5m_accuracy_percent: distbased_5_5m_accuracy,
        distbased_6m_accuracy_percent: distbased_6m_accuracy,
        distbased_6_5m_accuracy_percent: distbased_6_5m_accuracy,
        distbased_7m_accuracy_percent: distbased_7m_accuracy,
        distbased_12m_accuracy_percent: distbased_12m_accuracy,
        distbased_15m_accuracy_percent: distbased_15m_accuracy,
        distbased_20m_accuracy_percent: distbased_20m_accuracy,
        distbased_30m_accuracy_percent: distbased_30m_accuracy,
        distbased_50m_accuracy_percent: distbased_50m_accuracy,
        gps_quality_score: quality_metrics.quality_score as f32,
    })
}

fn convert_timestamps_to_seconds(timestamps: &[Option<Time>]) -> Vec<f64> {
    let mut seconds = Vec::new();
    let mut base_time: Option<chrono::DateTime<chrono::Utc>> = None;
    
    for ts_opt in timestamps {
        if let Some(ts) = ts_opt {
            if let Ok(time_str) = ts.format() {
                if let Ok(dt) = time_str.parse::<chrono::DateTime<chrono::Utc>>() {
                    if base_time.is_none() {
                        base_time = Some(dt);
                        seconds.push(0.0);
                    } else {
                        let elapsed = dt.signed_duration_since(base_time.unwrap());
                        seconds.push(elapsed.num_milliseconds() as f64 / 1000.0);
                    }
                }
            }
        }
    }
    
    seconds
}

fn calculate_average_time_interval(timestamps: &[Option<Time>]) -> u32 {
    let valid_timestamps: Vec<&Time> = timestamps.iter()
        .filter_map(|ts| ts.as_ref())
        .collect();
    
    if valid_timestamps.len() < 2 {
        return 0;
    }
    
    let mut intervals = Vec::new();
    for i in 1..valid_timestamps.len() {
        let time1_result = valid_timestamps[i-1].format();
        let time2_result = valid_timestamps[i].format();
        
        if let (Ok(time1_str), Ok(time2_str)) = (time1_result, time2_result) {
            let dt1_result = time1_str.parse::<chrono::DateTime<chrono::Utc>>();
            let dt2_result = time2_str.parse::<chrono::DateTime<chrono::Utc>>();
            
            if let (Ok(dt1), Ok(dt2)) = (dt1_result, dt2_result) {
                let interval = dt2.signed_duration_since(dt1);
                let seconds = interval.num_seconds();
                
                if seconds > 0 && seconds <= 3600 {
                    intervals.push(seconds as f64);
                }
            }
        }
    }
    
    if intervals.is_empty() {
        return 0;
    }
    
    let average_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
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