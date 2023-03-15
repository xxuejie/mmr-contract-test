# mmr-contract-test

A sample environment for running different [MMR](https://github.com/nervosnetwork/merkle-mountain-range) verifiers.

# Usage

Below shows the steps to run the example on a Ubuntu 22.04 machine:

```
$ git clone https://github.com/xxuejie/mmr-contract-test
$ cd mmr-contract-test
$ make build
$ podman run --rm -it -v `pwd`:/code:Z nervos/ckb-riscv-gnu-toolchain:jammy-20230214 bash
root@16402c9937d9:/# cd /code
root@16402c9937d9:/code# make build-c
root@16402c9937d9:/code# exit
$ ./target/release/tx-generator tx.json ckb_mmr_test target/riscv64imac-unknown-none-elf/release/rust-verifier target/riscv64imac-unknown-none-elf/release/rust-slow-verifier
Seed: 1677195688536733338
Total leafs: 1116, tested leafs: 71
MMR size: 2227
Proof bytes: 8570, leaf bytes: 2982 leaves: 71
$ RUST_LOG=debug ckb-debugger --tx-file tx.json --cell-type input --script-group-type lock --cell-index 0
Run result: 0
Total cycles consumed: 1310061(1.2M)
Transfer cycles: 2501(2.4K), running cycles: 1307560(1.2M)
$ RUST_LOG=debug ckb-debugger --tx-file tx.json --cell-type input --script-group-type lock --cell-index 1
Run result: 0
Total cycles consumed: 1905059(1.8M)
Transfer cycles: 10240(10.0K), running cycles: 1894819(1.8M)
$ RUST_LOG=debug ckb-debugger --tx-file tx.json --cell-type input --script-group-type lock --cell-index 2
Run result: 0
Total cycles consumed: 2502114(2.4M)
Transfer cycles: 12070(11.8K), running cycles: 2490044(2.4M)
```
