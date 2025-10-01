#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::UVec3;
use spirv_std::spirv;

// MD5 constants
const A0: u32 = 0x67452301;
const B0: u32 = 0xefcdab89;
const C0: u32 = 0x98badcfe;
const D0: u32 = 0x10325476;
const DIGEST_SIZE: usize = 16;
const CHUNK_SIZE: usize = 64;
const WORD_SIZE: usize = 4;
const MAX_MSG_SIZE: usize = 256; // Maximum message size we support

// Shift amounts in each MD5 round
const SHIFT_AMTS: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

// Integer parts of sines of integers
const K_TABLE: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

#[inline]
fn leftrotate(x: u32, amt: u32) -> u32 {
    (x << (amt % 32)) | (x >> (32 - (amt % 32)))
}

// Preprocess a message: add padding and length
#[inline]
fn preprocess_message(msg: &[u8], msg_len: u32, output: &mut [u8]) -> u32 {
    let mut i = 0u32;

    // Copy original message
    while i < msg_len {
        output[i as usize] = msg[i as usize];
        i += 1;
    }

    // Add 0x80 byte
    output[msg_len as usize] = 0x80;
    i = msg_len + 1;

    // Calculate preprocessed size (must be multiple of 64)
    let size_in_bits = msg_len * 8;
    let pre_processed_size = (msg_len + 8 + 1).div_ceil(64) * 64;

    // Zero padding (already handled by buffer initialization)
    while i < pre_processed_size - 8 {
        output[i as usize] = 0;
        i += 1;
    }

    // Add length as 64-bit little-endian integer
    let mut j = 0u32;
    while j < 8 {
        output[(pre_processed_size - 8 + j) as usize] = ((size_in_bits >> (j * 8)) & 0xff) as u8;
        j += 1;
    }

    pre_processed_size
}

// Compute MD5 hash
#[inline]
fn md5_compute(pre_processed_msg: &[u8], pre_processed_size: u32) -> [u32; 4] {
    let mut a = A0;
    let mut b = B0;
    let mut c = C0;
    let mut d = D0;

    let mut chunk_offset = 0u32;

    // Iterate over 64-byte chunks
    while chunk_offset < pre_processed_size {
        let mut words: [u32; 16] = [0; 16];

        // Break chunk into 16 32-bit words (little-endian)
        let mut word_idx = 0u32;
        while word_idx < 16 {
            let base = (chunk_offset + word_idx * 4) as usize;
            words[word_idx as usize] = (pre_processed_msg[base] as u32)
                | ((pre_processed_msg[base + 1] as u32) << 8)
                | ((pre_processed_msg[base + 2] as u32) << 16)
                | ((pre_processed_msg[base + 3] as u32) << 24);
            word_idx += 1;
        }

        // Save original values
        let aa = a;
        let bb = b;
        let cc = c;
        let dd = d;

        // 64 round operations
        let mut i = 0u32;
        while i < 64 {
            let (f, g) = if i <= 15 {
                ((b & c) | ((!b) & d), i)
            } else if i <= 31 {
                ((d & b) | ((!d) & c), (5 * i + 1) % 16)
            } else if i <= 47 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | (!d)), (7 * i) % 16)
            };

            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(leftrotate(
                a.wrapping_add(f)
                    .wrapping_add(K_TABLE[i as usize])
                    .wrapping_add(words[g as usize]),
                SHIFT_AMTS[i as usize],
            ));
            a = temp;

            i += 1;
        }

        // Add to original values
        a = a.wrapping_add(aa);
        b = b.wrapping_add(bb);
        c = c.wrapping_add(cc);
        d = d.wrapping_add(dd);

        chunk_offset += 64;
    }

    [a, b, c, d]
}

