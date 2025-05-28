use std::{fs::File, path::Path};
use std::path::PathBuf;
use std::collections::HashMap;
use csv::{Reader, Writer};
use serde::Serialize;
use rayon::prelude::*;

#[derive(Debug, Clone)]
struct FileAccuracyProfile {
    filename: String,
    accuracies: Vec<f32>,
    average_accuracy: f32,
    median_accuracy: f32,
    best_accuracy: f32,
    worst_accuracy: f32,
    percent_within_80_120: f32,
    outlier_score: f32,  // Higher score = more likely to be an outlier
}

#[derive(Debug, Serialize)]
struct OutlierFile {
    filename: String,
    average_accuracy: f32,
    median_accuracy: f32,
    best_accuracy: f32,
    worst_accuracy: f32,
    percent_within_80_120: f32,
    outlier_score: f32,
    most_accurate_interval_m: f32,
    most_accurate_interval_accuracy: f32,
}

#[derive(Debug, Serialize, Clone)]
struct IntervalScoreWithOutliers {
    interval_m: f32,
    // With outliers
    score_98_102_with: u32,
    score_95_105_with: u32,
    score_90_110_with: u32,
    score_85_115_with: u32,
    score_80_120_with: u32,
    files_outside_80_120_with: u32,
    total_files_with: u32,
    weighted_score_with: f32,
    average_accuracy_with: f32,
    median_accuracy_with: f32,
    worst_accuracy_with: f32,
    // Without outliers
    score_98_102_without: u32,
    score_95_105_without: u32,
    score_90_110_without: u32,
    score_85_115_without: u32,
    score_80_120_without: u32,
    files_outside_80_120_without: u32,
    total_files_without: u32,
    weighted_score_without: f32,
    average_accuracy_without: f32,
    median_accuracy_without: f32,
    worst_accuracy_without: f32,
}

pub fn run_outlier_analysis(gpx_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let input_csv = Path::new(gpx_folder).join("fine_grained_analysis_0.05_to_8m.csv");
    
    if !input_csv.exists() {
        eprintln!("Error: Fine-grained analysis CSV not found. Run the main analysis first.");
        return Ok(());
    }
    
    // First, identify outlier files
    let (file_profiles, interval_data, headers) = analyze_file_performance(&input_csv)?;
    let outlier_files = identify_outliers(&file_profiles);
    
    // Create comparison analysis
    let comparison_scores = calculate_scores_with_and_without_outliers(
        &interval_data, 
        &outlier_files,
        &headers
    )?;
    
    // Write results
    write_outlier_files(&outlier_files, Path::new(gpx_folder).join("outlier_gpx_files.csv"))?;
    write_comparison_scores(&comparison_scores, Path::new(gpx_folder).join("interval_scoring_with_without_outliers.csv"))?;
    
    // Print summary
    print_outlier_summary(&outlier_files, &comparison_scores);
    
    Ok(())
}

fn analyze_file_performance(input_csv: &Path) -> Result<(Vec<FileAccuracyProfile>, HashMap<String, HashMap<i32, f32>>, csv::StringRecord), Box<dyn std::error::Error>> {
    let file = File::open(input_csv)?;
    let mut rdr = Reader::from_reader(file);
    let headers = rdr.headers()?.clone();
    
    // Find interval accuracy columns
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
    
    let mut file_profiles = Vec::new();
    let mut interval_data: HashMap<String, HashMap<i32, f32>> = HashMap::new();
    
    for result in rdr.records() {
        let record = result?;
        
        // Only process files with official data
        if let Ok(official) = record.get(3).unwrap_or("0").parse::<u32>() {
            if official > 0 {
                let filename = record.get(0).unwrap_or("").to_string();
                let mut accuracies = Vec::new();
                let mut file_interval_data = HashMap::new();
                
                for &(idx, interval) in &interval_indices {
                    if let Some(accuracy_str) = record.get(idx) {
                        if let Ok(accuracy) = accuracy_str.parse::<f32>() {
                            accuracies.push(accuracy);
                            file_interval_data.insert((interval * 100.0) as i32, accuracy);
                        }
                    }
                }
                
                if !accuracies.is_empty() {
                    let profile = create_file_profile(filename.clone(), accuracies);
                    file_profiles.push(profile);
                    interval_data.insert(filename, file_interval_data);
                }
            }
        }
    }
    
    Ok((file_profiles, interval_data, headers))
}

