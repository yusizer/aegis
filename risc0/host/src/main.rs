//! Aegis host — builds a demo ASP allow-set Merkle tree off-chain, picks K
//! cleared counterparties, computes their Merkle paths, runs the zkVM guest with
//! a Groth16 receipt, and emits the selector-prefixed seal + image_id + journal
//! digest needed for on-chain verification by the Nethermind groth16-verifier.
use anyhow::Result;
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::sha::Sha256;
use risc0_zkvm::{default_prover, sha::Impl, Digest, ExecutorEnv, ProverOpts};
use sha2::{Digest as Sha2Digest, Sha256 as Sha2};

use aegis_methods::{COMPLIANCE_GUEST_ELF, COMPLIANCE_GUEST_ID};

fn sha256(b: &[u8]) -> [u8; 32] {
    let mut h = Sha2::new();
    h.update(b);
    let r = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&r);
    out
}

fn sha256_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha2::new();
    h.update(a);
    h.update(b);
    let r = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&r);
    out
}

/// Build a Merkle tree (power-of-two leaves) -> (root, paths). paths[i] is the
/// list of (sibling, dir) for leaf i, leaf -> root order. dir 0 = sibling right.
fn build_tree(leaf_ids: &[[u8; 32]]) -> ([u8; 32], Vec<Vec<([u8; 32], u8)>>) {
    let n = leaf_ids.len();
    assert!(n.is_power_of_two() && n >= 2, "need power-of-two leaves");
    let mut layer: Vec<[u8; 32]> = leaf_ids.iter().map(|id| sha256(id)).collect();
    // groups[k] = leaf indices under node k at the current layer
    let mut groups: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut paths = vec![Vec::<([u8; 32], u8)>::new(); n];
    while layer.len() > 1 {
        let mut next = Vec::with_capacity(layer.len() / 2);
        let mut next_groups = Vec::with_capacity(layer.len() / 2);
        for i in (0..layer.len()).step_by(2) {
            let left = layer[i];
            let right = layer[i + 1];
            let left_group = groups[i].clone();
            let right_group = groups[i + 1].clone();
            // left node's sibling is `right` (dir 0) for every leaf under left
            for &leaf in &left_group {
                paths[leaf].push((right, 0u8));
            }
            // right node's sibling is `left` (dir 1) for every leaf under right
            for &leaf in &right_group {
                paths[leaf].push((left, 1u8));
            }
            next.push(sha256_pair(&left, &right));
            let mut g = left_group;
            g.extend(right_group);
            next_groups.push(g);
        }
        layer = next;
        groups = next_groups;
    }
    (layer[0], paths)
}

fn encode_proof_blob(ids: &[[u8; 32]], paths: &[Vec<([u8; 32], u8)>]) -> Vec<u8> {
    let mut blob = Vec::new();
    for (id, path) in ids.iter().zip(paths.iter()) {
        blob.extend_from_slice(id);
        let depth = path.len() as u32;
        blob.extend_from_slice(&depth.to_le_bytes());
        for (sib, dir) in path {
            blob.extend_from_slice(sib);
            blob.push(*dir);
        }
    }
    blob
}

fn parse_hex32(s: &str, label: &str) -> [u8; 32] {
    let v = hex::decode(s).unwrap_or_else(|_| panic!("invalid {label} hex"));
    assert_eq!(v.len(), 32, "{label} must be 32 bytes (64 hex chars)");
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    a
}

