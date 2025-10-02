#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::UVec3;
use spirv_std::spirv;

// MD5 constants
const A0: u32 = 0x67452301;
const B0: u32 = 0xefcdab89;
const C0: u32 = 0x98badcfe;
const D0: u32 = 0x10325476;

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

/// Main compute shader entry point
/// Processes a batch of messages and checks them against a target hash
#[spirv(compute(threads(64)))]
pub fn md5_crack(
    #[spirv(global_invocation_id)] global_id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] messages: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] target_hash: &[u32; 4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] result_buffer: &mut [i32],
    #[spirv(uniform, descriptor_set = 0, binding = 3)] message_count: &u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] block_offsets: &[u32],
) {
    let idx = global_id.x as usize;

    // Bounds checking
    if idx >= *message_count as usize {
        return;
    }

    let block_start = block_offsets[idx] as usize;
    let block_end = block_offsets[idx + 1] as usize;
    let num_blocks = block_end - block_start;

    if num_blocks == 0 {
        return;
    }

    let mut h = [A0, B0, C0, D0];

    for block_idx in 0..num_blocks {
        // Load preprocessed MD5 block (16 u32 words)
        let base = (block_start + block_idx) * 16;
        let mut m = [0u32; 16];
        let mut i = 0;
        while i < 16 {
            m[i] = messages[base + i];
            i += 1;
        }

        // Compute MD5 round
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];

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

        h = [a, b, c, d];
    }

    // Compare with target
    if h[0] == target_hash[0] && h[1] == target_hash[1] && h[2] == target_hash[2] && h[3] == target_hash[3] {
        result_buffer[0] = idx as i32;
    }
}
