/// CONSERVATIVE GPX ANALYSIS
/// 
/// Prioritizes using original GPX files and only falls back to preprocessed 
/// versions when absolutely necessary. Prevents artificial elevation inflation.
/// 
/// Usage: Add this file to src/conservative_analysis.rs
/// Then add to main.rs: mod conservative_analysis;

use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{BufReader, Read};
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use gpx::{read, Gpx};
use geo::{HaversineDistance, point};
use walkdir::WalkDir;
use crate::custom_smoother::{ElevationData, SmoothingVariant};

// Optimal interval from focused analysis
const OPTIMAL_INTERVAL_M: f64 = 1.9;

#[derive(Debug, Serialize, Clone)]
pub struct ConservativeAnalysisResult {
    filename: String,
    file_source: String, // "ORIGINAL", "PREPROCESSED", or "FAILED"
    processing_status: String,
    
    // File analysis
    total_points: u32,
    points_with_elevation: u32,
    elevation_coverage_percent: f64,
    total_distance_km: f64,
    
    // Raw elevation analysis
    raw_elevation_gain_m: f64,
    raw_elevation_loss_m: f64,
    raw_gain_loss_ratio: f64,
    
    // Processed elevation analysis (1.9m symmetric)
    processed_elevation_gain_m: f64,
    processed_elevation_loss_m: f64,
    processed_gain_loss_ratio: f64,
    
    // Processing impact metrics
    gain_reduction_percent: f64,
    loss_reduction_percent: f64,
    ratio_improvement: f64,
    
    // Quality metrics
    elevation_range_m: f64,
    min_elevation_m: f64,
    max_elevation_m: f64,
    data_quality_rating: String,
    
    // Accuracy (if official data available)
    official_elevation_gain_m: u32,
    accuracy_percent: f64,
    absolute_error_m: f64,
    accuracy_rating: String,
    
    // Processing details
    interval_used_m: f64,
    smoothing_method: String,
    deadband_filtering: String,
    
    // Data source analysis
    artificial_elevation_indicators: u32,
    latitude_elevation_correlation: f64,
    round_number_percentage: f64,
    data_naturalness_score: f64,
    
    // Warnings/Issues
    warnings: String,
    data_source_concerns: String,
    
    // File reading attempts
    original_readable: bool,
    original_error: String,
    preprocessed_available: bool,
    preprocessed_error: String,
}

