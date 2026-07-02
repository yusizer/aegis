//! # Aegis Mock SEP-41 Token
//!
//! Minimal SEP-41-compatible token for the Aegis compliance-gate demo. Only the
//! methods the gate exercises are implemented (`transfer`, `balance`) plus setup
//! (`init`, `mint`). Not audited — hackathon demo only.

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Env};

#[contract]
pub struct MockToken;

#[contracttype]
enum DataKey {
    Admin,
    Bal(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NotInitialized = 1,
    InsufficientBalance = 2,
}

#[contractimpl]
impl MockToken {
    /// Initialize with an admin (who may mint). Admin must authorize.
    pub fn init(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    /// Admin: mint `amount` to `to`.
    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized));
        admin.require_auth();
        let bal = Self::balance(env.clone(), to.clone());
        env.storage().persistent().set(&DataKey::Bal(to), &(bal + amount));
    }

    /// SEP-41 `transfer`: move `amount` from `from` to `to`. `from` must authorize.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let fb = Self::balance(env.clone(), from.clone());
        if fb < amount {
            panic_with_error!(env, Error::InsufficientBalance);
        }
        env.storage().persistent().set(&DataKey::Bal(from.clone()), &(fb - amount));
        let tb = Self::balance(env.clone(), to.clone());
        env.storage().persistent().set(&DataKey::Bal(to), &(tb + amount));
    }

    /// SEP-41 `balance`: current balance of `addr`.
    pub fn balance(env: Env, addr: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Bal(addr))
            .unwrap_or(0)
    }
}