fn create_file_profile(filename: String, accuracies: Vec<f32>) -> FileAccuracyProfile {
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
    
    let within_80_120 = accuracies.iter()
        .filter(|&&acc| acc >= 80.0 && acc <= 120.0)
        .count() as f32 / accuracies.len() as f32 * 100.0;
    
    // Calculate outlier score (higher = worse)
    let outlier_score = 
        (average_accuracy - 100.0).abs() * 0.3 +
        (median_accuracy - 100.0).abs() * 0.3 +
        (worst_accuracy - 100.0).abs() * 0.2 +
        (100.0 - within_80_120) * 0.2;
    
    FileAccuracyProfile {
        filename,
        accuracies,
        average_accuracy,
        median_accuracy,
        best_accuracy,
        worst_accuracy,
        percent_within_80_120: within_80_120,
        outlier_score,
    }
}

fn identify_outliers(profiles: &[FileAccuracyProfile]) -> Vec<OutlierFile> {
    // Calculate outlier threshold (e.g., files with outlier score > 75th percentile + 1.5 * IQR)
    let mut outlier_scores: Vec<f32> = profiles.iter().map(|p| p.outlier_score).collect();
    outlier_scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let q1_idx = outlier_scores.len() / 4;
    let q3_idx = (outlier_scores.len() * 3) / 4;
    let q1 = outlier_scores[q1_idx];
    let q3 = outlier_scores[q3_idx];
    let iqr = q3 - q1;
    let threshold = q3 + 1.5 * iqr;
    
    // Alternative: Use fixed thresholds
    let fixed_threshold = 25.0; // Files consistently >25% off are outliers
    
    profiles.iter()
        .filter(|p| {
            p.outlier_score > threshold.max(fixed_threshold) ||
            p.percent_within_80_120 < 50.0 || // Less than 50% of intervals within 80-120%
            (p.worst_accuracy - 100.0).abs() > 50.0 // Any interval off by more than 50%
        })
        .map(|p| {
            // Find the most accurate interval for this file
            let best_idx = p.accuracies.iter()
                .enumerate()
                .min_by_key(|(_, &acc)| ((acc - 100.0).abs() * 1000.0) as i32)
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            
            let interval = (best_idx + 1) as f32 * 0.05; // Convert index to interval
            
            OutlierFile {
                filename: p.filename.clone(),
                average_accuracy: p.average_accuracy,
                median_accuracy: p.median_accuracy,
                best_accuracy: p.best_accuracy,
                worst_accuracy: p.worst_accuracy,
                percent_within_80_120: p.percent_within_80_120,
                outlier_score: p.outlier_score,
                most_accurate_interval_m: interval,
                most_accurate_interval_accuracy: p.best_accuracy,
            }
        })
        .collect()
}

fn calculate_scores_with_and_without_outliers(
    interval_data: &HashMap<String, HashMap<i32, f32>>,
    outlier_files: &[OutlierFile],
    headers: &csv::StringRecord
) -> Result<Vec<IntervalScoreWithOutliers>, Box<dyn std::error::Error>> {
    let outlier_filenames: Vec<&str> = outlier_files.iter()
        .map(|o| o.filename.as_str())
        .collect();
    
    // Find all intervals from headers
    let intervals: Vec<f32> = headers.iter()
        .filter_map(|header| {
            if header.ends_with("m Accuracy %") {
                let interval_str = header.trim_end_matches("m Accuracy %").trim();
                interval_str.parse::<f32>().ok()
            } else {
                None
            }
        })
        .collect();
    
    let scores: Vec<IntervalScoreWithOutliers> = intervals
        .par_iter()
        .map(|&interval| {
            let interval_key = (interval * 100.0) as i32;
            
            // Collect accuracies with and without outliers
            let mut accuracies_with = Vec::new();
            let mut accuracies_without = Vec::new();
            
            for (filename, file_data) in interval_data {
                if let Some(&accuracy) = file_data.get(&interval_key) {
                    accuracies_with.push(accuracy);
                    
                    if !outlier_filenames.contains(&filename.as_str()) {
                        accuracies_without.push(accuracy);
                    }
                }
            }
            
            // Calculate scores with outliers
            let score_with = calculate_interval_score(&accuracies_with);
            
            // Calculate scores without outliers
            let score_without = calculate_interval_score(&accuracies_without);
            
            IntervalScoreWithOutliers {
                interval_m: interval,
                // With outliers
                score_98_102_with: score_with.0,
                score_95_105_with: score_with.1,
                score_90_110_with: score_with.2,
                score_85_115_with: score_with.3,
                score_80_120_with: score_with.4,
                files_outside_80_120_with: score_with.5,
                total_files_with: accuracies_with.len() as u32,
                weighted_score_with: score_with.6,
                average_accuracy_with: score_with.7,
                median_accuracy_with: score_with.8,
                worst_accuracy_with: score_with.9,
                // Without outliers
                score_98_102_without: score_without.0,
                score_95_105_without: score_without.1,
                score_90_110_without: score_without.2,
                score_85_115_without: score_without.3,
                score_80_120_without: score_without.4,
                files_outside_80_120_without: score_without.5,
                total_files_without: accuracies_without.len() as u32,
                weighted_score_without: score_without.6,
                average_accuracy_without: score_without.7,
                median_accuracy_without: score_without.8,
                worst_accuracy_without: score_without.9,
            }
        })
        .collect();
    
    Ok(scores)
}

