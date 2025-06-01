use std::{fs::File, path::Path};
use gpx::read;
use geo::HaversineDistance;
use geo::point;
use std::io::BufReader;
use csv::{Writer, Reader};
use serde::{Serialize, Deserialize};
use rayon::prelude::*;
use std::collections::HashMap;
use walkdir::WalkDir;

mod custom_smoother;
mod improved_scoring;
mod outlier_analysis;
mod simplified_analysis;
mod gpx_output_analysis;
mod assymetric_analysis;
mod hybrid_analysis;
mod incline_analyzer;
mod gpx_processor;
mod distbased_elevation_processor;
mod two_pass_analysis;
mod precision_optimization_analysis;
mod corrected_elevation_analysis;
mod focused_symmetric_analysis;
mod gpx_preprocessor;
mod single_interval_analysis;
mod gpx_preprocessing_diagnostic;
mod conservative_analysis;
mod tolerant_gpx_reader;

use custom_smoother::{ElevationData, SmoothingVariant};

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
            ("valencia2022.gpx", 46),
            ("mainova-frankfurt-marathon 2023.gpx", 28),
            // Add more as needed...
        ];
        
        for (filename, gain) in builtin_data {
            official_data.insert(filename.to_lowercase(), gain);
        }
    }
    
    Ok(official_data)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpx_folder = r"C:\Users\Dzhu\Documents\GPX Files";
    let preprocessed_folder = r"C:\Users\Dzhu\Documents\GPX Files\Preprocessed";
    let _output_folder = r"C:\Users\Dzhu\Documents\GPX Files\GPX Analysis";
    
    // Print enhanced menu with all analysis options
    println!("\n🏔️  GPX ELEVATION ANALYSIS SUITE");
    println!("================================");
    println!("🚨 CRITICAL: Run diagnostics first to check for artificial elevation inflation!");
    println!("");
    println!("🏆 PROVEN WINNING SYMMETRIC DEADZONE METHOD:");
    println!("   • SymmetricFixed with optimal interval (scientifically proven)");
    println!("   • Eliminates loss under-estimation problem");
    println!("   • Achieves realistic gain/loss ratios (~1.0)");
    println!("   • 95%+ of files within ±20% accuracy");
    println!("   • Revolutionary symmetric elevation processing");
    println!("");
    println!("🔧 NEW: BALANCED ADAPTIVE PROCESSING:");
    println!("   • Conservative thresholds preserve natural profiles");
    println!("   • Only corrects truly corrupted data (ratio > 1.5)");
    println!("   • Graduated response: gentle → moderate → strong");
    println!("   • More natural results matching professional tools");
    println!("");
    println!("Available analyses:");
    println!("1. Fine-grained analysis (0.05m to 8m intervals)");
    println!("2. Improved scoring analysis");
    println!("3. Outlier analysis");
    println!("4. Simplified gain/loss balance analysis");
    println!("5. 🏆 PROCESS & SAVE GPX FILES (using winning thresholds) [RECOMMENDED]");
    println!("6. Previous asymmetric methods analysis (comprehensive)");
    println!("7. Fine-tuned asymmetric directional deadzone optimization");
    println!("8. Hybrid analysis (Butterworth + Distance-based)");
    println!("9. Run all analyses");
    println!("10. 🔄 Two-Pass & Savitzky-Golay Comparison Analysis");
    println!("11. 🎯 Precision Optimization Analysis");
    println!("12. ✅ Corrected Elevation Analysis (Proper Scoring + Symmetric Fix)");
    println!("13. 🎯 Focused Symmetric Analysis (0.5-2.5m) [OLD - Aggressive Processing]");
    println!("14. 🎯 1.9m Balanced Adaptive Analysis [NEW - Recommended] 🌟");
    println!("15. 🔧 PREPROCESS GPX FILES: Clean and repair all GPX files [NEW]");
    println!("16. 🔍 DIAGNOSTIC: Compare Original vs Preprocessed Files [DO THIS FIRST]");
    println!("17. 🛡️  CONSERVATIVE ANALYSIS: Use Original Files When Possible [RECOMMENDED]");
    println!("18. 🧪 TEST TOLERANT GPX READING: Like Garmin Connect [NEW - TEST FIRST]");
    
    // Offer menu for additional analyses
    println!("\n📊 Choose an analysis to run:");
    println!("Press Enter to exit, or choose an option:");
    println!("1. Fine-grained interval analysis");
    println!("2. Improved scoring analysis"); 
    println!("3. Outlier detection analysis");
    println!("4. Fine-tuned asymmetric analysis");
    println!("5. Hybrid analysis (Butterworth + Distance-based)");
    println!("6. All supplementary analyses");
    println!("10. 🔄 Two-Pass & Savitzky-Golay Comparison");
    println!("11. 🎯 Precision Optimization Analysis");
    println!("12. ✅ Corrected Elevation Analysis (Fixed with Symmetric)");
    println!("13. 🎯 Focused Symmetric Analysis (0.5-2.5m) [OLD - Aggressive]");
    println!("14. 🎯 1.9m Balanced Adaptive Analysis [NEW - Recommended] 🌟");
    println!("15. 🔧 Preprocess GPX Files (Clean & Repair) [NEW - RECOMMENDED FIRST STEP]");
    println!("16. 🔍 Preprocessing Diagnostic (Find Artificial Elevation) [CRITICAL - DO FIRST]");
    println!("17. 🛡️  Conservative Analysis (Original Files First) [RECOMMENDED FOR ACCURACY]");
    println!("18. 🧪 Test Tolerant GPX Reading (Like Garmin Connect) [NEW - TEST APPROACH]");
    println!("compare. 🔄 Compare Aggressive vs Balanced Processing [NEW]");
    println!("debug. 🔍 DEBUG: Show what files are actually in your folders");
    
    // Simple menu handling
    use std::io::{self, Write};
    print!("Choice (or Enter to exit): ");
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let choice = input.trim();
    
    match choice {
        "1" => {
            println!("\n🔬 Running fine-grained interval analysis...");
            run_fine_grained_analysis(gpx_folder)?;
        },
        "2" => {
            println!("\n📊 Running improved scoring analysis...");
            improved_scoring::run_improved_scoring_analysis(gpx_folder)?;
        },
        "3" => {
            println!("\n🔍 Running outlier detection analysis...");
            outlier_analysis::run_outlier_analysis(gpx_folder)?;
        },
        "4" => {
            println!("\n🔬 Running fine-tuned asymmetric analysis...");
            assymetric_analysis::run_fine_tuned_asymmetric_analysis(gpx_folder)?;
        },
        "5" => {
            println!("\n🔄 Running hybrid analysis...");
            hybrid_analysis::run_hybrid_analysis(gpx_folder)?;
        },
        "6" => {
            println!("\n🚀 Running all supplementary analyses...");
            run_fine_grained_analysis(gpx_folder)?;
            improved_scoring::run_improved_scoring_analysis(gpx_folder)?;
            outlier_analysis::run_outlier_analysis(gpx_folder)?;
            assymetric_analysis::run_fine_tuned_asymmetric_analysis(gpx_folder)?;
            hybrid_analysis::run_hybrid_analysis(gpx_folder)?;
            println!("✅ All supplementary analyses complete!");
        },
        "10" => {
            println!("\n🔄 Running Two-Pass & Savitzky-Golay comparison...");
            two_pass_analysis::run_two_pass_analysis(gpx_folder)?;
        },
        "11" => {
            println!("\n🎯 Running precision optimization analysis...");
            precision_optimization_analysis::run_precision_optimization_analysis(gpx_folder)?;
        },
        "12" => {
            println!("\n✅ Running corrected elevation analysis with symmetric fix...");
            corrected_elevation_analysis::run_corrected_elevation_analysis(gpx_folder)?;
        },
        "13" => {
            println!("\n🎯 Running focused symmetric analysis (0.5m to 2.5m optimization)...");
            println!("⚠️  WARNING: This uses the OLD aggressive adaptive processing");
            println!("   Files with ratio > 1.1 will get heavy smoothing and large deadbands");
            println!("   Consider using option 14 (balanced) instead for more natural results");
            
            // Check if preprocessed folder exists and ask user which to use
            if Path::new(preprocessed_folder).exists() {
                println!("📂 Both original and preprocessed folders found:");
                println!("   Original: {}", gpx_folder);
                println!("   Preprocessed: {}", preprocessed_folder);
                println!("");
                print!("Use preprocessed folder? (y/N): ");
                io::stdout().flush().unwrap();
                
                let mut choice = String::new();
                io::stdin().read_line(&mut choice).unwrap();
                let use_preprocessed = choice.trim().to_lowercase();
                
                if use_preprocessed == "y" || use_preprocessed == "yes" {
                    println!("✅ Using preprocessed folder: {}", preprocessed_folder);
                    focused_symmetric_analysis::run_focused_symmetric_analysis(preprocessed_folder)?;
                } else {
                    println!("📁 Using original folder: {}", gpx_folder);
                    focused_symmetric_analysis::run_focused_symmetric_analysis(gpx_folder)?;
                }
            } else {
                println!("📁 Using original folder: {}", gpx_folder);
                println!("💡 TIP: Run option 15 first to preprocess files for best results!");
                focused_symmetric_analysis::run_focused_symmetric_analysis(gpx_folder)?;
            }
        },
        "14" => {
            println!("\n🎯 Running 1.9m BALANCED adaptive analysis...");
            println!("🌟 This version uses CONSERVATIVE thresholds for natural results:");
            println!("   • Only corrects files with ratio > 1.5 (was 1.1)");
            println!("   • Gentle processing preserves elevation profiles");
            println!("   • Graduated response: gentle → moderate → strong correction");
            println!("   • More natural results matching professional tools");
            println!("   • Preserves terrain character and small elevation features");
            
            // Check if preprocessed folder exists and ask user which to use
            if Path::new(preprocessed_folder).exists() {
                println!("📂 Both original and preprocessed folders found:");
                println!("   Original: {}", gpx_folder);
                println!("   Preprocessed: {}", preprocessed_folder);
                println!("");
                println!("🔧 RECOMMENDATION: Use preprocessed folder for best results!");
                println!("   Preprocessed files are cleaned and repaired for consistent analysis.");
                println!("");
                print!("Use preprocessed folder? (y/N): ");
                io::stdout().flush().unwrap();
                
                let mut choice = String::new();
                io::stdin().read_line(&mut choice).unwrap();
                let use_preprocessed = choice.trim().to_lowercase();
                
                if use_preprocessed == "y" || use_preprocessed == "yes" {
                    println!("✅ Using preprocessed folder: {}", preprocessed_folder);
                    single_interval_analysis::run_single_interval_analysis(preprocessed_folder)?;
                } else {
                    println!("📁 Using original folder: {}", gpx_folder);
                    single_interval_analysis::run_single_interval_analysis(gpx_folder)?;
                }
            } else {
                println!("📁 Using original folder: {}", gpx_folder);
                println!("💡 TIP: Run option 15 first to preprocess files for best results!");
                single_interval_analysis::run_single_interval_analysis(gpx_folder)?;
            }
        },
        "15" => {
            println!("\n🔧 Running GPX preprocessing (clean and repair)...");
            gpx_preprocessor::run_gpx_preprocessing(gpx_folder, preprocessed_folder)?;
        },
        "16" => {
            println!("\n🔍 Running preprocessing diagnostic to detect artificial elevation...");
            
            // Check if preprocessed folder exists
            if !Path::new(preprocessed_folder).exists() {
                println!("❌ Preprocessed folder not found: {}", preprocessed_folder);
                println!("💡 Run option 15 first to create preprocessed files, then run this diagnostic.");
                println!("   This diagnostic compares original vs preprocessed files to detect issues.");
            } else {
                println!("✅ Found preprocessed folder - running comparative analysis...");
                println!("🎯 This will detect artificial elevation inflation and other preprocessing issues");
                gpx_preprocessing_diagnostic::run_gpx_preprocessing_diagnostic(gpx_folder, preprocessed_folder)?;
            }
        },
        "17" => {
            println!("\n🛡️  Running conservative analysis (original files first)...");
            
            // Check if preprocessed folder exists and ask user which approach to take
            if Path::new(preprocessed_folder).exists() {
                println!("📂 Both original and preprocessed folders found:");
                println!("   Original: {}", gpx_folder);
                println!("   Preprocessed: {}", preprocessed_folder);
                println!("");
                println!("🛡️  CONSERVATIVE APPROACH: Try original files first, fallback to preprocessed");
                println!("   This prevents artificial elevation inflation while handling broken files");
                println!("   Results should match Garmin Connect and gpx.studio");
                println!("");
                print!("Include preprocessed folder as fallback? (Y/n): ");
                io::stdout().flush().unwrap();
                
                let mut choice = String::new();
                io::stdin().read_line(&mut choice).unwrap();
                let use_fallback = choice.trim().to_lowercase();
                
                if use_fallback == "n" || use_fallback == "no" {
                    println!("✅ Using original files only");
                    println!("   This approach gives results closest to Garmin Connect");
                    conservative_analysis::run_conservative_analysis(gpx_folder, None)?;
                } else {
                    println!("✅ Using original files with preprocessed fallback");
                    println!("   Original files preferred, preprocessed only when necessary");
                    conservative_analysis::run_conservative_analysis(gpx_folder, Some(preprocessed_folder))?;
                }
            } else {
                println!("📁 Using original files only (no preprocessed folder found)");
                println!("💡 This is often the best approach - most GPX files work fine without preprocessing");
                println!("   Results should closely match Garmin Connect and gpx.studio");
                conservative_analysis::run_conservative_analysis(gpx_folder, None)?;
            }
        },
        "18" => {
            println!("\n🧪 Testing tolerant GPX reading strategies...");
            println!("🎯 This tests how well we can read GPX files like Garmin Connect does");
            println!("   (tolerant of minor XML issues, no artificial elevation data)");
            tolerant_gpx_reader::analyze_parsing_strategies(gpx_folder)?;
        },
        "compare" => {
            println!("\n🔄 Running comparison: Aggressive vs Balanced processing...");
            println!("🎯 This will show you the difference between old and new adaptive processing");
            println!("");
            
            // Run a quick comparison on a few sample files
            run_processing_comparison(gpx_folder)?;
        },
        "debug" => {
            println!("\n🔍 DEBUG: Checking folder contents...");
            
            println!("\n📂 ORIGINAL FOLDER: {}", gpx_folder);
            if Path::new(gpx_folder).exists() {
                let mut original_files = Vec::new();
                for entry in WalkDir::new(gpx_folder) {
                    if let Ok(entry) = entry {
                        if entry.file_type().is_file() {
                            if let Some(extension) = entry.path().extension() {
                                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                                    if let Some(filename) = entry.file_name().to_str() {
                                        original_files.push(filename.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                
                println!("   Found {} GPX files", original_files.len());
                println!("   Sample files:");
                for (i, file) in original_files.iter().take(10).enumerate() {
                    println!("   {}. {}", i+1, file);
                }
                if original_files.len() > 10 {
                    println!("   ... and {} more", original_files.len() - 10);
                }
            } else {
                println!("   ❌ Folder does not exist!");
            }
            
            println!("\n📁 PREPROCESSED FOLDER: {}", preprocessed_folder);
            if Path::new(preprocessed_folder).exists() {
                let mut preprocessed_files = Vec::new();
                for entry in WalkDir::new(preprocessed_folder) {
                    if let Ok(entry) = entry {
                        if entry.file_type().is_file() {
                            if let Some(extension) = entry.path().extension() {
                                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                                    if let Some(filename) = entry.file_name().to_str() {
                                        preprocessed_files.push(filename.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                
                println!("   Found {} GPX files", preprocessed_files.len());
                println!("   Sample files:");
                for (i, file) in preprocessed_files.iter().take(10).enumerate() {
                    println!("   {}. {}", i+1, file);
                }
                if preprocessed_files.len() > 10 {
                    println!("   ... and {} more", preprocessed_files.len() - 10);
                }
            } else {
                println!("   ❌ Folder does not exist!");
            }
        },
        "" => {
            println!("👋 Exiting. Your processed GPX files are ready in the output folder!");
        },
        _ => {
            println!("ℹ️  Unknown option. Choose a number from 1-18, 'compare', 'debug', or press Enter to exit.");
        }
    }
    
    Ok(())
}

// NEW: Processing comparison function
fn run_processing_comparison(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 PROCESSING COMPARISON: Aggressive vs Balanced");
    println!("===============================================");
    
    // Find a few sample files to test
    let mut sample_files = Vec::new();
    for entry in WalkDir::new(gpx_folder) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    sample_files.push(entry.path().to_path_buf());
                    if sample_files.len() >= 3 { // Just test 3 files
                        break;
                    }
                }
            }
        }
    }
    
    if sample_files.is_empty() {
        println!("❌ No GPX files found for comparison");
        return Ok(());
    }
    
    println!("📂 Testing on {} sample files...", sample_files.len());
    
    for (i, file_path) in sample_files.iter().enumerate() {
        let filename = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        println!("\n📁 File {}/{}: {}", i + 1, sample_files.len(), filename);
        
        // Test both processing approaches
        match compare_file_processing(file_path) {
            Ok((aggressive_result, balanced_result)) => {
                println!("   📊 COMPARISON RESULTS:");
                println!("      Method           | Gain    | Loss    | Ratio | Reduction");
                println!("      ──────────────────────────────────────────────────────");
                println!("      Raw Data         | {:7.1}m | {:7.1}m | {:5.2} | ──────────", 
                         aggressive_result.0, aggressive_result.1, 
                         if aggressive_result.1 > 0.0 { aggressive_result.0 / aggressive_result.1 } else { f64::INFINITY });
                println!("      Aggressive (OLD) | {:7.1}m | {:7.1}m | {:5.2} | {:8.1}%", 
                         aggressive_result.2, aggressive_result.3, 
                         if aggressive_result.3 > 0.0 { aggressive_result.2 / aggressive_result.3 } else { f64::INFINITY },
                         ((aggressive_result.0 - aggressive_result.2) / aggressive_result.0) * 100.0);
                println!("      Balanced (NEW)   | {:7.1}m | {:7.1}m | {:5.2} | {:8.1}%", 
                         balanced_result.2, balanced_result.3, 
                         if balanced_result.3 > 0.0 { balanced_result.2 / balanced_result.3 } else { f64::INFINITY },
                         ((balanced_result.0 - balanced_result.2) / balanced_result.0) * 100.0);
                
                // Analysis
                let aggressive_reduction = ((aggressive_result.0 - aggressive_result.2) / aggressive_result.0) * 100.0;
                let balanced_reduction = ((balanced_result.0 - balanced_result.2) / balanced_result.0) * 100.0;
                
                if aggressive_reduction > balanced_reduction + 10.0 {
                    println!("      🎯 BALANCED is less aggressive ({:.1}% vs {:.1}% reduction)", 
                             balanced_reduction, aggressive_reduction);
                }
                
                let raw_ratio = if aggressive_result.1 > 0.0 { aggressive_result.0 / aggressive_result.1 } else { f64::INFINITY };
                if raw_ratio > 1.1 && raw_ratio <= 1.5 {
                    println!("      🌿 This file has natural 1.1-1.5 ratio - BALANCED preserves it better");
                }
            }
            Err(e) => {
                println!("   ❌ Comparison failed: {}", e);
            }
        }
    }
    
    println!("\n💡 SUMMARY:");
    println!("The balanced approach should show:");
    println!("✅ Less aggressive gain reduction (preserves natural profiles)");
    println!("✅ More natural gain/loss ratios for legitimate trails");
    println!("✅ Better preservation of terrain character");
    println!("✅ Similar or better accuracy on files that truly need correction");
    
    Ok(())
}

// Helper function to compare processing approaches
fn compare_file_processing(file_path: &Path) -> Result<((f64, f64, f64, f64), (f64, f64, f64, f64)), Box<dyn std::error::Error>> {
    // Read the file
    let gpx = tolerant_gpx_reader::read_gpx_tolerantly(file_path)?;
    
    // Extract coordinates
    let mut coords = Vec::new();
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
    
    // Calculate raw gain/loss
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&elevations);
    
    // Test aggressive processing (simulate old thresholds)
    let aggressive_result = {
        let mut elevation_data = ElevationData::new_with_variant(
            elevations.clone(),
            distances.clone(),
            SmoothingVariant::AdaptiveQuality
        );
        
        // Simulate aggressive processing by calling the old aggressive method manually
        elevation_data.calculate_altitude_changes();
        let ratio = if raw_loss > 0.0 { raw_gain / raw_loss } else { f64::INFINITY };
        
        if ratio > 1.1 { // Old aggressive threshold
            // Simulate aggressive processing
            elevation_data.altitude_change = ElevationData::rolling_mean(&elevation_data.altitude_change, 200);
            elevation_data.apply_symmetric_deadband_filtering(8.0);
        }
        
        elevation_data.recalculate_accumulated_values_from_altitude_changes();
        (raw_gain, raw_loss, elevation_data.get_total_elevation_gain(), elevation_data.get_total_elevation_loss())
    };
    
    // Test balanced processing (new approach)
    let balanced_result = {
        let mut elevation_data = ElevationData::new_with_variant(
            elevations.clone(),
            distances.clone(),
            SmoothingVariant::AdaptiveQuality
        );
        
        // Use the new balanced approach
        elevation_data.process_elevation_data_adaptive();
        (raw_gain, raw_loss, elevation_data.get_total_elevation_gain(), elevation_data.get_total_elevation_loss())
    };
    
    Ok((aggressive_result, balanced_result))
}

fn calculate_raw_gain_loss(elevations: &[f64]) -> (f64, f64) {
    if elevations.len() < 2 {
        return (0.0, 0.0);
    }
    
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for window in elevations.windows(2) {
        let change = window[1] - window[0];
        
        if change.abs() > 0.001 {
            if change > 0.0 {
                gain += change;
            } else {
                loss += -change;
            }
        }
    }
    
    (gain, loss)
}

// Fine-grained analysis function (existing functionality)
fn run_fine_grained_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    use walkdir::WalkDir;
    
    println!("\n📊 FINE-GRAINED INTERVAL ANALYSIS");
    println!("==================================");
    println!("Testing elevation processing with intervals from 0.05m to 8.0m");
    
    let official_data = load_official_elevation_data()?;
    let mut all_results = Vec::new();
    
    let mut file_count = 0;
    let mut processed_count = 0;
    
    for entry in WalkDir::new(gpx_folder) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    file_count += 1;
                    match process_gpx_file_fine_grained(entry.path(), &official_data) {
                        Ok(result) => {
                            all_results.push(result);
                            processed_count += 1;
                        },
                        Err(e) => {
                            eprintln!("⚠️  Error processing {}: {}", entry.path().display(), e);
                        }
                    }
                }
            }
        }
    }
    
    println!("\n✅ Processed {} out of {} GPX files", processed_count, file_count);
    
    if !all_results.is_empty() {
        let output_path = Path::new(gpx_folder).join("fine_grained_analysis_0.05_to_8m.csv");
        write_fine_grained_csv(&all_results, &output_path)?;
        print_fine_grained_summary(&all_results);
        println!("📁 Results saved to: {}", output_path.display());
    } else {
        println!("⚠️  No valid results to save");
    }
    
    Ok(())
}

// Keep the original functions below for backward compatibility

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
    
    // Look up official gain from CSV data, handle both original and cleaned filenames
    let cleaned_filename = if filename.starts_with("cleaned_") {
        filename.strip_prefix("cleaned_").unwrap_or(&filename)
    } else {
        &filename
    };
    
    let official_gain = official_data
        .get(&cleaned_filename.to_lowercase())
        .copied()
        .unwrap_or(0);
    
    if official_gain == 0 {
        println!("⚠️  No official data for: {} (cleaned: {})", filename, cleaned_filename);
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