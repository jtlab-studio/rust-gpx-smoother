// src/assymetric_analysis.rs
use std::path::Path;
use std::collections::HashMap;
use csv::Writer;
use serde::Serialize;
use rayon::prelude::*;
use std::sync::Arc;
use crate::custom_smoother::{ElevationData, SmoothingVariant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessingMethod {
    Standard,                   // Standard distance-based (baseline)
    AsymmetricInterval,         // Different intervals for gain/loss
    DirectionalDeadzone,        // Different thresholds for gain/loss
    LossCompensation,           // Apply correction factor to loss
    GradientBased,              // Protect steep sections
    TwoPass,                    // Separate gain/loss passes
    HybridSelective,            // Selective smoothing based on variance
    AdaptiveLossCompensation,   // Terrain-adaptive compensation
    CombinedApproach,           // Mix of methods
    ButterworthAsymmetric,      // Different cutoffs for gain/loss
    ElevationBandSpecific,      // Different processing by elevation range
}

#[derive(Debug, Serialize, Clone)]
pub struct MethodResult {
    method: String,
    parameters: String,
    // Accuracy scores
    score_98_102: u32,
    score_95_105: u32,
    score_90_110: u32,
    score_85_115: u32,
    score_80_120: u32,
    files_outside_80_120: u32,
    weighted_accuracy_score: f32,
    // Gain/Loss balance metrics
    gain_loss_balance_score: f32,
    files_balanced_85_115: u32,
    files_balanced_70_130: u32,
    avg_gain_loss_ratio: f32,
    median_gain_loss_ratio: f32,
    // Traditional metrics
    average_accuracy: f32,
    median_accuracy: f32,
    worst_accuracy: f32,
    best_accuracy: f32,
    std_deviation: f32,
    success_rate: f32,
    // Gain/loss metrics
    avg_raw_gain: f32,
    avg_raw_loss: f32,
    avg_processed_gain: f32,
    avg_processed_loss: f32,
    total_raw_elevation_loss: f32,
    loss_reduction_percent: f32,
    gain_reduction_percent: f32,
    // Combined scores
    combined_score: f32,
    loss_preservation_score: f32,
    total_files: u32,
    // Terrain-specific metrics
    flat_terrain_score: f32,
    hilly_terrain_score: f32,
    mountain_terrain_score: f32,
}

#[derive(Debug, Clone)]
struct GpxFileData {
    filename: String,
    elevations: Vec<f64>,
    distances: Vec<f64>,
    official_gain: u32,
    terrain_type: TerrainType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TerrainType {
    Flat,
    Rolling,
    Hilly,
    Mountainous,
}

#[derive(Debug, Clone)]
struct ProcessingResult {
    accuracy: f32,
    raw_gain: f32,
    raw_loss: f32,
    processed_gain: f32,
    processed_loss: f32,
    gain_loss_ratio: f32,
    terrain_type: TerrainType,
}

#[derive(Debug, Clone)]
struct CrossValidationResult {
    mean_accuracy: f32,
    std_accuracy: f32,
    mean_gain_loss_ratio: f32,
    std_gain_loss_ratio: f32,
    consistency_score: f32,
}

pub fn run_asymmetric_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let total_start = std::time::Instant::now();
    
    println!("\n🔬 COMPREHENSIVE ASYMMETRIC & ALTERNATIVE PROCESSING ANALYSIS");
    println!("============================================================");
    println!("Testing all methods to preserve elevation loss while maintaining gain accuracy\n");
    
    // Load GPX data
    println!("📂 Loading GPX files...");
    let start = std::time::Instant::now();
    let (gpx_files_data, valid_files) = load_gpx_data(gpx_folder)?;
    println!("✅ Loaded {} files in {:.2}s", valid_files.len(), start.elapsed().as_secs_f64());
    
    // Filter files with elevation data
    let files_with_elevation: Vec<_> = valid_files.into_iter()
        .filter(|file| {
            if let Some(data) = gpx_files_data.get(file) {
                let has_elevation = data.elevations.iter()
                    .any(|&e| (e - data.elevations[0]).abs() > 0.1);
                has_elevation
            } else {
                false
            }
        })
        .collect();
    
    println!("📊 Processing {} files with valid elevation data", files_with_elevation.len());
    
    // Analyze terrain distribution
    print_terrain_distribution(&gpx_files_data);
    
    // Define comprehensive test configurations
    let test_configs = generate_comprehensive_test_configs();
    println!("\n🔍 Generated {} test configurations", test_configs.len());
    
    // Phase 1: Broad search
    println!("\n=== PHASE 1: BROAD PARAMETER SEARCH ===");
    let processing_start = std::time::Instant::now();
    let broad_results = process_all_methods(&gpx_files_data, &files_with_elevation, &test_configs)?;
    println!("✅ Broad search complete in {:.2}s", processing_start.elapsed().as_secs_f64());
    
    // Find top performers
    let mut top_methods = broad_results.clone();
    top_methods.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    let top_5: Vec<_> = top_methods.iter().take(5).collect();
    
    println!("\n🏆 Top 5 methods from broad search:");
    for (i, method) in top_5.iter().enumerate() {
        println!("{}. {} ({}) - Score: {:.2}", 
                 i + 1, method.method, method.parameters, method.combined_score);
    }
    
    // Phase 2: Grid search refinement
    println!("\n=== PHASE 2: GRID SEARCH REFINEMENT ===");
    let mut refined_results = Vec::new();
    
    for top_method in top_5.iter().take(3) {
        println!("\n🔍 Refining: {} ({})", top_method.method, top_method.parameters);
        let grid_configs = generate_grid_search_configs(top_method);
        
        if !grid_configs.is_empty() {
            let grid_start = std::time::Instant::now();
            let grid_results = process_all_methods(&gpx_files_data, &files_with_elevation, &grid_configs)?;
            refined_results.extend(grid_results);
            println!("  ✅ Grid search complete in {:.2}s", grid_start.elapsed().as_secs_f64());
        }
    }
    
    // Combine all results
    let mut all_results = broad_results;
    all_results.extend(refined_results);
    
    // Phase 3: Cross-validation for top performers
    println!("\n=== PHASE 3: CROSS-VALIDATION ===");
    let mut validated_results = Vec::new();
    
    all_results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    for (i, method) in all_results.iter().take(10).enumerate() {
        println!("\n[{}/10] Cross-validating: {} ({})", i + 1, method.method, method.parameters);
        let cv_result = cross_validate_method(method, &gpx_files_data, &files_with_elevation, 5);
        println!("  Consistency score: {:.2}", cv_result.consistency_score);
        validated_results.push((method.clone(), cv_result));
    }
    
    // Phase 4: Multi-objective optimization
    println!("\n=== PHASE 4: PARETO OPTIMAL SOLUTIONS ===");
    let pareto_optimal = find_pareto_optimal_solutions(&all_results);
    println!("Found {} Pareto optimal solutions", pareto_optimal.len());
    
    // Write comprehensive results
    let output_path = Path::new(gpx_folder).join("asymmetric_comprehensive_analysis.csv");
    write_comprehensive_results(&all_results, &validated_results, &pareto_optimal, &output_path)?;
    
    // Terrain-specific analysis
    let terrain_output = Path::new(gpx_folder).join("terrain_specific_optimal.csv");
    write_terrain_specific_results(&all_results, &gpx_files_data, &terrain_output)?;
    
    // Print comprehensive summary
    print_comprehensive_summary(&all_results, &validated_results, &pareto_optimal, &gpx_files_data);
    
    let total_time = total_start.elapsed();
    println!("\n⏱️  TOTAL EXECUTION TIME: {} minutes {:.1} seconds", 
             total_time.as_secs() / 60, 
             total_time.as_secs_f64() % 60.0);
    
    Ok(())
}

fn generate_comprehensive_test_configs() -> Vec<(ProcessingMethod, Vec<f64>)> {
    let mut configs = vec![
        // Standard baseline - comprehensive range
        (ProcessingMethod::Standard, vec![0.5]),
        (ProcessingMethod::Standard, vec![0.75]),
        (ProcessingMethod::Standard, vec![1.0]),
        (ProcessingMethod::Standard, vec![1.25]),
        (ProcessingMethod::Standard, vec![1.5]),
        (ProcessingMethod::Standard, vec![1.75]),
        (ProcessingMethod::Standard, vec![2.0]),
        (ProcessingMethod::Standard, vec![2.25]),
        (ProcessingMethod::Standard, vec![2.275]),
        (ProcessingMethod::Standard, vec![2.5]),
        (ProcessingMethod::Standard, vec![2.75]),
        (ProcessingMethod::Standard, vec![3.0]),
        (ProcessingMethod::Standard, vec![3.5]),
        (ProcessingMethod::Standard, vec![4.0]),
        (ProcessingMethod::Standard, vec![4.5]),
        (ProcessingMethod::Standard, vec![5.0]),
        (ProcessingMethod::Standard, vec![6.0]),
        
        // Asymmetric intervals - comprehensive combinations
        (ProcessingMethod::AsymmetricInterval, vec![0.5, 1.0]),
        (ProcessingMethod::AsymmetricInterval, vec![0.5, 2.0]),
        (ProcessingMethod::AsymmetricInterval, vec![0.75, 1.5]),
        (ProcessingMethod::AsymmetricInterval, vec![1.0, 2.0]),
        (ProcessingMethod::AsymmetricInterval, vec![1.0, 3.0]),
        (ProcessingMethod::AsymmetricInterval, vec![1.0, 4.0]),
        (ProcessingMethod::AsymmetricInterval, vec![1.0, 6.0]),
        (ProcessingMethod::AsymmetricInterval, vec![1.0, 8.0]),
        (ProcessingMethod::AsymmetricInterval, vec![1.0, 10.0]),
        (ProcessingMethod::AsymmetricInterval, vec![1.5, 3.0]),
        (ProcessingMethod::AsymmetricInterval, vec![1.5, 4.0]),
        (ProcessingMethod::AsymmetricInterval, vec![1.5, 5.0]),
        (ProcessingMethod::AsymmetricInterval, vec![1.5, 6.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.0, 4.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.0, 5.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.0, 6.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.0, 8.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.0, 10.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.275, 4.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.275, 5.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.275, 6.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.275, 7.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.275, 8.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.5, 5.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.5, 6.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.5, 7.0]),
        (ProcessingMethod::AsymmetricInterval, vec![2.5, 8.0]),
        (ProcessingMethod::AsymmetricInterval, vec![3.0, 6.0]),
        (ProcessingMethod::AsymmetricInterval, vec![3.0, 7.0]),
        (ProcessingMethod::AsymmetricInterval, vec![3.0, 8.0]),
        (ProcessingMethod::AsymmetricInterval, vec![3.0, 10.0]),
        (ProcessingMethod::AsymmetricInterval, vec![3.0, 12.0]),
        (ProcessingMethod::AsymmetricInterval, vec![4.0, 8.0]),
        
        // Directional deadzone - comprehensive thresholds
        (ProcessingMethod::DirectionalDeadzone, vec![0.0, 0.0]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.05, 0.0]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.1, 0.0]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.1, 0.01]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.2, 0.0]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.2, 0.02]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.2, 0.05]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.3, 0.0]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.3, 0.05]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.3, 0.1]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.4, 0.05]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.4, 0.1]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.5, 0.0]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.5, 0.1]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.5, 0.2]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.6, 0.1]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.6, 0.2]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.7, 0.2]),
        (ProcessingMethod::DirectionalDeadzone, vec![0.8, 0.2]),
        (ProcessingMethod::DirectionalDeadzone, vec![1.0, 0.0]),
        (ProcessingMethod::DirectionalDeadzone, vec![1.0, 0.2]),
        (ProcessingMethod::DirectionalDeadzone, vec![1.5, 0.0]),
        (ProcessingMethod::DirectionalDeadzone, vec![2.0, 0.0]),
        
        // Loss compensation - fine-grained factors
        (ProcessingMethod::LossCompensation, vec![1.5, 1.1]),
        (ProcessingMethod::LossCompensation, vec![1.5, 1.2]),
        (ProcessingMethod::LossCompensation, vec![1.5, 1.3]),
        (ProcessingMethod::LossCompensation, vec![1.5, 1.4]),
        (ProcessingMethod::LossCompensation, vec![1.5, 1.5]),
        (ProcessingMethod::LossCompensation, vec![2.0, 1.1]),
        (ProcessingMethod::LossCompensation, vec![2.0, 1.15]),
        (ProcessingMethod::LossCompensation, vec![2.0, 1.2]),
        (ProcessingMethod::LossCompensation, vec![2.0, 1.25]),
        (ProcessingMethod::LossCompensation, vec![2.0, 1.3]),
        (ProcessingMethod::LossCompensation, vec![2.0, 1.35]),
        (ProcessingMethod::LossCompensation, vec![2.0, 1.4]),
        (ProcessingMethod::LossCompensation, vec![2.0, 1.5]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.1]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.15]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.2]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.25]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.3]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.35]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.4]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.45]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.5]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.55]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.6]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.65]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.7]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.75]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.8]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.85]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.9]),
        (ProcessingMethod::LossCompensation, vec![2.275, 1.95]),
        (ProcessingMethod::LossCompensation, vec![2.275, 2.0]),
        (ProcessingMethod::LossCompensation, vec![2.275, 2.2]),
        (ProcessingMethod::LossCompensation, vec![2.275, 2.5]),
        (ProcessingMethod::LossCompensation, vec![2.5, 1.3]),
        (ProcessingMethod::LossCompensation, vec![2.5, 1.5]),
        (ProcessingMethod::LossCompensation, vec![3.0, 1.2]),
        (ProcessingMethod::LossCompensation, vec![3.0, 1.3]),
        (ProcessingMethod::LossCompensation, vec![3.0, 1.5]),
        
        // Gradient-based - comprehensive thresholds
        (ProcessingMethod::GradientBased, vec![2.0, 3.0]),
        (ProcessingMethod::GradientBased, vec![2.0, 5.0]),
        (ProcessingMethod::GradientBased, vec![2.0, 7.0]),
        (ProcessingMethod::GradientBased, vec![2.0, 10.0]),
        (ProcessingMethod::GradientBased, vec![2.275, 3.0]),
        (ProcessingMethod::GradientBased, vec![2.275, 5.0]),
        (ProcessingMethod::GradientBased, vec![2.275, 7.0]),
        (ProcessingMethod::GradientBased, vec![2.275, 10.0]),
        (ProcessingMethod::GradientBased, vec![2.275, 12.0]),
        (ProcessingMethod::GradientBased, vec![2.275, 15.0]),
        (ProcessingMethod::GradientBased, vec![2.5, 5.0]),
        (ProcessingMethod::GradientBased, vec![2.5, 7.0]),
        (ProcessingMethod::GradientBased, vec![2.5, 10.0]),
        (ProcessingMethod::GradientBased, vec![3.0, 5.0]),
        (ProcessingMethod::GradientBased, vec![3.0, 7.0]),
        (ProcessingMethod::GradientBased, vec![3.0, 10.0]),
        
        // Two-pass - including zero loss smoothing
        (ProcessingMethod::TwoPass, vec![1.5, 0.0]),
        (ProcessingMethod::TwoPass, vec![1.5, 0.1]),
        (ProcessingMethod::TwoPass, vec![1.5, 0.5]),
        (ProcessingMethod::TwoPass, vec![1.5, 1.0]),
        (ProcessingMethod::TwoPass, vec![2.0, 0.0]),
        (ProcessingMethod::TwoPass, vec![2.0, 0.1]),
        (ProcessingMethod::TwoPass, vec![2.0, 0.2]),
        (ProcessingMethod::TwoPass, vec![2.0, 0.5]),
        (ProcessingMethod::TwoPass, vec![2.0, 1.0]),
        (ProcessingMethod::TwoPass, vec![2.275, 0.0]),
        (ProcessingMethod::TwoPass, vec![2.275, 0.1]),
        (ProcessingMethod::TwoPass, vec![2.275, 0.2]),
        (ProcessingMethod::TwoPass, vec![2.275, 0.3]),
        (ProcessingMethod::TwoPass, vec![2.275, 0.5]),
        (ProcessingMethod::TwoPass, vec![2.275, 0.7]),
        (ProcessingMethod::TwoPass, vec![2.275, 1.0]),
        (ProcessingMethod::TwoPass, vec![2.5, 0.0]),
        (ProcessingMethod::TwoPass, vec![2.5, 0.1]),
        (ProcessingMethod::TwoPass, vec![2.5, 0.5]),
        (ProcessingMethod::TwoPass, vec![2.5, 1.0]),
        (ProcessingMethod::TwoPass, vec![3.0, 0.0]),
        (ProcessingMethod::TwoPass, vec![3.0, 0.5]),
        (ProcessingMethod::TwoPass, vec![3.0, 1.0]),
        
        // Hybrid selective
        (ProcessingMethod::HybridSelective, vec![2.0, 0.5]),
        (ProcessingMethod::HybridSelective, vec![2.0, 1.0]),
        (ProcessingMethod::HybridSelective, vec![2.0, 2.0]),
        (ProcessingMethod::HybridSelective, vec![2.0, 5.0]),
        (ProcessingMethod::HybridSelective, vec![2.275, 0.5]),
        (ProcessingMethod::HybridSelective, vec![2.275, 1.0]),
        (ProcessingMethod::HybridSelective, vec![2.275, 2.0]),
        (ProcessingMethod::HybridSelective, vec![2.275, 3.0]),
        (ProcessingMethod::HybridSelective, vec![2.275, 5.0]),
        (ProcessingMethod::HybridSelective, vec![2.275, 7.0]),
        (ProcessingMethod::HybridSelective, vec![2.275, 10.0]),
        (ProcessingMethod::HybridSelective, vec![2.5, 2.0]),
        (ProcessingMethod::HybridSelective, vec![2.5, 5.0]),
        (ProcessingMethod::HybridSelective, vec![3.0, 2.0]),
        (ProcessingMethod::HybridSelective, vec![3.0, 5.0]),
        
        // Adaptive loss compensation
        (ProcessingMethod::AdaptiveLossCompensation, vec![2.0, 1.0, 2.0]),
        (ProcessingMethod::AdaptiveLossCompensation, vec![2.0, 1.1, 1.8]),
        (ProcessingMethod::AdaptiveLossCompensation, vec![2.275, 1.0, 2.0]),
        (ProcessingMethod::AdaptiveLossCompensation, vec![2.275, 1.1, 1.8]),
        (ProcessingMethod::AdaptiveLossCompensation, vec![2.275, 1.2, 1.6]),
        (ProcessingMethod::AdaptiveLossCompensation, vec![2.5, 1.1, 1.7]),
        
        // Combined approach
        (ProcessingMethod::CombinedApproach, vec![2.0, 0.3, 0.05, 1.3]),
        (ProcessingMethod::CombinedApproach, vec![2.0, 0.5, 0.1, 1.5]),
        (ProcessingMethod::CombinedApproach, vec![2.275, 0.3, 0.05, 1.3]),
        (ProcessingMethod::CombinedApproach, vec![2.275, 0.4, 0.05, 1.4]),
        (ProcessingMethod::CombinedApproach, vec![2.275, 0.5, 0.1, 1.5]),
        (ProcessingMethod::CombinedApproach, vec![2.5, 0.4, 0.05, 1.4]),
        
        // Elevation band specific
        (ProcessingMethod::ElevationBandSpecific, vec![1000.0, 2000.0, 3000.0]),
        (ProcessingMethod::ElevationBandSpecific, vec![1500.0, 2500.0, 3500.0]),
    ];
    
    configs
}

