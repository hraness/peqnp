//! FIPS 180-4 SHA-256, one-shot and streaming, standard library only.
//!
//! The oracle uses this digest to bind protocol documents, formulas, and
//! proofs to their exact bytes. It is a plain reference implementation with
//! no platform intrinsics; correctness, not throughput, is the goal.

const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

const H0: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

/// One 64-byte block of the compression function. Additions are modular by
/// specification, so `wrapping_add` is the intended semantics here.
fn compress(state: &mut [u32; 8], block: &[u8]) {
    debug_assert_eq!(block.len(), 64);
    let mut w = [0_u32; 64];
    for (i, word) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *slot = slot.wrapping_add(value);
    }
}

/// Streaming SHA-256: feed bytes with [`Sha256::update`] and close with
/// [`Sha256::finish`]. The oracle uses it to digest proof text as it is read.
#[derive(Clone, Debug)]
pub struct Sha256 {
    state: [u32; 8],
    /// Unprocessed tail, always shorter than one block.
    pending: Vec<u8>,
    /// Bytes absorbed so far, checked against the 2^61-byte domain at finish.
    length: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: H0,
            pending: Vec::with_capacity(64),
            length: 0,
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.length = u64::try_from(bytes.len())
            .ok()
            .and_then(|n| self.length.checked_add(n))
            .expect("message length exceeds the SHA-256 domain");
        let mut rest = bytes;
        if !self.pending.is_empty() {
            let take = (64 - self.pending.len()).min(rest.len());
            self.pending.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.pending.len() == 64 {
                compress(&mut self.state, &self.pending);
                self.pending.clear();
            }
        }
        let mut blocks = rest.chunks_exact(64);
        for block in &mut blocks {
            compress(&mut self.state, block);
        }
        self.pending.extend_from_slice(blocks.remainder());
    }

    pub fn finish(mut self) -> [u8; 32] {
        let bit_length = self
            .length
            .checked_mul(8)
            .expect("message length exceeds the SHA-256 domain");
        // Padding: 0x80, zeros to 56 mod 64, then the big-endian 64-bit bit length.
        let mut tail = std::mem::take(&mut self.pending);
        tail.push(0x80);
        while tail.len() % 64 != 56 {
            tail.push(0);
        }
        tail.extend_from_slice(&bit_length.to_be_bytes());
        for block in tail.chunks_exact(64) {
            compress(&mut self.state, block);
        }
        let mut digest = [0_u8; 32];
        for (chunk, word) in digest.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        digest
    }
}

/// One-shot SHA-256 digest of `bytes`.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finish()
}

/// A digest as 64 lowercase hexadecimal characters.
pub fn hex(digest: &[u8; 32]) -> String {
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('?'));
        text.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('?'));
    }
    text
}

/// SHA-256 digest of `bytes` as 64 lowercase hexadecimal characters.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&sha256(bytes))
}

#[cfg(test)]
mod tests {
    use super::{hex, sha256, sha256_hex, Sha256};
    use std::process::Command;

    #[test]
    fn streaming_split_at_every_boundary_matches_one_shot() {
        let message = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        let expected = sha256(message);
        assert_eq!(
            hex(&expected),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        for split in 0..=message.len() {
            let mut hasher = Sha256::new();
            hasher.update(&message[..split]);
            hasher.update(&message[split..]);
            assert_eq!(hasher.finish(), expected, "split at {split}");
        }
        let mut byte_wise = Sha256::default();
        for byte in message {
            byte_wise.update(&[*byte]);
        }
        assert_eq!(byte_wise.finish(), expected);
    }

    #[test]
    fn nist_empty() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn nist_abc() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn nist_two_block() {
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn nist_million_a() {
        // Verified with `/usr/bin/shasum -a 256` on a generated file of 1,000,000 'a'.
        let message = vec![b'a'; 1_000_000];
        assert_eq!(
            sha256_hex(&message),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn padding_boundaries() {
        // Lengths around the 55/56/64-byte padding edges must all round-trip
        // through the two-block tail without disagreeing with the one-shot path.
        for len in [55_usize, 56, 63, 64, 65, 119, 120, 127, 128] {
            let message = vec![0x61_u8; len];
            let hex = sha256_hex(&message);
            assert_eq!(hex.len(), 64, "length {len}");
            assert!(hex.bytes().all(|b| b.is_ascii_hexdigit()), "length {len}");
        }
        assert_eq!(
            sha256_hex(&[0x61_u8; 56]),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
        );
    }

    #[test]
    fn matches_shasum_on_cargo_manifest() {
        let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        let bytes = std::fs::read(manifest).expect("read Cargo.toml");
        let output = match Command::new("/usr/bin/shasum")
            .arg("-a")
            .arg("256")
            .arg(manifest)
            .output()
        {
            Ok(output) if output.status.success() => output,
            _ => {
                eprintln!("skipping: /usr/bin/shasum unavailable");
                return;
            }
        };
        let text = String::from_utf8_lossy(&output.stdout);
        let expected = text.split_whitespace().next().unwrap_or_default();
        assert_eq!(sha256_hex(&bytes), expected);
    }
}