fn calculate_interval_score(accuracies: &[f32]) -> (u32, u32, u32, u32, u32, u32, f32, f32, f32, f32) {
    if accuracies.is_empty() {
        return (0, 0, 0, 0, 0, 0, 0.0, 0.0, 0.0, 0.0);
    }
    
    let score_98_102 = accuracies.iter().filter(|&&acc| acc >= 98.0 && acc <= 102.0).count() as u32;
    let score_95_105 = accuracies.iter().filter(|&&acc| acc >= 95.0 && acc <= 105.0).count() as u32;
    let score_90_110 = accuracies.iter().filter(|&&acc| acc >= 90.0 && acc <= 110.0).count() as u32;
    let score_85_115 = accuracies.iter().filter(|&&acc| acc >= 85.0 && acc <= 115.0).count() as u32;
    let score_80_120 = accuracies.iter().filter(|&&acc| acc >= 80.0 && acc <= 120.0).count() as u32;
    let files_outside_80_120 = accuracies.iter().filter(|&&acc| acc < 80.0 || acc > 120.0).count() as u32;
    
    let weighted_score = (score_98_102 as f32 * 10.0) +
                        ((score_95_105 - score_98_102) as f32 * 6.0) +
                        ((score_90_110 - score_95_105) as f32 * 3.0) +
                        ((score_85_115 - score_90_110) as f32 * 1.5) +
                        ((score_80_120 - score_85_115) as f32 * 1.0) -
                        (files_outside_80_120 as f32 * 5.0);
    
    let average_accuracy = accuracies.iter().sum::<f32>() / accuracies.len() as f32;
    
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
    
    (score_98_102, score_95_105, score_90_110, score_85_115, score_80_120, 
     files_outside_80_120, weighted_score, average_accuracy, median_accuracy, worst_accuracy)
}

