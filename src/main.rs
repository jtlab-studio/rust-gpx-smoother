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
    println!("5. 📊 Segment processed GPX files by gradient bands");
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
            println!("\n📊 Segmenting processed GPX files by gradient bands...");
            run_gradient_band_segmentation(output_folder)?;
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

fn run_gradient_band_segmentation(processed_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    use walkdir::WalkDir;
    
    println!("📊 GRADIENT BAND SEGMENTATION");
    println!("=============================");
    println!("Segmenting processed GPX files by gradient bands:");
    println!("• Gradient bands from -30% to +30% in 1% increments");
    println!("• Special bands for extreme gradients (>30% or <-30%)");
    println!("• Each segment shows length and average gradient");
    println!("• Creates detailed CSV analysis for each route");
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
        
        match segment_gpx_by_gradient_bands(gpx_path, processed_folder) {
            Ok(segments) => {
                println!("   ✅ Success: {} gradient segments identified", segments.len());
                
                // Print summary statistics
                let total_distance: f64 = segments.iter().map(|s| s.length_km).sum();
                let steepest_up = segments.iter()
                    .filter(|s| s.average_gradient_percent > 0.0)
                    .map(|s| s.average_gradient_percent)
                    .fold(0.0, f64::max);
                let steepest_down = segments.iter()
                    .filter(|s| s.average_gradient_percent < 0.0)
                    .map(|s| s.average_gradient_percent)
                    .fold(0.0, f64::min);
                
                let extreme_segments = segments.iter()
                    .filter(|s| s.average_gradient_percent.abs() > 30.0)
                    .count();
                
                println!("   📊 Distance: {:.2}km | Steepest: +{:.0}%/{:.0}% | Extreme: {} segments", 
                         total_distance, steepest_up, steepest_down, extreme_segments);
                
                processed_count += 1;
            }
            Err(e) => {
                println!("   ❌ Error: {}", e);
                error_count += 1;
            }
        }
    }
    
    println!("\n📊 GRADIENT BAND SEGMENTATION COMPLETE");
    println!("======================================");
    println!("• Files processed: {}", processed_count);
    println!("• Errors: {}", error_count);
    println!("• CSV files saved to: {}", processed_folder);
    println!("• Each CSV contains gradient band analysis with average gradients");
    
    Ok(())
}

#[derive(Debug, Clone)]
struct GradientSegment {
    segment_id: u32,
    gradient_band: i32,  // -31 for <-30%, -30 to +30 for normal bands, +31 for >+30%
    band_label: String,
    start_distance_km: f64,
    end_distance_km: f64,
    length_km: f64,
    average_gradient_percent: f64,
    min_elevation_m: f64,
    max_elevation_m: f64,
    elevation_change_m: f64,
}

