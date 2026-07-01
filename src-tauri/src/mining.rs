use flate2::Compression;
use flate2::write::ZlibEncoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;

use hexchain_p2p::block::HexBlock;
use hexchain_p2p::lattice_geometry::HCPCoord;
use hexchain_p2p::types::{BlockHash, TensorMeta, NEIGHBOR_SLOTS, NEIGHBOR_SLOT_EMPTY};

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
        let path_distance_max = c["path_distance_max"].as_i64().unwrap_or(8) as u32;
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
        let mut best_dist = u32::MAX;

        for nonce in 0..max_iter {
            let actual = compute_actual_path(&output_data, nonce as u64, &layer_widths);
            let dist = hamming_distance(&exp_path, &actual);
            if dist < best_dist {
                best_dist = dist;
            }

            if dist <= path_distance_max {
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

        let coord = HCPCoord {
            q: c["coord"]["q"].as_i64().unwrap_or(0) as i32,
            r: c["coord"]["r"].as_i64().unwrap_or(0) as i32,
            s: c["coord"]["s"].as_i64().unwrap_or(0) as i32,
        };

        let target_hex = c["target"].as_str().unwrap_or("");
        let target_bytes = hex::decode(target_hex).unwrap_or_default();
        let target: BlockHash = {
            let mut arr = [0u8; 32];
            let len = target_bytes.len().min(32);
            arr[..len].copy_from_slice(&target_bytes[..len]);
            arr
        };

        let created_at = c["created_at_unix"].as_i64().unwrap_or(0) as u64;

        let nb_arr = c["neighbor_hashes"].as_array().cloned().unwrap_or_default();
        let mut neighbor_hashes = [NEIGHBOR_SLOT_EMPTY; NEIGHBOR_SLOTS];
        for (i, val) in nb_arr.iter().enumerate() {
            if i >= NEIGHBOR_SLOTS {
                break;
            }
            let bytes = if let Some(s) = val.as_str() {
                hex::decode(s).unwrap_or_default()
            } else if let Some(arr) = val.as_array() {
                arr.iter().filter_map(|v| v.as_i64().map(|x| x as u8)).collect()
            } else {
                vec![0u8; 32]
            };
            let mut slot = [0u8; 32];
            let len = bytes.len().min(32);
            slot[..len].copy_from_slice(&bytes[..len]);
            neighbor_hashes[i] = slot;
        }

        let max_iter = config.max_iterations as usize;

        for nonce in 0..max_iter {
            let block = HexBlock {
                parent_hash: neighbor_hashes[0],
                tx_merkle_root: [0u8; 32],
                timestamp: created_at,
                nonce: nonce as u64,
                coord,
                neighbor_hashes,
                tensor: TensorMeta {
                    expected_capacity: 1000,
                    actual_capacity: 1000,
                    compression_num: 95,
                    compression_den: 100,
                },
            };

            let hv = block.pow_hash();
            if hv <= target {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let proof = serde_json::json!({
                    "challenge_id": challenge_id,
                    "block": {
                        "parent_hash": block.parent_hash.iter().map(|&b| b as i64).collect::<Vec<_>>(),
                        "tx_merkle_root": block.tx_merkle_root.iter().map(|&b| b as i64).collect::<Vec<_>>(),
                        "timestamp": block.timestamp,
                        "nonce": block.nonce,
                        "coord": {"q": block.coord.q, "r": block.coord.r, "s": block.coord.s},
                        "neighbor_hashes": block.neighbor_hashes.iter().map(|h| h.iter().map(|&b| b as i64).collect::<Vec<_>>()).collect::<Vec<_>>(),
                        "tensor": {
                            "expected_capacity": block.tensor.expected_capacity,
                            "actual_capacity": block.tensor.actual_capacity,
                            "compression_num": block.tensor.compression_num,
                            "compression_den": block.tensor.compression_den,
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

// ── Neural path (XOR nonce bits — matching pot-o-validator v0.7.3) ────────

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
    nonce: u64,
    layer_widths: &[usize],
) -> Vec<u8> {
    let mut activations: Vec<f64> = output_floats.to_vec();
    let mut path_bits = Vec::new();
    let mut bit_idx: u32 = 0;

    for &width in layer_widths {
        let stride = (activations.len() / width).max(1);
        let mut layer_out = Vec::with_capacity(width);
        for j in 0..width {
            let start = j * stride;
            let end = (start + stride).min(activations.len());
            let s: f64 = activations[start..end].iter().sum();
            let val = s.max(0.0);
            layer_out.push(val);

            let base_bit = if val > 0.0 { 1u8 } else { 0u8 };
            let shift = (bit_idx as u64) % 64;
            let nonce_bit = ((nonce >> shift) & 1) as u8;
            let bit = base_bit ^ nonce_bit;
            path_bits.push(bit);
            bit_idx = bit_idx.wrapping_add(1);
        }
        activations = layer_out;
    }
    path_bits
}

fn hamming_distance(a: &[u8], b: &[u8]) -> u32 {
    let min_len = a.len().min(b.len());
    (0..min_len).filter(|&i| a[i] != b[i]).count() as u32
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
