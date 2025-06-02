use std::collections::HashMap;
use csv::Reader;
use serde::Deserialize;
use std::fs::File;
use std::path::Path;

mod tolerant_gpx_reader;
mod garmin_23m_processor;

#[derive(Debug, Deserialize)]
struct OfficialElevationRecord {
    filename: String,
    official_elevation_gain_m: u32,
    #[serde(default)]
    source: String,
    #[serde(default)]
    notes: String,
}

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
        println!("⚠️  No official elevation data CSV found, continuing without accuracy comparison");
    }
    
    Ok(official_data)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_folder = r"C:\Users\Dzhu\Documents\GPX Files";
    let output_folder = r"C:\Users\Dzhu\Documents\GPX Files\Processed";
    
    // Simple menu for GPX processing options
    println!("\n🏔️  GPX ELEVATION PROCESSOR");
    println!("==========================");
    println!("🎯 Garmin-like processing with 23m interval");
    println!("• Minimal smoothing preserves natural terrain");
    println!("• Distance-based resampling for consistency");
    println!("• Removes GPS spikes while keeping detail");
    println!("• Saves processed GPX files for GPS devices");
    println!("");
    
    println!("📁 Input folder: {}", input_folder);
    println!("📁 Output folder: {}", output_folder);
    println!("");
    
    println!("Choose an option:");
    println!("1. 🚀 Process all GPX files with 23m Garmin-like algorithm");
    println!("2. 🧪 Test processing on first 5 files only");
    println!("3. 📊 Analyze accuracy without saving files");
    println!("4. 🔍 Check input folder contents");
    println!("5. 📈 Segment processed GPX files by gradient bands");
    println!("");
    
    use std::io::{self, Write};
    print!("Choice (1-5, or Enter to exit): ");
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let choice = input.trim();
    
    match choice {
        "1" => {
            println!("\n🚀 Processing ALL GPX files with 23m algorithm...");
            garmin_23m_processor::run_garmin_23m_processing(input_folder, output_folder)?;
        },
        "2" => {
            println!("\n🧪 Testing on first 5 files only...");
            run_test_processing(input_folder, output_folder)?;
        },
        "3" => {
            println!("\n📊 Running accuracy analysis without saving files...");
            run_analysis_only(input_folder)?;
        },
        "4" => {
            println!("\n🔍 Checking input folder contents...");
            check_folder_contents(input_folder)?;
        },
        "5" => {
            println!("\n📈 Segmenting processed GPX files by gradient bands...");
            run_gradient_segmentation(output_folder)?;
        },
        "" => {
            println!("👋 Exiting.");
        },
        _ => {
            println!("ℹ️  Invalid option. Choose 1-5 or press Enter to exit.");
        }
    }
    
    Ok(())
}

fn run_test_processing(input_folder: &str, output_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    use walkdir::WalkDir;
    use std::fs;
    
    // Create output directory
    fs::create_dir_all(output_folder)?;
    
    // Find first 5 GPX files
    let mut gpx_files = Vec::new();
    for entry in WalkDir::new(input_folder).max_depth(1) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    gpx_files.push(entry.path().to_path_buf());
                    if gpx_files.len() >= 5 {
                        break;
                    }
                }
            }
        }
    }
    
    if gpx_files.is_empty() {
        println!("❌ No GPX files found in: {}", input_folder);
        return Ok(());
    }
    
    println!("🔍 Found {} files, processing first {}...", 
             if gpx_files.len() < 5 { gpx_files.len() } else { 5 }, 
             gpx_files.len().min(5));
    
    // Create a temporary folder with just these files
    let temp_folder = format!("{}_test", input_folder);
    fs::create_dir_all(&temp_folder)?;
    
    for (i, file) in gpx_files.iter().take(5).enumerate() {
        let filename = file.file_name().unwrap();
        let dest = Path::new(&temp_folder).join(filename);
        fs::copy(file, dest)?;
        println!("{}. Copied: {}", i + 1, filename.to_string_lossy());
    }
    
    // Process the test files
    garmin_23m_processor::run_garmin_23m_processing(&temp_folder, output_folder)?;
    
    // Cleanup temp folder
    fs::remove_dir_all(&temp_folder)?;
    
    println!("✅ Test processing complete!");
    Ok(())
}

