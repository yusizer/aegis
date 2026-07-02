//! # Aegis — Provable Clean-Funds Compliance Registry
//!
//! A Soroban smart contract that gates stablecoin transfers on a **zero-knowledge
//! proof of clean funds**.
//!
//! A wallet proves (off-chain, via a RISC Zero zkVM guest program) that all of its
//! recent counterparties are members of an on-chain *Association Set Provider*
//! (ASP) allow-set of screened/cleared addresses — **without revealing which
//! counterparties it has**. The proof is a Groth16 seal verified on Stellar by the
//! Nethermind `stellar-risc0-verifier`. On success, the wallet is marked cleared
//! for a TTL window and may transfer USDC through the gate.
//!
//! ## Journal layout (109 bytes, committed via `env::commit_slice` in the guest)
//! ```text
//!   [  0..32 ] wallet_address      (32 bytes)
//!   [ 32..64 ] allow_set_root      (32 bytes)
//!   [ 64..96 ] nullifier           (32 bytes)
//!   [ 96..100] K                   (u32 little-endian)
//!   [100..108] as_of_block         (u64 little-endian)
//!   [ 108    ] pass                (1 byte, 1 = cleared)
//! ```
//!
//! > Status: hackathon prototype, not audited. The on-chain Groth16 verifier is
//! > itself a Stellar dev preview. See README for limitations.

#![no_std]

use soroban_sdk::address_payload::AddressPayload;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes, BytesN,
    Env, IntoVal, InvokeError, Symbol, Val, Vec,
};

/// Fixed journal length produced by the Aegis compliance guest.
const JOURNAL_LEN: usize = 109;

#[contract]
pub struct ComplianceRegistry;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Clearance {
    /// Ledger sequence until which the wallet is considered cleared.
    pub cleared_until_ledger: u32,
    /// Number of counterparties screened.
    pub k: u32,
    /// ASP-root block/sequence the proof was generated against.
    pub as_of_block: u64,
    /// Anti-replay nullifier from the proof.
    pub nullifier: BytesN<32>,
}

#[contracttype]
enum DataKey {
    Admin,
    VerifierId,
    ImageId,
    AllowSetRoot,
    Ttl,
    NullifierUsed(BytesN<32>),
    Cleared(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotAdmin = 3,
    BadJournalLength = 4,
    ProofNotPassed = 5,
    RootMismatch = 6,
    WalletMismatch = 7,
    NullifierReused = 8,
    NotCleared = 9,
    ProofVerificationFailed = 10,
    BadImageId = 11,
    UnsupportedAddress = 12,
}

fn require_admin(env: &Env) {
    let admin: Address = env
        .storage()
        .persistent()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized));
    admin.require_auth();
}

/// Extract the 32-byte identifier of an Address (ed25519 account pubkey or
/// contract hash), matching what the Aegis guest commits.
fn address_32(env: &Env, addr: &Address) -> [u8; 32] {
    match addr.to_payload() {
        Some(AddressPayload::AccountIdPublicKeyEd25519(pk)) => {
            let mut out = [0u8; 32];
            pk.copy_into_slice(&mut out);
            out
        }
        Some(AddressPayload::ContractIdHash(h)) => {
            let mut out = [0u8; 32];
            h.copy_into_slice(&mut out);
            out
        }
        _ => panic_with_error!(env, Error::UnsupportedAddress),
    }
}

/// Invoke the deployed RISC Zero Groth16 verifier's `verify(seal, image_id, journal)`.
fn verify_proof(
    env: &Env,
    verifier: &Address,
    seal: &Bytes,
    image_id: &BytesN<32>,
    journal_digest: &BytesN<32>,
) {
    let mut args: Vec<Val> = Vec::new(env);
    args.push_back(seal.into_val(env));
    args.push_back(image_id.into_val(env));
    args.push_back(journal_digest.into_val(env));
    let result =
        env.try_invoke_contract::<Val, InvokeError>(verifier, &Symbol::new(env, "verify"), args);
    match result {
        Ok(Ok(_)) => {}
        _ => panic_with_error!(env, Error::ProofVerificationFailed),
    }
}

