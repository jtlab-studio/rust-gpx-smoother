/// GPX PREPROCESSING DIAGNOSTIC TOOL
/// 
/// Compares original vs preprocessed GPX files to identify when preprocessing
/// is artificially inflating elevation gain or creating fake elevation profiles.
/// 
/// Usage: Add this file to src/gpx_preprocessing_diagnostic.rs
/// Then add to main.rs: mod gpx_preprocessing_diagnostic;

use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::BufReader;
use csv::Writer;
use serde::Serialize;
use gpx::read;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Clone)]
pub struct DiagnosticResult {
    filename: String,
    
    // File status
    original_readable: bool,
    preprocessed_exists: bool,
    preprocessing_needed: bool,
    
    // Original file analysis
    original_total_points: u32,
    original_points_with_elevation: u32,
    original_elevation_coverage_percent: f64,
    original_raw_gain: f64,
    original_raw_loss: f64,
    original_gain_loss_ratio: f64,
    original_elevation_range: f64,
    original_min_elevation: f64,
    original_max_elevation: f64,
    
    // Preprocessed file analysis
    preprocessed_total_points: u32,
    preprocessed_points_with_elevation: u32,
    preprocessed_elevation_coverage_percent: f64,
    preprocessed_raw_gain: f64,
    preprocessed_raw_loss: f64,
    preprocessed_gain_loss_ratio: f64,
    preprocessed_elevation_range: f64,
    preprocessed_min_elevation: f64,
    preprocessed_max_elevation: f64,
    
    // Comparison metrics
    gain_inflation_factor: f64,
    loss_inflation_factor: f64,
    ratio_change: f64,
    elevation_profile_similarity: String,
    
    // Artificial data detection
    artificial_elevation_detected: bool,
    latitude_elevation_correlation: f64,
    round_number_percentage: f64,
    
    // Recommendations
    recommendation: String,
    use_original: bool,
    concerns: String,
    
    // Error details
    original_error: String,
    preprocessed_error: String,
}

pub fn run_gpx_preprocessing_diagnostic(
    original_folder: &str,
    preprocessed_folder: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    
    println!("\n🔍 GPX PREPROCESSING DIAGNOSTIC");
    println!("===============================");
    println!("🎯 PURPOSE: Detect artificial elevation inflation from preprocessing");
    println!("📂 Original folder: {}", original_folder);
    println!("📁 Preprocessed folder: {}", preprocessed_folder);
    println!("");
    println!("🔍 CHECKING FOR:");
    println!("   • Files that don't need preprocessing");
    println!("   • Artificial elevation data creation");
    println!("   • Elevation gain/loss inflation");
    println!("   • Unrealistic elevation profile changes");
    println!("   • Latitude-based elevation patterns (fake data)");
    println!("   • When to use original vs preprocessed files\n");
    
    // Check if folders exist
    if !Path::new(original_folder).exists() {
        return Err(format!("Original folder does not exist: {}", original_folder).into());
    }
    
    if !Path::new(preprocessed_folder).exists() {
        return Err(format!("Preprocessed folder does not exist: {}", preprocessed_folder).into());
    }
    
    // Collect all GPX files from original folder
    let original_files = collect_gpx_files(original_folder)?;
    println!("📂 Found {} original GPX files", original_files.len());
    
    let mut results = Vec::new();
    let mut files_processed = 0;
    
    for original_path in &original_files {
        let filename = original_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        files_processed += 1;
        println!("🔄 Analyzing {}/{}: {}", files_processed, original_files.len(), filename);
        
        let preprocessed_path = Path::new(preprocessed_folder)
            .join(format!("cleaned_{}", filename));
        
        let result = analyze_original_vs_preprocessed(original_path, &preprocessed_path);
        
        // Print immediate findings
        if result.use_original {
            println!("   ✅ RECOMMENDATION: Use original file");
        } else {
            println!("   ⚠️  RECOMMENDATION: Use preprocessed file");
        }
        
        if result.gain_inflation_factor > 2.0 {
            println!("   🚨 WARNING: {:.1}x gain inflation detected!", result.gain_inflation_factor);
        }
        
        if result.artificial_elevation_detected {
            println!("   🏗️  ARTIFICIAL ELEVATION DETECTED!");
        }
        
        if !result.concerns.is_empty() {
            println!("   ⚠️  CONCERNS: {}", result.concerns);
        }
        
        results.push(result);
    }
    
    // Write detailed analysis to CSV
    let output_path = Path::new(original_folder).join("preprocessing_diagnostic.csv");
    write_diagnostic_csv(&results, &output_path)?;
    
    // Print comprehensive summary
    print_diagnostic_summary(&results);
    
    println!("\n📁 Detailed results saved to: {}", output_path.display());
    println!("✅ Diagnostic complete!");
    
    Ok(())
}