fn run_analysis_only(input_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    use walkdir::WalkDir;
    
    println!("📊 Analyzing GPX files for elevation accuracy...");
    
    let official_data = load_official_elevation_data()?;
    
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
    
    if gpx_files.is_empty() {
        println!("❌ No GPX files found");
        return Ok(());
    }
    
    println!("🔍 Found {} GPX files", gpx_files.len());
    
    let mut files_with_official = 0;
    let mut total_raw_accuracy = 0.0;
    let mut total_processed_accuracy = 0.0;
    
    for (i, gpx_path) in gpx_files.iter().take(10).enumerate() {
        let filename = gpx_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        println!("\n{}. Analyzing: {}", i + 1, filename);
        
        if let Ok((raw_gain, processed_gain)) = analyze_file_accuracy(gpx_path, &official_data) {
            let clean_filename = filename.to_lowercase().replace("_processed.gpx", ".gpx");
            
            if let Some(&official_gain) = official_data.get(&clean_filename) {
                if official_gain > 0 {
                    let raw_accuracy = (raw_gain / official_gain as f64) * 100.0;
                    let processed_accuracy = (processed_gain / official_gain as f64) * 100.0;
                    
                    println!("   Official: {}m", official_gain);
                    println!("   Raw: {:.1}m ({:.1}%)", raw_gain, raw_accuracy);
                    println!("   Processed: {:.1}m ({:.1}%)", processed_gain, processed_accuracy);
                    
                    total_raw_accuracy += raw_accuracy;
                    total_processed_accuracy += processed_accuracy;
                    files_with_official += 1;
                }
            } else {
                println!("   Raw: {:.1}m", raw_gain);
                println!("   Processed: {:.1}m", processed_gain);
                println!("   No official data for comparison");
            }
        } else {
            println!("   ❌ Failed to analyze");
        }
    }
    
    if files_with_official > 0 {
        println!("\n📊 SUMMARY (first 10 files):");
        println!("• Files with official data: {}", files_with_official);
        println!("• Average raw accuracy: {:.1}%", total_raw_accuracy / files_with_official as f64);
        println!("• Average processed accuracy: {:.1}%", total_processed_accuracy / files_with_official as f64);
    }
    
    Ok(())
}

fn analyze_file_accuracy(
    gpx_path: &Path, 
    _official_data: &HashMap<String, u32>
) -> Result<(f64, f64), Box<dyn std::error::Error>> {
    use geo::{HaversineDistance, point};
    
    let gpx = tolerant_gpx_reader::read_gpx_tolerantly(gpx_path)?;
    
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
    
    // Calculate raw gain
    let raw_gain = calculate_elevation_gain(&elevations);
    
    // Simulate 23m processing
    let processed_elevations = simulate_23m_processing(&elevations, &distances);
    let processed_gain = calculate_elevation_gain(&processed_elevations);
    
    Ok((raw_gain, processed_gain))
}

fn calculate_elevation_gain(elevations: &[f64]) -> f64 {
    if elevations.len() < 2 {
        return 0.0;
    }
    
    let mut gain = 0.0;
    for window in elevations.windows(2) {
        let change = window[1] - window[0];
        if change > 0.0 {
            gain += change;
        }
    }
    gain
}

fn simulate_23m_processing(elevations: &[f64], distances: &[f64]) -> Vec<f64> {
    // Simple simulation of 23m resampling + light smoothing
    if elevations.is_empty() {
        return vec![];
    }
    
    let resampled = resample_elevations(elevations, distances, 23.0);
    apply_simple_smoothing(&resampled, 5)
}

fn resample_elevations(elevations: &[f64], distances: &[f64], interval: f64) -> Vec<f64> {
    if elevations.is_empty() || distances.is_empty() {
        return elevations.to_vec();
    }
    
    let total_distance = distances.last().unwrap();
    let num_points = (total_distance / interval).ceil() as usize + 1;
    
    if num_points > 10000 {  // Prevent excessive memory usage
        return elevations.to_vec();
    }
    
    let mut resampled = Vec::new();
    
    for i in 0..num_points {
        let target_distance = i as f64 * interval;
        if target_distance > *total_distance {
            break;
        }
        
        let elevation = interpolate_elevation(elevations, distances, target_distance);
        resampled.push(elevation);
    }
    
    resampled
}

