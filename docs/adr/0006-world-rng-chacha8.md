# ADR-0006: World RNG — ChaCha8 with serialized generator state

## Context

The world loop (slice 2) needs deterministic randomness: grid seeding,
agent mutation, reproduction. To resume a run bit-exactly from a
checkpoint, the full generator state must be serializable — not just
the seed. Two candidates:

1. **ChaCha8** (`rand_chacha` 0.3.1): cryptographically seeded, and the
   crate exposes `SeedableRng::from_seed([u8; 32])`, `set_stream(u64)`,
   `get_stream() -> u64`, `get_word_pos() -> u128`, `set_word_pos(u128)`.
   The triple `(seed[32], stream, word_pos)` reproduces the exact
   generator position — verified by reading the crate source in the
   cargo registry (probe date 2026-08-26).
2. **Xorshift64\***: hand-rolled, trivially serializable (one u64), but
   weaker statistical quality and another custom PRNG to maintain.

## Decision

Use **ChaCha8** (`rand_chacha::ChaCha8Rng`). The world keeps
`rng_seed: [u8; 32]`, `rng_stream: u64` (fixed at init; always 0 in
slice 2), and the running generator. Checkpoints persist all three in
`world/rng_state.bin` (32 B seed + u64 LE stream + u128 LE word_pos);
load does `from_seed` → `set_stream` → `set_word_pos` and the
generator continues the exact sequence. No Xorshift fallback.

## Consequences

- RNG state is 60 bytes on disk — cheap, and bit-exact resume is a
  testable invariant (round-trip test in slice 2).
- If a future slice ever uses non-zero streams, load must read the
  stream from the file into both `rng_stream` and the generator —
  deriving one from the other is not safe.
- word_pos is only meaningful when set at multiples of the block size
  boundaries the crate supports; saving right after a step boundary
  (which the world loop guarantees) keeps it exact.