fn write_outlier_files(outliers: &[OutlierFile], output_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = Writer::from_path(output_path)?;
    
    wtr.write_record(&[
        "Filename",
        "Average Accuracy %",
        "Median Accuracy %",
        "Best Accuracy %",
        "Worst Accuracy %",
        "% Within 80-120%",
        "Outlier Score",
        "Best Interval (m)",
        "Best Interval Accuracy %",
    ])?;
    
    for outlier in outliers {
        wtr.write_record(&[
            &outlier.filename,
            &format!("{:.2}", outlier.average_accuracy),
            &format!("{:.2}", outlier.median_accuracy),
            &format!("{:.2}", outlier.best_accuracy),
            &format!("{:.2}", outlier.worst_accuracy),
            &format!("{:.1}", outlier.percent_within_80_120),
            &format!("{:.2}", outlier.outlier_score),
            &format!("{:.2}", outlier.most_accurate_interval_m),
            &format!("{:.2}", outlier.most_accurate_interval_accuracy),
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn write_comparison_scores(scores: &[IntervalScoreWithOutliers], output_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut sorted_scores_with = scores.to_vec();
    sorted_scores_with.sort_by(|a, b| b.weighted_score_with.partial_cmp(&a.weighted_score_with).unwrap());
    
    let mut sorted_scores_without = scores.to_vec();
    sorted_scores_without.sort_by(|a, b| b.weighted_score_without.partial_cmp(&a.weighted_score_without).unwrap());
    
    let mut wtr = Writer::from_path(output_path)?;
    
    wtr.write_record(&[
        "Interval (m)",
        // With outliers
        "Rank With",
        "Score With",
        "Files With",
        "98-102% With",
        "95-105% With", 
        "90-110% With",
        "Outside 80-120% With",
        "Avg % With",
        "Median % With",
        "Worst % With",
        // Without outliers
        "Rank Without",
        "Score Without",
        "Files Without",
        "98-102% Without",
        "95-105% Without",
        "90-110% Without", 
        "Outside 80-120% Without",
        "Avg % Without",
        "Median % Without",
        "Worst % Without",
        // Differences
        "Score Improvement",
        "Rank Change",
    ])?;
    
    for score in &sorted_scores_with {
        let rank_with = sorted_scores_with.iter().position(|s| s.interval_m == score.interval_m).unwrap() + 1;
        let rank_without = sorted_scores_without.iter().position(|s| s.interval_m == score.interval_m).unwrap() + 1;
        let rank_change = rank_with as i32 - rank_without as i32;
        let score_improvement = score.weighted_score_without - score.weighted_score_with;
        
        wtr.write_record(&[
            &format!("{:.2}", score.interval_m),
            // With outliers
            &rank_with.to_string(),
            &format!("{:.1}", score.weighted_score_with),
            &score.total_files_with.to_string(),
            &score.score_98_102_with.to_string(),
            &score.score_95_105_with.to_string(),
            &score.score_90_110_with.to_string(),
            &score.files_outside_80_120_with.to_string(),
            &format!("{:.1}", score.average_accuracy_with),
            &format!("{:.1}", score.median_accuracy_with),
            &format!("{:.1}", score.worst_accuracy_with),
            // Without outliers
            &rank_without.to_string(),
            &format!("{:.1}", score.weighted_score_without),
            &score.total_files_without.to_string(),
            &score.score_98_102_without.to_string(),
            &score.score_95_105_without.to_string(),
            &score.score_90_110_without.to_string(),
            &score.files_outside_80_120_without.to_string(),
            &format!("{:.1}", score.average_accuracy_without),
            &format!("{:.1}", score.median_accuracy_without),
            &format!("{:.1}", score.worst_accuracy_without),
            // Differences
            &format!("{:+.1}", score_improvement),
            &format!("{:+}", -rank_change), // Negative because lower rank is better
        ])?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn print_outlier_summary(outliers: &[OutlierFile], scores: &[IntervalScoreWithOutliers]) {
    println!("\n📊 OUTLIER ANALYSIS SUMMARY");
    println!("===========================");
    println!("Found {} outlier files out of {} total files", 
             outliers.len(), 
             scores.first().map(|s| s.total_files_with).unwrap_or(0));
    
    if !outliers.is_empty() {
        println!("\n🚫 OUTLIER FILES (consistently poor accuracy):");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Filename                                    | Avg % | Median % | Worst % | Best Interval");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        for outlier in outliers.iter().take(10) {
            println!("{:42} | {:5.1} | {:8.1} | {:7.1} | {:.2}m ({:.1}%)",
                     outlier.filename.chars().take(42).collect::<String>(),
                     outlier.average_accuracy,
                     outlier.median_accuracy,
                     outlier.worst_accuracy,
                     outlier.most_accurate_interval_m,
                     outlier.best_accuracy);
        }
        
        if outliers.len() > 10 {
            println!("... and {} more outlier files", outliers.len() - 10);
        }
    }
    
    // Compare top intervals with and without outliers
    let mut sorted_with = scores.to_vec();
    sorted_with.sort_by(|a, b| b.weighted_score_with.partial_cmp(&a.weighted_score_with).unwrap());
    
    let mut sorted_without = scores.to_vec();
    sorted_without.sort_by(|a, b| b.weighted_score_without.partial_cmp(&a.weighted_score_without).unwrap());
    
    println!("\n🏆 TOP 3 INTERVALS COMPARISON:");
    println!("\nWITH outliers included:");
    for (i, score) in sorted_with.iter().take(3).enumerate() {
        println!("{}. {:.2}m - Score: {:.1}, 98-102%: {}, Median: {:.1}%, Worst: {:.1}%",
                 i + 1,
                 score.interval_m,
                 score.weighted_score_with,
                 score.score_98_102_with,
                 score.median_accuracy_with,
                 score.worst_accuracy_with);
    }
    
    println!("\nWITHOUT outliers (cleaned dataset):");
    for (i, score) in sorted_without.iter().take(3).enumerate() {
        println!("{}. {:.2}m - Score: {:.1}, 98-102%: {}, Median: {:.1}%, Worst: {:.1}%",
                 i + 1,
                 score.interval_m,
                 score.weighted_score_without,
                 score.score_98_102_without,
                 score.median_accuracy_without,
                 score.worst_accuracy_without);
    }
    
    println!("\n📈 KEY INSIGHTS:");
    let best_with = &sorted_with[0];
    let best_without = &sorted_without[0];
    
    if best_with.interval_m != best_without.interval_m {
        println!("• Optimal interval CHANGES: {:.2}m (with outliers) → {:.2}m (without outliers)",
                 best_with.interval_m, best_without.interval_m);
    } else {
        println!("• Optimal interval remains {:.2}m with or without outliers", best_with.interval_m);
    }
    
    println!("• Removing outliers improves median accuracy: {:.1}% → {:.1}%",
             best_with.median_accuracy_with, best_without.median_accuracy_without);
    
    println!("• Files outside 80-120% drops from {} to {} at optimal interval",
             best_without.files_outside_80_120_with, best_without.files_outside_80_120_without);
}