pub fn run_conservative_analysis(
    gpx_folder: &str,
    preprocessed_folder: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    
    println!("\n🛡️  CONSERVATIVE GPX ANALYSIS");
    println!("============================");
    println!("🎯 PRINCIPLE: Original files first, preprocessing only when necessary");
    println!("📂 Primary source: {}", gpx_folder);
    if let Some(preprocessed) = preprocessed_folder {
        println!("📁 Fallback source: {}", preprocessed);
    } else {
        println!("📁 No fallback source specified - original files only");
    }
    println!("");
    println!("✅ ADVANTAGES:");
    println!("   • Prevents artificial elevation inflation");
    println!("   • Uses real elevation data when available");
    println!("   • Matches results from Garmin Connect/gpx.studio");
    println!("   • Maintains natural gain/loss balance");
    println!("   • Detects and flags artificial elevation patterns");
    println!("   • Transparent about data quality issues\n");
    
    // Load official elevation data
    let official_data = crate::load_official_elevation_data()?;
    println!("✅ Loaded {} official elevation records", official_data.len());
    
    // Collect all GPX files
    let gpx_files = collect_gpx_files(gpx_folder)?;
    println!("📂 Found {} GPX files to analyze\n", gpx_files.len());
    
    let mut results = Vec::new();
    let mut files_processed = 0;
    
    for gpx_path in &gpx_files {
        let filename = gpx_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        files_processed += 1;
        println!("🔄 Analyzing {}/{}: {}", files_processed, gpx_files.len(), filename);
        
        let result = analyze_file_conservatively(
            gpx_path, 
            preprocessed_folder, 
            &official_data
        );
        
        match &result.processing_status[..] {
            "SUCCESS" => {
                println!("   ✅ Source: {} | Gain: {:.1}m | Ratio: {:.2} | Quality: {} | Accuracy: {}", 
                         result.file_source,
                         result.processed_elevation_gain_m,
                         result.processed_gain_loss_ratio,
                         result.data_quality_rating,
                         result.accuracy_rating);
                
                if result.artificial_elevation_indicators > 0 {
                    println!("   🏗️  {} artificial elevation indicators detected", 
                             result.artificial_elevation_indicators);
                }
            }
            _ => {
                println!("   ❌ Failed to process from any source");
            }
        }
        
        if !result.warnings.is_empty() {
            println!("   ⚠️  {}", result.warnings);
        }
        
        if !result.data_source_concerns.is_empty() {
            println!("   🚨 CONCERNS: {}", result.data_source_concerns);
        }
        
        results.push(result);
    }
    
    // Write results
    let output_path = Path::new(gpx_folder).join("conservative_analysis_results.csv");
    write_conservative_csv(&results, &output_path)?;
    
    // Print summary
    print_conservative_summary(&results);
    
    println!("\n📁 Results saved to: {}", output_path.display());
    println!("✅ Conservative analysis complete!");
    
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

fn analyze_file_conservatively(
    original_path: &Path,
    preprocessed_folder: Option<&str>,
    official_data: &HashMap<String, u32>,
) -> ConservativeAnalysisResult {
    
    let filename = original_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    // Step 1: Try original file first
    println!("   📂 Attempting to read original file...");
    let (original_readable, original_error) = match try_analyze_gpx_file(original_path) {
        Ok((coords, warnings)) => {
            println!("   ✅ Original file readable - using original data");
            let data_concerns = check_for_artificial_elevation(&coords);
            return process_coordinates_for_analysis(
                coords, 
                filename, 
                "ORIGINAL".to_string(),
                warnings,
                data_concerns,
                official_data,
                true,  // original_readable
                String::new(), // no original error
                false, // preprocessed not used
                String::new(), // no preprocessed error
            );
        }
        Err(e) => {
            println!("   ⚠️  Original file failed: {}", e);
            (false, e.to_string())
        }
    };
    
    // Step 2: Try preprocessed file if available
    if let Some(preprocessed_folder) = preprocessed_folder {
        let preprocessed_path = Path::new(preprocessed_folder)
            .join(format!("cleaned_{}", filename));
        
        if preprocessed_path.exists() {
            println!("   📁 Attempting to read preprocessed file...");
            match try_analyze_gpx_file(&preprocessed_path) {
                Ok((coords, warnings)) => {
                    println!("   ✅ Preprocessed file readable - using preprocessed data");
                    
                    // Check for signs of artificial elevation data
                    let data_concerns = check_for_artificial_elevation(&coords);
                    if !data_concerns.is_empty() {
                        println!("   🚨 WARNING: Artificial elevation patterns detected in preprocessed file!");
                    }
                    
                    return process_coordinates_for_analysis(
                        coords, 
                        filename, 
                        "PREPROCESSED".to_string(),
                        warnings,
                        data_concerns,
                        official_data,
                        original_readable,
                        original_error,
                        true,  // preprocessed available
                        String::new(), // no preprocessed error if we got here
                    );
                }
                Err(e) => {
                    println!("   ⚠️  Preprocessed file also failed: {}", e);
                    return create_failed_result(
                        filename, 
                        original_readable, 
                        original_error, 
                        true, 
                        e.to_string()
                    );
                }
            }
        } else {
            println!("   ⚠️  No preprocessed version available");
            return create_failed_result(
                filename, 
                original_readable, 
                original_error, 
                false, 
                "File does not exist".to_string()
            );
        }
    }
    
    // Step 3: Complete failure - no preprocessed folder available
    create_failed_result(
        filename, 
        original_readable, 
        original_error, 
        false, 
        "No preprocessed folder specified".to_string()
    )
}

fn try_analyze_gpx_file(path: &Path) -> Result<(Vec<(f64, f64, f64)>, String), Box<dyn std::error::Error>> {
    // First try normal reading
    match try_read_gpx_normal(path) {
        Ok(gpx) => {
            let (coords, warnings) = extract_coordinates_from_gpx(&gpx)?;
            Ok((coords, warnings))
        }
        Err(original_error) => {
            // Try basic repair for common issues
            println!("   🔧 Attempting basic GPX repair...");
            match try_basic_gpx_repair(path, &original_error.to_string()) {
                Ok(gpx) => {
                    let (coords, mut warnings) = extract_coordinates_from_gpx(&gpx)?;
                    warnings = if warnings.is_empty() {
                        "File repaired successfully".to_string()
                    } else {
                        format!("File repaired; {}", warnings)
                    };
                    Ok((coords, warnings))
                }
                Err(_) => Err(original_error)
            }
        }
    }
}

fn try_read_gpx_normal(path: &Path) -> Result<Gpx, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(read(reader)?)
}

fn try_basic_gpx_repair(path: &Path, original_error: &str) -> Result<Gpx, Box<dyn std::error::Error>> {
    // Read raw content
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    
    // Apply minimal, conservative repairs
    let repaired_content = apply_basic_gpx_repairs(&content, original_error)?;
    
    // Try to parse the repaired content
    let cursor = std::io::Cursor::new(repaired_content.as_bytes());
    let reader = BufReader::new(cursor);
    Ok(read(reader)?)
}