fn main() -> Result<()> {
    // Demo ASP allow-set: 4 cleared counterparty ids.
    let leaf_ids: Vec<[u8; 32]> = (0u8..4).map(|i| [i + 1; 32]).collect();
    let (root, paths) = build_tree(&leaf_ids);

    // The wallet's counterparties are leaf 0 and leaf 2 (both cleared).
    let counterparty_ids = vec![leaf_ids[0], leaf_ids[2]];
    let counterparty_paths = vec![paths[0].clone(), paths[2].clone()];
    let proof_blob = encode_proof_blob(&counterparty_ids, &counterparty_paths);

    // CLI args: <wallet_pubkey_hex> <wallet_secret_hex> (both 32 bytes).
    // Defaults keep the M1 demo values so `cargo run` with no args still works.
    let mut args = std::env::args().skip(1);
    let wallet_address: [u8; 32] = match args.next() {
        Some(s) => parse_hex32(&s, "wallet_pubkey"),
        None => [0xA1; 32],
    };
    let wallet_secret: [u8; 32] = match args.next() {
        Some(s) => parse_hex32(&s, "wallet_secret"),
        None => [0x5E; 32],
    };
    eprintln!("wallet_address = {}", hex::encode(wallet_address));
    eprintln!("wallet_secret  = {}", hex::encode(wallet_secret));

    let as_of_block: u64 = 12345;
    let k: u32 = counterparty_ids.len() as u32;

    let env = ExecutorEnv::builder()
        .write(&wallet_address)?
        .write(&root)?
        .write(&as_of_block)?
        .write(&k)?
        .write(&wallet_secret)?
        .write(&proof_blob)?
        .build()?;

    let prover = default_prover();
    let prove_info = prover.prove_with_opts(env, &COMPLIANCE_GUEST_ELF, &ProverOpts::groth16())?;
    let receipt = prove_info.receipt;
    let stats = prove_info.stats;
    eprintln!(
        "STATS segments={} total_cycles={} user_cycles={} paging_cycles={} reserved_cycles={}",
        stats.segments, stats.total_cycles, stats.user_cycles, stats.paging_cycles, stats.reserved_cycles
    );
    receipt.verify(COMPLIANCE_GUEST_ID)?;

    // selector-prefixed seal (what the Nethermind verifier expects)
    let seal = encode_seal(&receipt)?;
    let journal = receipt.journal.bytes.clone();
    let image_id = Digest::from(COMPLIANCE_GUEST_ID);
    let journal_sha256 = Impl::hash_bytes(&journal);

    println!("Seal (hex): {}", hex::encode(&seal));
    println!("Image ID (hex): {}", hex::encode(image_id.as_bytes()));
    println!("Journal (hex): {}", hex::encode(&journal));
    println!("Journal SHA-256 (hex): {}", hex::encode(journal_sha256.as_bytes()));
    println!("K = {k}, root = {}", hex::encode(root));

    // proof.txt for the Stellar CLI: seal\nimage_id\njournal_digest
    let out = format!(
        "{}\n{}\n{}\n",
        hex::encode(&seal),
        hex::encode(image_id.as_bytes()),
        hex::encode(journal_sha256.as_bytes()),
    );
    std::fs::write("proof.txt", out)?;
    println!("wrote proof.txt");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SHA-256 of the empty input — canonical known vector.
    #[test]
    fn sha256_empty_known_vector() {
        assert_eq!(
            hex::encode(sha256(&[])),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// sha256_pair is order-sensitive: H(a||b) != H(b||a).
    #[test]
    fn sha256_pair_is_order_sensitive() {
        let a = [0x11u8; 32];
        let b = [0x22u8; 32];
        assert_ne!(sha256_pair(&a, &b), sha256_pair(&b, &a));
    }

    /// SHA-256 is deterministic.
    #[test]
    fn sha256_is_deterministic() {
        let id = [0x01u8; 32];
        assert_eq!(sha256(&id), sha256(&id));
    }

    /// build_tree on 4 leaves returns a 32-byte root (non-zero).
    #[test]
    fn build_tree_4_leaves_returns_nonzero_root() {
        let leaves: Vec<[u8; 32]> = (0u8..4).map(|i| [i + 1; 32]).collect();
        let (root, paths) = build_tree(&leaves);
        assert_ne!(root, [0u8; 32]);
        assert_eq!(paths.len(), 4);
        // depth-2 tree → 2 siblings per leaf
        assert_eq!(paths[0].len(), 2);
        assert_eq!(paths[3].len(), 2);
    }

    /// Each Merkle path must reconstruct the root from its leaf — the core
    /// membership invariant the Aegis guest relies on.
    #[test]
    fn merkle_paths_reconstruct_root_for_every_leaf() {
        let leaves: Vec<[u8; 32]> = (0u8..8).map(|i| [i + 1; 32]).collect();
        let (root, paths) = build_tree(&leaves);
        for (i, leaf) in leaves.iter().enumerate() {
            let mut node = sha256(leaf);
            for (sib, dir) in &paths[i] {
                node = if *dir == 0 {
                    sha256_pair(&node, sib)
                } else {
                    sha256_pair(sib, &node)
                };
            }
            assert_eq!(node, root, "path for leaf {i} does not reconstruct root");
        }
    }

    /// A wrong sibling must NOT reconstruct the root — tamper detection.
    #[test]
    fn merkle_tampered_sibling_rejected() {
        let leaves: Vec<[u8; 32]> = (0u8..4).map(|i| [i + 1; 32]).collect();
        let (root, paths) = build_tree(&leaves);
        let mut tampered = paths[0].clone();
        tampered[0].0[0] ^= 0xff; // flip bits in first sibling
        let mut node = sha256(&leaves[0]);
        for (sib, dir) in &tampered {
            node = if *dir == 0 { sha256_pair(&node, sib) } else { sha256_pair(sib, &node) };
        }
        assert_ne!(node, root, "tampered sibling must not reconstruct root");
    }

    /// encode_proof_blob round-trips: parse id + depth + (sib,dir) pairs back.
    #[test]
    fn encode_proof_blob_roundtrips() {
        let leaves: Vec<[u8; 32]> = (0u8..4).map(|i| [i + 1; 32]).collect();
        let (root, paths) = build_tree(&leaves);
        let blob = encode_proof_blob(&leaves, &paths);
        // parse manually
        let mut off = 0usize;
        for (i, _leaf) in leaves.iter().enumerate() {
            let mut id = [0u8; 32];
            id.copy_from_slice(&blob[off..off + 32]);
            off += 32;
            let depth = u32::from_le_bytes([blob[off], blob[off + 1], blob[off + 2], blob[off + 3]]);
            off += 4;
            assert_eq!(depth as usize, paths[i].len());
            let mut node = sha256(&id);
            for _ in 0..depth {
                let mut sib = [0u8; 32];
                sib.copy_from_slice(&blob[off..off + 32]);
                off += 32;
                let dir = blob[off];
                off += 1;
                node = if dir == 0 {
                    sha256_pair(&node, &sib)
                } else {
                    sha256_pair(&sib, &node)
                };
            }
            assert_eq!(node, root, "blob path for leaf {i} does not reconstruct root");
        }
        assert_eq!(off, blob.len(), "blob fully consumed");
    }

    /// build_tree rejects non-power-of-two leaf counts (guard for demo sizing).
    #[test]
    #[should_panic(expected = "need power-of-two leaves")]
    fn build_tree_rejects_non_power_of_two() {
        let leaves: Vec<[u8; 32]> = (0u8..3).map(|i| [i + 1; 32]).collect();
        let _ = build_tree(&leaves);
    }

    /// Demo invariant: the README's published root (depth-2, 4 leaves [1..]).
    /// Pins the exact value judges see on the web demo / stellar.expert.
    #[test]
    fn demo_allow_set_root_matches_readme() {
        let leaves: Vec<[u8; 32]> = (0u8..4).map(|i| [i + 1; 32]).collect();
        let (root, _) = build_tree(&leaves);
        // README §Contracts: demo allow-set root (depth-2, 4 leaves)
        assert_eq!(
            hex::encode(root),
            "48c73f7821a58a8d2a703e5b39c571c0aa20cf14abcd0af8f2b955bc202998de"
        );
    }
}

