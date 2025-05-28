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
mod incline_analyzer;
mod simple_smoother;

use custom_smoother::{create_custom_original, create_custom_distbased_adaptive, ElevationData, SmoothingVariant};
use simple_smoother::{simple_spike_removal_only, calculate_simple_elevation_gain_loss};
use incline_analyzer::{analyze_inclines_default};

#[derive(Debug, Serialize)]
struct GpxAnalysis {
    filename: String,
    raw_distance_km: u32,
    raw_elevation_gain_m: u32,
    average_time_interval_seconds: u32,
    custom_original_elevation_gain_m: u32,
    custom_distbased_elevation_gain_m: u32,
    simple_spike_only_elevation_gain_m: u32,
    spike_distbased_elevation_gain_m: u32,
    spike_original_elevation_gain_m: u32,
    official_elevation_gain_m: u32,
    distbased_vs_official_diff_m: i32,
    spike_distbased_vs_official_diff_m: i32,
    spike_original_vs_official_diff_m: i32,
    longest_incline_length_km: f32,
    longest_incline_gain_m: u32,
    longest_incline_grade_percent: f32,
    longest_decline_length_km: f32,
    longest_decline_loss_m: u32,
    longest_decline_grade_percent: f32,
    total_inclines_count: u32,
    total_declines_count: u32,
    climbing_percentage: f32,
    descending_percentage: f32,
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
    let output_path = Path::new(gpx_folder).join("spike_hybrid_smoothing_analysis.csv");
    