fn interpolate_elevation(elevations: &[f64], distances: &[f64], target_distance: f64) -> f64 {
    if target_distance <= 0.0 {
        return elevations.first().copied().unwrap_or(0.0);
    }
    
    if target_distance >= *distances.last().unwrap() {
        return elevations.last().copied().unwrap_or(0.0);
    }
    
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

fn apply_simple_smoothing(data: &[f64], window: usize) -> Vec<f64> {
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

fn check_folder_contents(input_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    use walkdir::WalkDir;
    
    println!("📂 Checking folder: {}", input_folder);
    
    if !Path::new(input_folder).exists() {
        println!("❌ Folder does not exist!");
        return Ok(());
    }
    
    let mut gpx_files = Vec::new();
    let mut other_files = Vec::new();
    
    for entry in WalkDir::new(input_folder).max_depth(1) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(filename) = entry.file_name().to_str() {
                if let Some(extension) = entry.path().extension() {
                    if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                        gpx_files.push(filename.to_string());
                    } else {
                        other_files.push(filename.to_string());
                    }
                }
            }
        }
    }
    
    println!("\n📊 FOLDER CONTENTS:");
    println!("• GPX files: {}", gpx_files.len());
    println!("• Other files: {}", other_files.len());
    
    if !gpx_files.is_empty() {
        println!("\n📄 GPX FILES (showing first 10):");
        for (i, file) in gpx_files.iter().take(10).enumerate() {
            println!("   {}. {}", i + 1, file);
        }
        if gpx_files.len() > 10 {
            println!("   ... and {} more", gpx_files.len() - 10);
        }
    }
    
    if !other_files.is_empty() && other_files.len() <= 20 {
        println!("\n📄 OTHER FILES:");
        for (i, file) in other_files.iter().enumerate() {
            println!("   {}. {}", i + 1, file);
        }
    }
    
    println!("\n✅ Folder check complete!");
    Ok(())
}

fn run_gradient_segmentation(processed_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    use walkdir::WalkDir;
    
    println!("📈 GRADIENT BAND SEGMENTATION");
    println!("=============================");
    println!("Analyzing processed GPX files and segmenting by gradient bands:");
    println!("• 14 gradient bands from -30%+ to +30%+");
    println!("• Enhanced granularity for gentle slopes (-5% to +5%)");
    println!("• Each segment shows length and average gradient");
    println!("• Band distribution with accumulated km and percentages");
    println!("• Saves CSV file for each GPX route");
    println!("");
    
    // Check if processed folder exists
    if !Path::new(processed_folder).exists() {
        println!("❌ Processed folder not found: {}", processed_folder);
        println!("💡 Run option 1 first to process GPX files");
        return Ok(());
    }
    
    // Find processed GPX files
    let mut gpx_files = Vec::new();
    for entry in WalkDir::new(processed_folder).max_depth(1) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    gpx_files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    
    if gpx_files.is_empty() {
        println!("❌ No processed GPX files found in: {}", processed_folder);
        println!("💡 Run option 1 first to create processed files");
        return Ok(());
    }
    
    println!("🔍 Found {} processed GPX files", gpx_files.len());
    
    let mut processed_count = 0;
    let mut error_count = 0;
    
    for (i, gpx_path) in gpx_files.iter().enumerate() {
        let filename = gpx_path.file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        println!("\n🔄 Processing {}/{}: {}", i + 1, gpx_files.len(), filename);
        
        match segment_gpx_by_gradient(gpx_path, processed_folder) {
            Ok(segments) => {
                println!("   ✅ Success: {} segments identified", segments.len());
                
                // Print summary of gradient distribution
                let mut band_counts = [0u32; 14];
                let mut band_distances = [0.0f64; 14];
                let mut band_gradients: Vec<Vec<f64>> = (0..14).map(|_| Vec::new()).collect();
                let mut total_distance = 0.0;
                
                for segment in &segments {
                    let band_idx = (segment.band_id as usize).saturating_sub(1);
                    if band_idx < 14 {
                        band_counts[band_idx] += 1;
                        band_distances[band_idx] += segment.length_km;
                        band_gradients[band_idx].push(segment.average_gradient_percent as f64);
                    }
                    total_distance += segment.length_km;
                }
                
                println!("   📊 Total distance: {:.2}km", total_distance);
                println!("   📈 Band distribution:");
                for (i, &count) in band_counts.iter().enumerate() {
                    if count > 0 {
                        let band = get_gradient_band_info(i + 1);
                        let distance = band_distances[i];
                        let percentage = if total_distance > 0.0 { (distance / total_distance) * 100.0 } else { 0.0 };
                        let avg_gradient = if !band_gradients[i].is_empty() {
                            band_gradients[i].iter().sum::<f64>() / band_gradients[i].len() as f64
                        } else {
                            0.0
                        };
                        println!("      Band {}: {} ({:.2}km, {:.1}%, avg {:.0}%)", 
                                 i + 1, band.label, distance, percentage, avg_gradient);
                    }
                }
                
                processed_count += 1;
            }
            Err(e) => {
                println!("   ❌ Error: {}", e);
                error_count += 1;
            }
        }
    }
    
    println!("\n📊 SEGMENTATION COMPLETE");
    println!("========================");
    println!("• Files processed: {}", processed_count);
    println!("• Errors: {}", error_count);
    println!("• CSV files saved to: {}", processed_folder);
    println!("• Each CSV contains segment analysis with length and gradient");
    
    Ok(())
}

