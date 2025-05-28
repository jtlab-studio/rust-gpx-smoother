use std::{fs::File, path::Path};
use std::collections::HashMap;
use csv::{Reader, Writer};
use serde::Serialize;
use rayon::prelude::*;

#[derive(Debug, Serialize, Clone)]
struct IntervalScore {
    interval_m: f32,
    score_98_102: u32,    // Files within 98-102% accuracy
    score_95_105: u32,    // Files within 95-105% accuracy
    score_90_110: u32,    // Files within 90-110% accuracy
    score_85_115: u32,    // Files within 85-115% accuracy
    score_80_120: u32,    // Files within 80-120% accuracy
    files_outside_80_120: u32,  // Files with <80% or >120% accuracy
    total_files_scored: u32,
    weighted_score: f32,   // Weighted combination of all bands
    average_accuracy: f32,
    median_accuracy: f32,
    std_deviation: f32,
    worst_accuracy: f32,   // Least similar accuracy (furthest from 100%)
    best_accuracy: f32,    // Most similar accuracy (closest to 100%)
}

pub fn run_improved_scoring_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let input_csv = Path::new(gpx_folder).join("fine_grained_analysis_0.05_to_8m.csv");
    let output_csv = Path::new(gpx_folder).join("interval_scoring_analysis.csv");
    
    if !input_csv.exists() {
        eprintln!("Error: Fine-grained analysis CSV not found. Run the main analysis first.");
        return Ok(());
    }
    
    analyze_fine_grained_results(&input_csv, &output_csv)?;
    
    Ok(())
}

