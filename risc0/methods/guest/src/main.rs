//! Aegis compliance guest — proves all of a wallet's counterparties are members
//! of an ASP allow-set Merkle tree (root public) without revealing them.
#![cfg_attr(target_os = "zkvm", no_std)]
#![cfg_attr(target_os = "zkvm", no_main)]
extern crate alloc;

#[cfg(target_os = "zkvm")]
mod guest {
    use alloc::vec::Vec;
    use risc0_zkvm::guest::env;
    use sha2::{Digest, Sha256};

    /// Journal layout (109 bytes), committed raw via env::commit_slice:
    ///   [  0..32 ] wallet_address
    ///   [ 32..64 ] allow_set_root
    ///   [ 64..96 ] nullifier = SHA256(wallet_secret || allow_set_root)
    ///   [ 96..100] K          (u32 LE)
    ///   [100..108] as_of_block (u64 LE)
    ///   [ 108    ] pass        (1 = cleared)
    const JOURNAL_LEN: usize = 109;

    pub fn main() {
        let wallet_address: [u8; 32] = env::read();
        let allow_set_root: [u8; 32] = env::read();
        let as_of_block: u64 = env::read();
        let k: u32 = env::read();
        let wallet_secret: [u8; 32] = env::read();
        // Encoded Merkle proofs: for each of k counterparties:
        //   32-byte id, u32 LE depth, then depth * (32-byte sibling + 1-byte dir)
        let proof_blob: Vec<u8> = env::read();

        let mut off = 0usize;
        for _ in 0..k {
            let mut id = [0u8; 32];
            id.copy_from_slice(&proof_blob[off..off + 32]);
            off += 32;
            let depth = u32::from_le_bytes([
                proof_blob[off],
                proof_blob[off + 1],
                proof_blob[off + 2],
                proof_blob[off + 3],
            ]);
            off += 4;
            let mut node = sha256(&id);
            for _ in 0..depth {
                let mut sib = [0u8; 32];
                sib.copy_from_slice(&proof_blob[off..off + 32]);
                off += 32;
                let dir = proof_blob[off];
                off += 1;
                node = if dir == 0 {
                    sha256_pair(&node, &sib)
                } else {
                    sha256_pair(&sib, &node)
                };
            }
            if node != allow_set_root {
                panic!("counterparty not in ASP allow-set");
            }
        }

        let nullifier = sha256_pair(&wallet_secret, &allow_set_root);
        let pass: u8 = 1u8;

        let mut journal = [0u8; JOURNAL_LEN];
        journal[0..32].copy_from_slice(&wallet_address);
        journal[32..64].copy_from_slice(&allow_set_root);
        journal[64..96].copy_from_slice(&nullifier);
        journal[96..100].copy_from_slice(&k.to_le_bytes());
        journal[100..108].copy_from_slice(&as_of_block.to_le_bytes());
        journal[108] = pass;
        env::commit_slice(&journal);
    }

    fn sha256(b: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b);
        let r = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&r);
        out
    }

    fn sha256_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(a);
        h.update(b);
        let r = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&r);
        out
    }
}

#[cfg(target_os = "zkvm")]
risc0_zkvm::guest::entry!(guest::main);

#[cfg(not(target_os = "zkvm"))]
fn main() {}
