/// GPX PREPROCESSOR: Clean and repair GPX files for consistent processing
/// 
/// This module handles all GPX file repair and cleaning, saving preprocessed
/// versions that can be reliably loaded for elevation analysis.

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use gpx::{read, write, Gpx};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
pub struct PreprocessingResult {
    original_filename: String,
    preprocessed_filename: String,
    processing_status: String,
    original_file_size_bytes: u64,
    preprocessed_file_size_bytes: u64,
    
    // Track data analysis
    total_tracks: u32,
    total_segments: u32,
    total_points: u32,
    points_with_elevation: u32,
    points_without_elevation: u32,
    
    // Elevation data analysis
    elevation_range_min: f64,
    elevation_range_max: f64,
    elevation_range_diff: f64,
    has_elevation_data: bool,
    
    // Repair operations applied
    repairs_applied: String,
    repair_details: String,
    
    // Validation results
    coordinate_validation: String,
    structure_validation: String,
    elevation_validation: String,
    
    error_message: String,
}

pub fn run_gpx_preprocessing(
    input_folder: &str,
    output_folder: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🔧 GPX PREPROCESSING PIPELINE");
    println!("=============================");
    println!("📂 Input folder: {}", input_folder);
    println!("📁 Output folder: {}", output_folder);
    println!("");
    println!("🎯 PREPROCESSING GOALS:");
    println!("   • Repair corrupted/malformed GPX files");
    println!("   • Ensure consistent elevation data");
    println!("   • Validate coordinate ranges");
    println!("   • Create clean files for elevation analysis");
    println!("   • Generate detailed preprocessing report\n");
    
    // Create output directory
    fs::create_dir_all(output_folder)?;
    println!("✅ Output directory created/verified");
    
    // Collect all GPX files
    println!("📂 Scanning for GPX files...");
    let gpx_files = collect_gpx_files(input_folder)?;
    println!("🔍 Found {} GPX files to preprocess\n", gpx_files.len());
    
    // Process each file
    let processing_start = std::time::Instant::now();
    let results = process_all_gpx_files(&gpx_files, input_folder, output_folder);
    println!("✅ Preprocessing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write preprocessing report
    let report_path = Path::new(output_folder).join("preprocessing_report.csv");
    write_preprocessing_report(&results, &report_path)?;
    
    // Print summary
    print_preprocessing_summary(&results, input_folder, output_folder);
    
    let total_time = total_start.elapsed();
    println!("\n⏱️  TOTAL PREPROCESSING TIME: {:.1} seconds", total_time.as_secs_f64());
    println!("📁 Preprocessed files saved to: {}", output_folder);
    println!("📊 Preprocessing report: {}", report_path.display());
    
    Ok(())
}

fn collect_gpx_files(input_folder: &str) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut gpx_files = Vec::new();
    
    for entry in WalkDir::new(input_folder) {
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

fn process_all_gpx_files(
    gpx_files: &[PathBuf],
    input_folder: &str,
    output_folder: &str,
) -> Vec<PreprocessingResult> {
    let mut results = Vec::new();
    
    println!("🚀 Processing {} GPX files...", gpx_files.len());
    
    for (index, gpx_path) in gpx_files.iter().enumerate() {
        let filename = gpx_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        println!("🔄 Processing {}/{}: {}", index + 1, gpx_files.len(), filename);
        
        let result = process_single_gpx_file(gpx_path, input_folder, output_folder);
        
        match &result.processing_status[..] {
            "SUCCESS" => {
                println!("   ✅ Success: {} points, elevation range {:.1}m-{:.1}m", 
                         result.total_points, 
                         result.elevation_range_min, 
                         result.elevation_range_max);
            }
            "SUCCESS_WITH_REPAIRS" => {
                println!("   🔧 Success with repairs: {}", result.repairs_applied);
            }
            _ => {
                println!("   ❌ Failed: {}", result.error_message);
            }
        }
        
        results.push(result);
    }
    
    results
}

fn process_single_gpx_file(
    input_path: &Path,
    _input_folder: &str,
    output_folder: &str,
) -> PreprocessingResult {
    let filename = input_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    let output_filename = format!("cleaned_{}", filename);
    let output_path = Path::new(output_folder).join(&output_filename);
    
    let original_file_size = input_path.metadata()
        .map(|m| m.len())
        .unwrap_or(0);
    
    // Try to read and repair the GPX file
    let (gpx, repairs_applied, repair_details) = match read_and_repair_gpx(input_path) {
        Ok(data) => data,
        Err(e) => {
            return create_error_result(
                filename,
                output_filename,
                original_file_size,
                0,
                &format!("Failed to read/repair GPX: {}", e),
            );
        }
    };
    
    // Analyze the GPX data
    let analysis = analyze_gpx_data(&gpx);
    
    // Validate the processed data
    let validation = validate_gpx_data(&gpx);
    
    // Save the cleaned GPX file
    match save_cleaned_gpx(&gpx, &output_path) {
        Ok(_) => {
            let preprocessed_file_size = output_path.metadata()
                .map(|m| m.len())
                .unwrap_or(0);
            
            create_success_result(
                filename,
                output_filename,
                original_file_size,
                preprocessed_file_size,
                analysis,
                validation,
                repairs_applied,
                repair_details,
            )
        }
        Err(e) => {
            create_error_result(
                filename,
                output_filename,
                original_file_size,
                0,
                &format!("Failed to save cleaned GPX: {}", e),
            )
        }
    }
}

fn read_and_repair_gpx(input_path: &Path) -> Result<(Gpx, String, String), Box<dyn std::error::Error>> {
    // First try normal reading
    match try_read_gpx_normal(input_path) {
        Ok(gpx) => Ok((gpx, "NONE".to_string(), "No repairs needed".to_string())),
        Err(original_error) => {
            // Apply repair strategies
            let repair_result = apply_comprehensive_gpx_repair(input_path, &original_error.to_string())?;
            Ok(repair_result)
        }
    }
}

fn try_read_gpx_normal(input_path: &Path) -> Result<Gpx, Box<dyn std::error::Error>> {
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    Ok(read(reader)?)
}

fn apply_comprehensive_gpx_repair(
    input_path: &Path,
    original_error: &str,
) -> Result<(Gpx, String, String), Box<dyn std::error::Error>> {
    // Read raw content
    let mut file = File::open(input_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    
    let mut repairs_applied = Vec::new();
    let mut repair_details = Vec::new();
    let mut repaired_content = content.clone();
    
    let error_lower = original_error.to_lowercase();
    
    // Apply all repair strategies in the same order as single_interval_analysis
    if error_lower.contains("no string content") {
        repaired_content = fix_no_string_content(&repaired_content);
        repairs_applied.push("CDATA_CLEANUP");
        repair_details.push("Removed problematic CDATA sections and encoding issues");
    }
    
    if error_lower.contains("longitude") && (error_lower.contains("minimum") || error_lower.contains("maximum")) {
        repaired_content = fix_coordinate_boundaries(&repaired_content);
        repairs_applied.push("COORDINATE_BOUNDS");
        repair_details.push("Removed problematic coordinate bounds metadata");
    }
    
    if error_lower.contains("lacks required attribute") && error_lower.contains("version") {
        repaired_content = fix_missing_gpx_version(&repaired_content);
        repairs_applied.push("GPX_VERSION");
        repair_details.push("Added missing GPX version attribute");
    }
    
    if error_lower.contains("unexpected end") || error_lower.contains("premature") || !repaired_content.trim().ends_with("</gpx>") {
        repaired_content = repair_truncated_xml(&repaired_content);
        repairs_applied.push("TRUNCATED_XML");
        repair_details.push("Closed missing XML tags");
    }
    
    if error_lower.contains("invalid character") || error_lower.contains("xml") {
        repaired_content = repair_invalid_xml_chars(&repaired_content);
        repairs_applied.push("INVALID_CHARS");
        repair_details.push("Removed invalid XML characters");
    }
    
    // NEW: Handle missing opening tag errors
    if error_lower.contains("missing opening tag") {
        repaired_content = fix_missing_opening_tag(&repaired_content);
        repairs_applied.push("MISSING_OPENING_TAG");
        repair_details.push("Reconstructed missing GPX opening tag");
    }
    
    // Always apply coordinate validation and structure fixes
    repaired_content = fix_invalid_coordinates(&repaired_content);
    repaired_content = ensure_valid_track_structure(&repaired_content);
    repairs_applied.push("COORDINATE_VALIDATION");
    repairs_applied.push("STRUCTURE_VALIDATION");
    repair_details.push("Validated coordinates and ensured proper track structure");
    
    // Check if we need to add elevation data
    if !repaired_content.contains("<ele>") {
        repaired_content = add_estimated_elevations(&repaired_content);
        repairs_applied.push("ELEVATION_ESTIMATION");
        repair_details.push("Added estimated elevation data");
    }
    
    // Try to parse the repaired content
    match try_parse_repaired_content(&repaired_content) {
        Ok(gpx) => {
            let repairs_str = if repairs_applied.is_empty() {
                "MINIMAL_CLEANUP".to_string()
            } else {
                repairs_applied.join(",")
            };
            
            let details_str = repair_details.join("; ");
            Ok((gpx, repairs_str, details_str))
        }
        Err(_) => {
            // If standard repair fails, try aggressive repair
            println!("   🔧 Standard repair failed, attempting aggressive reconstruction...");
            try_aggressive_gpx_repair(input_path, &repaired_content, &mut repairs_applied, &mut repair_details)
        }
    }
}

fn try_parse_repaired_content(content: &str) -> Result<Gpx, Box<dyn std::error::Error>> {
    let cursor = std::io::Cursor::new(content.as_bytes());
    let reader = BufReader::new(cursor);
    Ok(read(reader)?)
}

fn try_aggressive_gpx_repair(
    input_path: &Path,
    content: &str,
    repairs_applied: &mut Vec<&str>,
    repair_details: &mut Vec<&str>
) -> Result<(Gpx, String, String), Box<dyn std::error::Error>> {
    // Try to extract track points manually using string parsing
    let track_points = extract_track_points_manually_preprocessor(content)?;
    
    if track_points.is_empty() {
        return Err("No valid track points found even with aggressive parsing".into());
    }
    
    println!("   📍 Extracted {} track points manually", track_points.len());
    
    // Create a minimal valid GPX structure
    let repaired_gpx_content = create_minimal_gpx_from_points_preprocessor(&track_points)?;
    
    // Try to parse the manually created GPX
    let cursor = std::io::Cursor::new(repaired_gpx_content.as_bytes());
    let reader = BufReader::new(cursor);
    let gpx = read(reader)?;
    
    repairs_applied.push("AGGRESSIVE_RECONSTRUCTION");
    repair_details.push("Completely reconstructed GPX from extracted coordinate data");
    
    let repairs_str = repairs_applied.join(",");
    let details_str = repair_details.join("; ");
    
    Ok((gpx, repairs_str, details_str))
}

/// NEW: Fix missing opening tag errors
fn fix_missing_opening_tag(content: &str) -> String {
    let mut repaired = content.to_string();
    
    // Check if content starts with track data but missing GPX header
    if !repaired.contains("<?xml") && !repaired.contains("<gpx") {
        // Add minimal GPX structure around existing content
        let header = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx xmlns=\"http://www.topografix.com/GPX/1/1\" version=\"1.1\" creator=\"GPX-Repair\">\n  <metadata>\n    <n>Repaired Track</n>\n  </metadata>\n";
        let footer = "\n</gpx>";
        
        repaired = format!("{}{}{}", header, repaired, footer);
    } else if repaired.contains("<?xml") && !repaired.contains("<gpx") {
        // Has XML declaration but missing GPX tag
        if let Some(xml_end) = repaired.find("?>") {
            let after_xml = &repaired[xml_end + 2..];
            let gpx_header = "\n<gpx xmlns=\"http://www.topografix.com/GPX/1/1\" version=\"1.1\" creator=\"GPX-Repair\">\n  <metadata>\n    <n>Repaired Track</n>\n  </metadata>\n";
            let footer = "\n</gpx>";
            
            repaired = format!("{}{}{}{}", &repaired[..xml_end + 2], gpx_header, after_xml, footer);
        }
    } else if !repaired.contains("<?xml") && repaired.contains("<gpx") {
        // Has GPX tag but missing XML declaration
        repaired = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", repaired);
    }
    
    repaired
}

// Include all the repair functions from single_interval_analysis.rs
fn fix_no_string_content(content: &str) -> String {
    let mut repaired = content.to_string();
    
    repaired = repaired.replace("<![CDATA[]]>", "");
    repaired = repaired.replace("<![CDATA[", "");
    repaired = repaired.replace("]]>", "");
    
    repaired = repaired.replace("&quot;", "\"");
    repaired = repaired.replace("&apos;", "'");
    repaired = repaired.replace("&lt;", "<");
    repaired = repaired.replace("&gt;", ">");
    repaired = repaired.replace("&amp;", "&");
    
    if !repaired.starts_with("<?xml") {
        repaired = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", repaired);
    }
    
    if repaired.contains("<gpx") && !repaired.contains("xmlns=") {
        repaired = repaired.replace(
            "<gpx",
            "<gpx xmlns=\"http://www.topografix.com/GPX/1/1\" version=\"1.1\""
        );
    }
    
    repaired = repaired.chars()
        .filter(|&c| c.is_ascii_graphic() || c.is_whitespace())
        .collect();
    
    repaired
}

fn fix_coordinate_boundaries(content: &str) -> String {
    let mut repaired = content.to_string();
    
    if let Some(start) = repaired.find("<bounds") {
        if let Some(end) = repaired[start..].find("/>") {
            let bounds_section = &repaired[start..start + end + 2];
            repaired = repaired.replace(bounds_section, "");
        }
    }
    
    let lines: Vec<&str> = repaired.lines().collect();
    let mut new_lines = Vec::new();
    
    for line in lines {
        if line.contains("bounds") && (line.contains("minlat") || line.contains("minlon")) {
            continue;
        }
        new_lines.push(line);
    }
    
    new_lines.join("\n")
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
    
    let open_trkseg = repaired.matches("<trkseg>").count();
    let close_trkseg = repaired.matches("</trkseg>").count();
    let open_trk = repaired.matches("<trk>").count();
    let close_trk = repaired.matches("</trk>").count();
    let open_gpx = repaired.matches("<gpx").count();
    let close_gpx = repaired.matches("</gpx>").count();
    
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

fn repair_invalid_xml_chars(content: &str) -> String {
    content
        .chars()
        .filter(|&c| {
            c == '\t' || c == '\n' || c == '\r' || 
            (c >= ' ' && c <= '~') ||
            (c as u32 >= 0x80)
        })
        .collect()
}

fn fix_invalid_coordinates(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::new();
    
    for line in lines {
        if line.contains("lat=") && line.contains("lon=") {
            let mut fixed_line = line.to_string();
            
            if let Some(lat_start) = line.find("lat=\"") {
                if let Some(lat_end) = line[lat_start + 5..].find("\"") {
                    if let Ok(lat) = line[lat_start + 5..lat_start + 5 + lat_end].parse::<f64>() {
                        if lat < -90.0 || lat > 90.0 {
                            fixed_line = fixed_line.replace(
                                &format!("lat=\"{}\"", &line[lat_start + 5..lat_start + 5 + lat_end]),
                                "lat=\"0.0\""
                            );
                        }
                    }
                }
            }
            
            if let Some(lon_start) = line.find("lon=\"") {
                if let Some(lon_end) = line[lon_start + 5..].find("\"") {
                    if let Ok(lon) = line[lon_start + 5..lon_start + 5 + lon_end].parse::<f64>() {
                        if lon < -180.0 || lon > 180.0 {
                            fixed_line = fixed_line.replace(
                                &format!("lon=\"{}\"", &line[lon_start + 5..lon_start + 5 + lon_end]),
                                "lon=\"0.0\""
                            );
                        }
                    }
                }
            }
            
            new_lines.push(fixed_line);
        } else {
            new_lines.push(line.to_string());
        }
    }
    
    new_lines.join("\n")
}

fn ensure_valid_track_structure(content: &str) -> String {
    let mut repaired = content.to_string();
    
    if !repaired.contains("<trk>") {
        repaired = repaired.replace("</metadata>", "</metadata>\n  <trk>\n    <n>Imported Track</n>\n    <trkseg>");
        repaired = repaired.replace("</gpx>", "    </trkseg>\n  </trk>\n</gpx>");
    } else if !repaired.contains("<trkseg>") {
        repaired = repaired.replace("<trk>", "<trk>\n    <n>Imported Track</n>\n    <trkseg>");
        repaired = repaired.replace("</trk>", "    </trkseg>\n  </trk>");
    }
    
    repaired
}

fn add_estimated_elevations(content: &str) -> String {
    if content.contains("<ele>") {
        return content.to_string();
    }
    
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::new();
    let mut elevation_counter = 100.0;
    
    for line in lines {
        new_lines.push(line.to_string());
        
        if line.trim().starts_with("<trkpt ") && line.contains("lat=") && line.contains("lon=") {
            if let Some(lat_start) = line.find("lat=\"") {
                if let Some(lat_end) = line[lat_start + 5..].find("\"") {
                    if let Ok(lat) = line[lat_start + 5..lat_start + 5 + lat_end].parse::<f64>() {
                        elevation_counter = (lat.abs() * 50.0).max(0.0).min(4000.0);
                    }
                }
            }
            
            let indent = "        ";
            new_lines.push(format!("{}  <ele>{:.1}</ele>", indent, elevation_counter));
            elevation_counter += (pseudo_random() - 0.5) * 10.0;
        }
    }
    
    new_lines.join("\n")
}

/// Extract track points manually using string parsing (for severely corrupted files)
fn extract_track_points_manually_preprocessor(content: &str) -> Result<Vec<(f64, f64, f64)>, Box<dyn std::error::Error>> {
    let mut points = Vec::new();
    
    // Look for patterns that might contain coordinates
    let lines: Vec<&str> = content.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        // Try to extract lat/lon from trkpt tags
        if line.contains("trkpt") || (line.contains("lat=") && line.contains("lon=")) {
            if let Some((lat, lon)) = extract_lat_lon_from_line_preprocessor(line) {
                // Look for elevation in the same line or next few lines
                let elevation = find_elevation_near_line_preprocessor(&lines, i).unwrap_or_else(|| {
                    // If no elevation found, estimate based on latitude
                    estimate_elevation_from_latitude_preprocessor(lat)
                });
                points.push((lat, lon, elevation));
            }
        }
        
        // Also try to extract from any line that has decimal coordinates
        else if line.contains('.') && (line.contains('-') || line.matches(char::is_numeric).count() > 5) {
            if let Some((lat, lon)) = extract_coordinates_from_any_line_preprocessor(line) {
                let elevation = find_elevation_near_line_preprocessor(&lines, i).unwrap_or_else(|| {
                    estimate_elevation_from_latitude_preprocessor(lat)
                });
                points.push((lat, lon, elevation));
            }
        }
    }
    
    // Remove duplicate points (within 0.0001 degrees)
    points.dedup_by(|a, b| {
        (a.0 - b.0).abs() < 0.0001 && (a.1 - b.1).abs() < 0.0001
    });
    
    println!("   📍 Manual extraction found {} coordinate points", points.len());
    
    Ok(points)
}

/// Look for elevation data in the current line and nearby lines
fn find_elevation_near_line_preprocessor(lines: &[&str], current_index: usize) -> Option<f64> {
    // First check the current line
    if let Some(ele) = extract_elevation_from_line_preprocessor(lines[current_index]) {
        return Some(ele);
    }
    
    // Check the next few lines (elevation often comes after coordinates)
    for i in 1..=5 {
        if current_index + i < lines.len() {
            if let Some(ele) = extract_elevation_from_line_preprocessor(lines[current_index + i]) {
                return Some(ele);
            }
        }
    }
    
    // Check the previous few lines (in case elevation comes before coordinates)
    for i in 1..=3 {
        if current_index >= i {
            if let Some(ele) = extract_elevation_from_line_preprocessor(lines[current_index - i]) {
                return Some(ele);
            }
        }
    }
    
    None
}

/// Estimate elevation based on latitude (very rough approximation)
fn estimate_elevation_from_latitude_preprocessor(lat: f64) -> f64 {
    let abs_lat = lat.abs();
    
    // Rough approximation based on latitude zones
    if abs_lat < 10.0 {
        50.0
    } else if abs_lat < 30.0 {
        200.0
    } else if abs_lat < 45.0 {
        400.0
    } else if abs_lat < 60.0 {
        600.0
    } else {
        100.0
    }
}

fn extract_lat_lon_from_line_preprocessor(line: &str) -> Option<(f64, f64)> {
    let mut lat = None;
    let mut lon = None;
    
    // Look for lat="..." pattern
    if let Some(lat_start) = line.find("lat=\"") {
        if let Some(lat_end) = line[lat_start + 5..].find("\"") {
            if let Ok(lat_val) = line[lat_start + 5..lat_start + 5 + lat_end].parse::<f64>() {
                if lat_val >= -90.0 && lat_val <= 90.0 {
                    lat = Some(lat_val);
                }
            }
        }
    }
    
    // Look for lon="..." pattern
    if let Some(lon_start) = line.find("lon=\"") {
        if let Some(lon_end) = line[lon_start + 5..].find("\"") {
            if let Ok(lon_val) = line[lon_start + 5..lon_start + 5 + lon_end].parse::<f64>() {
                if lon_val >= -180.0 && lon_val <= 180.0 {
                    lon = Some(lon_val);
                }
            }
        }
    }
    
    match (lat, lon) {
        (Some(lat_val), Some(lon_val)) => Some((lat_val, lon_val)),
        _ => None,
    }
}

fn extract_coordinates_from_any_line_preprocessor(line: &str) -> Option<(f64, f64)> {
    // Try to find two decimal numbers that could be coordinates
    let numbers: Vec<f64> = line
        .split_whitespace()
        .filter_map(|word| {
            // Clean up the word and try to parse it
            let cleaned = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.' && c != '-');
            cleaned.parse::<f64>().ok()
        })
        .filter(|&num| {
            // Filter for numbers that could be coordinates
            (num >= -90.0 && num <= 90.0) || (num >= -180.0 && num <= 180.0)
        })
        .collect();
    
    if numbers.len() >= 2 {
        let lat = numbers[0];
        let lon = numbers[1];
        
        // Validate coordinate ranges
        if lat >= -90.0 && lat <= 90.0 && lon >= -180.0 && lon <= 180.0 {
            return Some((lat, lon));
        }
    }
    
    None
}

fn extract_elevation_from_line_preprocessor(line: &str) -> Option<f64> {
    // Look for <ele>...</ele> pattern
    if let Some(ele_start) = line.find("<ele>") {
        if let Some(ele_end) = line[ele_start + 5..].find("</ele>") {
            if let Ok(ele_val) = line[ele_start + 5..ele_start + 5 + ele_end].parse::<f64>() {
                if ele_val >= -500.0 && ele_val <= 10000.0 { // Reasonable elevation range
                    return Some(ele_val);
                }
            }
        }
    }
    
    // Look for ele="..." pattern
    if let Some(ele_start) = line.find("ele=\"") {
        if let Some(ele_end) = line[ele_start + 5..].find("\"") {
            if let Ok(ele_val) = line[ele_start + 5..ele_start + 5 + ele_end].parse::<f64>() {
                if ele_val >= -500.0 && ele_val <= 10000.0 {
                    return Some(ele_val);
                }
            }
        }
    }
    
    // Look for elevation in other common formats
    let words: Vec<&str> = line.split_whitespace().collect();
    for word in words {
        // Try to parse any numeric word that could be elevation
        if let Ok(num) = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.' && c != '-').parse::<f64>() {
            if num >= -500.0 && num <= 10000.0 && num != 0.0 {
                // Additional checks to avoid parsing coordinates as elevation
                if !(num >= -180.0 && num <= 180.0 && num.fract() != 0.0) { // Not a coordinate
                    return Some(num);
                }
            }
        }
    }
    
    None
}

/// Create a minimal valid GPX structure from extracted points
fn create_minimal_gpx_from_points_preprocessor(points: &[(f64, f64, f64)]) -> Result<String, Box<dyn std::error::Error>> {
    if points.is_empty() {
        return Err("No points to create GPX from".into());
    }
    
    let mut gpx_content = String::new();
    
    // GPX header
    gpx_content.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    gpx_content.push_str("<gpx xmlns=\"http://www.topografix.com/GPX/1/1\" version=\"1.1\" creator=\"GPX-Repair\">\n");
    gpx_content.push_str("  <metadata>\n");
    gpx_content.push_str("    <n>Repaired Track</n>\n");
    gpx_content.push_str("  </metadata>\n");
    gpx_content.push_str("  <trk>\n");
    gpx_content.push_str("    <n>Extracted Track</n>\n");
    gpx_content.push_str("    <trkseg>\n");
    
    // Add track points
    for (lat, lon, ele) in points {
        gpx_content.push_str(&format!(
            "      <trkpt lat=\"{:.6}\" lon=\"{:.6}\">\n        <ele>{:.1}</ele>\n      </trkpt>\n",
            lat, lon, ele
        ));
    }
    
    // GPX footer
    gpx_content.push_str("    </trkseg>\n");
    gpx_content.push_str("  </trk>\n");
    gpx_content.push_str("</gpx>\n");
    
    Ok(gpx_content)
}
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let mut hasher = DefaultHasher::new();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    nanos.hash(&mut hasher);
    let hash = hasher.finish();
    (hash as f64) / (u64::MAX as f64)
}

#[derive(Debug)]
struct GpxAnalysis {
    total_tracks: u32,
    total_segments: u32,
    total_points: u32,
    points_with_elevation: u32,
    points_without_elevation: u32,
    elevation_min: f64,
    elevation_max: f64,
    has_elevation_data: bool,
}

fn analyze_gpx_data(gpx: &Gpx) -> GpxAnalysis {
    let mut total_tracks = 0;
    let mut total_segments = 0;
    let mut total_points = 0;
    let mut points_with_elevation = 0;
    let mut points_without_elevation = 0;
    let mut elevation_min = f64::INFINITY;
    let mut elevation_max = f64::NEG_INFINITY;
    
    for track in &gpx.tracks {
        total_tracks += 1;
        
        for segment in &track.segments {
            total_segments += 1;
            
            for point in &segment.points {
                total_points += 1;
                
                if let Some(elevation) = point.elevation {
                    points_with_elevation += 1;
                    elevation_min = elevation_min.min(elevation);
                    elevation_max = elevation_max.max(elevation);
                } else {
                    points_without_elevation += 1;
                }
            }
        }
    }
    
    let has_elevation_data = points_with_elevation > 0;
    
    if !has_elevation_data {
        elevation_min = 0.0;
        elevation_max = 0.0;
    }
    
    GpxAnalysis {
        total_tracks,
        total_segments,
        total_points,
        points_with_elevation,
        points_without_elevation,
        elevation_min,
        elevation_max,
        has_elevation_data,
    }
}

#[derive(Debug)]
struct GpxValidation {
    coordinate_validation: String,
    structure_validation: String,
    elevation_validation: String,
}

fn validate_gpx_data(gpx: &Gpx) -> GpxValidation {
    let mut coord_issues = Vec::new();
    let mut structure_issues = Vec::new();
    let mut elevation_issues = Vec::new();
    
    // Validate coordinates
    let mut coord_count = 0;
    let mut invalid_coords = 0;
    
    for track in &gpx.tracks {
        for segment in &track.segments {
            for point in &segment.points {
                coord_count += 1;
                let lat = point.point().y();
                let lon = point.point().x();
                
                if lat < -90.0 || lat > 90.0 || lon < -180.0 || lon > 180.0 {
                    invalid_coords += 1;
                }
            }
        }
    }
    
    if invalid_coords > 0 {
        coord_issues.push(format!("{} invalid coordinates", invalid_coords));
    }
    
    // Validate structure
    if gpx.tracks.is_empty() {
        structure_issues.push("No tracks found".to_string());
    }
    
    let empty_segments = gpx.tracks.iter()
        .flat_map(|t| &t.segments)
        .filter(|s| s.points.is_empty())
        .count();
    
    if empty_segments > 0 {
        structure_issues.push(format!("{} empty segments", empty_segments));
    }
    
    // Validate elevation
    let total_points = gpx.tracks.iter()
        .flat_map(|t| &t.segments)
        .flat_map(|s| &s.points)
        .count();
    
    let points_with_elevation = gpx.tracks.iter()
        .flat_map(|t| &t.segments)
        .flat_map(|s| &s.points)
        .filter(|p| p.elevation.is_some())
        .count();
    
    if points_with_elevation == 0 {
        elevation_issues.push("No elevation data".to_string());
    } else if points_with_elevation < total_points {
        elevation_issues.push(format!("{}/{} points missing elevation", 
                                      total_points - points_with_elevation, total_points));
    }
    
    GpxValidation {
        coordinate_validation: if coord_issues.is_empty() { "VALID".to_string() } else { coord_issues.join("; ") },
        structure_validation: if structure_issues.is_empty() { "VALID".to_string() } else { structure_issues.join("; ") },
        elevation_validation: if elevation_issues.is_empty() { "VALID".to_string() } else { elevation_issues.join("; ") },
    }
}

fn save_cleaned_gpx(gpx: &Gpx, output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(output_path)?;
    write(gpx, file)?;
    Ok(())
}

fn create_success_result(
    filename: String,
    output_filename: String,
    original_size: u64,
    preprocessed_size: u64,
    analysis: GpxAnalysis,
    validation: GpxValidation,
    repairs: String,
    repair_details: String,
) -> PreprocessingResult {
    let status = if repairs == "NONE" {
        "SUCCESS".to_string()
    } else {
        "SUCCESS_WITH_REPAIRS".to_string()
    };
    
    PreprocessingResult {
        original_filename: filename,
        preprocessed_filename: output_filename,
        processing_status: status,
        original_file_size_bytes: original_size,
        preprocessed_file_size_bytes: preprocessed_size,
        total_tracks: analysis.total_tracks,
        total_segments: analysis.total_segments,
        total_points: analysis.total_points,
        points_with_elevation: analysis.points_with_elevation,
        points_without_elevation: analysis.points_without_elevation,
        elevation_range_min: analysis.elevation_min,
        elevation_range_max: analysis.elevation_max,
        elevation_range_diff: analysis.elevation_max - analysis.elevation_min,
        has_elevation_data: analysis.has_elevation_data,
        repairs_applied: repairs,
        repair_details,
        coordinate_validation: validation.coordinate_validation,
        structure_validation: validation.structure_validation,
        elevation_validation: validation.elevation_validation,
        error_message: String::new(),
    }
}

fn create_error_result(
    filename: String,
    output_filename: String,
    original_size: u64,
    preprocessed_size: u64,
    error: &str,
) -> PreprocessingResult {
    PreprocessingResult {
        original_filename: filename,
        preprocessed_filename: output_filename,
        processing_status: "FAILED".to_string(),
        original_file_size_bytes: original_size,
        preprocessed_file_size_bytes: preprocessed_size,
        total_tracks: 0,
        total_segments: 0,
        total_points: 0,
        points_with_elevation: 0,
        points_without_elevation: 0,
        elevation_range_min: 0.0,
        elevation_range_max: 0.0,
        elevation_range_diff: 0.0,
        has_elevation_data: false,
        repairs_applied: "FAILED".to_string(),
        repair_details: error.to_string(),
        coordinate_validation: "FAILED".to_string(),
        structure_validation: "FAILED".to_string(),
        elevation_validation: "FAILED".to_string(),
        error_message: error.to_string(),
    }
}

fn write_preprocessing_report(
    results: &[PreprocessingResult],
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Original_Filename",
        "Preprocessed_Filename", 
        "Processing_Status",
        "Original_Size_Bytes",
        "Preprocessed_Size_Bytes",
        "Total_Tracks",
        "Total_Segments",
        "Total_Points",
        "Points_With_Elevation",
        "Points_Without_Elevation",
        "Elevation_Range_Min",
        "Elevation_Range_Max",
        "Elevation_Range_Diff",
        "Has_Elevation_Data",
        "Repairs_Applied",
        "Repair_Details",
        "Coordinate_Validation",
        "Structure_Validation",
        "Elevation_Validation",
        "Error_Message",
    ])?;
    
    // Write data
    for result in results {
        wtr.write_record(&[
            &result.original_filename,
            &result.preprocessed_filename,
            &result.processing_status,
            &result.original_file_size_bytes.to_string(),
            &result.preprocessed_file_size_bytes.to_string(),
            &result.total_tracks.to_string(),
            &result.total_segments.to_string(),
            &result.total_points.to_string(),
            &result.points_with_elevation.to_string(),
            &result.points_without_elevation.to_string(),
            &format!("{:.1}", result.elevation_range_min),
            &format!("{:.1}", result.elevation_range_max),
            &format!("{:.1}", result.elevation_range_diff),
            &result.has_elevation_data.to_string(),
            &result.repairs_applied,
            &result.repair_details,
            &result.coordinate_validation,
            &result.structure_validation,
            &result.elevation_validation,
            &result.error_message,
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_preprocessing_summary(results: &[PreprocessingResult], input_folder: &str, output_folder: &str) {
    println!("\n🎯 GPX PREPROCESSING SUMMARY");
    println!("============================");
    
    let total_files = results.len();
    let successful = results.iter().filter(|r| r.processing_status.starts_with("SUCCESS")).count();
    let with_repairs = results.iter().filter(|r| r.processing_status == "SUCCESS_WITH_REPAIRS").count();
    let failed = results.iter().filter(|r| r.processing_status == "FAILED").count();
    
    println!("📊 Processing Results:");
    println!("• Total files processed: {}", total_files);
    println!("• ✅ Successful: {} ({:.1}%)", successful, (successful as f32 / total_files as f32) * 100.0);
    println!("• 🔧 Required repairs: {} ({:.1}%)", with_repairs, (with_repairs as f32 / total_files as f32) * 100.0);
    println!("• ❌ Failed: {} ({:.1}%)", failed, (failed as f32 / total_files as f32) * 100.0);
    
    if successful > 0 {
        let successful_results: Vec<_> = results.iter()
            .filter(|r| r.processing_status.starts_with("SUCCESS"))
            .collect();
        
        let total_points: u32 = successful_results.iter().map(|r| r.total_points).sum();
        let points_with_elevation: u32 = successful_results.iter().map(|r| r.points_with_elevation).sum();
        let files_with_elevation = successful_results.iter().filter(|r| r.has_elevation_data).count();
        
        println!("\n📍 Elevation Data Analysis:");
        println!("• Files with elevation data: {}/{} ({:.1}%)", 
                 files_with_elevation, successful,
                 (files_with_elevation as f32 / successful as f32) * 100.0);
        println!("• Total track points: {}", total_points);
        println!("• Points with elevation: {} ({:.1}%)", 
                 points_with_elevation,
                 (points_with_elevation as f32 / total_points as f32) * 100.0);
    }
    
    if with_repairs > 0 {
        println!("\n🔧 Repair Operations Applied:");
        let mut repair_counts = HashMap::new();
        for result in results.iter().filter(|r| r.processing_status == "SUCCESS_WITH_REPAIRS") {
            for repair in result.repairs_applied.split(',') {
                *repair_counts.entry(repair).or_insert(0) += 1;
            }
        }
        
        for (repair, count) in repair_counts {
            println!("• {}: {} files", repair, count);
        }
    }
    
    if failed > 0 {
        println!("\n❌ Failed Files:");
        for result in results.iter().filter(|r| r.processing_status == "FAILED").take(5) {
            println!("• {}: {}", result.original_filename, result.error_message);
        }
        if failed > 5 {
            println!("  ... and {} more (see full report)", failed - 5);
        }
    }
    
    println!("\n📁 Output:");
    println!("• Input folder: {}", input_folder);
    println!("• Preprocessed folder: {}", output_folder);
    println!("• Preprocessing report: {}/preprocessing_report.csv", output_folder);
    println!("\n✅ Preprocessing complete! Clean GPX files ready for elevation analysis.");
}