fn analyze_fine_grained_results(input_csv: &Path, output_csv: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 IMPROVED SCORING ANALYSIS WITH TOLERANCE BANDS");
    println!("================================================");
    
    // Read the fine-grained results
    let file = File::open(input_csv)?;
    let mut rdr = Reader::from_reader(file);
    
    // Get headers to find interval columns
    let headers = rdr.headers()?.clone();
    let interval_indices: Vec<(usize, f32)> = headers.iter()
        .enumerate()
        .filter_map(|(idx, header)| {
            if header.ends_with("m Accuracy %") {
                let interval_str = header.trim_end_matches("m Accuracy %").trim();
                if let Ok(interval) = interval_str.parse::<f32>() {
                    return Some((idx, interval));
                }
            }
            None
        })
        .collect();
    
    println!("Found {} distance intervals to analyze", interval_indices.len());
    
    // Collect all accuracy data
    let mut interval_accuracies: HashMap<i32, Vec<f32>> = HashMap::new();
    let mut total_valid_files = 0;
    
    for result in rdr.records() {
        let record = result?;
        
        // Only process files with official data
        if let Ok(official) = record.get(3).unwrap_or("0").parse::<u32>() {
            if official > 0 {
                total_valid_files += 1;
                for &(idx, interval) in &interval_indices {
                    if let Some(accuracy_str) = record.get(idx) {
                        if let Ok(accuracy) = accuracy_str.parse::<f32>() {
                            let key = (interval * 100.0) as i32; // Convert to integer key
                            interval_accuracies.entry(key).or_insert_with(Vec::new).push(accuracy);
                        }
                    }
                }
            }
        }
    }
    
    println!("Processing {} files with official elevation data", total_valid_files);
    
    // Calculate scores for each interval
    let mut scores: Vec<IntervalScore> = interval_accuracies
        .par_iter()
        .map(|(&interval_key, accuracies)| {
            let interval = interval_key as f32 / 100.0;
            
            // Count files in each tolerance band
            let score_98_102 = accuracies.iter().filter(|&&acc| acc >= 98.0 && acc <= 102.0).count() as u32;
            let score_95_105 = accuracies.iter().filter(|&&acc| acc >= 95.0 && acc <= 105.0).count() as u32;
            let score_90_110 = accuracies.iter().filter(|&&acc| acc >= 90.0 && acc <= 110.0).count() as u32;
            let score_85_115 = accuracies.iter().filter(|&&acc| acc >= 85.0 && acc <= 115.0).count() as u32;
            let score_80_120 = accuracies.iter().filter(|&&acc| acc >= 80.0 && acc <= 120.0).count() as u32;
            let files_outside_80_120 = accuracies.iter().filter(|&&acc| acc < 80.0 || acc > 120.0).count() as u32;
            
            // Calculate weighted score with emphasis on tighter tolerances
            // Weights: 98-102% = 10, 95-105% = 6, 90-110% = 3, 85-115% = 1.5, 80-120% = 1
            // Penalty for files outside 80-120%: -5 points each
            let weighted_score = (score_98_102 as f32 * 10.0) +
                               ((score_95_105 - score_98_102) as f32 * 6.0) +
                               ((score_90_110 - score_95_105) as f32 * 3.0) +
                               ((score_85_115 - score_90_110) as f32 * 1.5) +
                               ((score_80_120 - score_85_115) as f32 * 1.0) -
                               (files_outside_80_120 as f32 * 5.0);  // Penalty for outliers
            
            // Calculate statistics
            let average_accuracy = accuracies.iter().sum::<f32>() / accuracies.len() as f32;
            
            let mut sorted_accuracies = accuracies.clone();
            sorted_accuracies.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median_accuracy = if sorted_accuracies.len() % 2 == 0 {
                (sorted_accuracies[sorted_accuracies.len() / 2 - 1] + 
                 sorted_accuracies[sorted_accuracies.len() / 2]) / 2.0
            } else {
                sorted_accuracies[sorted_accuracies.len() / 2]
            };
            
            // Find worst and best accuracy (furthest and closest to 100%)
            let worst_accuracy = accuracies.iter()
                .max_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
                .copied()
                .unwrap_or(100.0);
                
            let best_accuracy = accuracies.iter()
                .min_by_key(|&&acc| ((acc - 100.0).abs() * 1000.0) as i32)
                .copied()
                .unwrap_or(100.0);
            
            let variance = accuracies.iter()
                .map(|&acc| (acc - average_accuracy).powi(2))
                .sum::<f32>() / accuracies.len() as f32;
            let std_deviation = variance.sqrt();
            
            IntervalScore {
                interval_m: interval,
                score_98_102,
                score_95_105,
                score_90_110,
                score_85_115,
                score_80_120,
                files_outside_80_120,
                total_files_scored: accuracies.len() as u32,
                weighted_score,
                average_accuracy,
                median_accuracy,
                std_deviation,
                worst_accuracy,
                best_accuracy,
            }
        })
        .collect();
    
    // Sort by weighted score (descending)
    scores.sort_by(|a, b| b.weighted_score.partial_cmp(&a.weighted_score).unwrap());
    
    // Write results to CSV
    write_scoring_results(&scores, output_csv)?;
    
    // Print top results
    print_analysis_summary(&scores);
    
    Ok(())
}

fn write_scoring_results(scores: &[IntervalScore], output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    // Write headers
    wtr.write_record(&[
        "Rank",
        "Interval (m)",
        "Weighted Score",
        "Files 98-102%",
        "Files 95-105%",
        "Files 90-110%",
        "Files 85-115%",
        "Files 80-120%",
        "Files Outside 80-120%",
        "Total Files",
        "% in 98-102%",
        "% in 95-105%",
        "% in 90-110%",
        "% in 85-115%",
        "% in 80-120%",
        "% Outside 80-120%",
        "Average Accuracy %",
        "Median Accuracy %",
        "Best Accuracy %",
        "Worst Accuracy %",
        "Std Deviation",
    ])?;
    
    // Write data
    for (rank, score) in scores.iter().enumerate() {
        let total = score.total_files_scored as f32;
        wtr.write_record(&[
            (rank + 1).to_string(),
            format!("{:.2}", score.interval_m),
            format!("{:.2}", score.weighted_score),
            score.score_98_102.to_string(),
            score.score_95_105.to_string(),
            score.score_90_110.to_string(),
            score.score_85_115.to_string(),
            score.score_80_120.to_string(),
            score.files_outside_80_120.to_string(),
            score.total_files_scored.to_string(),
            format!("{:.1}", (score.score_98_102 as f32 / total) * 100.0),
            format!("{:.1}", (score.score_95_105 as f32 / total) * 100.0),
            format!("{:.1}", (score.score_90_110 as f32 / total) * 100.0),
            format!("{:.1}", (score.score_85_115 as f32 / total) * 100.0),
            format!("{:.1}", (score.score_80_120 as f32 / total) * 100.0),
            format!("{:.1}", (score.files_outside_80_120 as f32 / total) * 100.0),
            format!("{:.2}", score.average_accuracy),
            format!("{:.2}", score.median_accuracy),
            format!("{:.2}", score.best_accuracy),
            format!("{:.2}", score.worst_accuracy),
            format!("{:.2}", score.std_deviation),
        ])?;
    }
    
    wtr.flush()?;
    println!("\n✅ Scoring results saved to: {}", output_path.display());
    
    Ok(())
}