/// Main compute shader entry point
/// Processes a batch of messages and checks them against a target hash
#[spirv(compute(threads(64)))]
pub fn md5_crack(
    #[spirv(global_invocation_id)] global_id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] messages: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] message_lengths: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] message_offsets: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] target_hash: &[u32; 4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] result_buffer: &mut [i32],
    #[spirv(uniform, descriptor_set = 0, binding = 5)] message_count: &u32,
) {
    let idx = global_id.x as usize;

    // Bounds checking
    if idx >= *message_count as usize {
        return;
    }

    let msg_len = message_lengths[idx];
    let msg_offset_bytes = message_offsets[idx] as usize;
    let msg_offset = msg_offset_bytes / 4; // Convert byte offset to word offset

    // Preprocess message inline - use u32 array to avoid Int8 capability
    let mut preprocessed: [u32; 128] = [0; 128];

    // Copy message (extract bytes from u32 words and pack into new u32s)
    let mut i = 0u32;
    while i < msg_len {
        let src_word_idx = (msg_offset_bytes + i as usize) / 4;
        let src_byte_in_word = (msg_offset_bytes + i as usize) % 4;
        let byte_val = (messages[src_word_idx] >> (src_byte_in_word * 8)) & 0xFF;

        let dst_word_idx = (i as usize) / 4;
        let dst_byte_in_word = (i as usize) % 4;
        preprocessed[dst_word_idx] |= byte_val << (dst_byte_in_word * 8);
        i += 1;
    }

    // Add padding byte (0x80)
    let pad_byte_idx = msg_len as usize;
    let pad_word_idx = pad_byte_idx / 4;
    let pad_byte_in_word = pad_byte_idx % 4;
    preprocessed[pad_word_idx] |= 0x80u32 << (pad_byte_in_word * 8);

    // Calculate size in bits (use two u32s to represent 64-bit value)
    let size_in_bits_low = msg_len * 8;
    let size_in_bits_high = 0u32; // For messages < 512MB
    let preprocessed_size_words = (msg_len + 8 + 1).div_ceil(64) * 16; // In words, not bytes

    // Add length at the end (64-bit little-endian, split into two u32s)
    preprocessed[(preprocessed_size_words - 2) as usize] = size_in_bits_low;
    preprocessed[(preprocessed_size_words - 1) as usize] = size_in_bits_high;

    // Compute MD5 inline
    let mut a = A0;
    let mut b = B0;
    let mut c = C0;
    let mut d = D0;

    // Process each 64-byte chunk (16 words)
    let num_chunks = preprocessed_size_words / 16;
    let mut chunk_idx = 0u32;

    while chunk_idx < num_chunks {
        let offset = (chunk_idx * 16) as usize;

        // Get 16 32-bit words (already in correct format)
        let mut m = [0u32; 16];
        let mut i = 0;
        while i < 16 {
            m[i] = preprocessed[offset + i];
            i += 1;
        }

        let aa = a;
        let bb = b;
        let cc = c;
        let dd = d;

        // 64 rounds
        let mut i = 0;
        while i < 64 {
            let mut f;
            let g;

            if i < 16 {
                f = (b & c) | ((!b) & d);
                g = i;
            } else if i < 32 {
                f = (d & b) | ((!d) & c);
                g = (5 * i + 1) % 16;
            } else if i < 48 {
                f = b ^ c ^ d;
                g = (3 * i + 5) % 16;
            } else {
                f = c ^ (b | (!d));
                g = (7 * i) % 16;
            }

            f = f
                .wrapping_add(a)
                .wrapping_add(K_TABLE[i])
                .wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(leftrotate(f, SHIFT_AMTS[i]));

            i += 1;
        }

        a = a.wrapping_add(aa);
        b = b.wrapping_add(bb);
        c = c.wrapping_add(cc);
        d = d.wrapping_add(dd);

        chunk_idx += 1;
    }

    // Compare with target
    if a == target_hash[0] && b == target_hash[1] && c == target_hash[2] && d == target_hash[3] {
        result_buffer[0] = idx as i32;
    }
}
