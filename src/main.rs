use std::{fs::File, path::Path};
use gpx::read;
use geo::HaversineDistance;
use geo::point;
use std::io::BufReader;
use walkdir::WalkDir;
use csv::Writer;
use serde::Serialize;

mod custom_smoother;
mod combo_smoother;
mod enhanced_combo_smoother;
mod hybrid_smoother;
mod advanced_smoother;
mod terrain_elevation;

use custom_smoother::ElevationData;
use combo_smoother::{universal_smooth, calculate_elevation_gain_loss};
use enhanced_combo_smoother::{
    enhanced_universal_smooth_conservative, 
    enhanced_universal_smooth_moderate, 
    enhanced_universal_smooth_experimental,
    calculate_elevation_gain_loss_enhanced
};
use hybrid_smoother::{
    hybrid_smooth_auto,
    hybrid_smooth_conservative,
    hybrid_smooth_aggressive,
    calculate_hybrid_elevation_gain_loss
};
use advanced_smoother::{
    advanced_smooth_conservative,
    advanced_smooth_moderate,
    advanced_smooth_aggressive,
    calculate_advanced_elevation_gain_loss
};
use terrain_elevation::{
    terrain_smooth_conservative,
    terrain_smooth_moderate,
    terrain_smooth_high_accuracy,
    calculate_terrain_elevation_gain_loss
};

#[derive(Debug, Serialize)]
struct GpxAnalysis {
    filename: String,
    raw_distance_km: u32,
    raw_elevation_gain_m: u32,
    custom_elevation_gain_m: u32,
    combo_elevation_gain_m: u32,
    enhanced_combo_conservative_elevation_gain_m: u32,
    enhanced_combo_moderate_elevation_gain_m: u32,
    enhanced_combo_experimental_elevation_gain_m: u32,
    hybrid_auto_elevation_gain_m: u32,
    hybrid_conservative_elevation_gain_m: u32,
    hybrid_aggressive_elevation_gain_m: u32,
    advanced_conservative_elevation_gain_m: u32,
    advanced_moderate_elevation_gain_m: u32,
    advanced_aggressive_elevation_gain_m: u32,
    terrain_conservative_elevation_gain_m: u32,
    terrain_moderate_elevation_gain_m: u32,
    terrain_high_accuracy_elevation_gain_m: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpx_folder = r"C:\Users\Dzhu\Documents\GPX Files";
    let output_path = Path::new(gpx_folder).join("comprehensive_data.csv");
    
    println!("Analyzing GPX files with all smoothing methods including terrain-based correction...");
    println!("Output will be saved to: {}", output_path.display());
    println!("Note: Terrain-based methods require internet connection for DEM tiles");
    
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
    
    // Write header (elevation gain only)
    wtr.write_record(&[
        "Filename",
        "Raw Distance (km)", 
        "Raw Elevation Gain (m)",
        "Custom Elevation Gain (m)",
        "Combo Elevation Gain (m)",
        "Enhanced Combo Conservative Elevation Gain (m)",
        "Enhanced Combo Moderate Elevation Gain (m)",
        "Enhanced Combo Experimental Elevation Gain (m)",
        "Hybrid Auto Elevation Gain (m)",
        "Hybrid Conservative Elevation Gain (m)",
        "Hybrid Aggressive Elevation Gain (m)",
        "Advanced Conservative Elevation Gain (m)",
        "Advanced Moderate Elevation Gain (m)",
        "Advanced Aggressive Elevation Gain (m)",
        "Terrain Conservative Elevation Gain (m)",
        "Terrain Moderate Elevation Gain (m)",
        "Terrain High Accuracy Elevation Gain (m)"
    ])?;
    
    // Store count before consuming the vector
    let file_count = results.len();
    
