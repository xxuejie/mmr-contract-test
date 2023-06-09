#include <stddef.h>
#include <stdint.h>

#include "ckb_mmr.h"

#include <ckb_syscalls.h>
#include <stdio.h>

int main() {
  uint8_t root_buffer[40];
  uint64_t len = 40;
  int ret = ckb_load_witness(root_buffer, &len, 0, 0, CKB_SOURCE_INPUT);
  if (ret != CKB_SUCCESS) {
    return ret;
  }
  if (len != 40) {
    ckb_debug("Error loading root buffer");
    return -1;
  }
  uint64_t mmr_size = *((uint64_t *)root_buffer);

  uint8_t proof_buffer[32 * 1024];
  uint64_t proof_length = 32 * 1024;
  ret = ckb_load_witness(proof_buffer, &proof_length, 0, 1, CKB_SOURCE_INPUT);
  if (ret != CKB_SUCCESS) {
    return ret;
  }

  uint8_t leaves_buffer[32 * 1024];
  uint64_t leaves_length = 32 * 1024;
  ret = ckb_load_witness(leaves_buffer, &leaves_length, 0, 2, CKB_SOURCE_INPUT);
  if (ret != CKB_SUCCESS) {
    return ret;
  }

  mmr_default_buffer_reader_t proof_buffer_reader;
  mmr_default_buffer_reader_init(&proof_buffer_reader, proof_buffer,
                                 proof_length);

  mmr_default_buffer_reader_t leaf_buffer_reader;
  mmr_default_buffer_reader_init(&leaf_buffer_reader, leaves_buffer,
                                 leaves_length);

  return mmr_verify(&root_buffer[8], 32, mmr_size, &proof_buffer_reader,
                    &leaf_buffer_reader);
}
