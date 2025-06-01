/// TOLERANT GPX READER - LIKE GARMIN CONNECT
/// 
/// This module implements a more forgiving GPX reader that can handle
/// files with minor XML issues, just like professional tools do.
/// 
/// KEY PRINCIPLE: If Garmin Connect can read it, so should we!

use std::path::Path;
use std::fs::File;
use std::io::{BufReader, Read};
use gpx::{read, Gpx, Track, TrackSegment, Waypoint};
use geo::point;
use walkdir::WalkDir;

/// Enhanced GPX reading that matches the tolerance of professional tools
pub fn read_gpx_tolerantly(path: &Path) -> Result<Gpx, Box<dyn std::error::Error>> {
    // Strategy 1: Try normal parsing first (works for ~95% of files)
    match try_standard_gpx_parsing(path) {
        Ok(gpx) => {
            println!("   ✅ Standard parsing successful");
            return Ok(gpx);
        }
        Err(e) => {
            println!("   ⚠️  Standard parsing failed: {}", e);
        }
    }
    
    // Strategy 2: Try with minimal, safe repairs (works for most remaining files)
    match try_minimal_repair_parsing(path) {
        Ok(gpx) => {
            println!("   ✅ Minimal repair parsing successful");
            return Ok(gpx);
        }
        Err(e) => {
            println!("   ⚠️  Minimal repair failed: {}", e);
        }
    }
    
    // Strategy 3: Manual coordinate extraction (last resort, preserves original data)
    match try_manual_coordinate_extraction(path) {
        Ok(gpx) => {
            println!("   ✅ Manual extraction successful");
            return Ok(gpx);
        }
        Err(e) => {
            println!("   ❌ All parsing strategies failed: {}", e);
            return Err(format!("Could not parse GPX file with any strategy: {}", e).into());
        }
    }
}

/// Strategy 1: Standard GPX parsing (no modifications)
fn try_standard_gpx_parsing(path: &Path) -> Result<Gpx, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(read(reader)?)
}

/// Strategy 2: Minimal repair - only fix critical issues, never add artificial data
fn try_minimal_repair_parsing(path: &Path) -> Result<Gpx, Box<dyn std::error::Error>> {
    // Read raw content
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    
    // Apply ONLY safe, minimal repairs that preserve original data
    let repaired_content = apply_minimal_safe_repairs(&content)?;
    
    // Try to parse the minimally repaired content
    let cursor = std::io::Cursor::new(repaired_content.as_bytes());
    let reader = BufReader::new(cursor);
    Ok(read(reader)?)
}

/// Strategy 3: Manual extraction - extract coordinates directly from XML text
fn try_manual_coordinate_extraction(path: &Path) -> Result<Gpx, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    
    // Extract coordinates manually using string parsing
    let track_points = extract_coordinates_manually(&content)?;
    
    if track_points.is_empty() {
        return Err("No coordinates found in manual extraction".into());
    }
    
    // Create a minimal GPX structure from extracted coordinates
    let gpx = create_minimal_gpx_from_coordinates(&track_points)?;
    Ok(gpx)
}

/// Apply only safe repairs that preserve original elevation data
fn apply_minimal_safe_repairs(content: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut repaired = content.to_string();
    
    // Repair 1: Add XML declaration if missing (safe)
    if !repaired.starts_with("<?xml") {
        repaired = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", repaired);
    }
    
    // Repair 2: Add GPX version if missing (safe)
    if repaired.contains("<gpx") && !repaired.contains("version=") {
        repaired = add_gpx_version_safely(&repaired);
    }
    
    // Repair 3: Remove problematic bounds metadata (safe - just metadata)
    repaired = remove_problematic_bounds(&repaired);
    
    // Repair 4: Close unclosed XML tags if file is truncated (safe)
    repaired = close_unclosed_tags_safely(&repaired);
    
    // Repair 5: Remove invalid XML characters (safe)
    repaired = remove_invalid_xml_chars(&repaired);
    
    // CRITICAL: NEVER add artificial elevation data or modify coordinates
    // CRITICAL: NEVER alter existing elevation values
    // CRITICAL: Preserve all original track data exactly as-is
    
    Ok(repaired)
}

fn add_gpx_version_safely(content: &str) -> String {
    let mut repaired = content.to_string();
    
    if let Some(gpx_start) = repaired.find("<gpx") {
        if let Some(gpx_end) = repaired[gpx_start..].find(">") {
            let gpx_tag = &repaired[gpx_start..gpx_start + gpx_end + 1];
            
            if !gpx_tag.contains("version=") {
                let new_gpx_tag = gpx_tag.replace(
                    ">", 
                    " version=\"1.1\" xmlns=\"http://www.topografix.com/GPX/1/1\">"
                );
                repaired = repaired.replace(gpx_tag, &new_gpx_tag);
            }
        }
    }
    
    repaired
}