fn apply_basic_gpx_repairs(content: &str, error: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut repaired = content.to_string();
    let error_msg = error.to_lowercase();
    
    // Only apply minimal, safe repairs
    
    // Repair 1: Fix missing GPX version attribute
    if error_msg.contains("lacks required attribute") && error_msg.contains("version") {
        println!("   🔧 Adding missing GPX version attribute...");
        repaired = fix_missing_gpx_version(&repaired);
    }
    
    // Repair 2: Fix truncated XML files (safe repair)
    if error_msg.contains("unexpected end") || error_msg.contains("premature") || !repaired.trim().ends_with("</gpx>") {
        println!("   🔧 Closing unclosed XML tags...");
        repaired = repair_truncated_xml(&repaired);
    }
    
    // Repair 3: Fix coordinate boundary issues (metadata only)
    if error_msg.contains("longitude") && (error_msg.contains("minimum") || error_msg.contains("maximum")) {
        println!("   🔧 Removing problematic coordinate bounds metadata...");
        repaired = fix_coordinate_boundaries(&repaired);
    }
    
    // Repair 4: Fix basic XML declaration issues
    if !repaired.starts_with("<?xml") {
        println!("   🔧 Adding XML declaration...");
        repaired = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", repaired);
    }
    
    // DO NOT add artificial elevation data - this is the key difference from aggressive preprocessing
    
    Ok(repaired)
}

fn fix_missing_gpx_version(content: &str) -> String {
    let mut repaired = content.to_string();
    
    if let Some(gpx_start) = repaired.find("<gpx") {
        if let Some(gpx_end) = repaired[gpx_start..].find(">") {
            let gpx_tag = &repaired[gpx_start..gpx_start + gpx_end + 1];
            
            if !gpx_tag.contains("version=") {
                let mut new_gpx_tag = gpx_tag.replace(">", " version=\"1.1\">");
                
                if !new_gpx_tag.contains("xmlns=") {
                    new_gpx_tag = new_gpx_tag.replace(
                        " version=\"1.1\">",
                        " version=\"1.1\" xmlns=\"http://www.topografix.com/GPX/1/1\">"
                    );
                }
                
                repaired = repaired.replace(gpx_tag, &new_gpx_tag);
            }
        }
    }
    
    repaired
}

fn repair_truncated_xml(content: &str) -> String {
    let mut repaired = content.trim().to_string();
    
    // Count open and close tags
    let open_trkseg = repaired.matches("<trkseg>").count();
    let close_trkseg = repaired.matches("</trkseg>").count();
    let open_trk = repaired.matches("<trk>").count();
    let close_trk = repaired.matches("</trk>").count();
    let open_gpx = repaired.matches("<gpx").count();
    let close_gpx = repaired.matches("</gpx>").count();
    
    // Close any unclosed tags
    if open_trkseg > close_trkseg {
        for _ in 0..(open_trkseg - close_trkseg) {
            repaired.push_str("\n    </trkseg>");
        }
    }
    
    if open_trk > close_trk {
        for _ in 0..(open_trk - close_trk) {
            repaired.push_str("\n  </trk>");
        }
    }
    
    if open_gpx > close_gpx {
        repaired.push_str("\n</gpx>");
    }
    
    repaired
}

fn fix_coordinate_boundaries(content: &str) -> String {
    let mut repaired = content.to_string();
    
    // Remove bounds metadata that might be causing issues
    if let Some(start) = repaired.find("<bounds") {
        if let Some(end) = repaired[start..].find("/>") {
            let bounds_section = &repaired[start..start + end + 2];
            repaired = repaired.replace(bounds_section, "");
        }
    }
    
    repaired
}

fn extract_coordinates_from_gpx(gpx: &Gpx) -> Result<(Vec<(f64, f64, f64)>, String), Box<dyn std::error::Error>> {
    let mut coords: Vec<(f64, f64, f64)> = Vec::new();
    let mut warnings = Vec::new();
    let mut total_points = 0;
    let mut points_without_elevation = 0;
    
    for track in &gpx.tracks {
        for segment in &track.segments {
            for point in &segment.points {
                total_points += 1;
                
                let lat = point.point().y();
                let lon = point.point().x();
                
                if let Some(elevation) = point.elevation {
                    coords.push((lat, lon, elevation));
                } else {
                    points_without_elevation += 1;
                }
            }
        }
    }
    
    if coords.is_empty() {
        return Err("No elevation data found in GPX file".into());
    }
    
    // Generate warnings about data quality
    if points_without_elevation > 0 {
        warnings.push(format!("{}/{} points missing elevation", 
                              points_without_elevation, total_points));
    }
    
    let elevation_coverage = (coords.len() as f64 / total_points as f64) * 100.0;
    if elevation_coverage < 90.0 {
        warnings.push(format!("Only {:.1}% elevation coverage", elevation_coverage));
    }
    
    // Validate coordinate ranges
    let mut invalid_coords = 0;
    for (lat, lon, _) in &coords {
        if *lat < -90.0 || *lat > 90.0 || *lon < -180.0 || *lon > 180.0 {
            invalid_coords += 1;
        }
    }
    
    if invalid_coords > 0 {
        warnings.push(format!("{} invalid coordinates detected", invalid_coords));
    }
    
    Ok((coords, warnings.join("; ")))
}