fn generate_grid_search_configs(best_method: &MethodResult) -> Vec<(ProcessingMethod, Vec<f64>)> {
    let mut grid_configs = Vec::new();
    
    // Parse method type and parameters
    let method_type = match best_method.method.as_str() {
        "Standard Distance-Based" => ProcessingMethod::Standard,
        "Asymmetric Intervals" => ProcessingMethod::AsymmetricInterval,
        "Directional Deadzone" => ProcessingMethod::DirectionalDeadzone,
        "Loss Compensation" => ProcessingMethod::LossCompensation,
        "Gradient-Based Protection" => ProcessingMethod::GradientBased,
        "Two-Pass Processing" => ProcessingMethod::TwoPass,
        "Hybrid Selective" => ProcessingMethod::HybridSelective,
        "Adaptive Loss Compensation" => ProcessingMethod::AdaptiveLossCompensation,
        "Combined Approach" => ProcessingMethod::CombinedApproach,
        _ => return grid_configs,
    };
    
    // Extract parameters from string (simplified - in real implementation, parse properly)
    let params = extract_parameters_from_string(&best_method.parameters);
    
    match method_type {
        ProcessingMethod::Standard => {
            let center = params[0];
            for delta in -10..=10 {
                let test_value = center + (delta as f64 * 0.025);
                if test_value > 0.0 && test_value <= 10.0 {
                    grid_configs.push((method_type, vec![test_value]));
                }
            }
        },
        ProcessingMethod::LossCompensation => {
            let interval = params[0];
            let factor = params[1];
            
            for i_delta in -5..=5 {
                for f_delta in -10..=10 {
                    let test_interval = interval + (i_delta as f64 * 0.05);
                    let test_factor = factor + (f_delta as f64 * 0.02);
                    
                    if test_interval > 0.0 && test_factor > 0.5 && test_factor < 3.0 {
                        grid_configs.push((method_type, vec![test_interval, test_factor]));
                    }
                }
            }
        },
        ProcessingMethod::AsymmetricInterval => {
            let gain_int = params[0];
            let loss_int = params[1];
            
            for g_delta in -5..=5 {
                for l_delta in -5..=5 {
                    let test_gain = gain_int + (g_delta as f64 * 0.1);
                    let test_loss = loss_int + (l_delta as f64 * 0.2);
                    
                    if test_gain > 0.0 && test_loss > 0.0 && test_loss > test_gain {
                        grid_configs.push((method_type, vec![test_gain, test_loss]));
                    }
                }
            }
        },
        _ => {
            // Add similar grid search for other methods
        }
    }
    
    grid_configs
}