#[derive(Debug)]
struct GradientSegment {
    segment_id: u32,
    band_id: u32,
    band_label: String,
    start_distance_km: f64,
    end_distance_km: f64,
    length_km: f64,
    average_gradient_percent: i32,
    min_elevation_m: f64,
    max_elevation_m: f64,
    elevation_change_m: f64,
}

#[derive(Debug, Clone)]
struct GradientBand {
    id: u32,
    min_gradient: f64,
    max_gradient: f64,
    label: String,
    notes: String,
}

fn get_gradient_band_info(band_id: usize) -> GradientBand {
    let bands = [
        GradientBand { id: 1, min_gradient: f64::NEG_INFINITY, max_gradient: -30.0, label: "Insanely Steep Downhill".to_string(), notes: "Scramble / dangerous terrain".to_string() },
        GradientBand { id: 2, min_gradient: -30.0, max_gradient: -20.0, label: "Extreme Downhill".to_string(), notes: "Sliding or ropes possibly needed".to_string() },
        GradientBand { id: 3, min_gradient: -20.0, max_gradient: -12.0, label: "Very Steep Downhill".to_string(), notes: "Hard braking, very technical".to_string() },
        GradientBand { id: 4, min_gradient: -12.0, max_gradient: -8.0, label: "Steep Downhill".to_string(), notes: "Technical running".to_string() },
        GradientBand { id: 5, min_gradient: -8.0, max_gradient: -5.0, label: "Moderate Downhill".to_string(), notes: "Controlled descent".to_string() },
        GradientBand { id: 6, min_gradient: -5.0, max_gradient: -2.5, label: "Gentle Downhill (Steep)".to_string(), notes: "Upper gentle downhill".to_string() },
        GradientBand { id: 7, min_gradient: -2.5, max_gradient: 0.0, label: "Gentle Downhill (Shallow)".to_string(), notes: "Lower gentle downhill".to_string() },
        GradientBand { id: 8, min_gradient: 0.0, max_gradient: 2.5, label: "Gentle Uphill (Shallow)".to_string(), notes: "Lower gentle uphill".to_string() },
        GradientBand { id: 9, min_gradient: 2.5, max_gradient: 5.0, label: "Gentle Uphill (Steep)".to_string(), notes: "Upper gentle uphill".to_string() },
        GradientBand { id: 10, min_gradient: 5.0, max_gradient: 8.0, label: "Moderate Uphill".to_string(), notes: "Gradual effort".to_string() },
        GradientBand { id: 11, min_gradient: 8.0, max_gradient: 12.0, label: "Steep Uphill".to_string(), notes: "Sustained climbing".to_string() },
        GradientBand { id: 12, min_gradient: 12.0, max_gradient: 20.0, label: "Very Steep Uphill".to_string(), notes: "Hiking grade".to_string() },
        GradientBand { id: 13, min_gradient: 20.0, max_gradient: 30.0, label: "Extreme Uphill".to_string(), notes: "Power-hiking or poles needed".to_string() },
        GradientBand { id: 14, min_gradient: 30.0, max_gradient: f64::INFINITY, label: "Insanely Steep Uphill".to_string(), notes: "Scramble or climb".to_string() },
    ];
    
    if band_id > 0 && band_id <= bands.len() {
        bands[band_id - 1].clone()
    } else {
        GradientBand { id: 8, min_gradient: 0.0, max_gradient: 2.5, label: "Gentle Uphill (Shallow)".to_string(), notes: "Default".to_string() }
    }
}

