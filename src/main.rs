use rustcracker::GpuCracker;
use std::env;
use std::fs;
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <wordlist_file> <md5_hash>", args[0]);
        eprintln!(
            "Example: {} wordlist.txt 5f4dcc3b5aa765d61d8327deb882cf99",
            args[0]
        );
        std::process::exit(1);
    }

    let wordlist_path = &args[1];
    let target_hash_str = &args[2];

    // Decode target hash
    let target_hash_vec = hex::decode(target_hash_str)?;
    if target_hash_vec.len() != 16 {
        eprintln!("Error: MD5 hash must be 32 hex characters (16 bytes)");
        std::process::exit(1);
    }
    let mut target_hash = [0u8; 16];
    target_hash.copy_from_slice(&target_hash_vec);

    // Read wordlist
    println!("Loading wordlist from {wordlist_path}...");
    let mut wordlist_file = fs::File::open(wordlist_path)?;
    let mut wordlist_data = String::new();
    wordlist_file.read_to_string(&mut wordlist_data)?;
    let wordlist: Vec<&str> = wordlist_data.lines().collect();
    println!("Loaded {} passwords", wordlist.len());

    // Initialize GPU cracker
    println!("Initializing GPU...");
    let cracker = pollster::block_on(GpuCracker::new())?;

    // Attempt to crack the hash
    println!("Cracking hash {target_hash_str}...");
    match cracker.crack(&target_hash, &wordlist) {
        Some(password) => {
            println!("✓ Hash cracked!");
            println!("  Password: {password}");
            println!("  md5({password}) = {target_hash_str}");
        }
        None => {
            println!("✗ Hash not found in wordlist");
        }
    }

    Ok(())
}