fn collect_gpx_files(folder: &str) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut gpx_files = Vec::new();
    
    for entry in WalkDir::new(folder) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    gpx_files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    
    gpx_files.sort();
    Ok(gpx_files)
}

fn analyze_original_vs_preprocessed(
    original_path: &Path,
    preprocessed_path: &Path,
) -> DiagnosticResult {
    
    let filename = original_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    // Try to read original file
    let (original_readable, original_analysis, original_error) = match try_read_and_analyze_gpx(original_path) {
        Ok(analysis) => (true, Some(analysis), String::new()),
        Err(e) => {
            println!("   ⚠️  Original file not readable: {}", e);
            (false, None, e.to_string())
        }
    };
    
    // Check if preprocessed file exists and read it
    let preprocessed_exists = preprocessed_path.exists();
    let (preprocessed_analysis, preprocessed_error) = if preprocessed_exists {
        match try_read_and_analyze_gpx(preprocessed_path) {
            Ok(analysis) => (Some(analysis), String::new()),
            Err(e) => {
                println!("   ❌ Preprocessed file exists but not readable: {}", e);
                (None, e.to_string())
            }
        }
    } else {
        (None, "File does not exist".to_string())
    };
    
    // Determine if preprocessing was actually needed
    let preprocessing_needed = !original_readable;
    
    let mut result = DiagnosticResult {
        filename,
        original_readable,
        preprocessed_exists,
        preprocessing_needed,
        original_total_points: 0,
        original_points_with_elevation: 0,
        original_elevation_coverage_percent: 0.0,
        original_raw_gain: 0.0,
        original_raw_loss: 0.0,
        original_gain_loss_ratio: 0.0,
        original_elevation_range: 0.0,
        original_min_elevation: 0.0,
        original_max_elevation: 0.0,
        preprocessed_total_points: 0,
        preprocessed_points_with_elevation: 0,
        preprocessed_elevation_coverage_percent: 0.0,
        preprocessed_raw_gain: 0.0,
        preprocessed_raw_loss: 0.0,
        preprocessed_gain_loss_ratio: 0.0,
        preprocessed_elevation_range: 0.0,
        preprocessed_min_elevation: 0.0,
        preprocessed_max_elevation: 0.0,
        gain_inflation_factor: 1.0,
        loss_inflation_factor: 1.0,
        ratio_change: 0.0,
        elevation_profile_similarity: "N/A".to_string(),
        artificial_elevation_detected: false,
        latitude_elevation_correlation: 0.0,
        round_number_percentage: 0.0,
        recommendation: String::new(),
        use_original: false,
        concerns: String::new(),
        original_error,
        preprocessed_error,
    };
    
    // Fill in original file data
    if let Some(analysis) = &original_analysis {
        result.original_total_points = analysis.total_points;
        result.original_points_with_elevation = analysis.points_with_elevation;
        result.original_elevation_coverage_percent = analysis.elevation_coverage_percent;
        result.original_raw_gain = analysis.raw_gain;
        result.original_raw_loss = analysis.raw_loss;
        result.original_gain_loss_ratio = analysis.gain_loss_ratio;
        result.original_elevation_range = analysis.elevation_range;
        result.original_min_elevation = analysis.min_elevation;
        result.original_max_elevation = analysis.max_elevation;
    }
    
    // Fill in preprocessed file data
    if let Some(analysis) = &preprocessed_analysis {
        result.preprocessed_total_points = analysis.total_points;
        result.preprocessed_points_with_elevation = analysis.points_with_elevation;
        result.preprocessed_elevation_coverage_percent = analysis.elevation_coverage_percent;
        result.preprocessed_raw_gain = analysis.raw_gain;
        result.preprocessed_raw_loss = analysis.raw_loss;
        result.preprocessed_gain_loss_ratio = analysis.gain_loss_ratio;
        result.preprocessed_elevation_range = analysis.elevation_range;
        result.preprocessed_min_elevation = analysis.min_elevation;
        result.preprocessed_max_elevation = analysis.max_elevation;
        
        // Check for artificial elevation patterns
        result.artificial_elevation_detected = analysis.artificial_elevation_detected;
        result.latitude_elevation_correlation = analysis.latitude_elevation_correlation;
        result.round_number_percentage = analysis.round_number_percentage;
    }
    
    // Calculate comparison metrics and recommendations
    calculate_comparison_metrics(&mut result);
    
    result
}

