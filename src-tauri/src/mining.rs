use flate2::Compression;
use flate2::write::ZlibEncoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;

#[derive(Debug, Serialize, Deserialize)]
pub struct MiningResult {
    pub status: String,
    pub proof: Option<Value>,
    pub mml_score: Option<f64>,
    pub reason: Option<String>,
}

pub struct MiningEngine;

impl MiningEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn mine_pot_o(&self, challenge: Value, config: &super::config::PotOConfig) -> MiningResult {
        let c = challenge;
        let challenge_id = c["id"].as_str().unwrap_or("").to_string();
        let slot_hash = c["slot_hash"].as_str().unwrap_or("");
        let op_name = if !config.operation.is_empty() {
            &config.operation
        } else {
            c["operation_type"].as_str().unwrap_or("relu")
        };
        let mml_threshold = if !config.mml_threshold.is_empty() {
            config.mml_threshold.parse::<f64>().unwrap_or(0.85)
        } else {
            c["mml_threshold"].as_f64().unwrap_or(0.85)
        };
        let path_distance_max = c["path_distance_max"].as_i64().unwrap_or(8) as u64;
        let max_dim = config.max_tensor_dim;
        let tensor_dim = c["max_tensor_dim"].as_i64().unwrap_or(64).min(max_dim as i64) as u64;

        let layer_widths: Vec<usize> = if config.path_layers.is_empty() {
            vec![32, 16, 8]
        } else {
            config
                .path_layers
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .collect()
        };
        let layer_widths = if layer_widths.is_empty() {
            vec![32, 16, 8]
        } else {
            layer_widths
        };

        let shape = c["input_tensor"]["shape"]["dims"].as_array();
        let (rows, cols) = if let Some(dims) = shape {
            let r = dims
                .first()
                .and_then(|v| v.as_i64())
                .unwrap_or(tensor_dim as i64)
                .min(max_dim as i64) as u64;
            let c = dims
                .get(1)
                .and_then(|v| v.as_i64())
                .unwrap_or(r as i64)
                .min(max_dim as i64) as u64;
            (r, c)
        } else {
            (tensor_dim, tensor_dim)
        };
        let total = (rows * cols) as usize;

        let f32_data = c["input_tensor"]["data"]["F32"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut input_data: Vec<f64> = f32_data
            .iter()
            .filter_map(|v| v.as_f64())
            .take(total)
            .collect();
        while input_data.len() < total {
            let seed = input_data.len() as f64 * 0.61803399;
            input_data.push(seed - seed.floor());
        }

        let output_data = tensor_op(op_name, &input_data, rows as usize, cols as usize);
        let (out_rows, out_cols) = match op_name {
            "convolution" => (1, output_data.len()),
            "dot_product" => (1, 1),
            _ => (rows as usize, cols as usize),
        };

        let mml_score = compute_mml_score(&input_data, &output_data);

        let mut result = MiningResult {
            status: "no_proof".into(),
            proof: None,
            mml_score: Some(mml_score),
            reason: None,
        };

        if mml_score > mml_threshold {
            result.reason = Some("mml_threshold_not_met".into());
            return result;
        }

        let exp_path = expected_path_signature(&challenge_id, &layer_widths);
        let tensor_hash = compute_tensor_hash(&output_data, &[out_rows, out_cols]);

        let max_iter = config.max_iterations as usize;
        let mut best_dist = usize::MAX;

        for nonce in 0..max_iter {
            let actual = compute_actual_path(&output_data, nonce, &layer_widths);
            let dist = hamming_distance(&exp_path, &actual);
            if dist < best_dist {
                best_dist = dist;
            }

            if dist as u64 <= path_distance_max {
                let path_sig = path_to_hex(&actual);
                let comp_hash = compute_proof_hash(
                    &challenge_id,
                    &tensor_hash,
                    mml_score,
                    &path_sig,
                    nonce as u64,
                );

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let proof = serde_json::json!({
                    "challenge_id": challenge_id,
                    "challenge_hash": slot_hash,
                    "tensor_result_hash": tensor_hash,
                    "mml_score": mml_score,
                    "path_signature": path_sig,
                    "path_distance": dist,
                    "computation_nonce": nonce,
                    "computation_hash": comp_hash,
                    "miner_pubkey": config.miner_pubkey,
                    "timestamp": now,
                });

                result.status = "proof_found".into();
                result.proof = Some(proof);
                return result;
            }
        }

        result.reason = Some("max_iterations_reached".into());
        result
    }

