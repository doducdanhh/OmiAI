# ADR-0008: Optional Local LLM Surface Layer (Pillar 9)

**Status:** Proposed  
**Date:** 2026-09-05  
**Supersedes:** None  
**Related:** bundle-v1.md (capabilities.language_model), omiai-runtime load contract

---

## Context

OmiAI's eight reasoning pillars (core logic, knowledge graph, probabilistic, causal, neuro/reservoir, world, evolution, meta) provide **verifiable reasoning**: every answer about facts, probabilities, or causality comes with a proof object, exact probability, or causal derivation that can be audited. However, the **surface realization** (turning `ReasoningResult` into natural language) is currently template-based — functional but repetitive and limited in linguistic variety.

To achieve fluent, diverse natural language output **without sacrificing verifiability**, we consider adding a **ninth pillar**: a local, open-source LLM used **only as a surface realizer** — never as a reasoner.

### Hardware Constraints
- Target: CPU-only (Intel i7-7700K), 8 GB RAM, no GPU
- Must fit alongside: world simulation (~100 MB), reservoir (~32 MB), knowledge graph, Bayesian nets, causal DAGs, OS + Rust runtime (~1.5 GB baseline)
- **Budget for LLM: ≤ 2.5 GB** (quantized), leaving ≥ 1.5 GB safety margin

---

## Decision

**Adopt an optional ninth pillar (`language_model`) that:**

1. **Runs 100% locally** — no external API calls, no network access at inference time
2. **Uses open weights with permissive license** (Apache-2.0, MIT, or equivalent OSI-approved)
3. **Is quantized to GGUF format** (Q4_K_M or similar) for CPU inference via `llama.cpp` / `llama-cpp-2`
4. **Receives only verified `ReasoningResult` as input** — prompt template:
   ```
   "The following fact has been verified by the reasoning core: {reasoning_result}.
    Rewrite this as a natural {language} sentence. Do not add any information not present above."
   ```
5. **Never decides truth values** — if the reasoning core returns `NoAnswer` or `Unverified`, the LLM is **not called**; the template fallback is used instead
6. **Is gated by `capabilities.language_model` in bundle manifest** — default `false`; bundles without this capability are smaller and load faster
7. **Has explicit policy in `language_model_info`** (bundle manifest):
   - `role: "surface_realization_only"`
   - `may_assert_unverified_facts: false`
   - Runtime **enforces** these at load time

---

## Consequences

### Positive
- Fluent, diverse natural language output for verified facts
- Zero training cost — uses pre-trained open weights
- Fully offline, no privacy concerns
- Opt-in: users who want pure symbolic output can disable it
- Audit trail preserved: every LLM output is traceable to a `ReasoningResult`

### Negative / Risks
- **Bundle size increases** by 500 MB – 2.5 GB depending on model
- **License compliance**: must track and redistribute model license with bundle (recorded in `language_model_info`)
- **Hallucination risk**: despite constrained prompting, LLM may occasionally add/omit details → **mitigated by mandatory grounding test** (see below)
- **RAM pressure**: may require reducing world/reservoir sizes on 8 GB machines

---

## Mandatory Grounding Test (Enforcement)

Any bundle with `capabilities.language_model = true` **must pass** the following test before release:

```
For 50 diverse queries covering all reasoning types (logic, probabilistic, causal, KG, world):
  1. Run query through full pipeline → get ReasoningResult + LLM response
  2. Extract all numbers, proper nouns, and factual claims from LLM response
  3. Verify EVERY extracted claim appears in the ReasoningResult
  4. If ANY claim is not grounded → TEST FAILS
```

This test is implemented in `crates/omiai-export/tests/grounding.rs` and runs in CI.

---

## Implementation Notes

### Recommended Inference Stack
| Option | Pros | Cons |
|--------|------|------|
| `llama-cpp-2` (bindings to `llama.cpp`) | Best CPU perf (AVX2 on i7-7700K), mature GGUF support, low-level control | Requires `clang`/`bindgen` at build |
| `candle` (pure Rust) | Pure Rust, no C toolchain, WASM-friendly | Slightly slower on CPU, newer ecosystem |

**Recommendation:** Start with `llama-cpp-2` for CPU performance on target hardware; evaluate `candle` for WASM targets later.

### Recommended Models (as of 2026-09-05)
| Model | Params | License | Quantized Size (Q4) | Notes |
|-------|--------|---------|---------------------|-------|
| Phi-4-mini | 3.8B | MIT | ~2.5 GB | Strong reasoning for size, good multilingual |
| Qwen2.5-1.5B-Instruct | 1.5B | Apache-2.0 | ~1 GB | Excellent multilingual (incl. Vietnamese), tiny |
| Gemma-2-2B-IT | 2B | Gemma (custom) | ~1.3 GB | Check license terms for redistribution |
| SmolLM2-1.7B | 1.7B | Apache-2.0 | ~1 GB | Designed for on-device, good instruction following |

**Default choice:** `Qwen2.5-1.5B-Instruct` (Apache-2.0, strong Vietnamese, fits RAM budget comfortably).

---

## Rollout Plan

1. **Phase 1 (Slice 10):** Implement `omiai-runtime` LLM loading via `llama-cpp-2`, constrained prompting, grounding test
2. **Phase 2:** Add `language_model` to `omiai-export` bundle creation (opt-in flag)
3. **Phase 3:** Update `omiai-serve`/`omiai-cli` to expose LLM toggle
4. **Default remains OFF** — users explicitly enable via `--with-llm` at export time

---

## Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| Train own LLM from scratch | Impossible on 8 GB CPU-only (see roadmap §3.4) |
| Use API (OpenAI, Anthropic, etc.) | Violates offline/zero-training principle; privacy; cost |
| Fine-tune on OmiAI data | Still needs base model; fine-tuning on CPU impractical; adds complexity |
| Larger model (7B+) | Exceeds RAM budget; OOM risk |
| No LLM (template only) | Valid choice — remains default; this ADR only adds **option** |

---

## Verification

- [ ] `cargo test --workspace` passes with `language_model` feature
- [ ] Grounding test (50 queries) passes with 0 ungrounded claims
- [ ] Bundle with `capabilities.language_model = true` loads and runs on 8 GB machine
- [ ] Bundle without LLM capability is functionally identical (template fallback)
- [ ] License info correctly propagated to `model_card.json` and manifest

---

*End of ADR-0008*