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
    println!("");
    
    use std::io::{self, Write};
    print!("Choice (1-4, or Enter to exit): ");
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
        "" => {
            println!("👋 Exiting.");
        },
        _ => {
            println!("ℹ️  Invalid option. Choose 1-4 or press Enter to exit.");
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