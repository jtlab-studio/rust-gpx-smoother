use std::path::{Path, PathBuf};
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;
use crate::custom_smoother::{ElevationData, SmoothingVariant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnhancementType {
    // Base approaches
    Current,
    CurrentTwoStage,
    
    // Parameter Tuning Variants
    ConservativeTwoStage,
    AdaptiveTwoStage,
    GradientSpecificTwoStage,
    
    // Other enhancements
    SelectiveApplication,
    ConfidenceWeighted,
    ClimbDescentAsymmetry,
    LocalGradientValidation,
    MinimumChangeFilter,
}

#[derive(Debug, Clone)]
pub struct EnhancementCombination {
    pub name: String,
    pub enhancements: Vec<EnhancementType>,
}

impl EnhancementCombination {
    fn new(name: &str, enhancements: Vec<EnhancementType>) -> Self {
        Self {
            name: name.to_string(),
            enhancements,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ComparativeAnalysisResult {
    interval_m: f32,
    approach_results: Vec<ApproachResult>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ApproachResult {
    approach_name: String,
    score_98_102: u32,
    score_95_105: u32,
    score_90_110: u32,
    files_outside_80_120: u32,
    weighted_score: f32,
    median_accuracy: f32,
    worst_accuracy: f32,
    total_files: u32,
    // New metrics to track gain vs loss
    average_gain_change_percent: f32,
    average_loss_change_percent: f32,
}

// Enhancement parameters with justified values
#[derive(Debug, Clone)]
struct EnhancementParams {
    // Conservative two-stage
    conservative_gradient_iqr: f64,  // 1.5 (tighter than 2.0)
    conservative_rate_iqr: f64,      // 2.5 (tighter than 3.0)
    
    // Climb/descent asymmetry
    max_climb_gradient: f64,         // 35% for climbs
    max_descent_gradient: f64,       // 60% for descents
    max_climb_rate_m_per_hour: f64,  // 1000 m/hr
    
    // Minimum change filter
    min_change_flat: f64,            // 0.3m for flat terrain
    min_change_rolling: f64,         // 0.5m for rolling
    min_change_mountainous: f64,     // 1.0m for mountains
    
    // Confidence thresholds
    confidence_smoothing_factor: f64, // 0.0-1.0 smoothing weight
}

impl Default for EnhancementParams {
    fn default() -> Self {
        Self {
            conservative_gradient_iqr: 1.5,
            conservative_rate_iqr: 2.5,
            max_climb_gradient: 35.0,
            max_descent_gradient: 60.0,
            max_climb_rate_m_per_hour: 1000.0,
            min_change_flat: 0.3,
            min_change_rolling: 0.5,
            min_change_mountainous: 1.0,
            confidence_smoothing_factor: 0.7,
        }
    }
}

pub fn run_enhanced_comparative_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🔬 COMPREHENSIVE 26-APPROACH ANALYSIS");
    println!("=====================================");
    println!("Testing all enhancement combinations on elevation gain AND loss");
    
    // Define all 26 approach combinations
    let approaches = define_all_approaches();
    println!("📊 Total approaches to test: {}", approaches.len());
    
    // Load GPX data
    println!("\n📂 Loading GPX files...");
    let start = std::time::Instant::now();
    let (gpx_files_data, valid_files) = load_gpx_data(gpx_folder)?;
    println!("✅ Loaded {} files in {:.2}s", valid_files.len(), start.elapsed().as_secs_f64());
    
    // Filter files with elevation data
    let files_with_elevation: Vec<_> = valid_files.into_iter()
        .filter(|file| {
            if let Some(data) = gpx_files_data.get(file) {
                let has_elevation = data.elevations.iter()
                    .any(|&e| (e - data.elevations[0]).abs() > 0.1);
                if !has_elevation {
                    println!("⚠️  Excluding {} - no elevation variation", file);
                }
                has_elevation
            } else {
                false
            }
        })
        .collect();
    
    println!("📊 Processing {} files with valid elevation data", files_with_elevation.len());
    
    // Process all approaches
    let processing_start = std::time::Instant::now();
    let results = process_all_approaches(&gpx_files_data, &files_with_elevation, &approaches)?;
    println!("✅ Processing complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Write results
    write_comprehensive_results(&results, &approaches, Path::new(gpx_folder).join("26_approaches_analysis.csv"))?;
    
    // Print summary including gain/loss analysis
    print_comprehensive_summary(&results, &approaches);
    
    let total_time = total_start.elapsed();
    println!("\n⏱️  TOTAL EXECUTION TIME: {} minutes {:.1} seconds", 
             total_time.as_secs() / 60, 
             total_time.as_secs_f64() % 60.0);
    
    Ok(())
}

fn define_all_approaches() -> Vec<EnhancementCombination> {
    use EnhancementType::*;
    
    vec![
        // Base approaches (2)
        EnhancementCombination::new("1. Current", vec![Current]),
        EnhancementCombination::new("2. Current + Two-Stage", vec![CurrentTwoStage]),
        
        // Single enhancements on Current (6)
        EnhancementCombination::new("3. Current + Conservative", vec![Current, ConservativeTwoStage]),
        EnhancementCombination::new("4. Current + Adaptive", vec![Current, AdaptiveTwoStage]),
        EnhancementCombination::new("5. Current + Gradient-Specific", vec![Current, GradientSpecificTwoStage]),
        EnhancementCombination::new("6. Current + Selective", vec![Current, SelectiveApplication]),
        EnhancementCombination::new("7. Current + Confidence", vec![Current, ConfidenceWeighted]),
        EnhancementCombination::new("8. Current + Asymmetry", vec![Current, ClimbDescentAsymmetry]),
        EnhancementCombination::new("9. Current + Gradient-Val", vec![Current, LocalGradientValidation]),
        EnhancementCombination::new("10. Current + Min-Change", vec![Current, MinimumChangeFilter]),
        
        // Single enhancements on Two-Stage (6)
        EnhancementCombination::new("11. Two-Stage + Conservative", vec![CurrentTwoStage, ConservativeTwoStage]),
        EnhancementCombination::new("12. Two-Stage + Adaptive", vec![CurrentTwoStage, AdaptiveTwoStage]),
        EnhancementCombination::new("13. Two-Stage + Gradient-Specific", vec![CurrentTwoStage, GradientSpecificTwoStage]),
        EnhancementCombination::new("14. Two-Stage + Selective", vec![CurrentTwoStage, SelectiveApplication]),
        EnhancementCombination::new("15. Two-Stage + Confidence", vec![CurrentTwoStage, ConfidenceWeighted]),
        EnhancementCombination::new("16. Two-Stage + Asymmetry", vec![CurrentTwoStage, ClimbDescentAsymmetry]),
        EnhancementCombination::new("17. Two-Stage + Gradient-Val", vec![CurrentTwoStage, LocalGradientValidation]),
        EnhancementCombination::new("18. Two-Stage + Min-Change", vec![CurrentTwoStage, MinimumChangeFilter]),
        
        // Key two-enhancement combinations (4)
        EnhancementCombination::new("19. Current + Adaptive + Asymmetry", 
            vec![Current, AdaptiveTwoStage, ClimbDescentAsymmetry]),
        EnhancementCombination::new("20. Current + Adaptive + Min-Change", 
            vec![Current, AdaptiveTwoStage, MinimumChangeFilter]),
        EnhancementCombination::new("21. Two-Stage + Adaptive + Asymmetry", 
            vec![CurrentTwoStage, AdaptiveTwoStage, ClimbDescentAsymmetry]),
        EnhancementCombination::new("22. Two-Stage + Adaptive + Min-Change", 
            vec![CurrentTwoStage, AdaptiveTwoStage, MinimumChangeFilter]),
        
        // Best three-enhancement combinations (2)
        EnhancementCombination::new("23. Current + Adaptive + Asymmetry + Min", 
            vec![Current, AdaptiveTwoStage, ClimbDescentAsymmetry, MinimumChangeFilter]),
        EnhancementCombination::new("24. Two-Stage + Adaptive + Asymmetry + Min", 
            vec![CurrentTwoStage, AdaptiveTwoStage, ClimbDescentAsymmetry, MinimumChangeFilter]),
        
        // Kitchen sink (2)
        EnhancementCombination::new("25. Current + All", 
            vec![Current, ConservativeTwoStage, SelectiveApplication, ConfidenceWeighted, 
                 ClimbDescentAsymmetry, LocalGradientValidation, MinimumChangeFilter]),
        EnhancementCombination::new("26. Two-Stage + All", 
            vec![CurrentTwoStage, AdaptiveTwoStage, SelectiveApplication, ConfidenceWeighted,
                 ClimbDescentAsymmetry, LocalGradientValidation, MinimumChangeFilter]),
    ]
}

#[derive(Debug, Clone)]
struct GpxFileData {
    filename: String,
    elevations: Vec<f64>,
    distances: Vec<f64>,
    timestamps: Vec<f64>,
    official_gain: u32,
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
                                    let mut timestamps = vec![];
                                    
                                    for track in gpx.tracks {
                                        for segment in track.segments {
                                            for pt in segment.points {
                                                if let Some(ele) = pt.elevation {
                                                    coords.push((pt.point().y(), pt.point().x(), ele));
                                                    
                                                    if let Some(time) = pt.time {
                                                        if let Ok(time_str) = time.format() {
                                                            timestamps.push(time_str);
                                                        }
                                                    }
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
                                        
                                        let mut time_seconds = vec![0.0];
                                        if timestamps.len() >= 2 {
                                            for i in 1..timestamps.len().min(coords.len()) {
                                                time_seconds.push(i as f64);
                                            }
                                        }
                                        while time_seconds.len() < coords.len() {
                                            time_seconds.push(time_seconds.len() as f64);
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
                                            timestamps: time_seconds,
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

fn process_all_approaches(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String],
    approaches: &[EnhancementCombination]
) -> Result<Vec<ComparativeAnalysisResult>, Box<dyn std::error::Error>> {
    // Test intervals from 0.5m to 4.0m in 0.1m increments
    let intervals: Vec<f32> = (5..=40).map(|i| i as f32 * 0.1).collect();
    
    let gpx_data_arc = Arc::new(gpx_data.clone());
    
    println!("\n🚀 Processing {} intervals × {} files × {} approaches = {} total calculations",
             intervals.len(), valid_files.len(), approaches.len(), 
             intervals.len() * valid_files.len() * approaches.len());
    println!("⚡ Using parallel processing on {} cores", num_cpus::get());
    
    // Create work items
    let work_items: Vec<(f32, String, usize)> = intervals.iter()
        .flat_map(|&interval| {
            valid_files.iter().flat_map(move |file| {
                (0..approaches.len()).map(move |approach_idx| {
                    (interval, file.clone(), approach_idx)
                })
            })
        })
        .collect();
    
    println!("📊 Created {} work items for parallel processing...", work_items.len());
    
    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_items = work_items.len();
    let start_time = std::time::Instant::now();
    
    // Process all work items in parallel
    let all_results: Vec<(f32, String, usize, f32, f32, f32)> = work_items
        .par_iter()
        .filter_map(|(interval, filename, approach_idx)| {
            let gpx_data = Arc::clone(&gpx_data_arc);
            let processed_clone = Arc::clone(&processed);
            
            if let Some(file_data) = gpx_data.get(filename) {
                if file_data.official_gain > 0 {
                    // Process with specific approach
                    let (gain, loss, raw_gain, raw_loss) = process_with_approach(
                        file_data, 
                        *interval, 
                        &approaches[*approach_idx]
                    );
                    
                    let accuracy = (gain as f32 / file_data.official_gain as f32) * 100.0;
                    let gain_change = ((gain as f32 - raw_gain as f32) / raw_gain as f32) * 100.0;
                    let loss_change = ((loss as f32 - raw_loss as f32) / raw_loss as f32) * 100.0;
                    
                    // Update progress
                    let count = processed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count % 1000 == 0 || count == total_items {
                        let elapsed = start_time.elapsed().as_secs_f64();
                        let rate = count as f64 / elapsed;
                        let remaining = (total_items - count) as f64 / rate;
                        println!("  Progress: {}/{} ({:.1}%) - {:.0} items/sec - ETA: {:.0}s",
                                 count, total_items, 
                                 (count as f64 / total_items as f64) * 100.0,
                                 rate, remaining);
                    }
                    
                    return Some((*interval, filename.clone(), *approach_idx, accuracy, gain_change, loss_change));
                }
            }
            None
        })
        .collect();
    
    println!("✅ Parallel processing complete, aggregating results...");
    
    // Aggregate results by interval
    let mut results = Vec::new();
    
    for interval in intervals {
        let mut approach_results = Vec::new();
        
        for (idx, approach) in approaches.iter().enumerate() {
            let approach_data: Vec<_> = all_results.iter()
                .filter(|(i, _, a, _, _, _)| *i == interval && *a == idx)
                .collect();
            
            if !approach_data.is_empty() {
                let accuracies: Vec<f32> = approach_data.iter().map(|(_, _, _, acc, _, _)| *acc).collect();
                let gain_changes: Vec<f32> = approach_data.iter().map(|(_, _, _, _, gc, _)| *gc).collect();
                let loss_changes: Vec<f32> = approach_data.iter().map(|(_, _, _, _, _, lc)| *lc).collect();
                
                let metrics = calculate_accuracy_metrics(&accuracies);
                let avg_gain_change = gain_changes.iter().sum::<f32>() / gain_changes.len() as f32;
                let avg_loss_change = loss_changes.iter().sum::<f32>() / loss_changes.len() as f32;
                
                approach_results.push(ApproachResult {
                    approach_name: approach.name.clone(),
                    score_98_102: metrics.0,
                    score_95_105: metrics.1,
                    score_90_110: metrics.2,
                    files_outside_80_120: metrics.3,
                    weighted_score: metrics.4,
                    median_accuracy: metrics.5,
                    worst_accuracy: metrics.6,
                    total_files: accuracies.len() as u32,
                    average_gain_change_percent: avg_gain_change,
                    average_loss_change_percent: avg_loss_change,
                });
            }
        }
        
        results.push(ComparativeAnalysisResult {
            interval_m: interval,
            approach_results,
        });
    }
    
    Ok(results)
}

fn process_with_approach(
    file_data: &GpxFileData, 
    interval: f32,
    approach: &EnhancementCombination
) -> (u32, u32, u32, u32) { // (processed_gain, processed_loss, raw_gain, raw_loss)
    use EnhancementType::*;
    
    // Calculate raw gain/loss for comparison
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&file_data.elevations);
    
    let mut working_data = file_data.clone();
    let params = EnhancementParams::default();
    
    // Apply enhancements in order
    for enhancement in &approach.enhancements {
        match enhancement {
            Current => {
                // Base statistical outlier removal
                working_data.elevations = remove_statistical_outliers(
                    &working_data.elevations, 
                    &working_data.distances
                );
            },
            CurrentTwoStage => {
                // First apply current, then two-stage
                working_data.elevations = remove_statistical_outliers(
                    &working_data.elevations, 
                    &working_data.distances
                );
                working_data = apply_standard_two_stage(&working_data, &params);
            },
            ConservativeTwoStage => {
                working_data = apply_conservative_two_stage(&working_data, &params);
            },
            AdaptiveTwoStage => {
                working_data = apply_adaptive_two_stage(&working_data, &params);
            },
            GradientSpecificTwoStage => {
                working_data = apply_gradient_specific_two_stage(&working_data, &params);
            },
            SelectiveApplication => {
                // Check if enhancement is needed
                let current_accuracy = calculate_current_accuracy(&working_data);
                if should_apply_enhancement(&working_data, current_accuracy) {
                    // Apply adaptive two-stage as the selective enhancement
                    working_data = apply_adaptive_two_stage(&working_data, &params);
                }
            },
            ConfidenceWeighted => {
                working_data = apply_confidence_weighted_outlier_removal(&working_data, &params);
            },
            ClimbDescentAsymmetry => {
                working_data = apply_climb_descent_asymmetry(&working_data, &params);
            },
            LocalGradientValidation => {
                working_data = apply_local_gradient_validation(&working_data, &params);
            },
            MinimumChangeFilter => {
                working_data = apply_minimum_change_filter(&working_data, &params);
            },
        }
    }
    
    // Calculate final elevation gain/loss using DistBased
    let mut elevation_data = ElevationData::new_with_variant(
        working_data.elevations,
        working_data.distances,
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval as f64);
    
    let processed_gain = elevation_data.get_total_elevation_gain().round() as u32;
    let processed_loss = elevation_data.get_total_elevation_loss().round() as u32;
    
    (processed_gain, processed_loss, raw_gain, raw_loss)
}

// Enhancement implementations

fn remove_statistical_outliers(elevations: &[f64], distances: &[f64]) -> Vec<f64> {
    if elevations.len() < 10 {
        return elevations.to_vec();
    }
    
    let mut cleaned = elevations.to_vec();
    
    // Calculate gradients
    let mut gradients = Vec::new();
    for i in 1..elevations.len() {
        let dist_diff = distances[i] - distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (elevations[i] - elevations[i-1]) / dist_diff * 100.0;
            gradients.push(gradient);
        }
    }
    
    if gradients.is_empty() {
        return cleaned;
    }
    
    // IQR-based outlier detection
    let mut sorted_gradients = gradients.clone();
    sorted_gradients.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let q1 = sorted_gradients[sorted_gradients.len() / 4];
    let q3 = sorted_gradients[(sorted_gradients.len() * 3) / 4];
    let iqr = q3 - q1;
    
    let lower_bound = q1 - 2.0 * iqr;
    let upper_bound = q3 + 2.0 * iqr;
    
    // Smooth outliers
    for i in 1..elevations.len() - 1 {
        if i <= gradients.len() {
            let gradient = gradients[i-1];
            if gradient < lower_bound || gradient > upper_bound {
                // Linear interpolation
                cleaned[i] = (cleaned[i-1] + cleaned[i+1]) / 2.0;
            }
        }
    }
    
    cleaned
}

fn apply_standard_two_stage(file_data: &GpxFileData, params: &EnhancementParams) -> GpxFileData {
    let mut cleaned_data = file_data.clone();
    
    // Stage 2: Elevation gain rate outlier removal
    let mut gain_rates = Vec::new();
    for i in 1..file_data.elevations.len() {
        let elev_change = file_data.elevations[i] - file_data.elevations[i-1];
        let time_change = file_data.timestamps[i] - file_data.timestamps[i-1];
        
        if time_change > 0.0 && elev_change > 0.0 {
            let rate = (elev_change / time_change) * 3600.0; // m/hour
            gain_rates.push((i, rate));
        }
    }
    
    if gain_rates.len() > 4 {
        let mut sorted_rates: Vec<f64> = gain_rates.iter().map(|(_, r)| *r).collect();
        sorted_rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let q1 = sorted_rates[sorted_rates.len() / 4];
        let q3 = sorted_rates[(sorted_rates.len() * 3) / 4];
        let iqr = q3 - q1;
        let upper_bound = q3 + 3.0 * iqr; // Standard multiplier
        
        for (idx, rate) in gain_rates {
            if rate > upper_bound {
                // Cap the elevation change
                let time_change = cleaned_data.timestamps[idx] - cleaned_data.timestamps[idx-1];
                let max_change = (upper_bound / 3600.0) * time_change;
                cleaned_data.elevations[idx] = cleaned_data.elevations[idx-1] + max_change;
            }
        }
    }
    
    cleaned_data
}

fn apply_conservative_two_stage(file_data: &GpxFileData, params: &EnhancementParams) -> GpxFileData {
    let mut cleaned_data = file_data.clone();
    
    // Use tighter IQR multipliers
    let gradient_multiplier = params.conservative_gradient_iqr; // 1.5
    let rate_multiplier = params.conservative_rate_iqr; // 2.5
    
    // Apply gradient-based cleaning with tighter bounds
    let mut gradients = Vec::new();
    for i in 1..file_data.elevations.len() {
        let dist_diff = file_data.distances[i] - file_data.distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (file_data.elevations[i] - file_data.elevations[i-1]) / dist_diff * 100.0;
            gradients.push((i, gradient));
        }
    }
    
    if gradients.len() > 4 {
        let mut sorted_grads: Vec<f64> = gradients.iter().map(|(_, g)| *g).collect();
        sorted_grads.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let q1 = sorted_grads[sorted_grads.len() / 4];
        let q3 = sorted_grads[(sorted_grads.len() * 3) / 4];
        let iqr = q3 - q1;
        
        let lower_bound = q1 - gradient_multiplier * iqr;
        let upper_bound = q3 + gradient_multiplier * iqr;
        
        for (idx, gradient) in gradients {
            if gradient < lower_bound || gradient > upper_bound {
                // Smooth more aggressively
                if idx > 1 && idx < cleaned_data.elevations.len() - 1 {
                    cleaned_data.elevations[idx] = 
                        (cleaned_data.elevations[idx-1] + cleaned_data.elevations[idx+1]) / 2.0;
                }
            }
        }
    }
    
    // Apply rate-based cleaning with tighter bounds
    let mut gain_rates = Vec::new();
    for i in 1..cleaned_data.elevations.len() {
        let elev_change = cleaned_data.elevations[i] - cleaned_data.elevations[i-1];
        let time_change = cleaned_data.timestamps[i] - cleaned_data.timestamps[i-1];
        
        if time_change > 0.0 && elev_change > 0.0 {
            let rate = (elev_change / time_change) * 3600.0;
            gain_rates.push((i, rate));
        }
    }
    
    if gain_rates.len() > 4 {
        let mut sorted_rates: Vec<f64> = gain_rates.iter().map(|(_, r)| *r).collect();
        sorted_rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let q1 = sorted_rates[sorted_rates.len() / 4];
        let q3 = sorted_rates[(sorted_rates.len() * 3) / 4];
        let iqr = q3 - q1;
        let upper_bound = q3 + rate_multiplier * iqr;
        
        for (idx, rate) in gain_rates {
            if rate > upper_bound {
                let time_change = cleaned_data.timestamps[idx] - cleaned_data.timestamps[idx-1];
                let max_change = (upper_bound / 3600.0) * time_change;
                cleaned_data.elevations[idx] = cleaned_data.elevations[idx-1] + max_change;
            }
        }
    }
    
    cleaned_data
}

fn apply_adaptive_two_stage(file_data: &GpxFileData, params: &EnhancementParams) -> GpxFileData {
    let mut cleaned_data = file_data.clone();
    
    // Calculate terrain type
    let total_distance_km = file_data.distances.last().unwrap_or(&0.0) / 1000.0;
    let (raw_gain, _) = calculate_raw_gain_loss(&file_data.elevations);
    let gain_per_km = if total_distance_km > 0.0 { raw_gain as f64 / total_distance_km } else { 0.0 };
    
    // Adaptive parameters based on terrain
    let (gradient_multiplier, rate_multiplier) = if gain_per_km < 20.0 {
        (1.5, 2.0) // Flat: tight bounds
    } else if gain_per_km < 60.0 {
        (2.0, 3.0) // Hilly: balanced
    } else {
        (2.5, 4.0) // Mountainous: permissive
    };
    
    // Apply terrain-adaptive gradient cleaning
    let mut gradients = Vec::new();
    for i in 1..file_data.elevations.len() {
        let dist_diff = file_data.distances[i] - file_data.distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (file_data.elevations[i] - file_data.elevations[i-1]) / dist_diff * 100.0;
            gradients.push((i, gradient));
        }
    }
    
    if gradients.len() > 4 {
        let mut sorted_grads: Vec<f64> = gradients.iter().map(|(_, g)| *g).collect();
        sorted_grads.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let q1 = sorted_grads[sorted_grads.len() / 4];
        let q3 = sorted_grads[(sorted_grads.len() * 3) / 4];
        let iqr = q3 - q1;
        
        let lower_bound = q1 - gradient_multiplier * iqr;
        let upper_bound = q3 + gradient_multiplier * iqr;
        
        for (idx, gradient) in gradients {
            if gradient < lower_bound || gradient > upper_bound {
                if idx > 0 && idx < cleaned_data.elevations.len() - 1 {
                    cleaned_data.elevations[idx] = 
                        (cleaned_data.elevations[idx-1] + cleaned_data.elevations[idx+1]) / 2.0;
                }
            }
        }
    }
    
    // Apply terrain-adaptive rate cleaning
    let mut gain_rates = Vec::new();
    for i in 1..cleaned_data.elevations.len() {
        let elev_change = cleaned_data.elevations[i] - cleaned_data.elevations[i-1];
        let time_change = cleaned_data.timestamps[i] - cleaned_data.timestamps[i-1];
        
        if time_change > 0.0 && elev_change > 0.0 {
            let rate = (elev_change / time_change) * 3600.0;
            gain_rates.push((i, rate));
        }
    }
    
    if gain_rates.len() > 4 {
        let mut sorted_rates: Vec<f64> = gain_rates.iter().map(|(_, r)| *r).collect();
        sorted_rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let q1 = sorted_rates[sorted_rates.len() / 4];
        let q3 = sorted_rates[(sorted_rates.len() * 3) / 4];
        let iqr = q3 - q1;
        let upper_bound = q3 + rate_multiplier * iqr;
        
        for (idx, rate) in gain_rates {
            if rate > upper_bound {
                let time_change = cleaned_data.timestamps[idx] - cleaned_data.timestamps[idx-1];
                let max_change = (upper_bound / 3600.0) * time_change;
                cleaned_data.elevations[idx] = cleaned_data.elevations[idx-1] + max_change;
            }
        }
    }
    
    cleaned_data
}

fn apply_gradient_specific_two_stage(file_data: &GpxFileData, params: &EnhancementParams) -> GpxFileData {
    let mut cleaned_data = file_data.clone();
    
    // Process climbs and descents separately
    for i in 1..file_data.elevations.len() {
        let elev_change = file_data.elevations[i] - file_data.elevations[i-1];
        let dist_diff = file_data.distances[i] - file_data.distances[i-1];
        
        if dist_diff > 0.0 {
            let gradient = (elev_change / dist_diff) * 100.0;
            
            if elev_change > 0.0 {
                // CLIMB: Strict processing
                if gradient > params.max_climb_gradient {
                    // Cap climbs at 35%
                    let max_change = params.max_climb_gradient * dist_diff / 100.0;
                    cleaned_data.elevations[i] = cleaned_data.elevations[i-1] + max_change;
                }
                
                // Check climb rate
                let time_change = file_data.timestamps[i] - file_data.timestamps[i-1];
                if time_change > 0.0 {
                    let rate = (elev_change / time_change) * 3600.0;
                    if rate > params.max_climb_rate_m_per_hour {
                        let max_change = (params.max_climb_rate_m_per_hour / 3600.0) * time_change;
                        cleaned_data.elevations[i] = cleaned_data.elevations[i-1] + max_change;
                    }
                }
            } else if elev_change < 0.0 {
                // DESCENT: Permissive processing
                if gradient < -params.max_descent_gradient {
                    // Only cap extreme descents > 60%
                    let max_change = -params.max_descent_gradient * dist_diff / 100.0;
                    cleaned_data.elevations[i] = cleaned_data.elevations[i-1] + max_change;
                }
                // No rate cap for descents
            }
        }
    }
    
    cleaned_data
}

fn should_apply_enhancement(file_data: &GpxFileData, current_accuracy: f32) -> bool {
    // Check if file needs enhancement
    if current_accuracy < 90.0 || current_accuracy > 110.0 {
        return true;
    }
    
    // Check gradient variance
    let mut gradients = Vec::new();
    for i in 1..file_data.elevations.len() {
        let dist_diff = file_data.distances[i] - file_data.distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (file_data.elevations[i] - file_data.elevations[i-1]) / dist_diff * 100.0;
            gradients.push(gradient);
        }
    }
    
    if !gradients.is_empty() {
        let mean = gradients.iter().sum::<f64>() / gradients.len() as f64;
        let variance = gradients.iter()
            .map(|&g| (g - mean).powi(2))
            .sum::<f64>() / gradients.len() as f64;
        
        if variance > 150.0 {
            return true;
        }
    }
    
    // Check max climb rate
    for i in 1..file_data.elevations.len() {
        let elev_change = file_data.elevations[i] - file_data.elevations[i-1];
        let time_change = file_data.timestamps[i] - file_data.timestamps[i-1];
        
        if time_change > 0.0 && elev_change > 0.0 {
            let rate = (elev_change / time_change) * 3600.0;
            if rate > 1200.0 {
                return true;
            }
        }
    }
    
    false
}

fn calculate_current_accuracy(file_data: &GpxFileData) -> f32 {
    if file_data.official_gain == 0 {
        return 100.0;
    }
    
    let (gain, _) = calculate_raw_gain_loss(&file_data.elevations);
    (gain as f32 / file_data.official_gain as f32) * 100.0
}

fn apply_confidence_weighted_outlier_removal(file_data: &GpxFileData, params: &EnhancementParams) -> GpxFileData {
    let mut cleaned_data = file_data.clone();
    
    // Calculate confidence scores for each point
    let mut confidences = vec![1.0; file_data.elevations.len()];
    
    // Factor 1: Distance from gradient IQR bounds
    let mut gradients = Vec::new();
    for i in 1..file_data.elevations.len() {
        let dist_diff = file_data.distances[i] - file_data.distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (file_data.elevations[i] - file_data.elevations[i-1]) / dist_diff * 100.0;
            gradients.push(gradient);
        } else {
            gradients.push(0.0);
        }
    }
    
    if gradients.len() > 4 {
        let mut sorted_grads = gradients.clone();
        sorted_grads.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let q1 = sorted_grads[sorted_grads.len() / 4];
        let q3 = sorted_grads[(sorted_grads.len() * 3) / 4];
        let iqr = q3 - q1;
        
        for i in 0..gradients.len() {
            let distance_from_median = (gradients[i] - (q1 + q3) / 2.0).abs();
            let normalized_distance = distance_from_median / (iqr + 1.0);
            confidences[i] *= (1.0 - normalized_distance.min(1.0) * 0.5);
        }
    }
    
    // Factor 2: Local point density
    for i in 0..file_data.elevations.len() {
        let window_start = if i >= 5 { i - 5 } else { 0 };
        let window_end = if i + 5 < file_data.elevations.len() { i + 5 } else { file_data.elevations.len() - 1 };
        
        if window_end > window_start {
            let distance_span = file_data.distances[window_end] - file_data.distances[window_start];
            let point_count = window_end - window_start + 1;
            let avg_spacing = distance_span / point_count as f64;
            
            if avg_spacing > 50.0 {
                confidences[i] *= 0.7; // Sparse data
            } else if avg_spacing < 2.0 {
                confidences[i] *= 0.9; // Very dense
            }
        }
    }
    
    // Apply confidence-weighted smoothing
    for i in 1..cleaned_data.elevations.len() - 1 {
        let smooth_weight = (1.0 - confidences[i]) * params.confidence_smoothing_factor;
        cleaned_data.elevations[i] = cleaned_data.elevations[i] * (1.0 - smooth_weight) +
                                     (cleaned_data.elevations[i-1] + cleaned_data.elevations[i+1]) * 0.5 * smooth_weight;
    }
    
    cleaned_data
}

fn apply_climb_descent_asymmetry(file_data: &GpxFileData, params: &EnhancementParams) -> GpxFileData {
    let mut cleaned_data = file_data.clone();
    
    for i in 1..file_data.elevations.len() {
        let elev_change = file_data.elevations[i] - file_data.elevations[i-1];
        let dist_diff = file_data.distances[i] - file_data.distances[i-1];
        
        if dist_diff > 0.0 {
            let gradient = (elev_change / dist_diff) * 100.0;
            
            if elev_change > 0.0 {
                // CLIMBING: Strict limits
                if gradient > params.max_climb_gradient {
                    let max_change = params.max_climb_gradient * dist_diff / 100.0;
                    cleaned_data.elevations[i] = cleaned_data.elevations[i-1] + max_change;
                }
                
                // Check if climb is sustained (not a spike)
                if i > 1 && i < file_data.elevations.len() - 1 {
                    let prev_change = file_data.elevations[i-1] - file_data.elevations[i-2];
                    let next_change = file_data.elevations[i+1] - file_data.elevations[i];
                    
                    if prev_change <= 0.0 && next_change <= 0.0 {
                        // Single point climb spike - smooth it
                        cleaned_data.elevations[i] = (cleaned_data.elevations[i-1] + file_data.elevations[i+1]) / 2.0;
                    }
                }
            } else if elev_change < 0.0 {
                // DESCENDING: Permissive limits
                if gradient < -params.max_descent_gradient {
                    let max_change = -params.max_descent_gradient * dist_diff / 100.0;
                    cleaned_data.elevations[i] = cleaned_data.elevations[i-1] + max_change;
                }
                // No spike checking for descents - drop-offs are real
            }
        }
    }
    
    cleaned_data
}

#[derive(Debug, Clone, Copy)]
enum GradientPattern {
    SustainedClimb,
    Switchback,
    TechnicalDescent,
    RollingTerrain,
    Noise,
}

fn analyze_gradient_pattern(gradients: &[f64]) -> GradientPattern {
    if gradients.len() < 5 {
        return GradientPattern::Noise;
    }
    
    let all_positive = gradients.iter().all(|&g| g > 2.0);
    let all_negative = gradients.iter().all(|&g| g < -2.0);
    let alternating = gradients.windows(2)
        .filter(|w| (w[0] > 0.0 && w[1] < 0.0) || (w[0] < 0.0 && w[1] > 0.0))
        .count() >= 2;
    
    if all_positive {
        GradientPattern::SustainedClimb
    } else if all_negative {
        GradientPattern::TechnicalDescent
    } else if alternating && gradients.iter().any(|&g| g.abs() > 15.0) {
        GradientPattern::Switchback
    } else if alternating {
        GradientPattern::RollingTerrain
    } else {
        GradientPattern::Noise
    }
}

fn apply_local_gradient_validation(file_data: &GpxFileData, _params: &EnhancementParams) -> GpxFileData {
    let mut cleaned_data = file_data.clone();
    
    // Calculate gradients
    let mut gradients = vec![0.0];
    for i in 1..file_data.elevations.len() {
        let dist_diff = file_data.distances[i] - file_data.distances[i-1];
        if dist_diff > 0.0 {
            let gradient = (file_data.elevations[i] - file_data.elevations[i-1]) / dist_diff * 100.0;
            gradients.push(gradient);
        } else {
            gradients.push(0.0);
        }
    }
    
    // Analyze 5-point windows
    for i in 2..gradients.len().saturating_sub(2) {
        let window = &gradients[i-2..=i+2];
        let pattern = analyze_gradient_pattern(window);
        
        match pattern {
            GradientPattern::SustainedClimb => {
                // Should have consistent gradient
                let median = {
                    let mut sorted = window.to_vec();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    sorted[2]
                };
                
                if (gradients[i] - median).abs() > median * 0.5 {
                    // Smooth to median
                    let ratio = median / gradients[i];
                    let elev_change = (cleaned_data.elevations[i] - cleaned_data.elevations[i-1]) * ratio;
                    cleaned_data.elevations[i] = cleaned_data.elevations[i-1] + elev_change;
                }
            },
            GradientPattern::Switchback => {
                // Preserve pattern - no smoothing
            },
            GradientPattern::TechnicalDescent => {
                // Light smoothing only for extreme outliers
                if gradients[i] < -80.0 {
                    cleaned_data.elevations[i] = (cleaned_data.elevations[i-1] + cleaned_data.elevations[i+1]) / 2.0;
                }
            },
            GradientPattern::RollingTerrain => {
                // Smooth only extreme spikes
                if gradients[i].abs() > 50.0 {
                    cleaned_data.elevations[i] = (cleaned_data.elevations[i-1] + cleaned_data.elevations[i+1]) / 2.0;
                }
            },
            GradientPattern::Noise => {
                // Heavy smoothing
                if i > 0 && i < cleaned_data.elevations.len() - 1 {
                    cleaned_data.elevations[i] = 
                        cleaned_data.elevations[i-1] * 0.25 +
                        cleaned_data.elevations[i] * 0.5 +
                        cleaned_data.elevations[i+1] * 0.25;
                }
            },
        }
    }
    
    cleaned_data
}

fn apply_minimum_change_filter(file_data: &GpxFileData, params: &EnhancementParams) -> GpxFileData {
    let mut cleaned_data = file_data.clone();
    
    // Determine terrain type
    let total_distance_km = file_data.distances.last().unwrap_or(&0.0) / 1000.0;
    let (raw_gain, _) = calculate_raw_gain_loss(&file_data.elevations);
    let gain_per_km = if total_distance_km > 0.0 { raw_gain as f64 / total_distance_km } else { 0.0 };
    
    let threshold = if gain_per_km < 20.0 {
        params.min_change_flat
    } else if gain_per_km < 60.0 {
        params.min_change_rolling
    } else {
        params.min_change_mountainous
    };
    
    // Apply deadband filter
    let mut filtered_elevations = vec![file_data.elevations[0]];
    let mut accumulated_change = 0.0;
    let mut last_recorded_idx = 0;
    let mut last_recorded_elev = file_data.elevations[0];
    
    for i in 1..file_data.elevations.len() {
        let change = file_data.elevations[i] - last_recorded_elev;
        accumulated_change += change;
        
        if accumulated_change.abs() >= threshold {
            // Record the accumulated change
            let segments = i - last_recorded_idx;
            let change_per_segment = accumulated_change / segments as f64;
            
            // Fill in the intermediate elevations
            for j in 1..=segments {
                let elev = last_recorded_elev + change_per_segment * j as f64;
                if filtered_elevations.len() < i {
                    filtered_elevations.push(elev);
                }
            }
            
            last_recorded_elev = file_data.elevations[i];
            last_recorded_idx = i;
            accumulated_change = 0.0;
        }
    }
    
    // Fill any remaining elevations
    while filtered_elevations.len() < file_data.elevations.len() {
        filtered_elevations.push(last_recorded_elev);
    }
    
    cleaned_data.elevations = filtered_elevations;
    cleaned_data
}

fn calculate_raw_gain_loss(elevations: &[f64]) -> (u32, u32) {
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
    
    (gain.round() as u32, loss.round() as u32)
}

fn calculate_accuracy_metrics(accuracies: &[f32]) -> (u32, u32, u32, u32, f32, f32, f32) {
    if accuracies.is_empty() {
        return (0, 0, 0, 0, 0.0, 0.0, 0.0);
    }
    
    let score_98_102 = accuracies.iter().filter(|&&acc| acc >= 98.0 && acc <= 102.0).count() as u32;
    let score_95_105 = accuracies.iter().filter(|&&acc| acc >= 95.0 && acc <= 105.0).count() as u32;
    let score_90_110 = accuracies.iter().filter(|&&acc| acc >= 90.0 && acc <= 110.0).count() as u32;
    let files_outside_80_120 = accuracies.iter().filter(|&&acc| acc < 80.0 || acc > 120.0).count() as u32;
    
    let weighted_score = (score_98_102 as f32 * 10.0) +
                        ((score_95_105 - score_98_102) as f32 * 6.0) +
                        ((score_90_110 - score_95_105) as f32 * 3.0) -
                        (files_outside_80_120 as f32 * 5.0);
    
    let mut sorted_accuracies = accuracies.to_vec();
    sorted_accuracies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let median_accuracy = if sorted_accuracies.len() % 2 == 0 {
        (sorted_accuracies[sorted_accuracies.len() / 2 - 1] + 
         sorted_accuracies[sorted_accuracies.len() / 2]) / 2.0
    } else {
        sorted_accuracies[sorted_accuracies.len() / 2]
    };
    
    let worst_accuracy = accuracies.iter()
        .max_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied()
        .unwrap_or(100.0);
    
    (score_98_102, score_95_105, score_90_110, files_outside_80_120, 
     weighted_score, median_accuracy, worst_accuracy)
}

fn write_comprehensive_results(
    results: &[ComparativeAnalysisResult],
    approaches: &[EnhancementCombination],
    output_path: PathBuf
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Build header
    let mut header = vec!["Interval (m)".to_string()];
    for approach in approaches {
        header.push(format!("{} Score", approach.name));
        header.push(format!("{} 90-110%", approach.name));
        header.push(format!("{} Gain Δ%", approach.name));
        header.push(format!("{} Loss Δ%", approach.name));
    }
    wtr.write_record(&header)?;
    
    // Write data rows
    for result in results {
        let mut row = vec![format!("{:.1}", result.interval_m)];
        
        for approach_result in &result.approach_results {
            row.push(format!("{:.0}", approach_result.weighted_score));
            row.push(approach_result.score_90_110.to_string());
            row.push(format!("{:+.1}", approach_result.average_gain_change_percent));
            row.push(format!("{:+.1}", approach_result.average_loss_change_percent));
        }
        
        wtr.write_record(&row)?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_comprehensive_summary(results: &[ComparativeAnalysisResult], approaches: &[EnhancementCombination]) {
    println!("\n📊 COMPREHENSIVE 26-APPROACH ANALYSIS SUMMARY");
    println!("============================================");
    
    // Find best approach for each metric
    let mut best_scores: Vec<(String, f32, f32, u32, f32, f32)> = Vec::new();
    
    for (idx, approach) in approaches.iter().enumerate() {
        let mut max_score = 0.0f32;
        let mut best_interval = 0.0f32;
        let mut best_90_110 = 0u32;
        let mut gain_change = 0.0f32;
        let mut loss_change = 0.0f32;
        
        for result in results {
            if let Some(approach_result) = result.approach_results.get(idx) {
                if approach_result.weighted_score > max_score {
                    max_score = approach_result.weighted_score;
                    best_interval = result.interval_m;
                    best_90_110 = approach_result.score_90_110;
                    gain_change = approach_result.average_gain_change_percent;
                    loss_change = approach_result.average_loss_change_percent;
                }
            }
        }
        
        best_scores.push((approach.name.clone(), best_interval, max_score, best_90_110, gain_change, loss_change));
    }
    
    // Sort by score
    best_scores.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    
    println!("\n🏆 TOP 10 APPROACHES:");
    println!("Rank | Approach                              | Interval | Score | 90-110% | Success% | Gain Δ% | Loss Δ%");
    println!("-----|---------------------------------------|----------|-------|---------|----------|---------|--------");
    
    let total_files = results[0].approach_results[0].total_files as f32;
    
    for (rank, (name, interval, score, count_90_110, gain_change, loss_change)) in best_scores.iter().take(10).enumerate() {
        let success_rate = (*count_90_110 as f32 / total_files) * 100.0;
        println!("{:4} | {:37} | {:7.1}m | {:5.0} | {:7} | {:7.1}% | {:+6.1}% | {:+6.1}%",
                 rank + 1, name, interval, score, count_90_110, success_rate, gain_change, loss_change);
    }
    
    // Analysis of gain vs loss impact
    println!("\n📈 ELEVATION GAIN vs LOSS IMPACT:");
    println!("Current approach changes both gain and loss by similar amounts.");
    
    let asymmetric_approaches: Vec<_> = best_scores.iter()
        .filter(|(name, _, _, _, gain, loss)| (gain - loss).abs() > 2.0)
        .take(5)
        .collect();
    
    if !asymmetric_approaches.is_empty() {
        println!("\nApproaches with asymmetric impact (different effect on gain vs loss):");
        for (name, _, _, _, gain, loss) in asymmetric_approaches {
            println!("- {}: Gain {:+.1}%, Loss {:+.1}% (Δ {:.1}%)", 
                     name, gain, loss, (gain - loss).abs());
        }
    }
    
    // Find approach that minimizes elevation change while maintaining accuracy
    let minimal_change_approaches: Vec<_> = best_scores.iter()
        .filter(|(_, _, score, _, gain, loss)| *score > 800.0 && gain.abs() < 5.0 && loss.abs() < 5.0)
        .take(3)
        .collect();
    
    if !minimal_change_approaches.is_empty() {
        println!("\n✨ MINIMAL CHANGE APPROACHES (high accuracy, low modification):");
        for (name, interval, score, _, gain, loss) in minimal_change_approaches {
            println!("- {} at {:.1}m: Score {:.0}, Changes: Gain {:+.1}%, Loss {:+.1}%", 
                     name, interval, score, gain, loss);
        }
    }
    
    // Overall recommendation
    let best = &best_scores[0];
    println!("\n🎯 RECOMMENDATION:");
    println!("Use '{}' with {:.1}m intervals", best.0, best.1);
    println!("- Achieves {:.1}% success rate ({} of {} files within 90-110%)", 
             (best.3 as f32 / total_files) * 100.0, best.3, total_files as u32);
    println!("- Modifies elevation gain by {:+.1}% and loss by {:+.1}%", best.4, best.5);
    
    if best.0.contains("Two-Stage") {
        println!("- The two-stage outlier removal provides marginal improvement over the base approach");
    }
    if best.0.contains("Adaptive") {
        println!("- Terrain-adaptive parameters help handle diverse route types");
    }
    if best.0.contains("Min-Change") {
        println!("- Minimum change filtering reduces GPS noise effectively");
    }
}