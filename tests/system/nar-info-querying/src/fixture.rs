use fastrand::Rng;

pub struct TestFixtures {
    contents: Vec<Vec<u8>>,
}

impl TestFixtures {
    pub fn new(count: usize, seed: u64) -> Self {
        let mut rng = Rng::with_seed(seed);
        let mut contents = Vec::with_capacity(count);

        for i in 0..count {
            contents.push(generate_content(i, count, &mut rng));
        }

        Self { contents }
    }

    pub fn contents(&self) -> &[Vec<u8>] {
        &self.contents
    }
}

fn generate_content(index: usize, total: usize, rng: &mut Rng) -> Vec<u8> {
    if index == 0 {
        return Vec::new();
    }
    if index == 1 {
        return b"\x00\x01\x02\x03\x04\x05\x06\x07".to_vec();
    }

    let size_bucket = index * 4 / total;
    match size_bucket {
        0 => random_bytes(rng.usize(1..100), rng),
        1 => random_bytes(rng.usize(100..1_000), rng),
        2 => random_bytes(rng.usize(1_000..50_000), rng),
        _ => random_bytes(rng.usize(50_000..500_000), rng),
    }
}

fn random_bytes(len: usize, rng: &mut Rng) -> Vec<u8> {
    (0..len).map(|_| rng.u8(..)).collect()
}

pub fn generate_invalid_hash(rng: &mut Rng) -> String {
    const NIX_BASE32: &[u8] = b"0123456789abcdfghijklmnpqrsvwxyz";
    (0..32)
        .map(|_| {
            let idx = rng.usize(..NIX_BASE32.len());
            NIX_BASE32[idx] as char
        })
        .collect()
}