    pub fn mine_hexchain(
        &self,
        challenge: Value,
        config: &super::config::PotOConfig,
    ) -> MiningResult {
        let c = challenge;
        let challenge_id = c["id"].as_str().unwrap_or("").to_string();
        let target_bytes = c["target"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_i64().map(|x| x as u8))
                    .collect::<Vec<u8>>()
            })
            .unwrap_or_default();

        let coord = (
            c["coord"]["q"].as_i64().unwrap_or(0) as i32,
            c["coord"]["r"].as_i64().unwrap_or(0) as i32,
            c["coord"]["s"].as_i64().unwrap_or(0) as i32,
        );

        let nb_hashes: Vec<Vec<u8>> = c["neighbor_hashes"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|h| {
                        if let Some(list) = h.as_array() {
                            list.iter()
                                .filter_map(|v| v.as_i64().map(|x| x as u8))
                                .collect()
                        } else if let Some(s) = h.as_str() {
                            hex::decode(s).unwrap_or_default()
                        } else {
                            vec![0u8; 32]
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let nb_merkle = merkle_root_neighbors(&nb_hashes);

        let parent_hash = nb_hashes.first().cloned().unwrap_or(vec![0u8; 32]);
        let created_at = c["created_at_unix"].as_i64().unwrap_or(0) as u64;

        let max_iter = config.max_iterations as usize;

        let pre_prefix = {
            let mut buf = Vec::new();
            buf.extend_from_slice(&parent_hash);
            buf.extend_from_slice(&[0u8; 32]); // tx_merkle_root placeholder
            buf.extend_from_slice(&created_at.to_le_bytes());
            // nonce bytes (8 bytes) go here at offset 72
            buf
        }; // length = 72 bytes

        let pre_suffix = {
            let mut buf = Vec::new();
            buf.extend_from_slice(&coord.0.to_le_bytes());
            buf.extend_from_slice(&coord.1.to_le_bytes());
            buf.extend_from_slice(&coord.2.to_le_bytes());
            buf.extend_from_slice(&nb_merkle);
            buf.extend_from_slice(&1000u64.to_le_bytes()); // expected_capacity
            buf.extend_from_slice(&1000u64.to_le_bytes()); // actual_capacity
            buf.extend_from_slice(&95u64.to_le_bytes()); // compression_num
            buf.extend_from_slice(&100u64.to_le_bytes()); // compression_den
            buf
        };

        for nonce in 0..max_iter {
            let mut preimage = Vec::new();
            preimage.extend_from_slice(&pre_prefix);
            preimage.extend_from_slice(&(nonce as u64).to_le_bytes());
            preimage.extend_from_slice(&pre_suffix);

            let hash = sha256_once(&preimage);
            if hash.as_slice() <= target_bytes.as_slice() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let proof = serde_json::json!({
                    "challenge_id": challenge_id,
                    "block": {
                        "parent_hash": parent_hash.iter().map(|&b| b as i64).collect::<Vec<_>>(),
                        "tx_merkle_root": vec![0i64; 32],
                        "timestamp": created_at,
                        "nonce": nonce,
                        "coord": {"q": coord.0, "r": coord.1, "s": coord.2},
                        "neighbor_hashes": nb_hashes.iter().map(|h| h.iter().map(|&b| b as i64).collect::<Vec<_>>()).collect::<Vec<_>>(),
                        "tensor": {
                            "expected_capacity": 1000u64,
                            "actual_capacity": 1000u64,
                            "compression_num": 95u64,
                            "compression_den": 100u64,
                        },
                    },
                    "miner_pubkey": config.miner_pubkey,
                    "timestamp_unix": now,
                });

                return MiningResult {
                    status: "proof_found".into(),
                    proof: Some(proof),
                    mml_score: None,
                    reason: None,
                };
            }
        }

        MiningResult {
            status: "no_proof".into(),
            proof: None,
            mml_score: None,
            reason: Some("max_iterations_reached".into()),
        }
    }
}