#[derive(Debug)]
struct GpxAnalysis {
    total_points: u32,
    points_with_elevation: u32,
    elevation_coverage_percent: f64,
    raw_gain: f64,
    raw_loss: f64,
    gain_loss_ratio: f64,
    elevation_range: f64,
    min_elevation: f64,
    max_elevation: f64,
    artificial_elevation_detected: bool,
    latitude_elevation_correlation: f64,
    round_number_percentage: f64,
}

fn try_read_and_analyze_gpx(path: &Path) -> Result<GpxAnalysis, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let gpx = read(reader)?;
    
    let mut coords: Vec<(f64, f64, Option<f64>)> = Vec::new();
    
    // Extract all coordinates, with or without elevation
    for track in &gpx.tracks {
        for segment in &track.segments {
            for point in &segment.points {
                let lat = point.point().y();
                let lon = point.point().x();
                coords.push((lat, lon, point.elevation));
            }
        }
    }
    
    if coords.is_empty() {
        return Err("No track points found".into());
    }
    
    let total_points = coords.len() as u32;
    let points_with_elevation = coords.iter()
        .filter(|(_, _, ele)| ele.is_some())
        .count() as u32;
    
    let elevation_coverage_percent = (points_with_elevation as f64 / total_points as f64) * 100.0;
    
    // Calculate raw gain/loss only for points with elevation
    let elevations: Vec<f64> = coords.iter()
        .filter_map(|(_, _, ele)| *ele)
        .collect();
    
    let (raw_gain, raw_loss) = if elevations.len() >= 2 {
        calculate_raw_gain_loss(&elevations)
    } else {
        (0.0, 0.0)
    };
    
    let gain_loss_ratio = if raw_loss > 0.0 {
        raw_gain / raw_loss
    } else if raw_gain > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };
    
    let (elevation_range, min_elevation, max_elevation) = if elevations.len() >= 2 {
        let min_ele = elevations.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_ele = elevations.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        (max_ele - min_ele, min_ele, max_ele)
    } else {
        (0.0, 0.0, 0.0)
    };
    
    // Check for artificial elevation patterns
    let (artificial_detected, lat_ele_correlation, round_percentage) = if elevations.len() >= 10 {
        let latitudes: Vec<f64> = coords.iter()
            .filter(|(_, _, ele)| ele.is_some())
            .map(|(lat, _, _)| *lat)
            .collect();
        
        detect_artificial_elevation(&latitudes, &elevations)
    } else {
        (false, 0.0, 0.0)
    };
    
    Ok(GpxAnalysis {
        total_points,
        points_with_elevation,
        elevation_coverage_percent,
        raw_gain,
        raw_loss,
        gain_loss_ratio,
        elevation_range,
        min_elevation,
        max_elevation,
        artificial_elevation_detected: artificial_detected,
        latitude_elevation_correlation: lat_ele_correlation,
        round_number_percentage: round_percentage,
    })
}