fn segment_gpx_by_gradient_bands(
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
        
        // Classify into gradient band
        let gradient_band = classify_gradient_to_band(gradient_percent);
        gradients.push((gradient_percent, gradient_band));
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
                let segment = create_gradient_segment(
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
        let segment = create_gradient_segment(
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
    save_gradient_segments_csv(gpx_path, output_folder, &segments)?;
    
    Ok(segments)
}

fn classify_gradient_to_band(gradient_percent: f64) -> i32 {
    if gradient_percent < -30.0 {
        -31  // Extreme downhill
    } else if gradient_percent > 30.0 {
        31   // Extreme uphill
    } else {
        // Round to nearest whole number for bands -30 to +30
        gradient_percent.round() as i32
    }
}

fn get_band_label(gradient_band: i32) -> String {
    match gradient_band {
        -31 => "Extreme Downhill (<-30%)".to_string(),
        31 => "Extreme Uphill (>+30%)".to_string(),
        _ => format!("{}%", gradient_band)
    }
}

fn create_gradient_segment(
    segment_id: u32,
    gradient_band: i32,
    start_idx: usize,
    end_idx: usize,
    distances: &[f64],
    coords: &[(f64, f64, f64)],
    gradients: &[f64]
) -> GradientSegment {
    let band_label = get_band_label(gradient_band);
    
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
        gradient_band,
        band_label,
        start_distance_km,
        end_distance_km,
        length_km,
        average_gradient_percent: average_gradient,
        min_elevation_m: min_elevation,
        max_elevation_m: max_elevation,
        elevation_change_m: elevation_change,
    }
}

fn save_gradient_segments_csv(
    gpx_path: &Path,
    output_folder: &str,
    segments: &[GradientSegment]
) -> Result<(), Box<dyn std::error::Error>> {
    use csv::Writer;
    
    // Generate CSV filename
    let filename = gpx_path.file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let csv_filename = format!("{}_gradient_bands.csv", filename);
    let csv_path = Path::new(output_folder).join(csv_filename);
    
    // Calculate band statistics
    let mut band_distances = std::collections::HashMap::new();
    let mut band_gradients: std::collections::HashMap<i32, Vec<f64>> = std::collections::HashMap::new();
    let mut band_longest_segments: std::collections::HashMap<i32, (f64, f64)> = std::collections::HashMap::new(); // (length, gradient)
    let mut total_distance = 0.0;
    
    // Calculate terrain distribution
    let mut km_below_minus2 = 0.0;
    let mut km_minus2_to_plus2 = 0.0;
    let mut km_above_plus2 = 0.0;
    
    // Detailed uphill segmentation (above +2%)
    let mut km_2_to_7 = 0.0;    // 2% to 7%
    let mut km_7_to_12 = 0.0;   // 7% to 12%
    let mut km_12_to_17 = 0.0;  // 12% to 17%
    let mut km_17_to_22 = 0.0;  // 17% to 22%
    let mut km_22_to_27 = 0.0;  // 22% to 27%
    let mut km_above_27 = 0.0;  // Above 27%
    
    // Detailed downhill segmentation (below -2%)
    let mut km_minus2_to_minus7 = 0.0;    // -2% to -7%
    let mut km_minus7_to_minus12 = 0.0;   // -7% to -12%
    let mut km_minus12_to_minus17 = 0.0;  // -12% to -17%
    let mut km_minus17_to_minus22 = 0.0;  // -17% to -22%
    let mut km_minus22_to_minus27 = 0.0;  // -22% to -27%
    let mut km_below_minus27 = 0.0;       // Below -27%
    
    for segment in segments {
        let band = segment.gradient_band;
        *band_distances.entry(band).or_insert(0.0) += segment.length_km;
        band_gradients.entry(band).or_insert_with(Vec::new).push(segment.average_gradient_percent);
        
        // Track longest segment per band
        let current_longest = band_longest_segments.get(&band).map(|(len, _)| *len).unwrap_or(0.0);
        if segment.length_km > current_longest {
            band_longest_segments.insert(band, (segment.length_km, segment.average_gradient_percent));
        }
        
        total_distance += segment.length_km;
        
        let grad = segment.average_gradient_percent;
        
        // Main terrain distribution
        if grad < -2.0 {
            km_below_minus2 += segment.length_km;
            
            // Detailed downhill segmentation
            if grad >= -7.0 {
                km_minus2_to_minus7 += segment.length_km;
            } else if grad >= -12.0 {
                km_minus7_to_minus12 += segment.length_km;
            } else if grad >= -17.0 {
                km_minus12_to_minus17 += segment.length_km;
            } else if grad >= -22.0 {
                km_minus17_to_minus22 += segment.length_km;
            } else if grad >= -27.0 {
                km_minus22_to_minus27 += segment.length_km;
            } else {
                km_below_minus27 += segment.length_km;
            }
        } else if grad >= -2.0 && grad <= 2.0 {
            km_minus2_to_plus2 += segment.length_km;
        } else {
            km_above_plus2 += segment.length_km;
            
            // Detailed uphill segmentation
            if grad <= 7.0 {
                km_2_to_7 += segment.length_km;
            } else if grad <= 12.0 {
                km_7_to_12 += segment.length_km;
            } else if grad <= 17.0 {
                km_12_to_17 += segment.length_km;
            } else if grad <= 22.0 {
                km_17_to_22 += segment.length_km;
            } else if grad <= 27.0 {
                km_22_to_27 += segment.length_km;
            } else {
                km_above_27 += segment.length_km;
            }
        }
    }
    
    // Write CSV
    let mut wtr = Writer::from_path(csv_path)?;
    
    // Write route summary header
    wtr.write_record(&["GRADIENT BAND ANALYSIS", "", "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Method", "Whole Number Gradient Bands (-30% to +30%)", "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Total Distance (km)", &format!("{:.3}", total_distance), "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Total Segments", &segments.len().to_string(), "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["", "", "", "", "", "", "", "", "", ""])?; // Empty row
    
    // Write terrain distribution
    wtr.write_record(&["TERRAIN DISTRIBUTION", "", "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Category", "Distance_km", "Percentage_%", "", "", "", "", "", "", ""])?;
    wtr.write_record(&[
        "Below -2% (Significant Downhill)",
        &format!("{:.3}", km_below_minus2),
        &format!("{:.1}", if total_distance > 0.0 { (km_below_minus2 / total_distance) * 100.0 } else { 0.0 }),
        "", "", "", "", "", "", ""
    ])?;
    wtr.write_record(&[
        "-2% to +2% (Nearly Flat)",
        &format!("{:.3}", km_minus2_to_plus2),
        &format!("{:.1}", if total_distance > 0.0 { (km_minus2_to_plus2 / total_distance) * 100.0 } else { 0.0 }),
        "", "", "", "", "", "", ""
    ])?;
    wtr.write_record(&[
        "Above +2% (Significant Uphill)",
        &format!("{:.3}", km_above_plus2),
        &format!("{:.1}", if total_distance > 0.0 { (km_above_plus2 / total_distance) * 100.0 } else { 0.0 }),
        "", "", "", "", "", "", ""
    ])?;
    wtr.write_record(&["", "", "", "", "", "", "", "", "", ""])?; // Empty row
    
    // Detailed uphill breakdown
    wtr.write_record(&["UPHILL BREAKDOWN (Above +2%)", "", "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Gradient_Range", "Distance_km", "Percentage_%", "% of Total Route", "", "", "", "", "", ""])?;
    
    if km_2_to_7 > 0.0 {
        wtr.write_record(&[
            "+2% to +7% (Moderate)",
            &format!("{:.3}", km_2_to_7),
            &format!("{:.1}", if km_above_plus2 > 0.0 { (km_2_to_7 / km_above_plus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_2_to_7 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_7_to_12 > 0.0 {
        wtr.write_record(&[
            "+7% to +12% (Steep)",
            &format!("{:.3}", km_7_to_12),
            &format!("{:.1}", if km_above_plus2 > 0.0 { (km_7_to_12 / km_above_plus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_7_to_12 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_12_to_17 > 0.0 {
        wtr.write_record(&[
            "+12% to +17% (Very Steep)",
            &format!("{:.3}", km_12_to_17),
            &format!("{:.1}", if km_above_plus2 > 0.0 { (km_12_to_17 / km_above_plus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_12_to_17 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_17_to_22 > 0.0 {
        wtr.write_record(&[
            "+17% to +22% (Extreme)",
            &format!("{:.3}", km_17_to_22),
            &format!("{:.1}", if km_above_plus2 > 0.0 { (km_17_to_22 / km_above_plus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_17_to_22 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_22_to_27 > 0.0 {
        wtr.write_record(&[
            "+22% to +27% (Brutal)",
            &format!("{:.3}", km_22_to_27),
            &format!("{:.1}", if km_above_plus2 > 0.0 { (km_22_to_27 / km_above_plus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_22_to_27 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_above_27 > 0.0 {
        wtr.write_record(&[
            "Above +27% (Insane)",
            &format!("{:.3}", km_above_27),
            &format!("{:.1}", if km_above_plus2 > 0.0 { (km_above_27 / km_above_plus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_above_27 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    wtr.write_record(&["", "", "", "", "", "", "", "", "", ""])?; // Empty row
    
    // Detailed downhill breakdown
    wtr.write_record(&["DOWNHILL BREAKDOWN (Below -2%)", "", "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Gradient_Range", "Distance_km", "Percentage_%", "% of Total Route", "", "", "", "", "", ""])?;
    
    if km_minus2_to_minus7 > 0.0 {
        wtr.write_record(&[
            "-2% to -7% (Moderate)",
            &format!("{:.3}", km_minus2_to_minus7),
            &format!("{:.1}", if km_below_minus2 > 0.0 { (km_minus2_to_minus7 / km_below_minus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_minus2_to_minus7 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_minus7_to_minus12 > 0.0 {
        wtr.write_record(&[
            "-7% to -12% (Steep)",
            &format!("{:.3}", km_minus7_to_minus12),
            &format!("{:.1}", if km_below_minus2 > 0.0 { (km_minus7_to_minus12 / km_below_minus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_minus7_to_minus12 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_minus12_to_minus17 > 0.0 {
        wtr.write_record(&[
            "-12% to -17% (Very Steep)",
            &format!("{:.3}", km_minus12_to_minus17),
            &format!("{:.1}", if km_below_minus2 > 0.0 { (km_minus12_to_minus17 / km_below_minus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_minus12_to_minus17 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_minus17_to_minus22 > 0.0 {
        wtr.write_record(&[
            "-17% to -22% (Extreme)",
            &format!("{:.3}", km_minus17_to_minus22),
            &format!("{:.1}", if km_below_minus2 > 0.0 { (km_minus17_to_minus22 / km_below_minus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_minus17_to_minus22 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_minus22_to_minus27 > 0.0 {
        wtr.write_record(&[
            "-22% to -27% (Brutal)",
            &format!("{:.3}", km_minus22_to_minus27),
            &format!("{:.1}", if km_below_minus2 > 0.0 { (km_minus22_to_minus27 / km_below_minus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_minus22_to_minus27 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    if km_below_minus27 > 0.0 {
        wtr.write_record(&[
            "Below -27% (Insane)",
            &format!("{:.3}", km_below_minus27),
            &format!("{:.1}", if km_below_minus2 > 0.0 { (km_below_minus27 / km_below_minus2) * 100.0 } else { 0.0 }),
            &format!("{:.1}", if total_distance > 0.0 { (km_below_minus27 / total_distance) * 100.0 } else { 0.0 }),
            "", "", "", "", "", ""
        ])?;
    }
    wtr.write_record(&["", "", "", "", "", "", "", "", "", ""])?; // Empty row
    
    // Write band distribution summary
    wtr.write_record(&["GRADIENT BAND DISTRIBUTION", "", "", "", "", "", "", "", "", ""])?;
    wtr.write_record(&["Band", "Band_Label", "Total_Distance_km", "Percentage_%", "Avg_Gradient_%", "Segment_Count", "Longest_Segment_km", "Longest_Gradient_%", "", ""])?;
    
    // Sort bands for logical display
    let mut band_keys: Vec<_> = band_distances.keys().collect();
    band_keys.sort();
    
    for &band in &band_keys {
        let distance = band_distances[band];
        let percentage = if total_distance > 0.0 { (distance / total_distance) * 100.0 } else { 0.0 };
        let avg_gradient = if let Some(gradients) = band_gradients.get(band) {
            if !gradients.is_empty() {
                gradients.iter().sum::<f64>() / gradients.len() as f64
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        let segment_count = segments.iter().filter(|s| s.gradient_band == *band).count();
        let (longest_length, longest_gradient) = band_longest_segments.get(band).unwrap_or(&(0.0, 0.0));
        
        wtr.write_record(&[
            band.to_string(),
            get_band_label(*band),
            format!("{:.3}", distance),
            format!("{:.1}", percentage),
            format!("{:.1}", avg_gradient),
            segment_count.to_string(),
            format!("{:.3}", longest_length),
            format!("{:.1}", longest_gradient),
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
        "Gradient_Band", 
        "Band_Label",
        "Start_Distance_km",
        "End_Distance_km", 
        "Length_km",
        "Average_Gradient_%",
        "Min_Elevation_m",
        "Max_Elevation_m",
        ""
    ])?;
    
    // Write detailed segment data
    for segment in segments {
        wtr.write_record(&[
            segment.segment_id.to_string(),
            segment.gradient_band.to_string(),
            segment.band_label.clone(),
            format!("{:.3}", segment.start_distance_km),
            format!("{:.3}", segment.end_distance_km),
            format!("{:.3}", segment.length_km),
            format!("{:.1}", segment.average_gradient_percent),
            format!("{:.1}", segment.min_elevation_m),
            format!("{:.1}", segment.max_elevation_m),
            "".to_string(),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}