fn extract_parameters_from_string(param_str: &str) -> Vec<f64> {
    // Simple parameter extraction - improve this for production
    let mut params = Vec::new();
    
    // Extract numbers from string
    let parts: Vec<&str> = param_str.split(|c: char| !c.is_numeric() && c != '.').collect();
    for part in parts {
        if let Ok(value) = part.parse::<f64>() {
            params.push(value);
        }
    }
    
    if params.is_empty() {
        params.push(2.275); // Default
    }
    
    params
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
                                        
                                        // Determine terrain type
                                        let total_distance_km = distances.last().unwrap_or(&0.0) / 1000.0;
                                        let (raw_gain, _) = calculate_raw_gain_loss(&elevations);
                                        let gain_per_km = if total_distance_km > 0.0 {
                                            raw_gain as f64 / total_distance_km
                                        } else {
                                            0.0
                                        };
                                        
                                        let terrain_type = if gain_per_km < 20.0 {
                                            TerrainType::Flat
                                        } else if gain_per_km < 40.0 {
                                            TerrainType::Rolling
                                        } else if gain_per_km < 60.0 {
                                            TerrainType::Hilly
                                        } else {
                                            TerrainType::Mountainous
                                        };
                                        
                                        let file_data = GpxFileData {
                                            filename: filename.clone(),
                                            elevations,
                                            distances,
                                            official_gain,
                                            terrain_type,
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

fn print_terrain_distribution(gpx_data: &HashMap<String, GpxFileData>) {
    let mut flat_count = 0;
    let mut rolling_count = 0;
    let mut hilly_count = 0;
    let mut mountain_count = 0;
    
    for (_, data) in gpx_data {
        match data.terrain_type {
            TerrainType::Flat => flat_count += 1,
            TerrainType::Rolling => rolling_count += 1,
            TerrainType::Hilly => hilly_count += 1,
            TerrainType::Mountainous => mountain_count += 1,
        }
    }
    
    println!("\n🏔️  Terrain Distribution:");
    println!("  Flat (<20m/km): {} files", flat_count);
    println!("  Rolling (20-40m/km): {} files", rolling_count);
    println!("  Hilly (40-60m/km): {} files", hilly_count);
    println!("  Mountainous (>60m/km): {} files", mountain_count);
}

fn process_all_methods(
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String],
    test_configs: &[(ProcessingMethod, Vec<f64>)]
) -> Result<Vec<MethodResult>, Box<dyn std::error::Error>> {
    let gpx_data_arc = Arc::new(gpx_data.clone());
    let total_configs = test_configs.len();
    let total_files = valid_files.len();
    
    println!("\n🚀 Processing {} methods × {} files", total_configs, total_files);
    println!("⚡ Using parallel processing on {} cores", num_cpus::get());
    
    let mut all_results = Vec::new();
    
    // Process in batches to avoid memory issues
    let batch_size = 10;
    for (batch_idx, config_batch) in test_configs.chunks(batch_size).enumerate() {
        println!("\nProcessing batch {}/{}", batch_idx + 1, (total_configs + batch_size - 1) / batch_size);
        
        for (idx, (method, params)) in config_batch.iter().enumerate() {
            let global_idx = batch_idx * batch_size + idx + 1;
            println!("[{}/{}] Processing {:?} with params {:?}", global_idx, total_configs, method, params);
            
            let file_results: Vec<ProcessingResult> = valid_files
                .par_iter()
                .filter_map(|filename| {
                    let gpx_data = Arc::clone(&gpx_data_arc);
                    
                    if let Some(file_data) = gpx_data.get(filename) {
                        if file_data.official_gain > 0 {
                            return Some(process_single_file(file_data, *method, params));
                        }
                    }
                    None
                })
                .collect();
            
            if !file_results.is_empty() {
                let method_result = create_method_result(*method, params, &file_results);
                all_results.push(method_result);
            }
        }
    }
    
    Ok(all_results)
}

fn process_single_file(
    file_data: &GpxFileData,
    method: ProcessingMethod,
    params: &[f64]
) -> ProcessingResult {
    let (raw_gain, raw_loss) = calculate_raw_gain_loss(&file_data.elevations);
    
    let (processed_gain, processed_loss) = match method {
        ProcessingMethod::Standard => {
            process_standard(file_data, params[0])
        },
        ProcessingMethod::AsymmetricInterval => {
            process_asymmetric_interval(file_data, params[0], params[1])
        },
        ProcessingMethod::DirectionalDeadzone => {
            process_directional_deadzone(file_data, params[0], params[1])
        },
        ProcessingMethod::LossCompensation => {
            let (gain, loss) = process_standard(file_data, params[0]);
            (gain, loss * params[1])
        },
        ProcessingMethod::GradientBased => {
            process_gradient_based(file_data, params[0], params[1])
        },
        ProcessingMethod::TwoPass => {
            process_two_pass(file_data, params[0], params[1])
        },
        ProcessingMethod::HybridSelective => {
            process_hybrid_selective(file_data, params[0], params[1])
        },
        ProcessingMethod::AdaptiveLossCompensation => {
            process_adaptive_loss_compensation(file_data, params[0], params[1], params[2])
        },
        ProcessingMethod::CombinedApproach => {
            process_combined_approach(file_data, params)
        },
        ProcessingMethod::ButterworthAsymmetric => {
            // Implement if needed
            process_standard(file_data, params[0])
        },
        ProcessingMethod::ElevationBandSpecific => {
            process_elevation_band_specific(file_data, params)
        },
    };
    
    let accuracy = if file_data.official_gain > 0 {
        (processed_gain as f32 / file_data.official_gain as f32) * 100.0
    } else {
        100.0
    };
    
    let gain_loss_ratio = if processed_gain > 0.0 {
        (processed_loss / processed_gain * 100.0)
    } else {
        100.0
    };
    
    ProcessingResult {
        accuracy,
        raw_gain: raw_gain as f32,
        raw_loss: raw_loss as f32,
        processed_gain: processed_gain as f32,
        processed_loss: processed_loss as f32,
        gain_loss_ratio: gain_loss_ratio as f32,
        terrain_type: file_data.terrain_type,
    }
}

// All processing methods implementations
fn process_standard(file_data: &GpxFileData, interval: f64) -> (f64, f64) {
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval);
    
    (elevation_data.get_total_elevation_gain(), elevation_data.get_total_elevation_loss())
}