fn calculate_raw_gain_loss(elevations: &[f64]) -> (f64, f64) {
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

fn detect_artificial_elevation(latitudes: &[f64], elevations: &[f64]) -> (bool, f64, f64) {
    if latitudes.len() != elevations.len() || latitudes.len() < 10 {
        return (false, 0.0, 0.0);
    }
    
    // Calculate latitude-elevation correlation
    let lat_ele_correlation = calculate_correlation(latitudes, elevations);
    
    // Check percentage of round number elevations
    let round_elevations = elevations.iter()
        .filter(|&&e| {
            // Check if elevation is close to a round number (within 0.1m)
            let rounded = (e / 10.0).round() * 10.0;
            (e - rounded).abs() < 0.1 || 
            (e % 1.0).abs() < 0.1 || // Whole numbers
            (e % 5.0).abs() < 0.1    // Multiples of 5
        })
        .count();
    
    let round_percentage = (round_elevations as f64 / elevations.len() as f64) * 100.0;
    
    // Check for suspicious patterns indicating artificial data
    let mut artificial_indicators = 0;
    
    // Strong latitude correlation suggests artificial elevation based on location
    if lat_ele_correlation.abs() > 0.7 {
        artificial_indicators += 2;
    } else if lat_ele_correlation.abs() > 0.5 {
        artificial_indicators += 1;
    }
    
    // Too many round numbers suggest estimation/generation
    if round_percentage > 80.0 {
        artificial_indicators += 2;
    } else if round_percentage > 60.0 {
        artificial_indicators += 1;
    }
    
    // Check for suspiciously regular elevation changes
    let elevation_changes: Vec<f64> = elevations.windows(2)
        .map(|w| w[1] - w[0])
        .collect();
    
    let similar_changes = elevation_changes.windows(2)
        .filter(|&w| (w[0] - w[1]).abs() < 0.1)
        .count();
    
    if similar_changes > elevation_changes.len() / 3 {
        artificial_indicators += 1; // Too many identical elevation changes
    }
    
    // Check for elevation ranges that correlate with typical artificial generation
    let elevation_range = elevations.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)) - 
                         elevations.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    
    if elevation_range > 0.0 && elevation_range < 10.0 && elevations.len() > 100 {
        // Very flat terrain with many points might be artificially generated
        artificial_indicators += 1;
    }
    
    let artificial_detected = artificial_indicators >= 2;
    
    (artificial_detected, lat_ele_correlation, round_percentage)
}

fn calculate_correlation(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.len() < 2 {
        return 0.0;
    }
    
    let n = x.len() as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_xy: f64 = x.iter().zip(y).map(|(a, b)| a * b).sum();
    let sum_x2: f64 = x.iter().map(|a| a * a).sum();
    let sum_y2: f64 = y.iter().map(|b| b * b).sum();
    
    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();
    
    if denominator.abs() < 1e-10 {
        0.0
    } else {
        numerator / denominator
    }
}