#[contractimpl]
impl ComplianceRegistry {
    /// Initialize the registry. `image_id` is the expected Aegis guest program id;
    /// `allow_set_root` is the current ASP allow-set Merkle root; `ttl_ledgers` is
    /// how many ledgers a clearance is valid after registration.
    pub fn init(
        env: Env,
        admin: Address,
        verifier_id: Address,
        image_id: BytesN<32>,
        allow_set_root: BytesN<32>,
        ttl_ledgers: u32,
    ) {
        let s = env.storage().persistent();
        if s.has(&DataKey::Admin) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::VerifierId, &verifier_id);
        s.set(&DataKey::ImageId, &image_id);
        s.set(&DataKey::AllowSetRoot, &allow_set_root);
        s.set(&DataKey::Ttl, &ttl_ledgers);
    }

    /// Admin: rotate the ASP allow-set root (e.g. after new clearances / revocations).
    pub fn set_allow_set_root(env: Env, root: BytesN<32>) {
        require_admin(&env);
        env.storage().persistent().set(&DataKey::AllowSetRoot, &root);
    }

    /// Register a ZK proof of clean funds for `wallet`. On success, marks the wallet
    /// cleared for the TTL window. `wallet` must authorize.
    pub fn register_compliance(
        env: Env,
        wallet: Address,
        journal: Bytes,
        seal: Bytes,
        image_id: BytesN<32>,
    ) {
        wallet.require_auth();

        let s = env.storage().persistent();

        // 1. Image id must match the expected Aegis guest program.
        let expected_image: BytesN<32> = s
            .get(&DataKey::ImageId)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
        if expected_image != image_id {
            panic_with_error!(&env, Error::BadImageId);
        }

        // 2. Hash the journal and verify the Groth16 seal on-chain.
        if journal.len() != JOURNAL_LEN as u32 {
            panic_with_error!(&env, Error::BadJournalLength);
        }
        let mut buf = [0u8; JOURNAL_LEN];
        journal.copy_into_slice(&mut buf);
        let journal_digest: BytesN<32> = env.crypto().sha256(&journal).into();
        let verifier: Address = s
            .get(&DataKey::VerifierId)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
        verify_proof(&env, &verifier, &seal, &image_id, &journal_digest);

        // 3. Parse the journal (raw bytes, committed via env::commit_slice).
        let mut wallet_bytes = [0u8; 32];
        wallet_bytes.copy_from_slice(&buf[0..32]);
        let mut root_bytes = [0u8; 32];
        root_bytes.copy_from_slice(&buf[32..64]);
        let mut nul = [0u8; 32];
        nul.copy_from_slice(&buf[64..96]);
        let nullifier = BytesN::from_array(&env, &nul);
        let k = u32::from_le_bytes([buf[96], buf[97], buf[98], buf[99]]);
        let as_of_block = u64::from_le_bytes([
            buf[100], buf[101], buf[102], buf[103], buf[104], buf[105], buf[106], buf[107],
        ]);
        let pass = buf[108];

        // 4. Enforce the public claims.
        if pass != 1 {
            panic_with_error!(&env, Error::ProofNotPassed);
        }
        let current_root: BytesN<32> = s
            .get(&DataKey::AllowSetRoot)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
        if current_root.to_array() != root_bytes {
            panic_with_error!(&env, Error::RootMismatch);
        }
        let expected_wallet = address_32(&env, &wallet);
        if expected_wallet != wallet_bytes {
            panic_with_error!(&env, Error::WalletMismatch);
        }

        // 5. Anti-replay via nullifier.
        if s.has(&DataKey::NullifierUsed(nullifier.clone())) {
            panic_with_error!(&env, Error::NullifierReused);
        }
        s.set(&DataKey::NullifierUsed(nullifier.clone()), &true);

        // 6. Record clearance.
        let ttl: u32 = s.get(&DataKey::Ttl).unwrap_or(0);
        let cleared_until_ledger = env.ledger().sequence().saturating_add(ttl);
        let clearance = Clearance {
            cleared_until_ledger,
            k,
            as_of_block,
            nullifier: nullifier.clone(),
        };
        s.set(&DataKey::Cleared(wallet.clone()), &clearance);
    }

    /// Is `wallet` currently cleared?
    pub fn is_cleared(env: Env, wallet: Address) -> bool {
        let Some(c): Option<Clearance> = env
            .storage()
            .persistent()
            .get(&DataKey::Cleared(wallet))
        else {
            return false;
        };
        c.cleared_until_ledger >= env.ledger().sequence()
    }

    /// Read a wallet's clearance record (if any).
    pub fn get_clearance(env: Env, wallet: Address) -> Option<Clearance> {
        env.storage().persistent().get(&DataKey::Cleared(wallet))
    }

    /// Compliance gate: transfer `amount` of SEP-41 `token` from `from` to `to`,
    /// but only if `from` is currently cleared. `from` must authorize.
    pub fn transfer_if_cleared(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
        token: Address,
    ) {
        from.require_auth();
        if !Self::is_cleared(env.clone(), from.clone()) {
            panic_with_error!(&env, Error::NotCleared);
        }
        let mut args: Vec<Val> = Vec::new(&env);
        args.push_back(from.into_val(&env));
        args.push_back(to.into_val(&env));
        args.push_back(amount.into_val(&env));
        let res =
            env.try_invoke_contract::<Val, InvokeError>(&token, &Symbol::new(&env, "transfer"), args);
        match res {
            Ok(Ok(_)) => {}
            _ => panic_with_error!(&env, Error::ProofVerificationFailed),
        }
    }
}
