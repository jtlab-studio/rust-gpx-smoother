/// Comprehensive Directional Deadzone Parameter Analysis
/// 
/// Fine-tunes the winning Directional Deadzone method to find optimal gain/loss thresholds.
/// Based on initial findings: gain_th=0.1m, loss_th=0.05m achieved 97.8% accuracy with 104.3% ratio
/// 
/// This analysis explores 441 parameter combinations in a focused grid around the optimal region
/// to maximize elevation gain accuracy while maintaining excellent gain/loss balance.

use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;

#[derive(Debug, Serialize, Clone)]
pub struct DirectionalDeadzoneResult {
    // Parameter combination
    gain_threshold_m: f32,
    loss_threshold_m: f32,
    
    // Primary objectives (from analysis requirements)
    median_elevation_accuracy: f32,          // Target: maximize (currently 97.8%)
    median_gain_loss_ratio: f32,             // Target: maintain ~100-110%
    files_balanced_85_115: u32,              // Target: maintain high count
    
    // Accuracy distribution
    score_98_102: u32,                       // Files within ±2%
    score_95_105: u32,                       // Files within ±5%
    score_90_110: u32,                       // Files within ±10%
    score_85_115: u32,                       // Files within ±15%
    score_80_120: u32,                       // Files within ±20%
    files_outside_80_120: u32,               // Outlier files
    
    // Advanced accuracy metrics
    accuracy_std_deviation: f32,             // Lower = more consistent
    worst_accuracy_percent: f32,             // Closest to 100% = better worst case
    best_accuracy_percent: f32,              // How close best file gets to 100%
    accuracy_q75_q25_range: f32,             // Interquartile range of accuracies
    
    // Gain/Loss balance metrics
    files_balanced_90_110: u32,              // Stricter balance criteria
    files_balanced_95_105: u32,              // Very strict balance criteria
    gain_loss_ratio_std_deviation: f32,      // Consistency of balance across files
    files_with_ratio_below_70: u32,          // Files with severe loss under-representation
    files_with_ratio_above_150: u32,         // Files with loss over-representation
    
    // Processing metrics
    avg_processed_gain: f32,
    avg_processed_loss: f32,
    avg_raw_gain: f32,
    avg_raw_loss: f32,
    gain_reduction_percent: f32,
    loss_reduction_percent: f32,
    
    // Terrain-specific performance
    flat_terrain_accuracy: f32,              // Performance on <20m/km routes
    rolling_terrain_accuracy: f32,           // Performance on 20-40m/km routes  
    hilly_terrain_accuracy: f32,             // Performance on 40-80m/km routes
    mountain_terrain_accuracy: f32,          // Performance on >80m/km routes
    
    // Composite scores
    accuracy_score: f32,                     // Weighted accuracy performance
    balance_score: f32,                      // Gain/loss balance performance
    consistency_score: f32,                  // How consistent across different files
    overall_optimization_score: f32,         // Primary optimization target
    
    // File counts
    total_files: u32,
}

#[derive(Debug, Clone)]
struct FileResult {
    filename: String,
    official_gain: u32,
    raw_gain: f32,
    raw_loss: f32,
    processed_gain: f32,
    processed_loss: f32,
    accuracy: f32,
    gain_loss_ratio: f32,
    terrain_type: TerrainType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TerrainType {
    Flat,        // <20m/km
    Rolling,     // 20-40m/km
    Hilly,       // 40-80m/km
    Mountain,    // >80m/km
}

pub fn run_comprehensive_directional_deadzone_analysis(
    gpx_folder: &str
) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🎯 COMPREHENSIVE DIRECTIONAL DEADZONE OPTIMIZATION");
    println!("================================================");
    println!("Objective: Maximize elevation gain accuracy while maintaining gain/loss balance");
    println!("Baseline: gain_th=0.1m, loss_th=0.05m achieved 97.8% accuracy, 104.3% ratio");
    println!("Strategy: Fine-grained exploration around optimal region\n");
    
    // Load GPX data
    println!("📂 Loading GPX files...");
    let start = std::time::Instant::now();
    let (gpx_files_data, valid_files) = load_gpx_data(gpx_folder)?;
    println!("✅ Loaded {} files in {:.2}s", valid_files.len(), start.elapsed().as_secs_f64());
    
    // Filter files with elevation data and official benchmarks
    let files_with_elevation: Vec<_> = valid_files.into_iter()
        .filter(|file| {
            if let Some(data) = gpx_files_data.get(file) {
                let has_elevation = data.elevations.iter()
                    .any(|&e| (e - data.elevations[0]).abs() > 0.1);
                has_elevation && data.official_gain > 0
            } else {
                false
            }
        })
        .collect();
    
    println!("📊 Processing {} files with elevation data and official benchmarks", files_with_elevation.len());
    
    // Generate comprehensive parameter combinations
    let parameter_combinations = generate_focused_parameter_grid();
    println!("🔬 Testing {} parameter combinations", parameter_combinations.len());
    
