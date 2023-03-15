CC := riscv64-unknown-elf-gcc
TARGET := riscv64imac-unknown-none-elf
CFLAGS := -O3 -nostdlib -nostdinc -g -I c -I deps/ckb-c-stdlib -I deps/ckb-c-stdlib/libc
LDFLAGS := -Wl,-static -fdata-sections -ffunction-sections -Wl,--gc-sections

build:
	cargo build --release --package tx-generator
	cargo build --release --package rust-verifier --target=$(TARGET)
	cargo build --release --package rust-slow-verifier --target=$(TARGET)

build-c:
	$(CC) c/ckb_mmr_test.c -o ckb_mmr_test $(CFLAGS) $(LDFLAGS)

.PHONY: build build-c
