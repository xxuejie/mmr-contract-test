#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]
#![feature(new_uninit)]

use alloc::{boxed::Box, format, vec::Vec};
use blake2b_rs::{Blake2b, Blake2bBuilder};
use ckb_merkle_mountain_range::{
    compiled_proof::{verify, Packable, PackedLeaves, PackedMerkleProof},
    Error, Merge, Result,
};
use ckb_std::{
    ckb_constants::Source,
    default_alloc,
    syscalls::{debug, load_witness},
};
use core::mem::MaybeUninit;

ckb_std::entry!(program_entry);
default_alloc!();

#[derive(Clone, Debug, PartialEq)]
enum FixedHash {
    Fixed(Box<[u8; 32]>),
    Dynamic(Vec<u8>),
}

impl FixedHash {
    fn from_fixed(s: &[u8]) -> Self {
        let mut r: Box<[u8; 32]> = unsafe { Box::new_uninit().assume_init() };
        r.copy_from_slice(s);
        FixedHash::Fixed(r)
    }

    fn as_bytes(&self) -> &[u8] {
        match self {
            FixedHash::Fixed(d) => &d[..],
            FixedHash::Dynamic(d) => &d,
        }
    }
}

impl Packable for FixedHash {
    fn pack(&self) -> Result<Vec<u8>> {
        let d = self.as_bytes();
        if d.len() > u16::MAX as usize {
            return Err(Error::UnpackEof);
        }
        let mut ret = Vec::new();
        ret.resize(d.len() + 2, 0);
        ret[0..2].copy_from_slice(&(d.len() as u16).to_le_bytes());
        ret[2..].copy_from_slice(d);
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
        if len == 32 {
            Ok((FixedHash::from_fixed(&data[2..2 + len]), 2 + len))
        } else {
            let mut r = Vec::new();
            r.resize(len, 0);
            r.copy_from_slice(&data[2..2 + len]);
            Ok((FixedHash::Dynamic(r), 2 + len))
        }
    }
}

const HASH_BUILDER: Blake2bBuilder = Blake2bBuilder::new_with_personal(32, *b"ckb-default-hash");

#[derive(Debug)]
struct Blake2bHash;

impl Merge for Blake2bHash {
    type Item = FixedHash;

    // Stack returned value requires a second memcpy, which will result in worse performance
    fn merge(lhs: &Self::Item, rhs: &Self::Item) -> Result<Self::Item> {
        let mut hasher = Blake2b::uninit();
        HASH_BUILDER.build_from_ref(&mut hasher);
        hasher.update(lhs.as_bytes());
        hasher.update(rhs.as_bytes());
        let mut hash: Box<[u8; 32]> = unsafe { Box::new_uninit().assume_init() };
        hasher.finalize_from_ref(hash.as_mut());
        Ok(FixedHash::Fixed(hash))
    }
}

pub fn program_entry() -> i8 {
    #[allow(invalid_value)]
    let mut root_buffer: [u8; 40] = unsafe { MaybeUninit::uninit().assume_init() };
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

    let root = FixedHash::from_fixed(&root_buffer[8..]);

    #[allow(invalid_value)]
    let mut proof_buffer: [u8; 32 * 1024] = unsafe { MaybeUninit::uninit().assume_init() };
    let proof_length = match load_witness(&mut proof_buffer, 0, 1, Source::Input) {
        Ok(l) => l,
        Err(e) => {
            debug(format!("Loading proof error {:?}", e));
            return -1;
        }
    };

    #[allow(invalid_value)]
    let mut leaves_buffer: [u8; 32 * 1024] = unsafe { MaybeUninit::uninit().assume_init() };
    let leaves_length = match load_witness(&mut leaves_buffer, 0, 2, Source::Input) {
        Ok(l) => l,
        Err(e) => {
            debug(format!("Loading leaves error {:?}", e));
            return -1;
        }
    };

    let mut packed_proof: PackedMerkleProof<_> =
        PackedMerkleProof::new(&proof_buffer[0..proof_length]);
    let mut packed_leaves = PackedLeaves::new(&leaves_buffer[0..leaves_length]);
    let result =
        verify::<_, Blake2bHash, _, _>(&mut packed_proof, root, mmr_size, &mut packed_leaves);
    if !result.unwrap() {
        debug(format!("verifying failure!"));
        return -1;
    }

    0
}