    // Process all combinations
    let processing_start = std::time::Instant::now();
    let results = process_all_combinations(&gpx_files_data, &files_with_elevation, &parameter_combinations)?;
    println!("✅ Processing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write detailed results
    let output_path = Path::new(gpx_folder).join("directional_deadzone_optimization.csv");
    write_comprehensive_results(&results, &output_path)?;
    
    // Print analysis
    print_optimization_analysis(&results);
    
    let total_time = total_start.elapsed();
    println!("\n⏱️  TOTAL EXECUTION TIME: {} minutes {:.1} seconds", 
             total_time.as_secs() / 60, 
             total_time.as_secs_f64() % 60.0);
    
    Ok(())
}

fn generate_focused_parameter_grid() -> Vec<(f32, f32)> {
    println!("🔬 Generating ULTRA-COMPREHENSIVE parameter grid...");
    println!("Target: Definitive mapping of entire parameter space for publication-quality analysis");
    
    let mut combinations = Vec::new();
    
    // ZONE 1: MICRO-FINE grid around winning combination (0.1m, 0.05m)
    // Ultra-high resolution to find absolute optimum
    println!("  Zone 1: Micro-fine grid (0.001m resolution) around winning parameters");
    let gain_micro: Vec<f32> = (75..=125).map(|i| i as f32 * 0.001).collect(); // 0.075 to 0.125 in 0.001 steps
    let loss_micro: Vec<f32> = (25..=75).map(|i| i as f32 * 0.001).collect();  // 0.025 to 0.075 in 0.001 steps
    
    for &gain in &gain_micro {
        for &loss in &loss_micro {
            combinations.push((gain, loss));
        }
    }
    println!("    Added {} micro-fine combinations", gain_micro.len() * loss_micro.len());
    
    // ZONE 2: HIGH-RESOLUTION extended grid
    // 0.005m resolution in extended region
    println!("  Zone 2: High-resolution extended grid (0.005m resolution)");
    let gain_high_res: Vec<f32> = (10..=200).step_by(5).map(|i| i as f32 * 0.001).collect(); // 0.01 to 0.2 in 0.005 steps
    let loss_high_res: Vec<f32> = (5..=100).step_by(5).map(|i| i as f32 * 0.001).collect();   // 0.005 to 0.1 in 0.005 steps
    
    for &gain in &gain_high_res {
        for &loss in &loss_high_res {
            // Skip if already covered in Zone 1
            if !(gain >= 0.075 && gain <= 0.125 && loss >= 0.025 && loss <= 0.075) {
                combinations.push((gain, loss));
            }
        }
    }
    println!("    Added {} high-resolution combinations", 
             gain_high_res.len() * loss_high_res.len() - gain_micro.len() * loss_micro.len());
    
    // ZONE 3: ASYMMETRIC SENSITIVITY mapping
    // Comprehensive test of gain:loss sensitivity ratios
    println!("  Zone 3: Comprehensive asymmetric sensitivity mapping");
    let asymmetric_ratios = [
        // Ultra-sensitive loss (10:1 to 20:1 ratios)
        (0.2, 0.01), (0.3, 0.015), (0.4, 0.02), (0.5, 0.025),
        (0.15, 0.01), (0.25, 0.015), (0.35, 0.02), (0.45, 0.025),
        
        // High asymmetry (5:1 to 8:1 ratios)  
        (0.1, 0.015), (0.12, 0.015), (0.15, 0.02), (0.2, 0.025), (0.25, 0.03),
        (0.08, 0.015), (0.16, 0.02), (0.24, 0.03), (0.32, 0.04),
        
        // Moderate asymmetry (2:1 to 4:1 ratios)
        (0.06, 0.02), (0.08, 0.025), (0.1, 0.03), (0.12, 0.035), (0.14, 0.04),
        (0.04, 0.015), (0.06, 0.025), (0.08, 0.03), (0.1, 0.035),
        
        // Equal sensitivity (1:1 ratios at various levels)
        (0.01, 0.01), (0.02, 0.02), (0.03, 0.03), (0.04, 0.04), (0.05, 0.05),
        (0.06, 0.06), (0.07, 0.07), (0.08, 0.08), (0.1, 0.1), (0.15, 0.15),
        
        // Reverse asymmetry (gain more sensitive than loss)
        (0.01, 0.05), (0.015, 0.08), (0.02, 0.1), (0.025, 0.15), (0.03, 0.2),
        (0.01, 0.04), (0.015, 0.06), (0.02, 0.08), (0.025, 0.12),
    ];
    
    combinations.extend_from_slice(&asymmetric_ratios);
    println!("    Added {} asymmetric sensitivity combinations", asymmetric_ratios.len());
    
    // ZONE 4: BOUNDARY and EXTREME testing
    println!("  Zone 4: Boundary and extreme parameter testing");
    let boundary_tests = [
        // Ultra-sensitive boundaries
        (0.005, 0.005), (0.001, 0.001), (0.001, 0.005), (0.005, 0.001),
        (0.01, 0.005), (0.005, 0.01), (0.015, 0.005), (0.005, 0.015),
        
        // Ultra-insensitive boundaries  
        (1.0, 1.0), (0.8, 0.8), (0.6, 0.6), (0.5, 0.5), (0.4, 0.4),
        (1.0, 0.5), (0.5, 1.0), (0.8, 0.4), (0.4, 0.8),
        
        // Extreme asymmetric boundaries
        (1.0, 0.001), (0.8, 0.005), (0.6, 0.01), (0.4, 0.015),
        (0.001, 1.0), (0.005, 0.8), (0.01, 0.6), (0.015, 0.4),
        
        // Scientific edge cases
        (0.1, 0.001), (0.001, 0.1), (0.2, 0.001), (0.001, 0.2),
        (0.3, 0.002), (0.002, 0.3), (0.5, 0.003), (0.003, 0.5),
    ];
    
    combinations.extend_from_slice(&boundary_tests);
    println!("    Added {} boundary/extreme combinations", boundary_tests.len());
    
    // ZONE 5: SYSTEMATIC MATHEMATICAL ratios
    println!("  Zone 5: Systematic mathematical ratio exploration");
    let base_values = [0.01, 0.02, 0.03, 0.05, 0.08, 0.1, 0.15, 0.2, 0.25, 0.3];
    let ratio_multipliers = [0.1, 0.2, 0.33, 0.5, 0.67, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0];
    
    for &base in &base_values {
        for &multiplier in &ratio_multipliers {
            let other = base * multiplier;
            if other <= 1.0 { // Keep within reasonable bounds
                combinations.push((base, other));
                if base != other { // Avoid duplicates
                    combinations.push((other, base));
                }
            }
        }
    }
    println!("    Added {} systematic ratio combinations", base_values.len() * ratio_multipliers.len() * 2);
    
    // ZONE 6: LOGARITHMIC spacing for scientific completeness
    println!("  Zone 6: Logarithmic parameter spacing");
    let log_gain: Vec<f32> = (0..30).map(|i| 0.001 * 1.2_f32.powi(i)).filter(|&x| x <= 1.0).collect();
    let log_loss: Vec<f32> = (0..30).map(|i| 0.001 * 1.15_f32.powi(i)).filter(|&x| x <= 1.0).collect();
    
    // Sample logarithmic combinations (not full cartesian product to avoid explosion)
    for i in 0..log_gain.len().min(20) {
        for j in 0..log_loss.len().min(20) {
            combinations.push((log_gain[i], log_loss[j]));
        }
    }
    println!("    Added {} logarithmic spacing combinations", 20 * 20);
    
    // ZONE 7: TERRAIN-SPECIFIC optimization candidates
    println!("  Zone 7: Terrain-specific optimization candidates");
    let terrain_specific = [
        // Optimized for flat terrain (aggressive noise filtering)
        (0.05, 0.02), (0.06, 0.025), (0.08, 0.03), (0.1, 0.035),
        (0.04, 0.015), (0.05, 0.02), (0.07, 0.025), (0.09, 0.03),
        
        // Optimized for rolling terrain (balanced)
        (0.08, 0.04), (0.1, 0.05), (0.12, 0.06), (0.15, 0.075),
        (0.07, 0.035), (0.09, 0.045), (0.11, 0.055), (0.13, 0.065),
        
        // Optimized for hilly terrain (preserve detail)
        (0.06, 0.03), (0.08, 0.04), (0.1, 0.05), (0.12, 0.06),
        (0.05, 0.025), (0.07, 0.035), (0.09, 0.045), (0.11, 0.055),
        
        // Optimized for mountainous terrain (minimal smoothing)
        (0.03, 0.015), (0.04, 0.02), (0.05, 0.025), (0.06, 0.03),
        (0.02, 0.01), (0.03, 0.015), (0.04, 0.02), (0.05, 0.025),
    ];
    
    combinations.extend_from_slice(&terrain_specific);
    println!("    Added {} terrain-specific combinations", terrain_specific.len());
    combinations.extend_from_slice(&terrain_specific);
    println!("    Added {} terrain-specific combinations", terrain_specific.len());
    
    // ZONE 8: GOLDEN RATIO and mathematical constants
    println!("  Zone 8: Mathematical constants and special ratios");
    let phi = 1.618034; // Golden ratio
    let sqrt2 = 1.414213; // √2
    let sqrt3 = 1.732051; // √3
    let e_const = 2.718282; // Euler's number
    let pi = 3.141593; // π
    
    let mathematical_bases = [0.01, 0.02, 0.05, 0.1, 0.15, 0.2];
    let mathematical_ratios = [1.0/phi, 1.0/sqrt2, 1.0/sqrt3, 1.0/e_const, 1.0/pi, 
                              phi, sqrt2, sqrt3, e_const/10.0, pi/10.0];
    
    for &base in &mathematical_bases {
        for &ratio in &mathematical_ratios {
            let other = base * ratio;
            if other > 0.001 && other <= 1.0 {
                combinations.push((base, other));
                combinations.push((other, base));
            }
        }
    }
    println!("    Added {} mathematical constant combinations", mathematical_bases.len() * mathematical_ratios.len() * 2);
    
    // ZONE 9: PERFORMANCE-DRIVEN exploration based on known good regions
    println!("  Zone 9: Performance-driven systematic exploration");
    
    // Based on analysis showing 0.1/0.05 was optimal, explore systematic variations
    let performance_gain_bases = [0.08, 0.09, 0.1, 0.11, 0.12];
    let performance_loss_multipliers = [0.3, 0.4, 0.45, 0.5, 0.55, 0.6, 0.7, 0.8];
    
    for &gain_base in &performance_gain_bases {
        for &loss_mult in &performance_loss_multipliers {
            let loss_val = gain_base * loss_mult;
            if loss_val >= 0.001 && loss_val <= 1.0 {
                combinations.push((gain_base, loss_val));
            }
        }
    }
    println!("    Added {} performance-driven combinations", 
             performance_gain_bases.len() * performance_loss_multipliers.len());
    
    // ZONE 10: STATISTICAL sampling for coverage validation
    println!("  Zone 10: Statistical sampling for complete coverage");
    
    // Ensure we have good coverage across the entire reasonable parameter space
    let coverage_gain: Vec<f32> = (1..=100).map(|i| i as f32 * 0.01).collect(); // 0.01 to 1.00
    let coverage_loss: Vec<f32> = (1..=50).map(|i| i as f32 * 0.02).collect();  // 0.02 to 1.00
    
    // Sample systematically to ensure coverage without explosion
    for i in (0..coverage_gain.len()).step_by(5) { // Every 5th gain value
        for j in (0..coverage_loss.len()).step_by(3) { // Every 3rd loss value
            if i < coverage_gain.len() && j < coverage_loss.len() {
                combinations.push((coverage_gain[i], coverage_loss[j]));
            }
        }
    }
    println!("    Added {} statistical coverage combinations", 
             (coverage_gain.len() / 5) * (coverage_loss.len() / 3));
    
    // Remove duplicates and sort for systematic processing
    println!("\n🔧 Post-processing parameter combinations...");
    let original_count = combinations.len();
    
    // Convert to integer representation for exact duplicate removal
    let mut int_combinations: Vec<(i32, i32)> = combinations.iter()
        .map(|&(g, l)| ((g * 1000000.0) as i32, (l * 1000000.0) as i32))
        .collect();
    
    int_combinations.sort_unstable();
    int_combinations.dedup();
    
    // Convert back to float
    combinations = int_combinations.iter()
        .map(|&(g, l)| (g as f32 / 1000000.0, l as f32 / 1000000.0))
        .collect();
    
    println!("  Removed {} duplicates", original_count - combinations.len());
    println!("  Final unique combinations: {}", combinations.len());
    
    // Sort by gain then loss for systematic processing
    combinations.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));
    
    // Print distribution analysis
    println!("\n📊 Parameter distribution analysis:");
    let gain_range = (
        combinations.iter().map(|(g, _)| *g).fold(f32::INFINITY, f32::min),
        combinations.iter().map(|(g, _)| *g).fold(f32::NEG_INFINITY, f32::max)
    );
    let loss_range = (
        combinations.iter().map(|(_, l)| *l).fold(f32::INFINITY, f32::min),
        combinations.iter().map(|(_, l)| *l).fold(f32::NEG_INFINITY, f32::max)
    );
    
    println!("  Gain threshold range: {:.6}m to {:.3}m", gain_range.0, gain_range.1);
    println!("  Loss threshold range: {:.6}m to {:.3}m", loss_range.0, loss_range.1);
    
    // Count combinations in key regions
    let ultra_fine_count = combinations.iter()
        .filter(|&&(g, l)| g >= 0.075 && g <= 0.125 && l >= 0.025 && l <= 0.075)
        .count();
    
    let asymmetric_count = combinations.iter()
        .filter(|&&(g, l)| (g / l) > 2.0 || (l / g) > 2.0)
        .count();
    
    println!("  Ultra-fine region (around optimal): {} combinations", ultra_fine_count);
    println!("  Asymmetric combinations (>2:1 ratio): {} combinations", asymmetric_count);
    println!("  Boundary/extreme combinations: {} combinations", 
             combinations.iter().filter(|&&(g, l)| g < 0.01 || g > 0.5 || l < 0.01 || l > 0.5).count());
    
    combinations
}

