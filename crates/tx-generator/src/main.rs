use blake2b_rs::{Blake2b, Blake2bBuilder};
use ckb_hash::blake2b_256;
use ckb_jsonrpc_types::JsonBytes;
use ckb_merkle_mountain_range::{
    compiled_proof::{pack_leaves, pack_compiled_merkle_proof, Packable, ValueMerge, verify, PackedMerkleProof, PackedLeaves},
    util::MemStore,
    Error, Merge, Result, MMR, MMRStoreReadOps,
};
use ckb_mock_tx_types::ReprMockTransaction;
use ckb_types::H256;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, RngCore, SeedableRng};
use serde_json::{from_str, to_string_pretty};
use std::time::SystemTime;

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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        println!(
            "Usage: {} <output json filename> <verifier binary 1> <verifier binary 2> <verifier binary 3>",
            args[0]
        );
        return;
    }

    let seed: u64 = match std::env::var("SEED") {
        Ok(val) => str::parse(&val).expect("parsing number"),
        Err(_) => SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
    };
    println!("Seed: {}", seed);

    let mut rng = StdRng::seed_from_u64(seed);

    let store = MemStore::default();
    let mut mmr = MMR::<_, Blake2bHash, _>::new(0, &store);

    let position = rng.gen_range(1000..3000);
    let leafs = rng.gen_range(1..100);

    println!("Total leafs: {}, tested leafs: {}", position, leafs);
    let positions: Vec<u64> = (0..position)
        .map(|_i| {
            let mut data = [0u8; 32];
            rng.fill_bytes(&mut data[..]);

            mmr.push(FixedHash(data.to_vec())).expect("push")
        })
        .collect();
    let mmr_size = mmr.mmr_size();
    mmr.commit().expect("commit");
    println!("MMR size: {}", mmr_size);

    let chosen = {
        let mut source: Vec<u64> = positions.clone();
        source.shuffle(&mut rng);
        let mut r = source[0..leafs].to_vec();
        r.sort();
        r
    };

    let mmr = MMR::<_, Blake2bHash, _>::new(mmr_size, &store);
    let root = mmr.get_root().expect("get_root");
    let proof = mmr.gen_proof(chosen.clone()).expect("gen proof");

    let raw_proof_bytes: Vec<u8> = {
        let mut ret = vec![];

        for item in proof.proof_items() {
            ret.extend(item.pack().expect("pack"));
        }

        ret
    };

    let leaves: Vec<_> = chosen
        .iter()
        .map(|i| {
            let value = (&store)
                .get_elem(*i)
                .expect("get_elem")
                .expect("item missing");
            (*i, value)
        })
        .collect();

    let compiled = proof.compile::<ValueMerge<_>>(chosen).expect("compile");
    let result = compiled.clone()
        .verify::<Blake2bHash>(root.clone(), mmr_size, leaves.clone())
        .expect("compiled verify");
    assert!(result);

    let root_bytes = {
        let mut buf = [0u8; 40];
        buf[0..8].copy_from_slice(&mmr_size.to_le_bytes());
        buf[8..40].copy_from_slice(&root.0);
        buf
    };

    let proof_bytes: Vec<u8> = pack_compiled_merkle_proof(&compiled).expect("try into");

    let leaves_bytes = pack_leaves(&leaves).expect("pack leaves");

    println!(
        "Proof bytes: {}, leaf bytes: {} leaves: {}",
        proof_bytes.len(),
        leaves_bytes.len(),
        leaves.len(),
    );

    {
        let mut proof: PackedMerkleProof<FixedHash> =
            PackedMerkleProof::new(&proof_bytes);
        let mut leaves = PackedLeaves::new(&leaves_bytes);

        let result = verify::<_, Blake2bHash, _, _>(
            &mut proof,
            root.clone(),
            mmr_size,
            &mut leaves,
        ).unwrap();
        assert!(result);
    }

    let mut tx: ReprMockTransaction =
        from_str(&String::from_utf8_lossy(include_bytes!("./dummy_tx.json"))).expect("json");    

    tx.tx.witnesses[0] = JsonBytes::from_vec(root_bytes.to_vec());
    tx.tx.witnesses[1] = JsonBytes::from_vec(proof_bytes);
    tx.tx.witnesses[2] = JsonBytes::from_vec(leaves_bytes);
    tx.tx.witnesses[3] = JsonBytes::from_vec(raw_proof_bytes);

    let binary1 = std::fs::read(&args[2]).expect("read");
    let hash1 = blake2b_256(&binary1).to_vec();

    tx.mock_info.inputs[0].output.lock.code_hash = H256::from_slice(&hash1).expect("H256");
    tx.mock_info.cell_deps[0].data = JsonBytes::from_vec(binary1);

    let binary2 = std::fs::read(&args[3]).expect("read");
    let hash2 = blake2b_256(&binary2).to_vec();

    tx.mock_info.inputs[1].output.lock.code_hash = H256::from_slice(&hash2).expect("H256");
    tx.mock_info.cell_deps[1].data = JsonBytes::from_vec(binary2);

    let binary3 = std::fs::read(&args[4]).expect("read");
    let hash3 = blake2b_256(&binary3).to_vec();

    tx.mock_info.inputs[2].output.lock.code_hash = H256::from_slice(&hash3).expect("H256");
    tx.mock_info.cell_deps[2].data = JsonBytes::from_vec(binary3);

    let json = to_string_pretty(&tx).expect("json");
    std::fs::write(&args[1], &json).expect("write");
}