fn classify_gradient(gradient_percent: f64) -> u32 {
    if gradient_percent <= -30.0 { 1 }        // Insanely Steep Downhill
    else if gradient_percent <= -20.0 { 2 }   // Extreme Downhill
    else if gradient_percent <= -12.0 { 3 }   // Very Steep Downhill
    else if gradient_percent <= -8.0 { 4 }    // Steep Downhill
    else if gradient_percent <= -5.0 { 5 }    // Moderate Downhill
    else if gradient_percent <= -2.5 { 6 }    // Gentle Downhill (Steep)
    else if gradient_percent < 0.0 { 7 }      // Gentle Downhill (Shallow)
    else if gradient_percent < 2.5 { 8 }      // Gentle Uphill (Shallow)
    else if gradient_percent < 5.0 { 9 }      // Gentle Uphill (Steep)
    else if gradient_percent < 8.0 { 10 }     // Moderate Uphill
    else if gradient_percent < 12.0 { 11 }    // Steep Uphill
    else if gradient_percent < 20.0 { 12 }    // Very Steep Uphill
    else if gradient_percent < 30.0 { 13 }    // Extreme Uphill
    else { 14 }                              // Insanely Steep Uphill
}

fn segment_gpx_by_gradient(
    gpx_path: &Path,
    output_folder: &str
) -> Result<Vec<GradientSegment>, Box<dyn std::error::Error>> {
    use geo::{HaversineDistance, point};
    
    // Read the processed GPX file
    let gpx = tolerant_gpx_reader::read_gpx_tolerantly(gpx_path)?;
    
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
    
    if coords.len() < 2 {
        return Err("Insufficient elevation data".into());
    }
    
    // Calculate distances and gradients between consecutive points
    let mut distances = vec![0.0];
    let mut gradients = Vec::new();
    
    for i in 1..coords.len() {
        // Calculate distance
        let a = point!(x: coords[i-1].1, y: coords[i-1].0);
        let b = point!(x: coords[i].1, y: coords[i].0);
        let dist = a.haversine_distance(&b);
        distances.push(distances[i-1] + dist);
        
        // Calculate gradient
        let elevation_change = coords[i].2 - coords[i-1].2;
        let gradient_percent = if dist > 0.0 {
            (elevation_change / dist) * 100.0
        } else {
            0.0
        };
        gradients.push((gradient_percent, classify_gradient(gradient_percent)));
    }
    
    // Group consecutive points with same gradient band into segments
    let mut segments = Vec::new();
    let mut segment_id = 1;
    
    if !gradients.is_empty() {
        let mut current_band = gradients[0].1;
        let mut segment_start = 0;
        let mut segment_gradients = vec![gradients[0].0];
        
        for i in 1..gradients.len() {
            if gradients[i].1 == current_band {
                // Continue current segment
                segment_gradients.push(gradients[i].0);
            } else {
                // End current segment and start new one
                let segment = create_segment(
                    segment_id,
                    current_band,
                    segment_start,
                    i,
                    &distances,
                    &coords,
                    &segment_gradients
                );
                segments.push(segment);
                
                // Start new segment
                segment_id += 1;
                current_band = gradients[i].1;
                segment_start = i;
                segment_gradients = vec![gradients[i].0];
            }
        }
        
        // Don't forget the last segment
        let segment = create_segment(
            segment_id,
            current_band,
            segment_start,
            gradients.len(),
            &distances,
            &coords,
            &segment_gradients
        );
        segments.push(segment);
    }
    
    // Save segments to CSV
    save_segments_to_csv(gpx_path, output_folder, &segments)?;
    
    Ok(segments)
}