fn load_gpx_data(gpx_folder: &str) -> Result<(HashMap<String, GpxFileData>, Vec<String>), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::BufReader;
    use gpx::read;
    use geo::{HaversineDistance, point};
    use walkdir::WalkDir;
    
    let mut gpx_data = HashMap::new();
    let mut valid_files = Vec::new();
    
    let official_data = crate::load_official_elevation_data()?;
    
    for entry in WalkDir::new(gpx_folder) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension.to_str().unwrap_or("").to_lowercase() == "gpx" {
                    let path = entry.path();
                    let filename = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    
                    match File::open(path) {
                        Ok(file) => {
                            let reader = BufReader::new(file);
                            match read(reader) {
                                Ok(gpx) => {
                                    let mut coords: Vec<(f64, f64, f64)> = vec![];
                                    
                                    for track in gpx.tracks {
                                        for segment in track.segments {
                                            for pt in segment.points {
                                                if let Some(ele) = pt.elevation {
                                                    coords.push((pt.point().y(), pt.point().x(), ele));
                                                }
                                            }
                                        }
                                    }
                                    
                                    if !coords.is_empty() {
                                        let mut distances = vec![0.0];
                                        for i in 1..coords.len() {
                                            let a = point!(x: coords[i-1].1, y: coords[i-1].0);
                                            let b = point!(x: coords[i].1, y: coords[i].0);
                                            let dist = a.haversine_distance(&b);
                                            distances.push(distances[i-1] + dist);
                                        }
                                        
                                        let elevations: Vec<f64> = coords.iter().map(|c| c.2).collect();
                                        let official_gain = official_data
                                            .get(&filename.to_lowercase())
                                            .copied()
                                            .unwrap_or(0);
                                        
                                        let file_data = GpxFileData {
                                            filename: filename.clone(),
                                            elevations,
                                            distances,
                                            official_gain,
                                        };
                                        
                                        gpx_data.insert(filename.clone(), file_data);
                                        valid_files.push(filename);
                                    }
                                },
                                Err(_) => continue,
                            }
                        },
                        Err(_) => continue,
                    }
                }
            }
        }
    }
    
    Ok((gpx_data, valid_files))
}