fn calculate_comparison_metrics(result: &mut DiagnosticResult) {
    let mut concerns = Vec::new();
    
    // Calculate inflation factors
    if result.original_raw_gain > 0.0 && result.preprocessed_raw_gain > 0.0 {
        result.gain_inflation_factor = result.preprocessed_raw_gain / result.original_raw_gain;
    } else if result.original_raw_gain == 0.0 && result.preprocessed_raw_gain > 0.0 {
        result.gain_inflation_factor = f64::INFINITY;
        concerns.push("ELEVATION_CREATED_FROM_NOTHING");
    }
    
    if result.original_raw_loss > 0.0 && result.preprocessed_raw_loss > 0.0 {
        result.loss_inflation_factor = result.preprocessed_raw_loss / result.original_raw_loss;
    } else if result.original_raw_loss == 0.0 && result.preprocessed_raw_loss > 0.0 {
        result.loss_inflation_factor = f64::INFINITY;
        concerns.push("ELEVATION_CREATED_FROM_NOTHING");
    }
    
    // Calculate ratio change
    if result.original_gain_loss_ratio > 0.0 && result.original_gain_loss_ratio.is_finite() {
        result.ratio_change = result.preprocessed_gain_loss_ratio - result.original_gain_loss_ratio;
    }
    
    // Analyze elevation profile similarity
    result.elevation_profile_similarity = if !result.original_readable {
        "ORIGINAL_UNREADABLE".to_string()
    } else if result.original_points_with_elevation == 0 && result.preprocessed_points_with_elevation > 0 {
        "COMPLETELY_ARTIFICIAL".to_string()
    } else if result.artificial_elevation_detected {
        "ARTIFICIAL_PATTERNS_DETECTED".to_string()
    } else if result.gain_inflation_factor > 5.0 || result.loss_inflation_factor > 5.0 {
        "SEVERELY_DISTORTED".to_string()
    } else if result.gain_inflation_factor > 2.0 || result.loss_inflation_factor > 2.0 {
        "SIGNIFICANTLY_DISTORTED".to_string()
    } else if result.gain_inflation_factor > 1.5 || result.loss_inflation_factor > 1.5 {
        "MODERATELY_DISTORTED".to_string()
    } else {
        "SIMILAR".to_string()
    };
    
    // Check for specific issues
    if result.original_elevation_coverage_percent == 0.0 && result.preprocessed_elevation_coverage_percent > 0.0 {
        concerns.push("ADDED_FAKE_ELEVATIONS");
    }
    
    if result.artificial_elevation_detected {
        concerns.push("ARTIFICIAL_ELEVATION_PATTERNS");
    }
    
    if result.latitude_elevation_correlation.abs() > 0.7 {
        concerns.push("LATITUDE_BASED_ELEVATION");
    }
    
    if result.round_number_percentage > 80.0 {
        concerns.push("TOO_MANY_ROUND_ELEVATIONS");
    }
    
    if result.gain_inflation_factor > 3.0 {
        concerns.push("EXCESSIVE_GAIN_INFLATION");
    }
    
    if result.loss_inflation_factor > 3.0 {
        concerns.push("EXCESSIVE_LOSS_INFLATION");
    }
    
    if result.original_gain_loss_ratio > 0.8 && result.original_gain_loss_ratio < 1.2 && 
       (result.preprocessed_gain_loss_ratio < 0.5 || result.preprocessed_gain_loss_ratio > 2.0) {
        concerns.push("NATURAL_BALANCE_DESTROYED");
    }
    
    // Make recommendation
    if !result.original_readable {
        result.recommendation = "USE_PREPROCESSED_NECESSARY".to_string();
        result.use_original = false;
    } else if result.original_points_with_elevation == 0 {
        if result.preprocessed_points_with_elevation > 0 && result.artificial_elevation_detected {
            result.recommendation = "NO_ELEVATION_AVAILABLE_FAKE_ADDED".to_string();
            result.use_original = true; // Better no elevation than fake elevation
        } else {
            result.recommendation = "NO_ELEVATION_DATA_EITHER_FILE".to_string();
            result.use_original = true;
        }
    } else if concerns.contains(&"ARTIFICIAL_ELEVATION_PATTERNS") || 
             concerns.contains(&"LATITUDE_BASED_ELEVATION") ||
             concerns.contains(&"EXCESSIVE_GAIN_INFLATION") || 
             concerns.contains(&"EXCESSIVE_LOSS_INFLATION") {
        result.recommendation = "USE_ORIGINAL_PREPROCESSING_HARMFUL".to_string();
        result.use_original = true;
    } else if result.gain_inflation_factor < 1.2 && result.loss_inflation_factor < 1.2 {
        result.recommendation = "EITHER_FILE_OK_PREFER_ORIGINAL".to_string();
        result.use_original = true; // Prefer original when equivalent
    } else {
        result.recommendation = "EVALUATE_CASE_BY_CASE".to_string();
        result.use_original = false;
    }
    
    result.concerns = concerns.join(", ");
}

