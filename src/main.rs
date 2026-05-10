use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("Factorio Settings Binary Converter (Rust Edition)");
        println!("------------------------------------------------");
        println!("Usage:");
        println!("  factorio_settings decode <in_dat> <out_json>");
        println!("  factorio_settings encode <in_json> <out_dat>");
        process::exit(1);
    }

    let command = &args[1];
    let in_path = &args[2];
    let out_path = &args[3];

    match command.as_str() {
        "decode" => {
            println!("[INFO] Reading binary file: {}...", in_path);
            let bytes = match fs::read(in_path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read input file '{}': {}", in_path, e);
                    process::exit(1);
                }
            };

            match factorio_settings::decode_dat_to_json(&bytes) {
                Ok(json_str) => {
                    if let Err(e) = fs::write(out_path, json_str) {
                        eprintln!(
                            "[ERROR] Failed to write output JSON file '{}': {}",
                            out_path, e
                        );
                        process::exit(1);
                    }
                    println!(
                        "[SUCCESS] Decoded configuration output written to: {}",
                        out_path
                    );
                }
                Err(err) => {
                    eprintln!("[ERROR] Decoding operation failed: {}", err);
                    process::exit(1);
                }
            }
        }
        "encode" => {
            println!("[INFO] Reading JSON configuration: {}...", in_path);
            let json_str = match fs::read_to_string(in_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "[ERROR] Failed to read input JSON file '{}': {}",
                        in_path, e
                    );
                    process::exit(1);
                }
            };

            match factorio_settings::encode_json_to_dat(&json_str) {
                Ok(bytes) => {
                    if let Err(e) = fs::write(out_path, bytes) {
                        eprintln!(
                            "[ERROR] Failed to compile output binary file '{}': {}",
                            out_path, e
                        );
                        process::exit(1);
                    }
                    println!(
                        "[SUCCESS] Compiled binary configuration written to: {}",
                        out_path
                    );
                }
                Err(err) => {
                    eprintln!("[ERROR] Compilation failed: {}", err);
                    process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("[ERROR] Unknown action sub-command: '{}'", command);
            process::exit(1);
        }
    }
}