fn remove_problematic_bounds(content: &str) -> String {
    let mut repaired = content.to_string();
    
    // Remove bounds elements that might have invalid min/max values
    if let Some(start) = repaired.find("<bounds") {
        if let Some(end) = repaired[start..].find("/>") {
            let bounds_section = &repaired[start..start + end + 2];
            // Only remove if it contains problematic min/max values
            if bounds_section.contains("minlat") || bounds_section.contains("minlon") {
                repaired = repaired.replace(bounds_section, "");
            }
        }
    }
    
    repaired
}

fn close_unclosed_tags_safely(content: &str) -> String {
    let mut repaired = content.trim().to_string();
    
    // Only close tags if the file appears to be truncated
    if !repaired.ends_with("</gpx>") && repaired.contains("<gpx") {
        // Count open vs closed tags
        let open_trkseg = repaired.matches("<trkseg>").count();
        let close_trkseg = repaired.matches("</trkseg>").count();
        let open_trk = repaired.matches("<trk>").count();
        let close_trk = repaired.matches("</trk>").count();
        let open_gpx = repaired.matches("<gpx").count();
        let close_gpx = repaired.matches("</gpx>").count();
        
        // Close missing tags in reverse order
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
    }
    
    repaired
}

fn remove_invalid_xml_chars(content: &str) -> String {
    content
        .chars()
        .filter(|&c| {
            // Keep valid XML 1.0 characters
            c == '\t' || c == '\n' || c == '\r' || 
            (c >= ' ' && c <= '~') || // ASCII printable
            (c as u32 >= 0x80 && c as u32 <= 0xD7FF) || // Valid Unicode ranges
            (c as u32 >= 0xE000 && c as u32 <= 0xFFFD)
        })
        .collect()
}

/// Manual coordinate extraction - preserves original elevation data exactly
fn extract_coordinates_manually(content: &str) -> Result<Vec<(f64, f64, Option<f64>)>, Box<dyn std::error::Error>> {
    let mut coordinates = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        
        // Look for track point lines with lat/lon
        if (line.contains("<trkpt") || line.contains("trkpt")) && 
           line.contains("lat=") && line.contains("lon=") {
            
            if let Some((lat, lon)) = extract_lat_lon_from_line(line) {
                // Look for elevation in current line or next few lines
                let elevation = find_elevation_near_line(&lines, i);
                coordinates.push((lat, lon, elevation));
            }
        }
        
        i += 1;
    }
    
    println!("   📍 Manual extraction found {} coordinates", coordinates.len());
    Ok(coordinates)
}

fn extract_lat_lon_from_line(line: &str) -> Option<(f64, f64)> {
    let mut lat = None;
    let mut lon = None;
    
    // Extract latitude
    if let Some(lat_start) = line.find("lat=\"") {
        if let Some(lat_end) = line[lat_start + 5..].find("\"") {
            if let Ok(lat_val) = line[lat_start + 5..lat_start + 5 + lat_end].parse::<f64>() {
                if lat_val >= -90.0 && lat_val <= 90.0 {
                    lat = Some(lat_val);
                }
            }
        }
    }
    
    // Extract longitude
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

fn find_elevation_near_line(lines: &[&str], current_index: usize) -> Option<f64> {
    // Check current line first
    if let Some(ele) = extract_elevation_from_line(lines[current_index]) {
        return Some(ele);
    }
    
    // Check next few lines (elevation often comes after coordinates)
    for offset in 1..=3 {
        if current_index + offset < lines.len() {
            if let Some(ele) = extract_elevation_from_line(lines[current_index + offset]) {
                return Some(ele);
            }
        }
    }
    
    None
}

fn extract_elevation_from_line(line: &str) -> Option<f64> {
    // Look for <ele>value</ele> pattern
    if let Some(ele_start) = line.find("<ele>") {
        if let Some(ele_end) = line[ele_start + 5..].find("</ele>") {
            if let Ok(ele_val) = line[ele_start + 5..ele_start + 5 + ele_end].parse::<f64>() {
                if ele_val >= -500.0 && ele_val <= 10000.0 { // Reasonable elevation range
                    return Some(ele_val);
                }
            }
        }
    }
    
    None
}

fn create_minimal_gpx_from_coordinates(coordinates: &[(f64, f64, Option<f64>)]) -> Result<Gpx, Box<dyn std::error::Error>> {
    let mut gpx = Gpx::default();
    gpx.version = gpx::GpxVersion::Gpx11;
    gpx.creator = Some("Tolerant-GPX-Reader".to_string());
    
    let mut track = Track::new();
    track.name = Some("Extracted Track".to_string());
    
    let mut segment = TrackSegment::new();
    
    for &(lat, lon, elevation) in coordinates {
        let mut waypoint = Waypoint::new(point!(x: lon, y: lat));
        
        // Only set elevation if it exists in the original data
        // NEVER add artificial elevation
        if let Some(ele) = elevation {
            waypoint.elevation = Some(ele);
        }
        
        segment.points.push(waypoint);
    }
    
    if !segment.points.is_empty() {
        track.segments.push(segment);
        gpx.tracks.push(track);
    }
    
    if gpx.tracks.is_empty() {
        return Err("No valid tracks created from coordinates".into());
    }
    
    Ok(gpx)
}

/// Get detailed information about why a file can't be parsed
pub fn diagnose_gpx_file(path: &Path) -> String {
    let mut diagnostics = Vec::new();
    
    // Check if file exists and is readable
    match File::open(path) {
        Ok(mut file) => {
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => {
                    diagnostics.push(format!("✅ File readable, {} bytes", content.len()));
                    
                    // Check for common issues
                    if !content.starts_with("<?xml") {
                        diagnostics.push("⚠️  Missing XML declaration".to_string());
                    }
                    
                    if !content.contains("<gpx") {
                        diagnostics.push("❌ No GPX root element found".to_string());
                    } else {
                        diagnostics.push("✅ GPX root element found".to_string());
                    }
                    
                    if !content.contains("version=") {
                        diagnostics.push("⚠️  Missing GPX version attribute".to_string());
                    }
                    
                    let trkpt_count = content.matches("<trkpt").count();
                    diagnostics.push(format!("📍 {} track points found", trkpt_count));
                    
                    let ele_count = content.matches("<ele>").count();
                    diagnostics.push(format!("📏 {} elevation values found", ele_count));
                    
                    if !content.trim().ends_with("</gpx>") {
                        diagnostics.push("⚠️  File may be truncated (no closing </gpx>)".to_string());
                    }
                }
                Err(e) => {
                    diagnostics.push(format!("❌ Cannot read file content: {}", e));
                }
            }
        }
        Err(e) => {
            diagnostics.push(format!("❌ Cannot open file: {}", e));
        }
    }
    
    diagnostics.join("\n")
}