    // Write data (elevation gain only)
    for result in results {
        wtr.write_record(&[
            &result.filename,
            &result.raw_distance_km.to_string(),
            &result.raw_elevation_gain_m.to_string(),
            &result.custom_elevation_gain_m.to_string(),
            &result.combo_elevation_gain_m.to_string(),
            &result.enhanced_combo_conservative_elevation_gain_m.to_string(),
            &result.enhanced_combo_moderate_elevation_gain_m.to_string(),
            &result.enhanced_combo_experimental_elevation_gain_m.to_string(),
            &result.hybrid_auto_elevation_gain_m.to_string(),
            &result.hybrid_conservative_elevation_gain_m.to_string(),
            &result.hybrid_aggressive_elevation_gain_m.to_string(),
            &result.advanced_conservative_elevation_gain_m.to_string(),
            &result.advanced_moderate_elevation_gain_m.to_string(),
            &result.advanced_aggressive_elevation_gain_m.to_string(),
            &result.terrain_conservative_elevation_gain_m.to_string(),
            &result.terrain_moderate_elevation_gain_m.to_string(),
            &result.terrain_high_accuracy_elevation_gain_m.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    
    println!("\nAnalysis complete! Results saved to: {}", output_path.display());
    println!("Processed {} GPX files", file_count);
    Ok(())
}

fn process_gpx_file(path: &Path) -> Result<GpxAnalysis, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let gpx = read(reader)?;
    
    let mut coords: Vec<(f64, f64, f64)> = vec![];
    
    // Extract coordinates from GPX
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
    
    // Apply all existing smoothing methods
    let custom_data = ElevationData::new(raw_elevations.clone(), distances.clone());
    let custom_gain = custom_data.get_total_elevation_gain();
    
    let combo_elevations = universal_smooth(&distances, &raw_elevations);
    let (combo_gain, _) = calculate_elevation_gain_loss(&combo_elevations);
    
    let enhanced_combo_conservative_elevations = enhanced_universal_smooth_conservative(&distances, &raw_elevations);
    let (enhanced_combo_conservative_gain, _) = 
        calculate_elevation_gain_loss_enhanced(&enhanced_combo_conservative_elevations);
    
    let enhanced_combo_moderate_elevations = enhanced_universal_smooth_moderate(&distances, &raw_elevations);
    let (enhanced_combo_moderate_gain, _) = 
        calculate_elevation_gain_loss_enhanced(&enhanced_combo_moderate_elevations);
    
    let enhanced_combo_experimental_elevations = enhanced_universal_smooth_experimental(&distances, &raw_elevations);
    let (enhanced_combo_experimental_gain, _) = 
        calculate_elevation_gain_loss_enhanced(&enhanced_combo_experimental_elevations);
    
    let hybrid_auto_elevations = hybrid_smooth_auto(&distances, &raw_elevations);
    let (hybrid_auto_gain, _) = calculate_hybrid_elevation_gain_loss(&hybrid_auto_elevations);
    
    let hybrid_conservative_elevations = hybrid_smooth_conservative(&distances, &raw_elevations);
    let (hybrid_conservative_gain, _) = calculate_hybrid_elevation_gain_loss(&hybrid_conservative_elevations);
    
    let hybrid_aggressive_elevations = hybrid_smooth_aggressive(&distances, &raw_elevations);
    let (hybrid_aggressive_gain, _) = calculate_hybrid_elevation_gain_loss(&hybrid_aggressive_elevations);
    
    let advanced_conservative_elevations = advanced_smooth_conservative(&distances, &raw_elevations);
    let (advanced_conservative_gain, _) = calculate_advanced_elevation_gain_loss(&advanced_conservative_elevations);
    
    let advanced_moderate_elevations = advanced_smooth_moderate(&distances, &raw_elevations);
    let (advanced_moderate_gain, _) = calculate_advanced_elevation_gain_loss(&advanced_moderate_elevations);
    
    let advanced_aggressive_elevations = advanced_smooth_aggressive(&distances, &raw_elevations);
    let (advanced_aggressive_gain, _) = calculate_advanced_elevation_gain_loss(&advanced_aggressive_elevations);
    
    // Apply terrain-based elevation correction (NEW)
    println!("  → Applying terrain-based elevation correction...");
    
    let terrain_conservative_elevations = terrain_smooth_conservative(&distances, &coords);
    let (terrain_conservative_gain, _) = calculate_terrain_elevation_gain_loss(&terrain_conservative_elevations);
    
    let terrain_moderate_elevations = terrain_smooth_moderate(&distances, &coords);
    let (terrain_moderate_gain, _) = calculate_terrain_elevation_gain_loss(&terrain_moderate_elevations);
    
    let terrain_high_accuracy_elevations = terrain_smooth_high_accuracy(&distances, &coords);
    let (terrain_high_accuracy_gain, _) = calculate_terrain_elevation_gain_loss(&terrain_high_accuracy_elevations);
    
    let filename = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    Ok(GpxAnalysis {
        filename,
        raw_distance_km: total_distance_km.round() as u32,
        raw_elevation_gain_m: raw_gain.round() as u32,
        custom_elevation_gain_m: custom_gain.round() as u32,
        combo_elevation_gain_m: combo_gain.round() as u32,
        enhanced_combo_conservative_elevation_gain_m: enhanced_combo_conservative_gain.round() as u32,
        enhanced_combo_moderate_elevation_gain_m: enhanced_combo_moderate_gain.round() as u32,
        enhanced_combo_experimental_elevation_gain_m: enhanced_combo_experimental_gain.round() as u32,
        hybrid_auto_elevation_gain_m: hybrid_auto_gain.round() as u32,
        hybrid_conservative_elevation_gain_m: hybrid_conservative_gain.round() as u32,
        hybrid_aggressive_elevation_gain_m: hybrid_aggressive_gain.round() as u32,
        advanced_conservative_elevation_gain_m: advanced_conservative_gain.round() as u32,
        advanced_moderate_elevation_gain_m: advanced_moderate_gain.round() as u32,
        advanced_aggressive_elevation_gain_m: advanced_aggressive_gain.round() as u32,
        terrain_conservative_elevation_gain_m: terrain_conservative_gain.round() as u32,
        terrain_moderate_elevation_gain_m: terrain_moderate_gain.round() as u32,
        terrain_high_accuracy_elevation_gain_m: terrain_high_accuracy_gain.round() as u32,
    })
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
