//! Air traffic control: the controllers' voices for now; the phrases and
//! the controllers themselves come next.
//!
//! - `install`: the voice pack (Kokoro-82M and ONNX Runtime), downloaded on request;
//! - `lexicon`: text to Kokoro's tokens, from pronunciations built ahead of time;
//! - `tts`: the speaking thread, with its cache.
//!
//! What a voice sounds like on the radio is `audio::radio`.

// Not driven by the game yet: the ATC controllers will call `tts::Tts::say`.
#![allow(dead_code)]

pub mod install;
pub mod lexicon;
pub mod tts;
