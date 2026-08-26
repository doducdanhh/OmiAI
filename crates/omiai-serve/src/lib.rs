//! HTTP inference server: loads a `model.omiai` bundle and serves
//! `POST /infer` (JSON in, JSON out) via axum, so any language or tool
//! can plug into an evolved model over plain HTTP.
//!
//! **Scaffold** — implemented in the final slice.
#![allow(dead_code)]