// ── Tensor operations ─────────────────────────────────────────────────────────

fn tensor_op(op_name: &str, data: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    match op_name {
        "matrix_multiply" => op_matrix_multiply(data, rows, cols),
        "convolution" => op_convolution(data),
        "relu" => data.iter().map(|&x| x.max(0.0)).collect(),
        "sigmoid" => data.iter().map(|&x| sigmoid(x)).collect(),
        "tanh" => data.iter().map(|&x| x.tanh()).collect(),
        "dot_product" => op_dot_product(data),
        "normalize" => op_normalize(data),
        _ => data.iter().map(|&x| x.max(0.0)).collect(),
    }
}

fn op_matrix_multiply(data: &[f64], _rows: usize, _cols: usize) -> Vec<f64> {
    let dim = (data.len() as f64).sqrt() as usize;
    if dim == 0 {
        return vec![0.0];
    }
    let size = dim * dim;
    let a: Vec<f64> = data.iter().take(size).copied().collect();
    let mut result = vec![0.0f64; size];
    for i in 0..dim {
        for j in 0..dim {
            let mut s = 0.0;
            for k in 0..dim {
                let ai = a.get(i * dim + k).copied().unwrap_or(0.0);
                let bj = a.get(k * dim + j).copied().unwrap_or(0.0);
                s += ai * bj;
            }
            result[i * dim + j] = s;
        }
    }
    result
}

fn op_convolution(data: &[f64]) -> Vec<f64> {
    let kernel = [0.25, 0.5, 0.25];
    let klen = kernel.len();
    if data.len() < klen {
        return data.to_vec();
    }
    let out_len = data.len() - klen + 1;
    let mut result = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let s: f64 = kernel
            .iter()
            .enumerate()
            .map(|(j, &k)| data[i + j] * k)
            .sum();
        result.push(s);
    }
    result
}

fn sigmoid(x: f64) -> f64 {
    if x < -500.0 {
        return 0.0;
    }
    if x > 500.0 {
        return 1.0;
    }
    1.0 / (1.0 + (-x).exp())
}

fn op_dot_product(data: &[f64]) -> Vec<f64> {
    let half = data.len() / 2;
    let dot: f64 = data[..half]
        .iter()
        .zip(data[half..].iter())
        .map(|(&a, &b)| a * b)
        .sum();
    vec![dot]
}

fn op_normalize(data: &[f64]) -> Vec<f64> {
    let mag: f64 = data.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag > 1e-7 {
        data.iter().map(|x| x / mag).collect()
    } else {
        data.to_vec()
    }
}

// ── MML Compression ─────────────────────────────────────────────────────────

fn float_list_to_bytes(floats: &[f64]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(floats.len() * 4);
    for &f in floats {
        bytes.extend_from_slice(&(f as f32).to_le_bytes());
    }
    bytes
}

fn compressed_length(data: &[u8]) -> usize {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(6));
    let _ = encoder.write_all(data);
    encoder.finish().map(|v| v.len()).unwrap_or(0)
}

fn compute_mml_score(input: &[f64], output: &[f64]) -> f64 {
    let in_bytes = float_list_to_bytes(input);
    let out_bytes = float_list_to_bytes(output);
    let in_comp = compressed_length(&in_bytes);
    let out_comp = compressed_length(&out_bytes);
    if in_comp == 0 {
        return 1.0;
    }
    out_comp as f64 / in_comp as f64
}