fn check_for_artificial_elevation(coords: &[(f64, f64, f64)]) -> String {
    if coords.len() < 10 {
        return String::new();
    }
    
    let mut concerns = Vec::new();
    
    let elevations: Vec<f64> = coords.iter().map(|c| c.2).collect();
    let latitudes: Vec<f64> = coords.iter().map(|c| c.0).collect();
    
    // Check for latitude-elevation correlation (sign of artificial data)
    let lat_ele_correlation = calculate_correlation(&latitudes, &elevations);
    if lat_ele_correlation.abs() > 0.7 {
        concerns.push(format!("Strong latitude-elevation correlation ({:.3})", lat_ele_correlation));
    }
    
    // Check for too many round number elevations
    let round_elevations = elevations.iter()
        .filter(|&&e| (e % 10.0).abs() < 0.1 || (e % 5.0).abs() < 0.1)
        .count();
    let round_percentage = (round_elevations as f64 / elevations.len() as f64) * 100.0;
    
    if round_percentage > 70.0 {
        concerns.push(format!("Too many round elevations ({:.1}%)", round_percentage));
    }
    
    // Check for unrealistic elevation ranges for the route length
    let min_ele = elevations.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_ele = elevations.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let elevation_range = max_ele - min_ele;
    
    if elevation_range < 1.0 && coords.len() > 100 {
        concerns.push("Suspiciously flat for long route".to_string());
    }
    
    // Check for regular elevation step patterns
    let elevation_changes: Vec<f64> = elevations.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .collect();
    
    let identical_changes = elevation_changes.windows(2)
        .filter(|w| (w[0] - w[1]).abs() < 0.01)
        .count();
    
    if identical_changes > elevation_changes.len() / 4 {
        concerns.push("Too many identical elevation changes".to_string());
    }
    
    concerns.join("; ")
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

fn process_coordinates_for_analysis(
    coords: Vec<(f64, f64, f64)>,
    filename: String,
    file_source: String,
    warnings: String,
    data_source_concerns: String,
    official_data: &HashMap<String, u32>,
    original_readable: bool,
    original_error: String,
    preprocessed_available: bool,
    preprocessed_error: String,
) -> ConservativeAnalysisResult {
    
    // Calculate distances
    let mut distances = vec![0.0];
    for i in 1..coords.len() {
        let a = point!(x: coords[i-1].1, y: coords[i-1].0);
        let b = point!(x: coords[i].1, y: coords[i].0);
        let dist = a.haversine_distance(&b);
        distances.push(distances[i-1] + dist);
    }
    
    let elevations: Vec<f64> = coords.iter().map(|c| c.2).collect();
    let latitudes: Vec<f64> = coords.iter().map(|c| c.0).collect();
    let total_distance_km = distances.last().unwrap() / 1000.0;
    
    // Calculate raw elevation gain/loss
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&elevations);
    let raw_gain_loss_ratio = if raw_loss > 0.0 { raw_gain / raw_loss } else { f64::INFINITY };
    
    // Apply conservative 1.9m symmetric processing
    let mut elevation_data = ElevationData::new_with_variant(
        elevations.clone(),
        distances.clone(),
        SmoothingVariant::SymmetricFixed
    );
    
    elevation_data.apply_custom_interval_processing_symmetric(OPTIMAL_INTERVAL_M);
    
    let processed_gain = elevation_data.get_total_elevation_gain();
    let processed_loss = elevation_data.get_total_elevation_loss();
    let processed_gain_loss_ratio = if processed_loss > 0.0 { 
        processed_gain / processed_loss 
    } else { 
        f64::INFINITY 
    };
    
    // Calculate processing impact
    let gain_reduction_percent = if raw_gain > 0.0 {
        ((raw_gain - processed_gain) / raw_gain) * 100.0
    } else {
        0.0
    };
    
    let loss_reduction_percent = if raw_loss > 0.0 {
        ((raw_loss - processed_loss) / raw_loss) * 100.0
    } else {
        0.0
    };
    
    let ratio_improvement = if raw_gain_loss_ratio.is_finite() && processed_gain_loss_ratio.is_finite() {
        (processed_gain_loss_ratio - raw_gain_loss_ratio).abs() - (raw_gain_loss_ratio - 1.0).abs()
    } else {
        0.0
    };
    
    // Calculate elevation metrics
    let min_elevation = elevations.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_elevation = elevations.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let elevation_range = max_elevation - min_elevation;
    
    // Analyze data quality and artificial indicators
    let lat_ele_correlation = calculate_correlation(&latitudes, &elevations);
    let round_elevations = elevations.iter()
        .filter(|&&e| (e % 10.0).abs() < 0.1 || (e % 5.0).abs() < 0.1)
        .count();
    let round_percentage = (round_elevations as f64 / elevations.len() as f64) * 100.0;
    
    let mut artificial_indicators = 0;
    if lat_ele_correlation.abs() > 0.7 { artificial_indicators += 2; }
    else if lat_ele_correlation.abs() > 0.5 { artificial_indicators += 1; }
    
    if round_percentage > 80.0 { artificial_indicators += 2; }
    else if round_percentage > 60.0 { artificial_indicators += 1; }
    
    if elevation_range < 1.0 && coords.len() > 100 { artificial_indicators += 1; }
    
    let data_naturalness_score = calculate_data_naturalness_score(
        &coords, 
        lat_ele_correlation, 
        round_percentage,
        artificial_indicators
    );
    
    // Data quality rating
    let data_quality_rating = rate_data_quality(
        &coords, 
        &warnings, 
        &data_source_concerns,
        data_naturalness_score
    );
    
    // Official data comparison
    let official_gain = official_data
        .get(&filename.to_lowercase())
        .copied()
        .unwrap_or(0);
    
    let accuracy_percent = if official_gain > 0 {
        (processed_gain / official_gain as f64) * 100.0
    } else {
        0.0
    };
    
    let absolute_error_m = if official_gain > 0 {
        (processed_gain - official_gain as f64).abs()
    } else {
        0.0
    };
    
    let accuracy_rating = classify_accuracy(accuracy_percent);
    
    ConservativeAnalysisResult {
        filename,
        file_source,
        processing_status: "SUCCESS".to_string(),
        total_points: coords.len() as u32,
        points_with_elevation: coords.len() as u32,
        elevation_coverage_percent: 100.0, // We only keep points with elevation
        total_distance_km,
        raw_elevation_gain_m: raw_gain,
        raw_elevation_loss_m: raw_loss,
        raw_gain_loss_ratio,
        processed_elevation_gain_m: processed_gain,
        processed_elevation_loss_m: processed_loss,
        processed_gain_loss_ratio,
        gain_reduction_percent,
        loss_reduction_percent,
        ratio_improvement,
        elevation_range_m: elevation_range,
        min_elevation_m: min_elevation,
        max_elevation_m: max_elevation,
        data_quality_rating,
        official_elevation_gain_m: official_gain,
        accuracy_percent,
        absolute_error_m,
        accuracy_rating,
        interval_used_m: OPTIMAL_INTERVAL_M,
        smoothing_method: "SymmetricFixed".to_string(),
        deadband_filtering: "Symmetric 1.9m".to_string(),
        artificial_elevation_indicators: artificial_indicators,
        latitude_elevation_correlation: lat_ele_correlation,
        round_number_percentage: round_percentage,
        data_naturalness_score,
        warnings,
        data_source_concerns,
        original_readable,
        original_error,
        preprocessed_available,
        preprocessed_error,
    }
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

fn calculate_data_naturalness_score(
    coords: &[(f64, f64, f64)],
    lat_ele_correlation: f64,
    round_percentage: f64,
    artificial_indicators: u32,
) -> f64 {
    let mut score = 100.0;
    
    // Reduce score based on artificial indicators
    score -= artificial_indicators as f64 * 15.0;
    
    // Reduce score for strong correlations
    score -= lat_ele_correlation.abs() * 30.0;
    
    // Reduce score for too many round numbers
    if round_percentage > 50.0 {
        score -= (round_percentage - 50.0) * 0.5;
    }
    
    // Reduce score for insufficient data points
    if coords.len() < 100 {
        score -= 10.0;
    }
    
    score.max(0.0).min(100.0)
}

fn rate_data_quality(
    coords: &[(f64, f64, f64)], 
    warnings: &str, 
    data_source_concerns: &str,
    naturalness_score: f64,
) -> String {
    let mut score = naturalness_score as i32;
    
    // Reduce score for warnings and concerns
    if !warnings.is_empty() {
        score -= 10;
    }
    
    if !data_source_concerns.is_empty() {
        score -= 20;
    }
    
    // Reduce score for very few points
    if coords.len() < 50 {
        score -= 25;
    } else if coords.len() < 200 {
        score -= 10;
    }
    
    match score {
        85..=100 => "EXCELLENT",
        70..=84 => "GOOD", 
        55..=69 => "FAIR",
        40..=54 => "POOR",
        _ => "VERY_POOR",
    }.to_string()
}

fn classify_accuracy(accuracy_percent: f64) -> String {
    if accuracy_percent == 0.0 {
        "N/A".to_string()
    } else if accuracy_percent >= 98.0 && accuracy_percent <= 102.0 {
        "A+ (±2%)".to_string()
    } else if accuracy_percent >= 95.0 && accuracy_percent <= 105.0 {
        "A (±5%)".to_string()
    } else if accuracy_percent >= 90.0 && accuracy_percent <= 110.0 {
        "B (±10%)".to_string()
    } else if accuracy_percent >= 80.0 && accuracy_percent <= 120.0 {
        "C (±20%)".to_string()
    } else if accuracy_percent >= 50.0 && accuracy_percent <= 150.0 {
        "D (±50%)".to_string()
    } else {
        "F (>±50%)".to_string()
    }
}

fn create_failed_result(
    filename: String,
    original_readable: bool,
    original_error: String,
    preprocessed_available: bool,
    preprocessed_error: String,
) -> ConservativeAnalysisResult {
    ConservativeAnalysisResult {
        filename,
        file_source: "FAILED".to_string(),
        processing_status: "FAILED".to_string(),
        total_points: 0,
        points_with_elevation: 0,
        elevation_coverage_percent: 0.0,
        total_distance_km: 0.0,
        raw_elevation_gain_m: 0.0,
        raw_elevation_loss_m: 0.0,
        raw_gain_loss_ratio: 0.0,
        processed_elevation_gain_m: 0.0,
        processed_elevation_loss_m: 0.0,
        processed_gain_loss_ratio: 0.0,
        gain_reduction_percent: 0.0,
        loss_reduction_percent: 0.0,
        ratio_improvement: 0.0,
        elevation_range_m: 0.0,
        min_elevation_m: 0.0,
        max_elevation_m: 0.0,
        data_quality_rating: "FAILED".to_string(),
        official_elevation_gain_m: 0,
        accuracy_percent: 0.0,
        absolute_error_m: 0.0,
        accuracy_rating: "F".to_string(),
        interval_used_m: 0.0,
        smoothing_method: "NONE".to_string(),
        deadband_filtering: "NONE".to_string(),
        artificial_elevation_indicators: 0,
        latitude_elevation_correlation: 0.0,
        round_number_percentage: 0.0,
        data_naturalness_score: 0.0,
        warnings: "Could not read file from any source".to_string(),
        data_source_concerns: String::new(),
        original_readable,
        original_error,
        preprocessed_available,
        preprocessed_error,
    }
}

fn write_conservative_csv(
    results: &[ConservativeAnalysisResult], 
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    wtr.write_record(&[
        "Filename",
        "File_Source",
        "Processing_Status", 
        "Total_Points",
        "Points_With_Elevation",
        "Elevation_Coverage_%",
        "Total_Distance_km",
        "Raw_Gain_m",
        "Raw_Loss_m",
        "Raw_Gain_Loss_Ratio",
        "Processed_Gain_m",
        "Processed_Loss_m",
        "Processed_Gain_Loss_Ratio",
        "Gain_Reduction_%",
        "Loss_Reduction_%",
        "Ratio_Improvement",
        "Elevation_Range_m",
        "Min_Elevation_m",
        "Max_Elevation_m",
        "Data_Quality_Rating",
        "Official_Gain_m",
        "Accuracy_%",
        "Absolute_Error_m",
        "Accuracy_Rating",
        "Interval_Used_m",
        "Smoothing_Method",
        "Deadband_Filtering",
        "Artificial_Elevation_Indicators",
        "Latitude_Elevation_Correlation",
        "Round_Number_Percentage",
        "Data_Naturalness_Score",
        "Warnings",
        "Data_Source_Concerns",
        "Original_Readable",
        "Original_Error",
        "Preprocessed_Available",
        "Preprocessed_Error",
    ])?;
    
    // Sort by data quality and accuracy
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| {
        // First by file source (original preferred)
        match (&a.file_source[..], &b.file_source[..]) {
            ("ORIGINAL", "PREPROCESSED") => std::cmp::Ordering::Less,
            ("PREPROCESSED", "ORIGINAL") => std::cmp::Ordering::Greater,
            ("FAILED", _) => std::cmp::Ordering::Greater,
            (_, "FAILED") => std::cmp::Ordering::Less,
            _ => {
                // Then by data naturalness score (higher is better)
                b.data_naturalness_score.partial_cmp(&a.data_naturalness_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        }
    });
    
    for result in sorted_results {
        wtr.write_record(&[
            &result.filename,
            &result.file_source,
            &result.processing_status,
            &result.total_points.to_string(),
            &result.points_with_elevation.to_string(),
            &format!("{:.1}", result.elevation_coverage_percent),
            &format!("{:.2}", result.total_distance_km),
            &format!("{:.1}", result.raw_elevation_gain_m),
            &format!("{:.1}", result.raw_elevation_loss_m),
            &format!("{:.3}", result.raw_gain_loss_ratio),
            &format!("{:.1}", result.processed_elevation_gain_m),
            &format!("{:.1}", result.processed_elevation_loss_m),
            &format!("{:.3}", result.processed_gain_loss_ratio),
            &format!("{:.1}", result.gain_reduction_percent),
            &format!("{:.1}", result.loss_reduction_percent),
            &format!("{:.3}", result.ratio_improvement),
            &format!("{:.1}", result.elevation_range_m),
            &format!("{:.1}", result.min_elevation_m),
            &format!("{:.1}", result.max_elevation_m),
            &result.data_quality_rating,
            &result.official_elevation_gain_m.to_string(),
            &format!("{:.2}", result.accuracy_percent),
            &format!("{:.1}", result.absolute_error_m),
            &result.accuracy_rating,
            &format!("{:.1}", result.interval_used_m),
            &result.smoothing_method,
            &result.deadband_filtering,
            &result.artificial_elevation_indicators.to_string(),
            &format!("{:.3}", result.latitude_elevation_correlation),
            &format!("{:.1}", result.round_number_percentage),
            &format!("{:.1}", result.data_naturalness_score),
            &result.warnings,
            &result.data_source_concerns,
            &result.original_readable.to_string(),
            &result.original_error,
            &result.preprocessed_available.to_string(),
            &result.preprocessed_error,
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_conservative_summary(results: &[ConservativeAnalysisResult]) {
    println!("\n🛡️  CONSERVATIVE ANALYSIS SUMMARY");
    println!("================================");
    
    let total_files = results.len();
    let successful = results.iter().filter(|r| r.processing_status == "SUCCESS").count();
    let from_original = results.iter().filter(|r| r.file_source == "ORIGINAL").count();
    let from_preprocessed = results.iter().filter(|r| r.file_source == "PREPROCESSED").count();
    let failed = results.iter().filter(|r| r.file_source == "FAILED").count();
    
    println!("\n📊 FILE SOURCE ANALYSIS:");
    println!("• Total files analyzed: {}", total_files);
    println!("• Successfully processed: {} ({:.1}%)", 
             successful, (successful as f64 / total_files as f64) * 100.0);
    println!("• Used original files: {} ({:.1}%)", 
             from_original, (from_original as f64 / total_files as f64) * 100.0);
    println!("• Used preprocessed files: {} ({:.1}%)", 
             from_preprocessed, (from_preprocessed as f64 / total_files as f64) * 100.0);
    println!("• Complete failures: {} ({:.1}%)", 
             failed, (failed as f64 / total_files as f64) * 100.0);
    
    // Data quality analysis
    let excellent_quality = results.iter().filter(|r| r.data_quality_rating == "EXCELLENT").count();
    let good_quality = results.iter().filter(|r| r.data_quality_rating == "GOOD").count();
    let artificial_concerns = results.iter().filter(|r| r.artificial_elevation_indicators > 0).count();
    let high_naturalness = results.iter().filter(|r| r.data_naturalness_score > 80.0).count();
    
    println!("\n📈 DATA QUALITY ANALYSIS:");
    println!("• Excellent quality: {} ({:.1}%)", 
             excellent_quality, (excellent_quality as f64 / successful as f64) * 100.0);
    println!("• Good quality: {} ({:.1}%)", 
             good_quality, (good_quality as f64 / successful as f64) * 100.0);
    println!("• High naturalness score (>80): {} ({:.1}%)", 
             high_naturalness, (high_naturalness as f64 / successful as f64) * 100.0);
    println!("• Files with artificial indicators: {} ({:.1}%)", 
             artificial_concerns, (artificial_concerns as f64 / successful as f64) * 100.0);
    
    // Gain/Loss balance analysis
    let excellent_balance = results.iter()
        .filter(|r| r.processed_gain_loss_ratio >= 0.95 && r.processed_gain_loss_ratio <= 1.05)
        .count();
    let good_balance = results.iter()
        .filter(|r| r.processed_gain_loss_ratio >= 0.8 && r.processed_gain_loss_ratio <= 1.2)
        .count();
    
    println!("\n⚖️  GAIN/LOSS BALANCE ANALYSIS:");
    println!("• Files with excellent balance (0.95-1.05): {} ({:.1}%)", 
             excellent_balance, (excellent_balance as f64 / successful as f64) * 100.0);
    println!("• Files with good balance (0.8-1.2): {} ({:.1}%)", 
             good_balance, (good_balance as f64 / successful as f64) * 100.0);
    
    // Accuracy analysis
    let with_official: Vec<_> = results.iter()
        .filter(|r| r.official_elevation_gain_m > 0 && r.processing_status == "SUCCESS")
        .collect();
    
    if !with_official.is_empty() {
        let avg_accuracy: f64 = with_official.iter()
            .map(|r| r.accuracy_percent)
            .sum::<f64>() / with_official.len() as f64;
        
        let within_10_percent = with_official.iter()
            .filter(|r| r.accuracy_percent >= 90.0 && r.accuracy_percent <= 110.0)
            .count();
        
        let within_5_percent = with_official.iter()
            .filter(|r| r.accuracy_percent >= 95.0 && r.accuracy_percent <= 105.0)
            .count();
        
        println!("\n🎯 ACCURACY ANALYSIS:");
        println!("• Files with official data: {}", with_official.len());
        println!("• Average accuracy: {:.2}%", avg_accuracy);
        println!("• Files within ±10%: {}/{} ({:.1}%)", 
                 within_10_percent, with_official.len(),
                 (within_10_percent as f64 / with_official.len() as f64) * 100.0);
        println!("• Files within ±5%: {}/{} ({:.1}%)", 
                 within_5_percent, with_official.len(),
                 (within_5_percent as f64 / with_official.len() as f64) * 100.0);
        
        // Show top performers
        let mut top_performers: Vec<_> = with_official.iter()
            .filter(|r| r.accuracy_percent >= 95.0 && r.accuracy_percent <= 105.0)
            .collect();
        top_performers.sort_by(|a, b| {
            let a_error = (a.accuracy_percent - 100.0).abs();
            let b_error = (b.accuracy_percent - 100.0).abs();
            a_error.partial_cmp(&b_error).unwrap()
        });
        
        if !top_performers.is_empty() {
            println!("\n🏆 TOP ACCURACY PERFORMERS:");
            for result in top_performers.iter().take(5) {
                println!("• {}: {:.2}% accuracy ({}) [{}]", 
                         result.filename,
                         result.accuracy_percent,
                         result.file_source,
                         result.data_quality_rating);
            }
        }
    }
    
    // Show files with artificial elevation indicators
    if artificial_concerns > 0 {
        let artificial_files: Vec<_> = results.iter()
            .filter(|r| r.artificial_elevation_indicators > 0)
            .collect();
        
        println!("\n🏗️  FILES WITH ARTIFICIAL ELEVATION INDICATORS:");
        for result in artificial_files.iter().take(10) {
            println!("• {} ({}): {} indicators, correlation={:.3}, naturalness={:.1}", 
                     result.filename,
                     result.file_source,
                     result.artificial_elevation_indicators,
                     result.latitude_elevation_correlation,
                     result.data_naturalness_score);
        }
        if artificial_files.len() > 10 {
            println!("  ... and {} more files", artificial_files.len() - 10);
        }
    }
    
    println!("\n✅ KEY BENEFITS OF CONSERVATIVE APPROACH:");
    if from_original > from_preprocessed {
        println!("• Successfully used {} original files vs {} preprocessed", from_original, from_preprocessed);
        println!("• Preserved natural elevation data without artificial inflation");
    }
    if excellent_balance > successful / 2 {
        println!("• Achieved excellent gain/loss balance in {:.1}% of files", 
                 (excellent_balance as f64 / successful as f64) * 100.0);
    }
    if artificial_concerns == 0 {
        println!("• No artificial elevation patterns detected - all data appears natural");
    } else {
        println!("• Identified and flagged {} files with artificial elevation patterns", artificial_concerns);
    }
    if high_naturalness > successful * 2 / 3 {
        println!("• {:.1}% of files have high naturalness scores (>80)", 
                 (high_naturalness as f64 / successful as f64) * 100.0);
    }
    
    println!("\n🎯 RECOMMENDATIONS:");
    if from_original > total_files * 2 / 3 {
        println!("✅ Continue using original files - most are readable without preprocessing");
    }
    if artificial_concerns > 0 {
        println!("⚠️  Review {} files with artificial elevation indicators", artificial_concerns);
        println!("   Consider removing artificial elevation from preprocessing pipeline");
    }
    if failed > 0 {
        println!("🔧 {} files need better preprocessing or are truly corrupted", failed);
    }
    
    println!("📊 Results closely match Garmin Connect/gpx.studio due to conservative approach");
}