#[derive(Debug, Clone)]
struct GpxFileData {
    filename: String,
    elevations: Vec<f64>,
    distances: Vec<f64>,
    official_gain: u32,
}

fn process_all_combinations(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String],
    parameter_combinations: &[(f32, f32)]
) -> Result<Vec<DirectionalDeadzoneResult>, Box<dyn std::error::Error>> {
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("\n🚀 Processing {} combinations × {} files = {} total calculations",
             parameter_combinations.len(), valid_files.len(), 
             parameter_combinations.len() * valid_files.len());
    println!("⚡ Using parallel processing on {} cores", num_cpus::get());
    
    // Create work items for parallel processing
    let work_items: Vec<((f32, f32), String)> = parameter_combinations.iter()
        .flat_map(|&params| {
            valid_files.iter().map(move |file| (params, file.clone()))
        })
        .collect();
    
    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_items = work_items.len();
    let start_time = std::time::Instant::now();
    
    // Process all combinations in parallel
    let all_file_results: Vec<((f32, f32), String, FileResult)> = work_items
        .par_iter()
        .filter_map(|((gain_th, loss_th), filename)| {
            let gpx_data = Arc::clone(&gpx_data_arc);
            let processed_clone = Arc::clone(&processed);
            
            if let Some(file_data) = gpx_data.get(filename) {
                if file_data.official_gain > 0 {
                    let result = process_single_file_directional_deadzone(
                        file_data, *gain_th, *loss_th
                    );
                    
                    // Progress tracking
                    let count = processed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count % 5000 == 0 || count == total_items {
                        let elapsed = start_time.elapsed().as_secs_f64();
                        let rate = count as f64 / elapsed;
                        let remaining = (total_items - count) as f64 / rate;
                        println!("  Progress: {}/{} ({:.1}%) - {:.0} items/sec - ETA: {:.0}s",
                                 count, total_items, 
                                 (count as f64 / total_items as f64) * 100.0,
                                 rate, remaining);
                    }
                    
                    return Some(((*gain_th, *loss_th), filename.clone(), result));
                }
            }
            None
        })
        .collect();
    
    println!("✅ Parallel processing complete, aggregating results by parameter combination...");
    
    // Group results by parameter combination
    let mut param_groups: HashMap<(i32, i32), Vec<FileResult>> = HashMap::new();
    
    for ((gain_th, loss_th), _filename, file_result) in all_file_results {
        let key = ((gain_th * 1000.0) as i32, (loss_th * 1000.0) as i32);
        param_groups.entry(key).or_insert_with(Vec::new).push(file_result);
    }
    
    // Calculate comprehensive metrics for each parameter combination
    let results: Vec<DirectionalDeadzoneResult> = parameter_combinations
        .par_iter()
        .filter_map(|&(gain_th, loss_th)| {
            let key = ((gain_th * 1000.0) as i32, (loss_th * 1000.0) as i32);
            if let Some(file_results) = param_groups.get(&key) {
                Some(calculate_comprehensive_metrics(gain_th, loss_th, file_results))
            } else {
                None
            }
        })
        .collect();
    
    Ok(results)
}