fn print_analysis_summary(scores: &[IntervalScore]) {
    // Print top 3 results
    println!("\n🏆 TOP 3 DISTANCE INTERVALS BY WEIGHTED SCORE:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Rank | Interval | Score  | 98-102% | 95-105% | 90-110% | Outside | Median | Avg Acc | Worst % | Std Dev");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for (rank, score) in scores.iter().take(3).enumerate() {
        println!("{:4} | {:7.2}m | {:6.1} | {:7} | {:7} | {:7} | {:7} | {:6.1}% | {:7.1}% | {:7.1}% | {:7.1}",
                 rank + 1,
                 score.interval_m,
                 score.weighted_score,
                 score.score_98_102,
                 score.score_95_105,
                 score.score_90_110,
                 score.files_outside_80_120,
                 score.median_accuracy,
                 score.average_accuracy,
                 score.worst_accuracy,
                 score.std_deviation);
    }
    
    println!("\n📈 ANALYSIS INSIGHTS:");
    
    // Best by different criteria
    let best_tight = scores.iter().max_by_key(|s| s.score_98_102).unwrap();
    let best_avg = scores.iter().min_by_key(|s| ((s.average_accuracy - 100.0).abs() * 100.0) as i32).unwrap();
    let best_consistency = scores.iter().min_by(|a, b| a.std_deviation.partial_cmp(&b.std_deviation).unwrap()).unwrap();
    let fewest_outliers = scores.iter().min_by_key(|s| s.files_outside_80_120).unwrap();
    
    println!("• Best for tight tolerance (98-102%): {:.2}m interval ({} files, {:.1}%)", 
             best_tight.interval_m, 
             best_tight.score_98_102,
             (best_tight.score_98_102 as f32 / best_tight.total_files_scored as f32) * 100.0);
    
    println!("• Best average accuracy: {:.2}m interval ({:.1}% average, {:.1}% median)", 
             best_avg.interval_m, best_avg.average_accuracy, best_avg.median_accuracy);
    
    println!("• Most consistent results: {:.2}m interval (σ = {:.1}, worst: {:.1}%)", 
             best_consistency.interval_m, best_consistency.std_deviation, best_consistency.worst_accuracy);
    
    println!("• Fewest outliers (outside 80-120%): {:.2}m interval ({} files, {:.1}%)",
             fewest_outliers.interval_m,
             fewest_outliers.files_outside_80_120,
             (fewest_outliers.files_outside_80_120 as f32 / fewest_outliers.total_files_scored as f32) * 100.0);
    
    // Recommendation
    if let Some(top_score) = scores.first() {
        println!("\n🎯 RECOMMENDATION:");
        println!("Use {:.2}m distance intervals for optimal elevation gain accuracy across diverse terrain types.",
                 top_score.interval_m);
        println!("This interval achieves:");
        println!("  • {:.1}% of files within ±2% accuracy", 
                 (top_score.score_98_102 as f32 / top_score.total_files_scored as f32) * 100.0);
        println!("  • {:.1}% of files within ±10% accuracy",
                 (top_score.score_90_110 as f32 / top_score.total_files_scored as f32) * 100.0);
        println!("  • Only {} files ({:.1}%) outside ±20% accuracy",
                 top_score.files_outside_80_120,
                 (top_score.files_outside_80_120 as f32 / top_score.total_files_scored as f32) * 100.0);
        println!("  • Median accuracy: {:.1}%, Worst case: {:.1}%",
                 top_score.median_accuracy, top_score.worst_accuracy);
    }
}