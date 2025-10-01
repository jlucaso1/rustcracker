/// Utilities for benchmark test data generation
use std::fs;
use std::io::Write;
use std::path::Path;

/// Generate a wordlist file with the specified number of words
pub fn generate_wordlist(size: usize, prefix: &str) -> Vec<String> {
    let mut wordlist = Vec::with_capacity(size);

    // Generate various password patterns
    for i in 0..size {
        let password = match i % 10 {
            0 => format!("password{i}"),
            1 => format!("user{i}_2024"),
            2 => format!("{prefix}@Test{i}"),
            3 => format!("SecurePass{i}"),
            4 => format!("admin{i}"),
            5 => format!("qwerty{i}"),
            6 => format!("{i}123456{i}"),
            7 => format!("letmein{i}"),
            8 => format!("welcome{i}"),
            _ => format!("{prefix}{i}"),
        };
        wordlist.push(password);
    }

    wordlist
}

/// Generate a wordlist with varied password lengths
#[allow(dead_code)]
pub fn generate_varied_length_wordlist(size: usize) -> Vec<String> {
    let mut wordlist = Vec::with_capacity(size);

    for i in 0..size {
        // Create passwords of varying lengths (4 to 64 characters)
        let length = 4 + (i % 60);
        let password: String = (0..length)
            .map(|j| {
                let chars = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
                chars[(i + j) % chars.len()] as char
            })
            .collect();
        wordlist.push(password);
    }

    wordlist
}

/// Save wordlist to a file
#[allow(dead_code)]
pub fn save_wordlist_to_file<P: AsRef<Path>>(wordlist: &[String], path: P) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    for word in wordlist {
        writeln!(file, "{word}")?;
    }
    Ok(())
}

/// Generate a specific password at a given position in the wordlist
#[allow(dead_code)] // Used in cracker_benchmark.rs
pub fn generate_wordlist_with_target(
    size: usize,
    target_password: &str,
    target_position: usize,
) -> Vec<String> {
    let mut wordlist = generate_wordlist(size, "bench");

    if target_position < size {
        wordlist[target_position] = target_password.to_string();
    }

    wordlist
}

/// Calculate MD5 hash of a string (for creating test targets)
pub fn md5_hash(input: &str) -> [u8; 16] {
    let digest = md5::compute(input.as_bytes());
    digest.0
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_generate_wordlist() {
        let wordlist = generate_wordlist(100, "test");
        assert_eq!(wordlist.len(), 100);
        assert!(wordlist.iter().all(|w| !w.is_empty()));
    }

    #[test]
    fn test_generate_varied_length() {
        let wordlist = generate_varied_length_wordlist(50);
        assert_eq!(wordlist.len(), 50);

        // Check that we have varied lengths
        let lengths: Vec<usize> = wordlist.iter().map(|w| w.len()).collect();
        let min_len = lengths.iter().min().unwrap();
        let max_len = lengths.iter().max().unwrap();
        assert!(max_len > min_len);
    }

    #[test]
    fn test_target_placement() {
        let target = "my_secret_password";
        let wordlist = generate_wordlist_with_target(1000, target, 500);
        assert_eq!(wordlist[500], target);
    }
}
