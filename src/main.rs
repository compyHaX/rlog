use crossterm::{execute, style::{Color, SetForegroundColor}};
use regex::Regex;
use serde_json::Value;
use std::{collections::HashMap, env, fs::{File, metadata}, io::{BufRead, BufReader, Seek, SeekFrom}, path::Path, thread, time::Duration};

/// Returns terminal color based on log level.
fn get_color(level: &str) -> Color {
    match level {
        "DEBUG" => Color::White,
        "INFO" => Color::Blue,
        "NOTICE" => Color::Cyan,
        "WARNING" => Color::Yellow,
        "ERROR" => Color::Red,
        "CRITICAL" => Color::Magenta,
        "ALERT" => Color::DarkRed,
        "EMERGENCY" => Color::DarkMagenta,
        _ => Color::Reset,
    }
}

/// Parses a log line using regex and associates captured groups with headers.
fn parse_line<'a>(line: &'a str, regex: &Regex, headers: &[&'a str]) -> Option<HashMap<&'a str, &'a str>> {
    regex.captures(line).map(|caps| {
        headers.iter().enumerate()
            .filter_map(|(i, &header)| Some((header, caps.get(i + 1)?.as_str())))
            .collect()
    })
}

/// Entry point of the log viewer program.
///
/// Command-line arguments:
/// - `--filter` or `--f`: Filters log entries containing a specific word.
/// - `--level` or `--l`: Filters log entries by log level.
/// - `--start` or `--s`: Filters log entries from a specific start date.
/// - `--to` or `--t`: Filters log entries up to a specific end date.
/// - `--width` or `--w`: Sets individual column widths for formatted output (comma-separated).
/// - `--verbose` or `--v`: Includes the Data field in the output.
/// - `--detailed` or `--V`: Includes and pretty-prints the Data field as JSON.
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: log_viewer <log_file> [--filter WORD|--f WORD] [--level LEVEL|--l LEVEL] [--start DATE|--s DATE] [--to DATE|--t DATE] [--width W1,W2,...|--w W1,W2,...] [--verbose|--v] [--detailed|--V]");
        return;
    }

    let log_file = &args[1];
    let path = Path::new(log_file);
    if !path.exists() {
        eprintln!("File not found: {}", log_file);
        return;
    }

    let verbose = args.contains(&"--verbose".to_string()) || args.contains(&"--v".to_string());
    let detailed = args.contains(&"--detailed".to_string()) || args.contains(&"--V".to_string());

    let mut filter_word = None;
    let mut filter_level = None;
    let mut from_date = None;
    let mut to_date = None;
    let mut col_widths: Vec<usize> = vec![20, 10, 50, 30];

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--filter" | "--f" => { filter_word = args.get(i + 1); i += 1; },
            "--level" | "--l" => { filter_level = args.get(i + 1).map(|l| l.to_uppercase()); i += 1; },
            "--start" | "--s" => { from_date = args.get(i + 1); i += 1; },
            "--to" | "--t" => { to_date = args.get(i + 1); i += 1; },
            "--width" | "--w" => {
                if let Some(width_str) = args.get(i + 1) {
                    col_widths = width_str.split(',').filter_map(|w| w.parse().ok()).collect();
                }
                i += 1;
            },
            _ => {}
        }
        i += 1;
    }

    let file = File::open(path).expect("Failed to open file");
    let mut reader = BufReader::new(file);

    let mut header_line = String::new();
    reader.read_line(&mut header_line).expect("Failed to read header");
    let headers: Vec<&str> = header_line.trim().split('|').collect();

    let regex_pattern = headers.iter().map(|_| "(.*?)").collect::<Vec<&str>>().join("\\|");
    let regex = Regex::new(&format!("^{}$", regex_pattern)).expect("Invalid regex");

    let mut position = reader.stream_position().unwrap();

    loop {
        if metadata(path).unwrap().len() < position {
            position = 0;
            reader.seek(SeekFrom::Start(0)).unwrap();
        }

        if metadata(path).unwrap().len() > position {
            reader.seek(SeekFrom::Start(position)).unwrap();
            let mut line = String::new();

            while reader.read_line(&mut line).unwrap() > 0 {
                position += line.len() as u64;

                if let Some(columns) = parse_line(&line.trim(), &regex, &headers) {
                    let date_ok = from_date.map_or(true, |fd| columns["DateTime"] >= fd)
                        && to_date.map_or(true, |td| columns["DateTime"] <= td);

                    let level_ok = filter_level.as_ref().map_or(true, |lvl| columns["Level"].to_uppercase() == *lvl);
                    let word_ok = filter_word.map_or(true, |word| line.contains(word));

                    if date_ok && level_ok && word_ok {
                        let color = get_color(columns["Level"].to_uppercase().as_str());
                        execute!(std::io::stdout(), SetForegroundColor(color)).unwrap();

                        for (idx, &header) in headers.iter().enumerate() {
                            if header == "Data" && detailed {
                                if let Ok(json) = serde_json::from_str::<Value>(columns["Data"]) {
                                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                                } else {
                                    println!("{}", columns["Data"]);
                                }
                            } else if header != "Data" || verbose {
                                print!("{:width$} | ", columns[header], width = col_widths.get(idx).unwrap_or(&15));
                            }
                        }

                        execute!(std::io::stdout(), SetForegroundColor(Color::Reset)).unwrap();
                        println!();
                    }
                }
                line.clear();
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
}