// ── Neural path ────────────────────────────────────────────────────────────

fn expected_path_signature(challenge_id_hex: &str, layer_widths: &[usize]) -> Vec<u8> {
    let hash_bytes = hex::decode(challenge_id_hex).unwrap_or(vec![0u8; 32]);
    let mut seed = Sha256::digest(&hash_bytes);
    let mut sig = Vec::new();
    for &width in layer_widths {
        for i in 0..width {
            let byte_idx = i % seed.len();
            let bit = (seed[byte_idx] >> (i % 8)) & 1;
            sig.push(bit);
        }
        seed = Sha256::digest(&seed);
    }
    sig
}

fn compute_actual_path(
    output_floats: &[f64],
    nonce: usize,
    layer_widths: &[usize],
) -> Vec<u8> {
    let mut activations: Vec<f64> = output_floats.to_vec();
    for i in 0..activations.len() {
        let nc = ((nonce + i) as f64 * 1e-6).sin() * 0.1;
        activations[i] += nc;
    }
    let mut path_bits = Vec::new();
    let mut current = activations;
    for &width in layer_widths {
        let stride = (current.len() / width).max(1);
        let mut layer_out = Vec::with_capacity(width);
        for j in 0..width {
            let start = j * stride;
            let end = (start + stride).min(current.len());
            let s: f64 = current[start..end].iter().sum();
            let val = s.max(0.0);
            layer_out.push(val);
            path_bits.push(if val > 0.0 { 1 } else { 0 });
        }
        current = layer_out;
    }
    path_bits
}

fn hamming_distance(a: &[u8], b: &[u8]) -> usize {
    let min_len = a.len().min(b.len());
    (0..min_len).filter(|&i| a[i] != b[i]).count()
}

fn path_to_hex(path: &[u8]) -> String {
    let mut bytes = Vec::new();
    for chunk in path.chunks(8) {
        let mut byte_val = 0u8;
        for (bi, &bit) in chunk.iter().enumerate() {
            if bit != 0 {
                byte_val |= 1 << bi;
            }
        }
        bytes.push(byte_val);
    }
    hex::encode(&bytes)
}

// ── Hashing ────────────────────────────────────────────────────────────────

fn compute_tensor_hash(floats: &[f64], shape_dims: &[usize]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(&float_list_to_bytes(floats));
    for &d in shape_dims {
        hasher.update(&(d as u64).to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn compute_proof_hash(
    challenge_id: &str,
    tensor_hash: &str,
    mml_score: f64,
    path_sig: &str,
    nonce: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(challenge_id.as_bytes());
    hasher.update(tensor_hash.as_bytes());
    hasher.update(&mml_score.to_le_bytes());
    hasher.update(path_sig.as_bytes());
    hasher.update(&nonce.to_le_bytes());
    hex::encode(hasher.finalize())
}

// ── Hexchain helpers ──────────────────────────────────────────────────────

fn sha256_once(data: &[u8]) -> Vec<u8> {
    Sha256::digest(data).to_vec()
}

fn sha256_pair(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(a);
    hasher.update(b);
    hasher.finalize().to_vec()
}

fn sha256_double_pair(a: &[u8], b: &[u8]) -> Vec<u8> {
    let inner = sha256_pair(a, b);
    sha256_once(&inner)
}

fn merkle_root_neighbors(leaves: &[Vec<u8>]) -> Vec<u8> {
    let mut level: Vec<Vec<u8>> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::new();
        for i in (0..level.len()).step_by(2) {
            if i + 1 < level.len() {
                next.push(sha256_double_pair(&level[i], &level[i + 1]));
            } else {
                next.push(sha256_double_pair(&level[i], &level[i]));
            }
        }
        level = next;
    }
    level.into_iter().next().unwrap_or_else(|| vec![0u8; 32])
}