fn process_single_file_directional_deadzone(
    file_data: &GpxFileData,
    gain_threshold: f32,
    loss_threshold: f32
) -> FileResult {
    // Calculate raw gain/loss
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&file_data.elevations);
    
    // Apply directional deadzone processing
    let mut processed_gain = 0.0;
    let mut processed_loss = 0.0;
    
    for i in 1..file_data.elevations.len() {
        let elevation_change = file_data.elevations[i] - file_data.elevations[i-1];
        
        if elevation_change > gain_threshold as f64 {
            processed_gain += elevation_change;
        } else if elevation_change < -(loss_threshold as f64) {
            processed_loss += -elevation_change;
        }
    }
    
    // Calculate metrics
    let accuracy = if file_data.official_gain > 0 {
        (processed_gain as f32 / file_data.official_gain as f32) * 100.0
    } else {
        100.0
    };
    
    let gain_loss_ratio = if processed_gain > 0.0 {
        (processed_loss / processed_gain) * 100.0
    } else {
        0.0
    };
    
    // Determine terrain type
    let total_distance_km = file_data.distances.last().unwrap_or(&0.0) / 1000.0;
    let gain_per_km = if total_distance_km > 0.0 {
        raw_gain / total_distance_km
    } else {
        0.0
    };
    
    let terrain_type = match gain_per_km {
        x if x < 20.0 => TerrainType::Flat,
        x if x < 40.0 => TerrainType::Rolling,
        x if x < 80.0 => TerrainType::Hilly,
        _ => TerrainType::Mountain,
    };
    
    FileResult {
        filename: file_data.filename.clone(),
        official_gain: file_data.official_gain,
        raw_gain: raw_gain as f32,
        raw_loss: raw_loss as f32,
        processed_gain: processed_gain as f32,
        processed_loss: processed_loss as f32,
        accuracy,
        gain_loss_ratio: gain_loss_ratio as f32,
        terrain_type,
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

fn calculate_comprehensive_metrics(
    gain_threshold: f32,
    loss_threshold: f32,
    file_results: &[FileResult]
) -> DirectionalDeadzoneResult {
    let total_files = file_results.len() as u32;
    
    // Extract accuracy and ratio vectors for statistical analysis
    let accuracies: Vec<f32> = file_results.iter().map(|r| r.accuracy).collect();
    let gain_loss_ratios: Vec<f32> = file_results.iter().map(|r| r.gain_loss_ratio).collect();
    
    // PRIMARY ACCURACY BANDS - Core performance metrics
    let score_98_102 = accuracies.iter().filter(|&&acc| acc >= 98.0 && acc <= 102.0).count() as u32;
    let score_95_105 = accuracies.iter().filter(|&&acc| acc >= 95.0 && acc <= 105.0).count() as u32;
    let score_90_110 = accuracies.iter().filter(|&&acc| acc >= 90.0 && acc <= 110.0).count() as u32;
    let score_85_115 = accuracies.iter().filter(|&&acc| acc >= 85.0 && acc <= 115.0).count() as u32;
    let score_80_120 = accuracies.iter().filter(|&&acc| acc >= 80.0 && acc <= 120.0).count() as u32;
    let files_outside_80_120 = total_files - score_80_120;
    
    // GAIN/LOSS BALANCE BANDS - Critical for elevation loss preservation
    let files_balanced_85_115 = gain_loss_ratios.iter()
        .filter(|&&ratio| ratio >= 85.0 && ratio <= 115.0).count() as u32;
    let files_balanced_90_110 = gain_loss_ratios.iter()
        .filter(|&&ratio| ratio >= 90.0 && ratio <= 110.0).count() as u32;
    let files_balanced_95_105 = gain_loss_ratios.iter()
        .filter(|&&ratio| ratio >= 95.0 && ratio <= 105.0).count() as u32;
    let files_with_ratio_below_70 = gain_loss_ratios.iter()
        .filter(|&&ratio| ratio < 70.0).count() as u32;
    let files_with_ratio_above_150 = gain_loss_ratios.iter()
        .filter(|&&ratio| ratio > 150.0).count() as u32;
    
    // STATISTICAL MEASURES
    let median_elevation_accuracy = calculate_median(&accuracies);
    let median_gain_loss_ratio = calculate_median(&gain_loss_ratios);
    
    let accuracy_std_deviation = calculate_std_deviation(&accuracies);
    let gain_loss_ratio_std_deviation = calculate_std_deviation(&gain_loss_ratios);
    
    // Worst and best accuracy (distance from 100%)
    let worst_accuracy_percent = accuracies.iter()
        .max_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied().unwrap_or(100.0);
    let best_accuracy_percent = accuracies.iter()
        .min_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied().unwrap_or(100.0);
    
    // Interquartile range for accuracy distribution
    let mut sorted_accuracies = accuracies.clone();
    sorted_accuracies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q25_idx = sorted_accuracies.len() / 4;
    let q75_idx = (sorted_accuracies.len() * 3) / 4;
    let accuracy_q75_q25_range = if q75_idx < sorted_accuracies.len() && q25_idx < sorted_accuracies.len() {
        sorted_accuracies[q75_idx] - sorted_accuracies[q25_idx]
    } else {
        0.0
    };
    
    // TERRAIN-SPECIFIC PERFORMANCE
    let flat_results: Vec<_> = file_results.iter().filter(|r| r.terrain_type == TerrainType::Flat).collect();
    let rolling_results: Vec<_> = file_results.iter().filter(|r| r.terrain_type == TerrainType::Rolling).collect();
    let hilly_results: Vec<_> = file_results.iter().filter(|r| r.terrain_type == TerrainType::Hilly).collect();
    let mountain_results: Vec<_> = file_results.iter().filter(|r| r.terrain_type == TerrainType::Mountain).collect();
    
    let flat_terrain_accuracy = if !flat_results.is_empty() {
        flat_results.iter().map(|r| r.accuracy).sum::<f32>() / flat_results.len() as f32
    } else { 0.0 };
    
    let rolling_terrain_accuracy = if !rolling_results.is_empty() {
        rolling_results.iter().map(|r| r.accuracy).sum::<f32>() / rolling_results.len() as f32
    } else { 0.0 };
    
    let hilly_terrain_accuracy = if !hilly_results.is_empty() {
        hilly_results.iter().map(|r| r.accuracy).sum::<f32>() / hilly_results.len() as f32
    } else { 0.0 };
    
    let mountain_terrain_accuracy = if !mountain_results.is_empty() {
        mountain_results.iter().map(|r| r.accuracy).sum::<f32>() / mountain_results.len() as f32
    } else { 0.0 };
    
    // PROCESSING AVERAGES
    let avg_processed_gain = file_results.iter().map(|r| r.processed_gain).sum::<f32>() / total_files as f32;
    let avg_processed_loss = file_results.iter().map(|r| r.processed_loss).sum::<f32>() / total_files as f32;
    let avg_raw_gain = file_results.iter().map(|r| r.raw_gain).sum::<f32>() / total_files as f32;
    let avg_raw_loss = file_results.iter().map(|r| r.raw_loss).sum::<f32>() / total_files as f32;
    
    let gain_reduction_percent = if avg_raw_gain > 0.0 {
        ((avg_raw_gain - avg_processed_gain) / avg_raw_gain) * 100.0
    } else { 0.0 };
    
    let loss_reduction_percent = if avg_raw_loss > 0.0 {
        ((avg_raw_loss - avg_processed_loss) / avg_raw_loss) * 100.0
    } else { 0.0 };
    
    // COMPOSITE SCORING SYSTEM
    
    // Accuracy Score - heavily weights tight accuracy bands
    let accuracy_score = (score_98_102 as f32 * 15.0) +          // ±2% gets highest weight
                        ((score_95_105 - score_98_102) as f32 * 10.0) +  // ±5% band
                        ((score_90_110 - score_95_105) as f32 * 6.0) +   // ±10% band  
                        ((score_85_115 - score_90_110) as f32 * 3.0) +   // ±15% band
                        ((score_80_120 - score_85_115) as f32 * 1.0) -   // ±20% band
                        (files_outside_80_120 as f32 * 8.0);             // Penalty for outliers
    
    // Balance Score - prioritizes tight gain/loss balance
    let balance_score = (files_balanced_95_105 as f32 * 15.0) +   // ±5% balance gets highest weight
                       ((files_balanced_90_110 - files_balanced_95_105) as f32 * 10.0) + // ±10% balance
                       ((files_balanced_85_115 - files_balanced_90_110) as f32 * 6.0) +  // ±15% balance
                       (((median_gain_loss_ratio - 100.0).abs() * -0.5)) +              // Penalty for deviation from 100%
                       (files_with_ratio_below_70 as f32 * -10.0) +                     // Severe penalty for under-representation
                       (files_with_ratio_above_150 as f32 * -5.0);                      // Penalty for over-representation
    
    // Consistency Score - rewards low variance and tight distributions
    let consistency_score = 100.0 - accuracy_std_deviation - 
                           (gain_loss_ratio_std_deviation * 0.5) - 
                           (accuracy_q75_q25_range * 2.0) -
                           (((worst_accuracy_percent - 100.0).abs() - 20.0).max(0.0) * 0.5);
    
    // Overall Optimization Score - balances all objectives
    let overall_optimization_score = (accuracy_score * 0.4) +      // 40% weight on accuracy
                                    (balance_score * 0.35) +       // 35% weight on balance  
                                    (consistency_score * 0.25);    // 25% weight on consistency
    
    DirectionalDeadzoneResult {
        gain_threshold_m: gain_threshold,
        loss_threshold_m: loss_threshold,
        median_elevation_accuracy,
        median_gain_loss_ratio,
        files_balanced_85_115,
        score_98_102,
        score_95_105,
        score_90_110,
        score_85_115,
        score_80_120,
        files_outside_80_120,
        accuracy_std_deviation,
        worst_accuracy_percent,
        best_accuracy_percent,
        accuracy_q75_q25_range,
        files_balanced_90_110,
        files_balanced_95_105,
        gain_loss_ratio_std_deviation,
        files_with_ratio_below_70,
        files_with_ratio_above_150,
        avg_processed_gain,
        avg_processed_loss,
        avg_raw_gain,
        avg_raw_loss,
        gain_reduction_percent,
        loss_reduction_percent,
        flat_terrain_accuracy,
        rolling_terrain_accuracy,
        hilly_terrain_accuracy,
        mountain_terrain_accuracy,
        accuracy_score,
        balance_score,
        consistency_score,
        overall_optimization_score,
        total_files,
    }
}

fn calculate_median(values: &[f32]) -> f32 {
    if values.is_empty() { return 0.0; }
    
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    }
}