fn write_diagnostic_csv(
    results: &[DiagnosticResult], 
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Filename",
        "Original_Readable",
        "Preprocessed_Exists", 
        "Preprocessing_Needed",
        "Original_Total_Points",
        "Original_Points_With_Elevation",
        "Original_Elevation_Coverage_%",
        "Original_Raw_Gain_m",
        "Original_Raw_Loss_m",
        "Original_Gain_Loss_Ratio",
        "Original_Elevation_Range_m",
        "Original_Min_Elevation_m",
        "Original_Max_Elevation_m",
        "Preprocessed_Total_Points",
        "Preprocessed_Points_With_Elevation",
        "Preprocessed_Elevation_Coverage_%",
        "Preprocessed_Raw_Gain_m",
        "Preprocessed_Raw_Loss_m",
        "Preprocessed_Gain_Loss_Ratio",
        "Preprocessed_Elevation_Range_m",
        "Preprocessed_Min_Elevation_m",
        "Preprocessed_Max_Elevation_m",
        "Gain_Inflation_Factor",
        "Loss_Inflation_Factor",
        "Ratio_Change",
        "Elevation_Profile_Similarity",
        "Artificial_Elevation_Detected",
        "Latitude_Elevation_Correlation",
        "Round_Number_Percentage",
        "Recommendation",
        "Use_Original",
        "Concerns",
        "Original_Error",
        "Preprocessed_Error",
    ])?;
    
    // Sort by gain inflation factor (most problematic first)
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| {
        // First sort by artificial elevation detection
        match (a.artificial_elevation_detected, b.artificial_elevation_detected) {
            (true, false) => std::cmp::Ordering::Less,  // Artificial first
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                // Then by gain inflation factor
                b.gain_inflation_factor.partial_cmp(&a.gain_inflation_factor)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        }
    });
    
    // Write data
    for result in sorted_results {
        wtr.write_record(&[
            &result.filename,
            &result.original_readable.to_string(),
            &result.preprocessed_exists.to_string(),
            &result.preprocessing_needed.to_string(),
            &result.original_total_points.to_string(),
            &result.original_points_with_elevation.to_string(),
            &format!("{:.1}", result.original_elevation_coverage_percent),
            &format!("{:.1}", result.original_raw_gain),
            &format!("{:.1}", result.original_raw_loss),
            &format!("{:.3}", result.original_gain_loss_ratio),
            &format!("{:.1}", result.original_elevation_range),
            &format!("{:.1}", result.original_min_elevation),
            &format!("{:.1}", result.original_max_elevation),
            &result.preprocessed_total_points.to_string(),
            &result.preprocessed_points_with_elevation.to_string(),
            &format!("{:.1}", result.preprocessed_elevation_coverage_percent),
            &format!("{:.1}", result.preprocessed_raw_gain),
            &format!("{:.1}", result.preprocessed_raw_loss),
            &format!("{:.3}", result.preprocessed_gain_loss_ratio),
            &format!("{:.1}", result.preprocessed_elevation_range),
            &format!("{:.1}", result.preprocessed_min_elevation),
            &format!("{:.1}", result.preprocessed_max_elevation),
            &format!("{:.2}", result.gain_inflation_factor),
            &format!("{:.2}", result.loss_inflation_factor),
            &format!("{:.3}", result.ratio_change),
            &result.elevation_profile_similarity,
            &result.artificial_elevation_detected.to_string(),
            &format!("{:.3}", result.latitude_elevation_correlation),
            &format!("{:.1}", result.round_number_percentage),
            &result.recommendation,
            &result.use_original.to_string(),
            &result.concerns,
            &result.original_error,
            &result.preprocessed_error,
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_diagnostic_summary(results: &[DiagnosticResult]) {
    println!("\n🔍 PREPROCESSING DIAGNOSTIC SUMMARY");
    println!("==================================");
    
    let total_files = results.len();
    let original_readable = results.iter().filter(|r| r.original_readable).count();
    let preprocessing_needed = results.iter().filter(|r| r.preprocessing_needed).count();
    let use_original_recommended = results.iter().filter(|r| r.use_original).count();
    let artificial_detected = results.iter().filter(|r| r.artificial_elevation_detected).count();
    
    println!("\n📊 FILE READABILITY ANALYSIS:");
    println!("• Total files analyzed: {}", total_files);
    println!("• Original files readable: {} ({:.1}%)", 
             original_readable, (original_readable as f64 / total_files as f64) * 100.0);
    println!("• Files requiring preprocessing: {} ({:.1}%)", 
             preprocessing_needed, (preprocessing_needed as f64 / total_files as f64) * 100.0);
    println!("• Files where original is recommended: {} ({:.1}%)", 
             use_original_recommended, (use_original_recommended as f64 / total_files as f64) * 100.0);
    
    // Artificial elevation analysis
    println!("\n🏗️  ARTIFICIAL ELEVATION DETECTION:");
    println!("• Files with artificial elevation patterns: {} ({:.1}%)", 
             artificial_detected, (artificial_detected as f64 / total_files as f64) * 100.0);
    
    if artificial_detected > 0 {
        let avg_correlation: f64 = results.iter()
            .filter(|r| r.artificial_elevation_detected)
            .map(|r| r.latitude_elevation_correlation.abs())
            .sum::<f64>() / artificial_detected as f64;
        
        let avg_round_percentage: f64 = results.iter()
            .filter(|r| r.artificial_elevation_detected)
            .map(|r| r.round_number_percentage)
            .sum::<f64>() / artificial_detected as f64;
        
        println!("  • Average latitude-elevation correlation: {:.3}", avg_correlation);
        println!("  • Average round number percentage: {:.1}%", avg_round_percentage);
    }
    
    // Count different types of issues
    let elevation_created = results.iter()
        .filter(|r| r.concerns.contains("ELEVATION_CREATED_FROM_NOTHING"))
        .count();
    
    let excessive_inflation = results.iter()
        .filter(|r| r.gain_inflation_factor > 3.0 || r.loss_inflation_factor > 3.0)
        .count();
    
    let balance_destroyed = results.iter()
        .filter(|r| r.concerns.contains("NATURAL_BALANCE_DESTROYED"))
        .count();
    
    let latitude_based = results.iter()
        .filter(|r| r.concerns.contains("LATITUDE_BASED_ELEVATION"))
        .count();
    
    println!("\n🚨 PREPROCESSING ISSUES DETECTED:");
    println!("• Files with elevation created from nothing: {}", elevation_created);
    println!("• Files with latitude-based elevation: {}", latitude_based);
    println!("• Files with excessive gain/loss inflation (>3x): {}", excessive_inflation);
    println!("• Files where natural balance was destroyed: {}", balance_destroyed);
    
    // Show worst offenders
    let worst_artificial: Vec<_> = results.iter()
        .filter(|r| r.artificial_elevation_detected)
        .collect();
    
    if !worst_artificial.is_empty() {
        println!("\n🏗️  FILES WITH ARTIFICIAL ELEVATION (Top 10):");
        let mut sorted_artificial = worst_artificial;
        sorted_artificial.sort_by(|a, b| {
            b.latitude_elevation_correlation.abs().partial_cmp(&a.latitude_elevation_correlation.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        for result in sorted_artificial.iter().take(10) {
            println!("• {}: correlation={:.3}, round%={:.1}%, gain={:.1}m→{:.1}m", 
                     result.filename,
                     result.latitude_elevation_correlation,
                     result.round_number_percentage,
                     result.original_raw_gain,
                     result.preprocessed_raw_gain);
        }
    }
    
    let worst_gain_inflation: Vec<_> = results.iter()
        .filter(|r| r.gain_inflation_factor > 2.0 && r.gain_inflation_factor.is_finite())
        .collect();
    
    if !worst_gain_inflation.is_empty() {
        println!("\n⚠️  WORST GAIN INFLATION CASES (Top 10):");
        let mut sorted_worst = worst_gain_inflation;
        sorted_worst.sort_by(|a, b| b.gain_inflation_factor.partial_cmp(&a.gain_inflation_factor).unwrap());
        
        for result in sorted_worst.iter().take(10) {
            println!("• {}: {:.1}x inflation ({:.1}m → {:.1}m)", 
                     result.filename,
                     result.gain_inflation_factor,
                     result.original_raw_gain,
                     result.preprocessed_raw_gain);
        }
    }
    
    // Files with completely artificial elevation
    let completely_artificial: Vec<_> = results.iter()
        .filter(|r| r.original_points_with_elevation == 0 && r.preprocessed_points_with_elevation > 0)
        .collect();
    
    if !completely_artificial.is_empty() {
        println!("\n🏗️  FILES WITH COMPLETELY ARTIFICIAL ELEVATION:");
        for result in completely_artificial.iter().take(10) {
            println!("• {}: 0 → {} elevation points ({:.1}m gain created)", 
                     result.filename,
                     result.preprocessed_points_with_elevation,
                     result.preprocessed_raw_gain);
        }
        if completely_artificial.len() > 10 {
            println!("  ... and {} more files", completely_artificial.len() - 10);
        }
    }
    
    println!("\n💡 KEY FINDINGS & RECOMMENDATIONS:");
    
    if use_original_recommended > total_files / 2 {
        println!("✅ PREFER ORIGINAL FILES: {}/{} files work better with original data", 
                 use_original_recommended, total_files);
        println!("   Most GPX files are readable without preprocessing");
    } else {
        println!("⚠️  MIXED RESULTS: {}/{} files benefit from preprocessing", 
                 total_files - use_original_recommended, total_files);
    }
    
    if artificial_detected > 0 {
        println!("🚨 ARTIFICIAL ELEVATION DETECTED in {} files", artificial_detected);
        println!("   Preprocessing is creating fake elevation data!");
        println!("   This explains discrepancies with Garmin Connect/gpx.studio");
    }
    
    if elevation_created > 0 {
        println!("🏗️  {} files had elevation created from nothing", elevation_created);
        println!("   Better to have no elevation than fake elevation");
    }
    
    if excessive_inflation > 0 {
        println!("📈 {} files show excessive elevation inflation", excessive_inflation);
        println!("   Preprocessing algorithms need review");
    }
    
    if latitude_based > 0 {
        println!("🌍 {} files use latitude-based elevation (clearly artificial)", latitude_based);
        println!("   Disable artificial elevation generation immediately");
    }
    
    println!("\n🎯 IMMEDIATE ACTIONS:");
    println!("1. Use original files whenever possible ({} files)", use_original_recommended);
    println!("2. Disable artificial elevation creation in preprocessing");
    println!("3. Only preprocess files that truly cannot be read ({} files)", preprocessing_needed);
    if artificial_detected > 0 {
        println!("4. Review {} files flagged with artificial elevation", artificial_detected);
    }
    println!("5. Compare results with Garmin Connect to validate accuracy");
}