    println!("🔧 SPIKE-ONLY + HYBRID SMOOTHER ANALYSIS");
    println!("===============================================");
    println!("📊 SMOOTHING METHODS:");
    println!("1. Custom Original: Proven adaptive method");
    println!("2. Custom DistBased: Terrain-aware processing");
    println!("3. Simple Spike-Only: GPS spike removal (100% raw after)");
    println!("4. Spike→DistBased: Spike removal + DistBased processing");
    println!("5. Spike→Original: Spike removal + Original processing");
    println!();
    println!("🏔️  INCLINE ANALYSIS:");
    println!("• Longest incline/decline detection");
    println!("• Grade analysis and climbing statistics");
    println!();
    println!("📈 Output: {}", output_path.display());
    println!("===============================================");
    
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
        "Custom DistBased Elevation Gain (m)",
        "Simple Spike Only Elevation Gain (m)",
        "Spike DistBased Elevation Gain (m)",
        "Spike Original Elevation Gain (m)",
        "Official Elevation Gain (m)",
        "DistBased vs Official Diff (m)",
        "Spike DistBased vs Official Diff (m)",
        "Spike Original vs Official Diff (m)",
        "Longest Incline Length (km)",
        "Longest Incline Gain (m)",
        "Longest Incline Grade (%)",
        "Longest Decline Length (km)",
        "Longest Decline Loss (m)",
        "Longest Decline Grade (%)",
        "Total Inclines Count",
        "Total Declines Count",
        "Climbing Percentage (%)",
        "Descending Percentage (%)",
    ])?;
    
    for result in &results {
        wtr.write_record(&[
            &result.filename,
            &result.raw_distance_km.to_string(),
            &result.raw_elevation_gain_m.to_string(),
            &result.average_time_interval_seconds.to_string(),
            &result.custom_original_elevation_gain_m.to_string(),
            &result.custom_distbased_elevation_gain_m.to_string(),
            &result.simple_spike_only_elevation_gain_m.to_string(),
            &result.spike_distbased_elevation_gain_m.to_string(),
            &result.spike_original_elevation_gain_m.to_string(),
            &result.official_elevation_gain_m.to_string(),
            &result.distbased_vs_official_diff_m.to_string(),
            &result.spike_distbased_vs_official_diff_m.to_string(),
            &result.spike_original_vs_official_diff_m.to_string(),
            &result.longest_incline_length_km.to_string(),
            &result.longest_incline_gain_m.to_string(),
            &result.longest_incline_grade_percent.to_string(),
            &result.longest_decline_length_km.to_string(),
            &result.longest_decline_loss_m.to_string(),
            &result.longest_decline_grade_percent.to_string(),
            &result.total_inclines_count.to_string(),
            &result.total_declines_count.to_string(),
            &result.climbing_percentage.to_string(),
            &result.descending_percentage.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    
    println!("\n�� ANALYSIS COMPLETE!");
    println!("📊 Results saved to: {}", output_path.display());
    println!("📁 Processed {} GPX files", results.len());
    
    // Print comparison summaries
    println!("\n🏆 SPIKE-ENHANCED ACCURACY COMPARISON:");
    for result in &results {
        if result.official_elevation_gain_m > 0 {
            let distbased_accuracy = (result.custom_distbased_elevation_gain_m as f64 / result.official_elevation_gain_m as f64) * 100.0;
            let spike_distbased_accuracy = (result.spike_distbased_elevation_gain_m as f64 / result.official_elevation_gain_m as f64) * 100.0;
            let spike_original_accuracy = (result.spike_original_elevation_gain_m as f64 / result.official_elevation_gain_m as f64) * 100.0;
            
            println!("{}: DistBased {:.1}% | Spike+DistBased {:.1}% | Spike+Original {:.1}% | Official {}m", 
                     result.filename,
                     distbased_accuracy,
                     spike_distbased_accuracy,
                     spike_original_accuracy,
                     result.official_elevation_gain_m);
        }
    }
    
    println!("\n🏔️  INCLINE HIGHLIGHTS:");
    for result in &results {
        if result.longest_incline_length_km > 0.0 {
            println!("{}: Longest climb {:.2}km @ {:.1}%, Longest descent {:.2}km @ {:.1}%", 
                     result.filename,
                     result.longest_incline_length_km,
                     result.longest_incline_grade_percent,
                     result.longest_decline_length_km,
                     result.longest_decline_grade_percent);
        }
    }
    
    Ok(())
}

// Hybrid smoother functions
fn spike_then_distbased(raw_elevations: &[f64], distances: &[f64]) -> f64 {
    println!("  🔧 Applying Spike → DistBased hybrid...");
    
    // Step 1: Remove spikes from raw data
    let spike_removed = simple_spike_removal_only(raw_elevations, distances);
    
    // Step 2: Apply DistBased to spike-removed data
    let distbased_result = create_custom_distbased_adaptive(spike_removed, distances.to_vec());
    distbased_result.get_total_elevation_gain()
}

fn spike_then_original(raw_elevations: &[f64], distances: &[f64]) -> f64 {
    println!("  🔧 Applying Spike → Original hybrid...");
    
    // Step 1: Remove spikes from raw data
    let spike_removed = simple_spike_removal_only(raw_elevations, distances);
    
    // Step 2: Apply Original to spike-removed data
    let original_result = create_custom_original(spike_removed, distances.to_vec());
    original_result.get_total_elevation_gain()
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
    
    // Apply all smoothing methods
    println!("  🔄 Applying smoothing methods...");
    
    let custom_original = create_custom_original(raw_elevations.clone(), distances.clone());
    let custom_original_gain = custom_original.get_total_elevation_gain();
    
    let custom_distbased = create_custom_distbased_adaptive(raw_elevations.clone(), distances.clone());
    let custom_distbased_gain = custom_distbased.get_total_elevation_gain();
    
    let simple_spike_only_smoothed = simple_spike_removal_only(&raw_elevations, &distances);
    let (simple_spike_only_gain, _) = calculate_simple_elevation_gain_loss(&simple_spike_only_smoothed);
    
    // Apply hybrid methods
    let spike_distbased_gain = spike_then_distbased(&raw_elevations, &distances);
    let spike_original_gain = spike_then_original(&raw_elevations, &distances);
    
    // Perform incline analysis
    println!("  🏔️  Analyzing inclines and declines...");
    let incline_analysis = analyze_inclines_default(raw_elevations.clone(), distances.clone());
    
    let filename = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    let official_gain = get_official_elevation_gain(&filename);
    let distbased_vs_official_diff = custom_distbased_gain as i32 - official_gain as i32;
    let spike_distbased_vs_official_diff = spike_distbased_gain as i32 - official_gain as i32;
    let spike_original_vs_official_diff = spike_original_gain as i32 - official_gain as i32;
    
    // Extract incline/decline data
    let (longest_incline_length_km, longest_incline_gain_m, longest_incline_grade_percent) = 
        if let Some(ref incline) = incline_analysis.longest_incline {
            (incline.length_km as f32, incline.elevation_gain_m as u32, incline.average_grade_percent as f32)
        } else {
            (0.0, 0, 0.0)
        };
    
    let (longest_decline_length_km, longest_decline_loss_m, longest_decline_grade_percent) = 
        if let Some(ref decline) = incline_analysis.longest_decline {
            (decline.length_km as f32, decline.elevation_loss_m as u32, decline.average_grade_percent as f32)
        } else {
            (0.0, 0, 0.0)
        };
    
    if official_gain > 0 {
        println!("  📊 DistBased: {}m vs Official: {}m ({}m diff)", 
                 custom_distbased_gain.round() as u32, official_gain, distbased_vs_official_diff);
        println!("  📊 Spike+DistBased: {}m vs Official: {}m ({}m diff)", 
                 spike_distbased_gain.round() as u32, official_gain, spike_distbased_vs_official_diff);
        println!("  📊 Spike+Original: {}m vs Official: {}m ({}m diff)",
                 spike_original_gain.round() as u32, official_gain, spike_original_vs_official_diff);
    }
    
    println!("  🏔️  Longest climb: {:.2}km @ {:.1}%, Longest descent: {:.2}km @ {:.1}%",
             longest_incline_length_km, longest_incline_grade_percent,
             longest_decline_length_km, longest_decline_grade_percent);
    
    Ok(GpxAnalysis {
        filename,
        raw_distance_km: total_distance_km.round() as u32,
        raw_elevation_gain_m: raw_gain.round() as u32,
        average_time_interval_seconds: average_time_interval,
        custom_original_elevation_gain_m: custom_original_gain.round() as u32,
        custom_distbased_elevation_gain_m: custom_distbased_gain.round() as u32,
        simple_spike_only_elevation_gain_m: simple_spike_only_gain.round() as u32,
        spike_distbased_elevation_gain_m: spike_distbased_gain.round() as u32,
        spike_original_elevation_gain_m: spike_original_gain.round() as u32,
        official_elevation_gain_m: official_gain,
        distbased_vs_official_diff_m: distbased_vs_official_diff,
        spike_distbased_vs_official_diff_m: spike_distbased_vs_official_diff,
        spike_original_vs_official_diff_m: spike_original_vs_official_diff,
        longest_incline_length_km,
        longest_incline_gain_m,
        longest_incline_grade_percent,
        longest_decline_length_km,
        longest_decline_loss_m,
        longest_decline_grade_percent,
        total_inclines_count: incline_analysis.all_inclines.len() as u32,
        total_declines_count: incline_analysis.all_declines.len() as u32,
        climbing_percentage: incline_analysis.climbing_percentage as f32,
        descending_percentage: incline_analysis.descending_percentage as f32,
    })
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