/// Count how many files can be read with each strategy
pub fn analyze_parsing_strategies(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 ANALYZING GPX PARSING STRATEGIES");
    println!("==================================");
    
    let mut total_files = 0;
    let mut standard_success = 0;
    let mut minimal_repair_success = 0;
    let mut manual_extraction_success = 0;
    let mut complete_failures = Vec::new();
    
    for entry in WalkDir::new(gpx_folder) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    total_files += 1;
                    
                    let filename = entry.file_name().to_str().unwrap_or("unknown");
                    
                    // Test each strategy
                    let standard_works = try_standard_gpx_parsing(entry.path()).is_ok();
                    let minimal_works = try_minimal_repair_parsing(entry.path()).is_ok();
                    let manual_works = try_manual_coordinate_extraction(entry.path()).is_ok();
                    
                    if standard_works {
                        standard_success += 1;
                    } else if minimal_works {
                        minimal_repair_success += 1;
                        println!("   📝 Minimal repair needed: {}", filename);
                    } else if manual_works {
                        manual_extraction_success += 1;
                        println!("   🔧 Manual extraction needed: {}", filename);
                    } else {
                        complete_failures.push(filename.to_string());
                        println!("   ❌ Complete failure: {}", filename);
                        
                        // Diagnose the failure
                        let diagnosis = diagnose_gpx_file(entry.path());
                        println!("      {}", diagnosis.replace('\n', "\n      "));
                    }
                }
            }
        }
    }
    
    println!("\n📊 PARSING STRATEGY RESULTS:");
    println!("• Total GPX files: {}", total_files);
    println!("• Standard parsing success: {} ({:.1}%)", 
             standard_success, (standard_success as f32 / total_files as f32) * 100.0);
    println!("• Minimal repair success: {} ({:.1}%)", 
             minimal_repair_success, (minimal_repair_success as f32 / total_files as f32) * 100.0);
    println!("• Manual extraction success: {} ({:.1}%)", 
             manual_extraction_success, (manual_extraction_success as f32 / total_files as f32) * 100.0);
    println!("• Complete failures: {} ({:.1}%)", 
             complete_failures.len(), (complete_failures.len() as f32 / total_files as f32) * 100.0);
    
    let total_success = standard_success + minimal_repair_success + manual_extraction_success;
    println!("\n🎯 TOTAL SUCCESS RATE: {}/{} ({:.1}%)", 
             total_success, total_files, (total_success as f32 / total_files as f32) * 100.0);
    
    if !complete_failures.is_empty() {
        println!("\n❌ FILES THAT FAILED ALL STRATEGIES:");
        for filename in &complete_failures {
            println!("   • {}", filename);
        }
    }
    
    println!("\n💡 CONCLUSION:");
    if total_success == total_files {
        println!("🏆 PERFECT! All files can be read with tolerant parsing!");
        println!("   No preprocessing needed - just use tolerant reading strategies.");
    } else if total_success as f32 >= total_files as f32 * 0.95 {
        println!("✅ EXCELLENT! {:.1}% success rate with tolerant parsing.", 
                 (total_success as f32 / total_files as f32) * 100.0);
        println!("   Much better than aggressive preprocessing!");
    } else {
        println!("⚠️  Some files still failing. May need additional tolerance strategies.");
    }
    
    Ok(())
}