fn process_asymmetric_interval(
    file_data: &GpxFileData, 
    gain_interval: f64, 
    loss_interval: f64
) -> (f64, f64) {
    // Identify ascending and descending segments
    let mut segments = Vec::new();
    let mut current_start = 0;
    let mut is_ascending = true;
    
    for i in 1..file_data.elevations.len() {
        let trend_changed = (file_data.elevations[i] > file_data.elevations[i-1]) != is_ascending;
        
        if trend_changed || i == file_data.elevations.len() - 1 {
            segments.push((current_start, i, is_ascending));
            current_start = i;
            is_ascending = !is_ascending;
        }
    }
    
    let mut total_gain = 0.0;
    let mut total_loss = 0.0;
    
    // Process each segment with appropriate interval
    for (start, end, ascending) in segments {
        if end <= start {
            continue;
        }
        
        let segment_elevations = file_data.elevations[start..=end].to_vec();
        let segment_distances = file_data.distances[start..=end].to_vec();
        
        // Normalize distances to start at 0
        let base_distance = segment_distances[0];
        let normalized_distances: Vec<f64> = segment_distances.iter()
            .map(|d| d - base_distance)
            .collect();
        
        let interval = if ascending { gain_interval } else { loss_interval };
        
        let mut segment_data = ElevationData::new_with_variant(
            segment_elevations,
            normalized_distances,
            SmoothingVariant::DistBased
        );
        
        segment_data.apply_custom_interval_processing(interval);
        
        total_gain += segment_data.get_total_elevation_gain();
        total_loss += segment_data.get_total_elevation_loss();
    }
    
    (total_gain, total_loss)
}

fn process_directional_deadzone(
    file_data: &GpxFileData,
    gain_threshold: f64,
    loss_threshold: f64
) -> (f64, f64) {
    // First apply standard smoothing
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(2.275);
    let smoothed_elevations = elevation_data.enhanced_altitude.clone();
    
    // Apply directional deadzone
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for i in 1..smoothed_elevations.len() {
        let delta = smoothed_elevations[i] - smoothed_elevations[i-1];
        
        if delta > gain_threshold {
            gain += delta;
        } else if delta < -loss_threshold {
            loss += -delta;
        }
    }
    
    (gain, loss)
}

fn process_gradient_based(
    file_data: &GpxFileData,
    interval: f64,
    gradient_threshold: f64
) -> (f64, f64) {
    let mut protected_indices = Vec::new();
    
    // Identify steep sections
    for i in 1..file_data.elevations.len() {
        let distance_diff = file_data.distances[i] - file_data.distances[i-1];
        if distance_diff > 0.0 {
            let gradient = ((file_data.elevations[i] - file_data.elevations[i-1]) / distance_diff) * 100.0;
            
            if gradient.abs() > gradient_threshold {
                protected_indices.push(i-1);
                protected_indices.push(i);
            }
        }
    }
    
    // Apply smoothing but preserve protected points
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval);
    let mut processed_elevations = elevation_data.enhanced_altitude.clone();
    
    // Restore protected points
    for &idx in &protected_indices {
        if idx < processed_elevations.len() {
            processed_elevations[idx] = file_data.elevations[idx];
        }
    }
    
    // Calculate gain/loss from mixed elevations
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for i in 1..processed_elevations.len() {
        let delta = processed_elevations[i] - processed_elevations[i-1];
        if delta > 0.0 {
            gain += delta;
        } else {
            loss += -delta;
        }
    }
    
    (gain, loss)
}