fn create_segment(
    segment_id: u32,
    band_id: u32,
    start_idx: usize,
    end_idx: usize,
    distances: &[f64],
    coords: &[(f64, f64, f64)],
    gradients: &[f64]
) -> GradientSegment {
    let band_info = get_gradient_band_info(band_id as usize);
    
    let start_distance_km = distances[start_idx] / 1000.0;
    let end_distance_km = distances[end_idx] / 1000.0;
    let length_km = end_distance_km - start_distance_km;
    
    let average_gradient = if !gradients.is_empty() {
        gradients.iter().sum::<f64>() / gradients.len() as f64
    } else {
        0.0
    };
    
    // Find min/max elevation in this segment
    let mut min_elevation = f64::INFINITY;
    let mut max_elevation = f64::NEG_INFINITY;
    
    for i in start_idx..=end_idx.min(coords.len() - 1) {
        let elevation = coords[i].2;
        min_elevation = min_elevation.min(elevation);
        max_elevation = max_elevation.max(elevation);
    }
    
    let elevation_change = max_elevation - min_elevation;
    
    GradientSegment {
        segment_id,
        band_id,
        band_label: band_info.label,
        start_distance_km,
        end_distance_km,
        length_km,
        average_gradient_percent: average_gradient.round() as i32,
        min_elevation_m: min_elevation,
        max_elevation_m: max_elevation,
        elevation_change_m: elevation_change,
    }
}

fn save_segments_to_csv(
    gpx_path: &Path,
    output_folder: &str,
    segments: &[GradientSegment]
) -> Result<(), Box<dyn std::error::Error>> {
    use csv::Writer;
    
    // Generate CSV filename
    let filename = gpx_path.file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let csv_filename = format!("{}.csv", filename);
    let csv_path = Path::new(output_folder).join(csv_filename);
    
    // Calculate band statistics
    let mut band_distances = [0.0f64; 14];
    let mut band_gradients: Vec<Vec<f64>> = (0..14).map(|_| Vec::new()).collect();
    let mut total_distance = 0.0;
    
    for segment in segments {
        let band_idx = (segment.band_id as usize).saturating_sub(1);
        if band_idx < 14 {
            band_distances[band_idx] += segment.length_km;
            band_gradients[band_idx].push(segment.average_gradient_percent as f64);
        }
        total_distance += segment.length_km;
    }
    
    // Write CSV
    let mut wtr = Writer::from_path(csv_path)?;
    
    // Write route summary header
    wtr.write_record(&["ROUTE SUMMARY", "", "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Total Distance (km)", &format!("{:.3}", total_distance), "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Total Segments", &segments.len().to_string(), "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["", "", "", "", "", "", "", "", "", ""])?; // Empty row
    
    // Write band distribution summary
    wtr.write_record(&["GRADIENT BAND DISTRIBUTION", "", "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Band_ID", "Band_Label", "Total_Distance_km", "Percentage_%", "Avg_Gradient_%", "Notes", "", "", "", ""])?;
    
    for i in 0..14 {
        let band = get_gradient_band_info(i + 1);
        let distance = band_distances[i];
        let percentage = if total_distance > 0.0 { (distance / total_distance) * 100.0 } else { 0.0 };
        let avg_gradient = if !band_gradients[i].is_empty() {
            band_gradients[i].iter().sum::<f64>() / band_gradients[i].len() as f64
        } else {
            0.0
        };
        
        wtr.write_record(&[
            (i + 1).to_string(),
            band.label,
            format!("{:.3}", distance),
            format!("{:.1}", percentage),
            format!("{:.1}", avg_gradient),  // Changed to 1 decimal place
            band.notes,
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
        ])?;
    }
    
    wtr.write_record(&["", "", "", "", "", "", "", "", "", ""])?; // Empty row
    wtr.write_record(&["", "", "", "", "", "", "", "", "", ""])?; // Empty row
    
    // Write detailed segments header
    wtr.write_record(&["DETAILED SEGMENTS", "", "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&[
        "Segment_ID",
        "Band_ID", 
        "Gradient_Band",
        "Start_Distance_km",
        "End_Distance_km", 
        "Length_km",
        "Average_Gradient_%",
        "Min_Elevation_m",
        "Max_Elevation_m",
        "Elevation_Change_m"
    ])?;
    
    // Write detailed segment data
    for segment in segments {
        wtr.write_record(&[
            segment.segment_id.to_string(),
            segment.band_id.to_string(),
            segment.band_label.clone(),
            format!("{:.3}", segment.start_distance_km),
            format!("{:.3}", segment.end_distance_km),
            format!("{:.3}", segment.length_km),
            format!("{:.1}", segment.average_gradient_percent),  // Changed to 1 decimal place
            format!("{:.1}", segment.min_elevation_m),
            format!("{:.1}", segment.max_elevation_m),
            format!("{:.1}", segment.elevation_change_m),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}