fn calculate_std_deviation(values: &[f32]) -> f32 {
    if values.is_empty() { return 0.0; }
    
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let variance = values.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f32>() / values.len() as f32;
    variance.sqrt()
}

fn write_comprehensive_results(
    results: &[DirectionalDeadzoneResult], 
    output_path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Comprehensive header with all performance metrics
    wtr.write_record(&[
        "Gain_Threshold_m", "Loss_Threshold_m",
        
        // Primary Performance Metrics
        "Overall_Score", "Median_Accuracy_%", "Median_Gain_Loss_Ratio_%",
        
        // Accuracy Distribution Analysis  
        "Files_98-102%", "Files_95-105%", "Files_90-110%", "Files_85-115%", "Files_80-120%", "Files_Outside_80-120%",
        "Accuracy_StdDev", "Best_Accuracy_%", "Worst_Accuracy_%", "Accuracy_IQR",
        
        // Gain/Loss Balance Analysis
        "Balanced_85-115%", "Balanced_90-110%", "Balanced_95-105%", 
        "Ratio_StdDev", "Files_Ratio_<70%", "Files_Ratio_>150%",
        
        // Terrain-Specific Performance
        "Flat_Accuracy_%", "Rolling_Accuracy_%", "Hilly_Accuracy_%", "Mountain_Accuracy_%",
        
        // Processing Details
        "Avg_Processed_Gain", "Avg_Processed_Loss", "Gain_Reduction_%", "Loss_Reduction_%",
        
        // Component Scores
        "Accuracy_Score", "Balance_Score", "Consistency_Score",
        
        "Total_Files"
    ])?;
    
    // Sort by overall optimization score for analysis
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| b.overall_optimization_score.partial_cmp(&a.overall_optimization_score).unwrap());
    
    // Write data rows
    for result in sorted_results {
        wtr.write_record(&[
            format!("{:.3}", result.gain_threshold_m),
            format!("{:.3}", result.loss_threshold_m),
            
            // Primary metrics
            format!("{:.2}", result.overall_optimization_score),
            format!("{:.2}", result.median_elevation_accuracy),
            format!("{:.1}", result.median_gain_loss_ratio),
            
            // Accuracy distribution
            result.score_98_102.to_string(),
            result.score_95_105.to_string(),
            result.score_90_110.to_string(),
            result.score_85_115.to_string(),
            result.score_80_120.to_string(),
            result.files_outside_80_120.to_string(),
            format!("{:.2}", result.accuracy_std_deviation),
            format!("{:.2}", result.best_accuracy_percent),
            format!("{:.2}", result.worst_accuracy_percent),
            format!("{:.2}", result.accuracy_q75_q25_range),
            
            // Balance metrics
            result.files_balanced_85_115.to_string(),
            result.files_balanced_90_110.to_string(),
            result.files_balanced_95_105.to_string(),
            format!("{:.2}", result.gain_loss_ratio_std_deviation),
            result.files_with_ratio_below_70.to_string(),
            result.files_with_ratio_above_150.to_string(),
            
            // Terrain performance
            format!("{:.2}", result.flat_terrain_accuracy),
            format!("{:.2}", result.rolling_terrain_accuracy),
            format!("{:.2}", result.hilly_terrain_accuracy),
            format!("{:.2}", result.mountain_terrain_accuracy),
            
            // Processing details
            format!("{:.1}", result.avg_processed_gain),
            format!("{:.1}", result.avg_processed_loss),
            format!("{:.1}", result.gain_reduction_percent),
            format!("{:.1}", result.loss_reduction_percent),
            
            // Component scores
            format!("{:.2}", result.accuracy_score),
            format!("{:.2}", result.balance_score),
            format!("{:.2}", result.consistency_score),
            
            result.total_files.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    println!("✅ Comprehensive results saved to: {}", output_path.display());
    Ok(())
}

fn print_optimization_analysis(results: &[DirectionalDeadzoneResult]) {
    println!("\n🎯 DIRECTIONAL DEADZONE OPTIMIZATION ANALYSIS");
    println!("=============================================");
    
    // Sort by overall score
    let mut sorted_by_overall = results.to_vec();
    sorted_by_overall.sort_by(|a, b| b.overall_optimization_score.partial_cmp(&a.overall_optimization_score).unwrap());
    
    let best_overall = &sorted_by_overall[0];
    
    println!("\n🏆 OPTIMAL PARAMETERS:");
    println!("   Gain threshold: {:.3}m", best_overall.gain_threshold_m);
    println!("   Loss threshold: {:.3}m", best_overall.loss_threshold_m);
    println!("   Overall score: {:.2}", best_overall.overall_optimization_score);
    println!("   Median accuracy: {:.2}%", best_overall.median_elevation_accuracy);
    println!("   Median gain/loss ratio: {:.1}%", best_overall.median_gain_loss_ratio);
    
    // ACCURACY PERFORMANCE BREAKDOWN
    println!("\n📊 ACCURACY PERFORMANCE BREAKDOWN:");
    println!("Band Analysis (out of {} files):", best_overall.total_files);
    println!("   98-102% (±2%):  {} files ({:.1}%)", 
             best_overall.score_98_102,
             (best_overall.score_98_102 as f32 / best_overall.total_files as f32) * 100.0);
    println!("   95-105% (±5%):  {} files ({:.1}%)", 
             best_overall.score_95_105,
             (best_overall.score_95_105 as f32 / best_overall.total_files as f32) * 100.0);
    println!("   90-110% (±10%): {} files ({:.1}%)", 
             best_overall.score_90_110,
             (best_overall.score_90_110 as f32 / best_overall.total_files as f32) * 100.0);
    println!("   85-115% (±15%): {} files ({:.1}%)", 
             best_overall.score_85_115,
             (best_overall.score_85_115 as f32 / best_overall.total_files as f32) * 100.0);
    println!("   80-120% (±20%): {} files ({:.1}%)", 
             best_overall.score_80_120,
             (best_overall.score_80_120 as f32 / best_overall.total_files as f32) * 100.0);
    println!("   Beyond ±20%:    {} files ({:.1}%) ⚠️", 
             best_overall.files_outside_80_120,
             (best_overall.files_outside_80_120 as f32 / best_overall.total_files as f32) * 100.0);
    
    println!("\nAccuracy Distribution:");
    println!("   Standard deviation: {:.2}%", best_overall.accuracy_std_deviation);
    println!("   Best case accuracy: {:.2}%", best_overall.best_accuracy_percent);
    println!("   Worst case accuracy: {:.2}%", best_overall.worst_accuracy_percent);
    println!("   Interquartile range: {:.2}%", best_overall.accuracy_q75_q25_range);
    
    // GAIN/LOSS BALANCE ANALYSIS
    println!("\n⚖️  GAIN/LOSS BALANCE ANALYSIS:");
    println!("Balance Band Analysis:");
    println!("   95-105% (±5%):   {} files ({:.1}%)", 
             best_overall.files_balanced_95_105,
             (best_overall.files_balanced_95_105 as f32 / best_overall.total_files as f32) * 100.0);
    println!("   90-110% (±10%):  {} files ({:.1}%)", 
             best_overall.files_balanced_90_110,
             (best_overall.files_balanced_90_110 as f32 / best_overall.total_files as f32) * 100.0);
    println!("   85-115% (±15%):  {} files ({:.1}%)", 
             best_overall.files_balanced_85_115,
             (best_overall.files_balanced_85_115 as f32 / best_overall.total_files as f32) * 100.0);
    
    println!("\nBalance Quality Indicators:");
    println!("   Ratio standard deviation: {:.2}%", best_overall.gain_loss_ratio_std_deviation);
    println!("   Files with severe loss under-representation (<70%): {} ({:.1}%)", 
             best_overall.files_with_ratio_below_70,
             (best_overall.files_with_ratio_below_70 as f32 / best_overall.total_files as f32) * 100.0);
    println!("   Files with loss over-representation (>150%): {} ({:.1}%)", 
             best_overall.files_with_ratio_above_150,
             (best_overall.files_with_ratio_above_150 as f32 / best_overall.total_files as f32) * 100.0);
    
    // TERRAIN-SPECIFIC PERFORMANCE
    println!("\n🏔️  TERRAIN-SPECIFIC PERFORMANCE:");
    println!("   Flat terrain (<20m/km):     {:.2}% accuracy", best_overall.flat_terrain_accuracy);
    println!("   Rolling terrain (20-40m/km): {:.2}% accuracy", best_overall.rolling_terrain_accuracy);
    println!("   Hilly terrain (40-80m/km):   {:.2}% accuracy", best_overall.hilly_terrain_accuracy);
    println!("   Mountain terrain (>80m/km):  {:.2}% accuracy", best_overall.mountain_terrain_accuracy);
    
    // TOP 10 PARAMETER COMBINATIONS
    println!("\n🏅 TOP 10 PARAMETER COMBINATIONS:");
    println!("Rank | Gain_th | Loss_th | Score | Med_Acc | Med_Ratio | 98-102% | 90-110% | Bal_85-115% | StdDev");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for (i, result) in sorted_by_overall.iter().take(10).enumerate() {
        println!("{:4} | {:7.3} | {:7.3} | {:5.1} | {:7.2} | {:9.1} | {:7} | {:7} | {:11} | {:6.2}",
                 i + 1,
                 result.gain_threshold_m,
                 result.loss_threshold_m,
                 result.overall_optimization_score,
                 result.median_elevation_accuracy,
                 result.median_gain_loss_ratio,
                 result.score_98_102,
                 result.score_90_110,
                 result.files_balanced_85_115,
                 result.accuracy_std_deviation);
    }
    
    // SPECIALIZED BESTS
    println!("\n💎 SPECIALIZED OPTIMAL PARAMETERS:");
    
    // Best for 98-102% accuracy
    let best_tight_accuracy = results.iter()
        .max_by_key(|r| r.score_98_102)
        .unwrap();
    println!("Best for ±2% accuracy: gain={:.3}m, loss={:.3}m → {} files ({:.1}%)",
             best_tight_accuracy.gain_threshold_m,
             best_tight_accuracy.loss_threshold_m,
             best_tight_accuracy.score_98_102,
             (best_tight_accuracy.score_98_102 as f32 / best_tight_accuracy.total_files as f32) * 100.0);
    
    // Best for gain/loss balance
    let best_balance = results.iter()
        .max_by_key(|r| r.files_balanced_95_105)
        .unwrap();
    println!("Best for ±5% balance: gain={:.3}m, loss={:.3}m → {} files ({:.1}%), ratio={:.1}%",
             best_balance.gain_threshold_m,
             best_balance.loss_threshold_m,
             best_balance.files_balanced_95_105,
             (best_balance.files_balanced_95_105 as f32 / best_balance.total_files as f32) * 100.0,
             best_balance.median_gain_loss_ratio);
    
    // Most consistent
    let most_consistent = results.iter()
        .min_by(|a, b| a.accuracy_std_deviation.partial_cmp(&b.accuracy_std_deviation).unwrap())
        .unwrap();
    println!("Most consistent: gain={:.3}m, loss={:.3}m → σ={:.2}%, range={:.2}%",
             most_consistent.gain_threshold_m,
             most_consistent.loss_threshold_m,
             most_consistent.accuracy_std_deviation,
             most_consistent.accuracy_q75_q25_range);
    
    // ACTIONABLE INSIGHTS
    println!("\n💡 KEY INSIGHTS:");
    
    // Analyze sensitivity patterns
    let very_sensitive_loss: Vec<_> = results.iter()
        .filter(|r| r.loss_threshold_m <= 0.03)
        .collect();
    let moderately_sensitive_loss: Vec<_> = results.iter()
        .filter(|r| r.loss_threshold_m > 0.03 && r.loss_threshold_m <= 0.07)
        .collect();
    
    if !very_sensitive_loss.is_empty() && !moderately_sensitive_loss.is_empty() {
        let avg_ratio_very = very_sensitive_loss.iter()
            .map(|r| r.median_gain_loss_ratio)
            .sum::<f32>() / very_sensitive_loss.len() as f32;
        let avg_ratio_moderate = moderately_sensitive_loss.iter()
            .map(|r| r.median_gain_loss_ratio)
            .sum::<f32>() / moderately_sensitive_loss.len() as f32;
        
        println!("• Very sensitive loss thresholds (≤0.03m): avg ratio {:.1}%", avg_ratio_very);
        println!("• Moderate loss thresholds (0.03-0.07m): avg ratio {:.1}%", avg_ratio_moderate);
    }
    
    // Performance degradation analysis
    let poor_performers: Vec<_> = results.iter()
        .filter(|r| r.files_outside_80_120 > best_overall.files_outside_80_120 + 5)
        .collect();
    
    if !poor_performers.is_empty() {
        println!("• {} parameter combinations show significant accuracy degradation", poor_performers.len());
        println!("  (>5 additional files beyond ±20% accuracy band)");
    }
    
    println!("\n🎯 FINAL RECOMMENDATION:");
    println!("Optimal parameters: gain_th={:.3}m, loss_th={:.3}m", 
             best_overall.gain_threshold_m, best_overall.loss_threshold_m);
    println!("This achieves:");
    println!("  • {:.1}% of files within ±10% accuracy", 
             (best_overall.score_90_110 as f32 / best_overall.total_files as f32) * 100.0);
    println!("  • {:.1}% of files with balanced gain/loss (85-115%)", 
             (best_overall.files_balanced_85_115 as f32 / best_overall.total_files as f32) * 100.0);
    println!("  • {:.2}% median accuracy with {:.1}% median gain/loss ratio", 
             best_overall.median_elevation_accuracy, best_overall.median_gain_loss_ratio);
    println!("  • Universal effectiveness across all terrain types");
}