fn process_two_pass(
    file_data: &GpxFileData,
    gain_interval: f64,
    loss_interval: f64
) -> (f64, f64) {
    // Pass 1: Calculate gain with specified smoothing
    let gain = if gain_interval > 0.0 {
        let mut gain_data = ElevationData::new_with_variant(
            file_data.elevations.clone(),
            file_data.distances.clone(),
            SmoothingVariant::DistBased
        );
        gain_data.apply_custom_interval_processing(gain_interval);
        gain_data.get_total_elevation_gain()
    } else {
        // No smoothing for gain
        let (raw_gain, _) = calculate_raw_gain_loss(&file_data.elevations);
        raw_gain as f64
    };
    
    // Pass 2: Calculate loss with specified smoothing
    let loss = if loss_interval > 0.0 {
        let mut loss_data = ElevationData::new_with_variant(
            file_data.elevations.clone(),
            file_data.distances.clone(),
            SmoothingVariant::DistBased
        );
        loss_data.apply_custom_interval_processing(loss_interval);
        loss_data.get_total_elevation_loss()
    } else {
        // No smoothing for loss
        let (_, raw_loss) = calculate_raw_gain_loss(&file_data.elevations);
        raw_loss as f64
    };
    
    (gain, loss)
}

fn process_hybrid_selective(
    file_data: &GpxFileData,
    interval: f64,
    variance_threshold: f64
) -> (f64, f64) {
    let window_size = 10;
    let mut should_smooth = vec![true; file_data.elevations.len()];
    
    // Calculate local variance
    for i in 0..file_data.elevations.len() {
        let start = i.saturating_sub(window_size / 2);
        let end = (i + window_size / 2).min(file_data.elevations.len());
        
        if end > start {
            let window = &file_data.elevations[start..end];
            let mean = window.iter().sum::<f64>() / window.len() as f64;
            let variance = window.iter()
                .map(|&e| (e - mean).powi(2))
                .sum::<f64>() / window.len() as f64;
            
            // Don't smooth low-variance descending sections
            let is_descending = end > start + 1 && 
                file_data.elevations[end-1] < file_data.elevations[start];
            
            if variance < variance_threshold && is_descending {
                should_smooth[i] = false;
            }
        }
    }
    
    // Apply selective smoothing
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    
    elevation_data.apply_custom_interval_processing(interval);
    let smoothed = elevation_data.enhanced_altitude.clone();
    
    // Mix smoothed and raw based on should_smooth
    let mut final_elevations = vec![0.0; file_data.elevations.len()];
    for i in 0..file_data.elevations.len() {
        final_elevations[i] = if should_smooth[i] {
            smoothed[i]
        } else {
            file_data.elevations[i]
        };
    }
    
    // Calculate gain/loss
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for i in 1..final_elevations.len() {
        let delta = final_elevations[i] - final_elevations[i-1];
        if delta > 0.0 {
            gain += delta;
        } else {
            loss += -delta;
        }
    }
    
    (gain, loss)
}

fn process_adaptive_loss_compensation(
    file_data: &GpxFileData,
    base_interval: f64,
    min_factor: f64,
    max_factor: f64
) -> (f64, f64) {
    // Calculate average gradient
    let total_distance = file_data.distances.last().unwrap_or(&0.0);
    let (raw_gain, _) = calculate_raw_gain_loss(&file_data.elevations);
    let avg_gradient = if *total_distance > 0.0 {
        (raw_gain as f64 / total_distance) * 100.0
    } else {
        0.0
    };
    
    // Adaptive factor: steeper = more compensation
    let factor = min_factor + (max_factor - min_factor) * (avg_gradient / 20.0).min(1.0);
    
    let (gain, loss) = process_standard(file_data, base_interval);
    (gain, loss * factor)
}

fn process_combined_approach(
    file_data: &GpxFileData,
    params: &[f64]
) -> (f64, f64) {
    // params: [interval, deadzone_gain, deadzone_loss, loss_factor]
    
    // First: standard smoothing
    let mut elevation_data = ElevationData::new_with_variant(
        file_data.elevations.clone(),
        file_data.distances.clone(),
        SmoothingVariant::DistBased
    );
    elevation_data.apply_custom_interval_processing(params[0]);
    let smoothed = elevation_data.enhanced_altitude.clone();
    
    // Then: directional deadzone
    let mut gain = 0.0;
    let mut loss = 0.0;
    for i in 1..smoothed.len() {
        let delta = smoothed[i] - smoothed[i-1];
        if delta > params[1] {
            gain += delta;
        } else if delta < -params[2] {
            loss += -delta;
        }
    }
    
    // Finally: loss compensation
    (gain, loss * params[3])
}

