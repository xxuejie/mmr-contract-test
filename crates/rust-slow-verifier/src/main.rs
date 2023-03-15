#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]

use alloc::{format, vec::Vec};
use blake2b_rs::{Blake2b, Blake2bBuilder};
use ckb_merkle_mountain_range::{
    compiled_proof::{Packable, PackedLeaves},
    MerkleProof,
    Error, Merge, Result,
};
use ckb_std::{
    ckb_constants::Source,
    default_alloc,
    syscalls::{debug, load_witness},
};
use core::marker::PhantomData;

ckb_std::entry!(program_entry);
default_alloc!();

#[derive(Clone, Debug, PartialEq)]
struct FixedHash(Vec<u8>);

impl Packable for FixedHash {
    fn pack(&self) -> Result<Vec<u8>> {
        if self.0.len() > u16::MAX as usize {
            return Err(Error::UnpackEof);
        }
        let mut ret = Vec::new();
        ret.resize(self.0.len() + 2, 0);
        ret[0..2].copy_from_slice(&(self.0.len() as u16).to_le_bytes());
        ret[2..].copy_from_slice(&self.0);
        Ok(ret)
    }

    fn unpack(data: &[u8]) -> Result<(Self, usize)> {
        if data.len() < 2 {
            return Err(Error::UnpackEof);
        }
        let len = {
            let mut buf = [0u8; 2];
            buf.copy_from_slice(&data[0..2]);
            u16::from_le_bytes(buf)
        } as usize;
        if data.len() < 2 + len {
            return Err(Error::UnpackEof);
        }
        let mut r = Vec::new();
        r.resize(len, 0);
        r.copy_from_slice(&data[2..2 + len]);
        Ok((FixedHash(r), 2 + len))
    }
}

fn new_blake2b() -> Blake2b {
    Blake2bBuilder::new(32)
        .personal(b"ckb-default-hash")
        .build()
}

#[derive(Debug)]
struct Blake2bHash;

impl Merge for Blake2bHash {
    type Item = FixedHash;

    fn merge(lhs: &Self::Item, rhs: &Self::Item) -> Result<Self::Item> {
        let mut hasher = new_blake2b();
        hasher.update(&lhs.0[..]);
        hasher.update(&rhs.0[..]);
        let mut hash = Vec::new();
        hash.resize(32, 0);
        hasher.finalize(&mut hash);
        Ok(FixedHash(hash))
    }
}

pub struct PackedProofs<'a, T> {
    index: usize,
    data: &'a [u8],
    t: PhantomData<T>,
}

impl<'a, T> PackedProofs<'a, T> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            index: 0,
            data,
            t: PhantomData,
        }
    }
}

impl<'a, T: Packable> Iterator for PackedProofs<'a, T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.data.len() {
            return None;
        }

        match T::unpack(&self.data[self.index..]) {
            Ok((item, size)) => {
                self.index += size;
                Some(Ok(item))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

pub fn program_entry() -> i8 {
    let mut root_buffer = [0u8; 40];
    let root_length = match load_witness(&mut root_buffer, 0, 0, Source::Input) {
        Ok(l) => l,
        Err(e) => {
            debug(format!("Loading root error {:?}", e));
            return -1;
        }
    };
    assert!(root_length == 40);

    let mmr_size = {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&root_buffer[0..8]);
        u64::from_le_bytes(buf)
    };

    let root = {
        let mut buf = Vec::new();
        buf.resize(32, 0);
        buf.copy_from_slice(&root_buffer[8..]);
        FixedHash(buf)
    };

    let mut proof_buffer = [0u8; 32 * 1024];
    let proof_length = match load_witness(&mut proof_buffer, 0, 3, Source::Input) {
        Ok(l) => l,
        Err(e) => {
            debug(format!("Loading proof error {:?}", e));
            return -1;
        }
    };

    let mut leaves_buffer = [0u8; 32 * 1024];
    let leaves_length = match load_witness(&mut leaves_buffer, 0, 2, Source::Input) {
        Ok(l) => l,
        Err(e) => {
            debug(format!("Loading leaves error {:?}", e));
            return -1;
        }
    };

    let packed_leaves: PackedLeaves<FixedHash> =
        PackedLeaves::new(&leaves_buffer[0..leaves_length]);
    let leaves: Vec<_> = packed_leaves.map(|l| l.unwrap()).collect();

    let packed_proofs: PackedProofs<FixedHash> = PackedProofs::new(&proof_buffer[0..proof_length]);
    let proofs: Vec<_> = packed_proofs.map(|l| l.unwrap()).collect();

    let merkle_proof = MerkleProof::<_, Blake2bHash>::new(mmr_size, proofs);
    let result = merkle_proof.verify(root, leaves).expect("verify");
    assert!(result);

    0
}