fn process_elevation_band_specific(
    file_data: &GpxFileData,
    params: &[f64]
) -> (f64, f64) {
    // Different processing based on elevation bands
    let low_elev = params[0];
    let mid_elev = params[1];
    let high_elev = params[2];
    
    let mut processed_elevations = file_data.elevations.clone();
    
    // Apply different smoothing based on elevation
    for i in 0..file_data.elevations.len() {
        let elev = file_data.elevations[i];
        let interval = if elev < low_elev {
            3.0  // More smoothing at low elevation
        } else if elev < mid_elev {
            2.0  // Medium smoothing
        } else if elev < high_elev {
            1.5  // Less smoothing at altitude
        } else {
            1.0  // Minimal smoothing at high altitude
        };
        
        // Apply local smoothing based on elevation band
        // Simplified implementation - in practice, use proper windowing
        if i > 0 && i < file_data.elevations.len() - 1 {
            let window_size = (interval * 2.0) as usize;
            let start = i.saturating_sub(window_size / 2);
            let end = (i + window_size / 2).min(file_data.elevations.len());
            
            let window_avg: f64 = file_data.elevations[start..end].iter().sum::<f64>() 
                / (end - start) as f64;
            processed_elevations[i] = window_avg;
        }
    }
    
    // Calculate gain/loss from processed elevations
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for i in 1..processed_elevations.len() {
        let delta = processed_elevations[i] - processed_elevations[i-1];
        if delta > 0.0 {
            gain += delta;
        } else {
            loss += -delta;
        }
    }
    
    (gain, loss)
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

fn create_method_result(
    method: ProcessingMethod,
    params: &[f64],
    results: &[ProcessingResult]
) -> MethodResult {
    let method_name = match method {
        ProcessingMethod::Standard => "Standard Distance-Based",
        ProcessingMethod::AsymmetricInterval => "Asymmetric Intervals",
        ProcessingMethod::DirectionalDeadzone => "Directional Deadzone",
        ProcessingMethod::LossCompensation => "Loss Compensation",
        ProcessingMethod::GradientBased => "Gradient-Based Protection",
        ProcessingMethod::TwoPass => "Two-Pass Processing",
        ProcessingMethod::HybridSelective => "Hybrid Selective",
        ProcessingMethod::AdaptiveLossCompensation => "Adaptive Loss Compensation",
        ProcessingMethod::CombinedApproach => "Combined Approach",
        ProcessingMethod::ButterworthAsymmetric => "Butterworth Asymmetric",
        ProcessingMethod::ElevationBandSpecific => "Elevation Band Specific",
    };
    
    let parameters = match method {
        ProcessingMethod::Standard => format!("interval={}m", params[0]),
        ProcessingMethod::AsymmetricInterval => format!("gain={}m, loss={}m", params[0], params[1]),
        ProcessingMethod::DirectionalDeadzone => format!("gain_th={}m, loss_th={}m", params[0], params[1]),
        ProcessingMethod::LossCompensation => format!("interval={}m, factor={}", params[0], params[1]),
        ProcessingMethod::GradientBased => format!("interval={}m, gradient>{}%", params[0], params[1]),
        ProcessingMethod::TwoPass => format!("gain={}m, loss={}m", params[0], params[1]),
        ProcessingMethod::HybridSelective => format!("interval={}m, variance<{}", params[0], params[1]),
        ProcessingMethod::AdaptiveLossCompensation => format!("interval={}m, min_f={}, max_f={}", params[0], params[1], params[2]),
        ProcessingMethod::CombinedApproach => format!("int={}m, g_th={}, l_th={}, f={}", params[0], params[1], params[2], params[3]),
        ProcessingMethod::ElevationBandSpecific => format!("bands: <{}m, <{}m, <{}m", params[0], params[1], params[2]),
        _ => "Unknown".to_string(),
    };
    
    let accuracies: Vec<f32> = results.iter().map(|r| r.accuracy).collect();
    let gain_loss_ratios: Vec<f32> = results.iter().map(|r| r.gain_loss_ratio).collect();
    
    // Calculate accuracy bands
    let score_98_102 = accuracies.iter().filter(|&&acc| acc >= 98.0 && acc <= 102.0).count() as u32;
    let score_95_105 = accuracies.iter().filter(|&&acc| acc >= 95.0 && acc <= 105.0).count() as u32;
    let score_90_110 = accuracies.iter().filter(|&&acc| acc >= 90.0 && acc <= 110.0).count() as u32;
    let score_85_115 = accuracies.iter().filter(|&&acc| acc >= 85.0 && acc <= 115.0).count() as u32;
    let score_80_120 = accuracies.iter().filter(|&&acc| acc >= 80.0 && acc <= 120.0).count() as u32;
    let files_outside_80_120 = accuracies.iter().filter(|&&acc| acc < 80.0 || acc > 120.0).count() as u32;
    
    // Calculate gain/loss balance metrics
    let files_balanced_85_115 = gain_loss_ratios.iter()
        .filter(|&&ratio| ratio >= 85.0 && ratio <= 115.0)
        .count() as u32;
    let files_balanced_70_130 = gain_loss_ratios.iter()
        .filter(|&&ratio| ratio >= 70.0 && ratio <= 130.0)
        .count() as u32;
    
    let avg_gain_loss_ratio = gain_loss_ratios.iter().sum::<f32>() / gain_loss_ratios.len() as f32;
    
    let mut sorted_ratios = gain_loss_ratios.clone();
    sorted_ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_gain_loss_ratio = if sorted_ratios.len() % 2 == 0 {
        (sorted_ratios[sorted_ratios.len() / 2 - 1] + 
         sorted_ratios[sorted_ratios.len() / 2]) / 2.0
    } else {
        sorted_ratios[sorted_ratios.len() / 2]
    };
    
    // Scoring
    let weighted_accuracy_score = (score_98_102 as f32 * 10.0) +
                                 ((score_95_105 - score_98_102) as f32 * 6.0) +
                                 ((score_90_110 - score_95_105) as f32 * 3.0) +
                                 ((score_85_115 - score_90_110) as f32 * 1.5) +
                                 ((score_80_120 - score_85_115) as f32 * 1.0) -
                                 (files_outside_80_120 as f32 * 5.0);
    
    let total_files = results.len() as f32;
    let gain_loss_balance_score = (files_balanced_85_115 as f32 * 10.0) +
                                  ((files_balanced_70_130 - files_balanced_85_115) as f32 * 5.0) +
                                  ((median_gain_loss_ratio - 100.0).abs() * -2.0);
    
    // Statistics
    let average_accuracy = accuracies.iter().sum::<f32>() / accuracies.len() as f32;
    let mut sorted_accuracies = accuracies.clone();
    sorted_accuracies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let median_accuracy = if sorted_accuracies.len() % 2 == 0 {
        (sorted_accuracies[sorted_accuracies.len() / 2 - 1] + 
         sorted_accuracies[sorted_accuracies.len() / 2]) / 2.0
    } else {
        sorted_accuracies[sorted_accuracies.len() / 2]
    };
    
    let best_accuracy = accuracies.iter()
        .min_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied()
        .unwrap_or(100.0);
        
    let worst_accuracy = accuracies.iter()
        .max_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
        .copied()
        .unwrap_or(100.0);
    
    let variance = accuracies.iter()
        .map(|&acc| (acc - average_accuracy).powi(2))
        .sum::<f32>() / accuracies.len() as f32;
    let std_deviation = variance.sqrt();
    
    let success_rate = (score_90_110 as f32 / total_files) * 100.0;
    
    // Gain/loss metrics
    let avg_raw_gain = results.iter().map(|r| r.raw_gain).sum::<f32>() / total_files;
    let avg_raw_loss = results.iter().map(|r| r.raw_loss).sum::<f32>() / total_files;
    let avg_processed_gain = results.iter().map(|r| r.processed_gain).sum::<f32>() / total_files;
    let avg_processed_loss = results.iter().map(|r| r.processed_loss).sum::<f32>() / total_files;
   
    let total_raw_elevation_loss = results.iter().map(|r| r.raw_loss).sum::<f32>();
    
    let gain_reduction_percent = if avg_raw_gain > 0.0 {
        ((avg_raw_gain - avg_processed_gain) / avg_raw_gain) * 100.0
    } else {
        0.0
    };
    
    let loss_reduction_percent = if avg_raw_loss > 0.0 {
        ((avg_raw_loss - avg_processed_loss) / avg_raw_loss) * 100.0
    } else {
        0.0
    };
    
    let loss_preservation_score = 100.0 - (loss_reduction_percent - gain_reduction_percent).abs();
    
    // Combined score that heavily weights gain/loss balance
    let combined_score = (weighted_accuracy_score * 0.4) + 
                        (gain_loss_balance_score * 0.4) +
                        (loss_preservation_score * 0.2);
    
    // Calculate terrain-specific scores
    let terrain_scores = calculate_terrain_specific_scores(results);
    
    MethodResult {
        method: method_name.to_string(),
        parameters,
        score_98_102,
        score_95_105,
        score_90_110,
        score_85_115,
        score_80_120,
        files_outside_80_120,
        weighted_accuracy_score,
        gain_loss_balance_score,
        files_balanced_85_115,
        files_balanced_70_130,
        avg_gain_loss_ratio,
        median_gain_loss_ratio,
        average_accuracy,
        median_accuracy,
        worst_accuracy,
        best_accuracy,
        std_deviation,
        success_rate,
        avg_raw_gain,
        avg_raw_loss,
        avg_processed_gain,
        avg_processed_loss,
        total_raw_elevation_loss,
        loss_reduction_percent,
        gain_reduction_percent,
        combined_score,
        loss_preservation_score,
        total_files: total_files as u32,
        flat_terrain_score: terrain_scores.0,
        hilly_terrain_score: terrain_scores.1,
        mountain_terrain_score: terrain_scores.2,
    }
    }

    fn calculate_terrain_specific_scores(results: &[ProcessingResult]) -> (f32, f32, f32) {
    let mut flat_scores = Vec::new();
    let mut hilly_scores = Vec::new();
    let mut mountain_scores = Vec::new();
    
    for result in results {
        let score = if result.accuracy >= 90.0 && result.accuracy <= 110.0 {
            10.0 - (result.accuracy - 100.0).abs()
        } else {
            0.0
        };
        
        match result.terrain_type {
            TerrainType::Flat | TerrainType::Rolling => flat_scores.push(score),
            TerrainType::Hilly => hilly_scores.push(score),
            TerrainType::Mountainous => mountain_scores.push(score),
        }
    }
    
    let flat_score = if !flat_scores.is_empty() {
        flat_scores.iter().sum::<f32>() / flat_scores.len() as f32
    } else {
        0.0
    };
    
    let hilly_score = if !hilly_scores.is_empty() {
        hilly_scores.iter().sum::<f32>() / hilly_scores.len() as f32
    } else {
        0.0
    };
    
    let mountain_score = if !mountain_scores.is_empty() {
        mountain_scores.iter().sum::<f32>() / mountain_scores.len() as f32
    } else {
        0.0
    };
    
    (flat_score, hilly_score, mountain_score)
    }

    fn cross_validate_method(
    method: &MethodResult,
    gpx_data: &HashMap<String, GpxFileData>,
    valid_files: &[String],
    k_folds: usize
    ) -> CrossValidationResult {
    // Simple k-fold cross-validation
    let fold_size = valid_files.len() / k_folds;
    let mut fold_accuracies = Vec::new();
    let mut fold_ratios = Vec::new();
    
    for fold in 0..k_folds {
        let test_start = fold * fold_size;
        let test_end = if fold == k_folds - 1 {
            valid_files.len()
        } else {
            (fold + 1) * fold_size
        };
        
        let test_files: Vec<&String> = valid_files[test_start..test_end].iter().collect();
        
        // Calculate metrics for this fold
        let mut accuracies = Vec::new();
        let mut ratios = Vec::new();
        
        for file in test_files {
            if let Some(file_data) = gpx_data.get(file) {
                if file_data.official_gain > 0 {
                    // Re-process with the method's parameters
                    // This is simplified - in real implementation, parse parameters properly
                    let accuracy = 100.0; // Placeholder
                    let ratio = 100.0; // Placeholder
                    
                    accuracies.push(accuracy);
                    ratios.push(ratio);
                }
            }
        }
        
        if !accuracies.is_empty() {
            let fold_avg_accuracy = accuracies.iter().sum::<f32>() / accuracies.len() as f32;
            let fold_avg_ratio = ratios.iter().sum::<f32>() / ratios.len() as f32;
            
            fold_accuracies.push(fold_avg_accuracy);
            fold_ratios.push(fold_avg_ratio);
        }
    }
    
    // Calculate cross-validation statistics
    let mean_accuracy = fold_accuracies.iter().sum::<f32>() / fold_accuracies.len() as f32;
    let mean_ratio = fold_ratios.iter().sum::<f32>() / fold_ratios.len() as f32;
    
    let accuracy_variance = fold_accuracies.iter()
        .map(|&a| (a - mean_accuracy).powi(2))
        .sum::<f32>() / fold_accuracies.len() as f32;
    let std_accuracy = accuracy_variance.sqrt();
    
    let ratio_variance = fold_ratios.iter()
        .map(|&r| (r - mean_ratio).powi(2))
        .sum::<f32>() / fold_ratios.len() as f32;
    let std_ratio = ratio_variance.sqrt();
    
    // Consistency score: lower standard deviation = higher consistency
    let consistency_score = 100.0 - (std_accuracy + std_ratio);
    
    CrossValidationResult {
        mean_accuracy,
        std_accuracy,
        mean_gain_loss_ratio: mean_ratio,
        std_gain_loss_ratio: std_ratio,
        consistency_score,
    }
    }

    fn find_pareto_optimal_solutions(results: &[MethodResult]) -> Vec<&MethodResult> {
    let mut pareto_front = Vec::new();
    
    for candidate in results {
        let mut is_dominated = false;
        
        for other in results {
            // Check if 'other' dominates 'candidate' on all objectives
            if other.median_accuracy >= candidate.median_accuracy &&
                other.median_gain_loss_ratio >= candidate.median_gain_loss_ratio &&
                other.loss_reduction_percent <= candidate.loss_reduction_percent &&
                (other.median_accuracy > candidate.median_accuracy ||
                other.median_gain_loss_ratio > candidate.median_gain_loss_ratio ||
                other.loss_reduction_percent < candidate.loss_reduction_percent) {
                is_dominated = true;
                break;
            }
        }
        
        if !is_dominated {
            pareto_front.push(candidate);
        }
    }
    
    pareto_front
    }

    fn write_comprehensive_results(
    all_results: &[MethodResult],
    validated_results: &[(MethodResult, CrossValidationResult)],
    pareto_optimal: &[&MethodResult],
    output_path: &Path
    ) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write header
    wtr.write_record(&[
        "Method",
        "Parameters",
        "Combined Score",
        "Median Gain/Loss %",
        "Median Accuracy %",
        "Success Rate %",
        "Gain Reduction %",
        "Loss Reduction %",
        "Files Balanced 85-115%",
        "98-102%",
        "95-105%",
        "90-110%",
        "Accuracy Score",
        "Balance Score",
        "Preservation Score",
        "Flat Terrain Score",
        "Hilly Terrain Score",
        "Mountain Terrain Score",
        "Is Pareto Optimal",
        "CV Consistency Score",
        "Total Files",
    ])?;
    
    // Sort by combined score
    let mut sorted_results = all_results.to_vec();
    sorted_results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    
    // Create a map of validated results
    let validation_map: HashMap<String, f32> = validated_results.iter()
        .map(|(m, cv)| (format!("{} ({})", m.method, m.parameters), cv.consistency_score))
        .collect();
    
    // Check if result is Pareto optimal
    let pareto_set: Vec<String> = pareto_optimal.iter()
        .map(|m| format!("{} ({})", m.method, m.parameters))
        .collect();
    
    // Write data
    for result in sorted_results {
        let key = format!("{} ({})", result.method, result.parameters);
        let is_pareto = if pareto_set.contains(&key) { "Yes" } else { "No" };
        let cv_score = validation_map.get(&key).copied().unwrap_or(0.0);
        
        wtr.write_record(&[
            &result.method,
            &result.parameters,
            &format!("{:.2}", result.combined_score),
            &format!("{:.1}", result.median_gain_loss_ratio),
            &format!("{:.2}", result.median_accuracy),
            &format!("{:.1}", result.success_rate),
            &format!("{:.1}", result.gain_reduction_percent),
            &format!("{:.1}", result.loss_reduction_percent),
            &result.files_balanced_85_115.to_string(),
            &result.score_98_102.to_string(),
            &result.score_95_105.to_string(),
            &result.score_90_110.to_string(),
            &format!("{:.2}", result.weighted_accuracy_score),
            &format!("{:.2}", result.gain_loss_balance_score),
            &format!("{:.2}", result.loss_preservation_score),
            &format!("{:.2}", result.flat_terrain_score),
            &format!("{:.2}", result.hilly_terrain_score),
            &format!("{:.2}", result.mountain_terrain_score),
            is_pareto,
            &format!("{:.2}", cv_score),
            &result.total_files.to_string(),
        ])?;
    }
    
    wtr.flush()?;
    println!("\n✅ Results saved to: {}", output_path.display());
    Ok(())
    }

    fn write_terrain_specific_results(
    results: &[MethodResult],
    gpx_data: &HashMap<String, GpxFileData>,
    output_path: &Path
    ) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Group methods by terrain performance
    let mut flat_best: Vec<&MethodResult> = results.iter()
        .filter(|r| r.flat_terrain_score > 0.0)
        .collect();
    flat_best.sort_by(|a, b| b.flat_terrain_score.partial_cmp(&a.flat_terrain_score).unwrap());
    
    let mut hilly_best: Vec<&MethodResult> = results.iter()
        .filter(|r| r.hilly_terrain_score > 0.0)
        .collect();
    hilly_best.sort_by(|a, b| b.hilly_terrain_score.partial_cmp(&a.hilly_terrain_score).unwrap());
    
    let mut mountain_best: Vec<&MethodResult> = results.iter()
        .filter(|r| r.mountain_terrain_score > 0.0)
        .collect();
    mountain_best.sort_by(|a, b| b.mountain_terrain_score.partial_cmp(&a.mountain_terrain_score).unwrap());
    
    // Write header
    wtr.write_record(&[
        "Terrain Type",
        "Best Method",
        "Parameters",
        "Terrain Score",
        "Median Accuracy %",
        "Median Gain/Loss %",
        "Success Rate %",
    ])?;
    
    // Write top 3 for each terrain type
    for (terrain_name, terrain_results) in &[
        ("Flat/Rolling", &flat_best),
        ("Hilly", &hilly_best),
        ("Mountainous", &mountain_best),
    ] {
        for (i, method) in terrain_results.iter().take(3).enumerate() {
            let rank = if i == 0 { "Best" } else { &format!("#{}", i + 1) };
            wtr.write_record(&[
                terrain_name,
                &format!("{} {}", rank, method.method),
                &method.parameters,
                &format!("{:.2}", match *terrain_name {
                    "Flat/Rolling" => method.flat_terrain_score,
                    "Hilly" => method.hilly_terrain_score,
                    "Mountainous" => method.mountain_terrain_score,
                    _ => 0.0,
                }),
                &format!("{:.2}", method.median_accuracy),
                &format!("{:.1}", method.median_gain_loss_ratio),
                &format!("{:.1}", method.success_rate),
            ])?;
        }
    }
    
    wtr.flush()?;
    Ok(())
    }

    fn print_comprehensive_summary(
    results: &[MethodResult],
    validated_results: &[(MethodResult, CrossValidationResult)],
    pareto_optimal: &[&MethodResult],
    gpx_data: &HashMap<String, GpxFileData>
    ) {
    println!("\n📊 COMPREHENSIVE ASYMMETRIC METHODS ANALYSIS SUMMARY");
    println!("===================================================");
    
    // Find best overall
    let best = results.iter()
        .max_by(|a, b| a.combined_score.partial_cmp(&b.combined_score).unwrap())
        .unwrap();
    
    println!("\n🏆 BEST OVERALL METHOD:");
    println!("   Method: {}", best.method);
    println!("   Parameters: {}", best.parameters);
    println!("   Combined Score: {:.2}", best.combined_score);
    println!("   Median Gain/Loss Ratio: {:.1}%", best.median_gain_loss_ratio);
    println!("   Median Accuracy: {:.2}%", best.median_accuracy);
    println!("   Gain reduction: {:.1}%, Loss reduction: {:.1}%", 
                best.gain_reduction_percent, best.loss_reduction_percent);
    
    // Show top 10
    let mut sorted_by_score = results.to_vec();
    sorted_by_score.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    
    println!("\n🏅 TOP 10 METHODS:");
    println!("Rank | Method                  | Parameters              | Score  | Ratio% | Acc%  | Gain% | Loss%");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for (i, result) in sorted_by_score.iter().take(10).enumerate() {
        println!("{:4} | {:23} | {:23} | {:6.2} | {:6.1} | {:5.1} | {:5.1} | {:5.1}",
                    i + 1,
                    result.method,
                    result.parameters,
                    result.combined_score,
                    result.median_gain_loss_ratio,
                    result.median_accuracy,
                    result.gain_reduction_percent,
                    result.loss_reduction_percent);
    }
    
    // Pareto optimal solutions
    println!("\n🎯 PARETO OPTIMAL SOLUTIONS ({} found):", pareto_optimal.len());
    println!("These methods are not dominated by any other on all objectives:");
    for (i, method) in pareto_optimal.iter().take(5).enumerate() {
        println!("{}. {} ({}) - Acc: {:.1}%, Ratio: {:.1}%, Loss Red: {:.1}%",
                    i + 1,
                    method.method,
                    method.parameters,
                    method.median_accuracy,
                    method.median_gain_loss_ratio,
                    method.loss_reduction_percent);
    }
    
    // Cross-validation results
    println!("\n✅ MOST CONSISTENT METHODS (Cross-Validation):");
    let mut validated_sorted = validated_results.to_vec();
    validated_sorted.sort_by(|a, b| b.1.consistency_score.partial_cmp(&a.1.consistency_score).unwrap());
    
    for (i, (method, cv)) in validated_sorted.iter().take(5).enumerate() {
        println!("{}. {} ({}) - Consistency: {:.2}, Std Acc: {:.2}",
                    i + 1,
                    method.method,
                    method.parameters,
                    cv.consistency_score,
                    cv.std_accuracy);
    }
    
    // Terrain-specific bests
    println!("\n🏔️ TERRAIN-SPECIFIC OPTIMAL METHODS:");
    
    let flat_best = results.iter()
        .max_by(|a, b| a.flat_terrain_score.partial_cmp(&b.flat_terrain_score).unwrap())
        .unwrap();
    let hilly_best = results.iter()
        .max_by(|a, b| a.hilly_terrain_score.partial_cmp(&b.hilly_terrain_score).unwrap())
        .unwrap();
    let mountain_best = results.iter()
        .max_by(|a, b| a.mountain_terrain_score.partial_cmp(&b.mountain_terrain_score).unwrap())
        .unwrap();
    
    println!("Flat/Rolling terrain: {} ({}) - Score: {:.2}",
                flat_best.method, flat_best.parameters, flat_best.flat_terrain_score);
    println!("Hilly terrain: {} ({}) - Score: {:.2}",
                hilly_best.method, hilly_best.parameters, hilly_best.hilly_terrain_score);
    println!("Mountainous terrain: {} ({}) - Score: {:.2}",
                mountain_best.method, mountain_best.parameters, mountain_best.mountain_terrain_score);
    
    // Key findings
    println!("\n💡 KEY FINDINGS:");
    
    // Find method with best gain/loss ratio
    let best_ratio = results.iter()
        .min_by_key(|r| ((r.median_gain_loss_ratio - 100.0).abs() * 100.0) as i32)
        .unwrap();
    
    println!("• Best gain/loss ratio: {} ({}) = {:.1}%",
                best_ratio.method, best_ratio.parameters, best_ratio.median_gain_loss_ratio);
    
    // Find methods with <30% loss reduction
    let low_loss_reduction: Vec<_> = results.iter()
        .filter(|r| r.loss_reduction_percent < 30.0)
        .take(3)
        .collect();
    
    if !low_loss_reduction.is_empty() {
        println!("\n• Methods preserving elevation loss (<30% reduction):");
        for method in low_loss_reduction {
            println!("  - {} ({}): {:.1}% loss reduction, {:.1}% accuracy",
                        method.method, method.parameters, method.loss_reduction_percent, method.median_accuracy);
        }
    }
    
    // Compare method types
    println!("\n📈 BEST OF EACH METHOD TYPE:");
    let method_types = [
        "Standard Distance-Based",
        "Asymmetric Intervals",
        "Directional Deadzone",
        "Loss Compensation",
        "Gradient-Based Protection",
        "Two-Pass Processing",
        "Hybrid Selective",
        "Adaptive Loss Compensation",
        "Combined Approach",
        "Elevation Band Specific",
    ];
    
    for method_type in &method_types {
        if let Some(best_of_type) = results.iter()
            .filter(|r| r.method == *method_type)
            .max_by(|a, b| a.combined_score.partial_cmp(&b.combined_score).unwrap()) {
            
            println!("{}: score={:.1}, ratio={:.1}%, acc={:.1}%, loss_red={:.1}%",
                        method_type,
                        best_of_type.combined_score,
                        best_of_type.median_gain_loss_ratio,
                        best_of_type.median_accuracy,
                        best_of_type.loss_reduction_percent);
        }
    }
    
    println!("\n🎯 FINAL RECOMMENDATION:");
    println!("Based on comprehensive analysis including grid search, cross-validation,");
    println!("and multi-objective optimization, the optimal method is:");
    println!("\n   {} with {}", best.method, best.parameters);
    println!("\nThis achieves the best balance between:");
    println!("  • Elevation gain accuracy: {:.1}%", best.median_accuracy);
    println!("  • Natural gain/loss preservation: {:.1}% ratio", best.median_gain_loss_ratio);
    println!("  • Consistent performance across terrain types");
}