use crate::SamVoice;
use libc::c_void;
use std::ffi::CString;
use thiserror::Error;

#[allow(dead_code, non_snake_case, non_camel_case_types)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
use bindings::{setupSpeak, speakText};

pub type SAMAudio = Vec<u8>;

#[derive(Debug, Error)]
pub enum SamError {
    #[error("text contains a null byte")]
    ContainsNull,
    #[error("SAM error code {0}")]
    Code(i32),
}

pub struct SamEngine;

impl SamEngine {
    pub fn speak(text: &str, voice: SamVoice) -> Result<SAMAudio, SamError> {
        Self::apply_voice(voice);

        if text.len() <= 255 {
            return Self::render_chunk(text);
        }

        let mut audio = Vec::new();
        let mut chunk_words: Vec<&str> = Vec::new();

        for word in text.split_whitespace() {
            let chunk_len = chunk_words.iter().map(|w| w.len()).sum::<usize>()
                + chunk_words.len().saturating_sub(1)
                + word.len();

            if chunk_len <= 255 {
                chunk_words.push(word);
            } else {
                if !chunk_words.is_empty() {
                    audio.extend(Self::render_chunk(&chunk_words.join(" "))?);
                    chunk_words.clear();
                }
                chunk_words.push(word);
            }
        }

        if !chunk_words.is_empty() {
            audio.extend(Self::render_chunk(&chunk_words.join(" "))?);
        }

        Ok(audio)
    }

    fn apply_voice(voice: SamVoice) {
        unsafe {
            setupSpeak(
                voice.pitch,
                voice.speed,
                voice.throat,
                voice.mouth,
            );
        }
    }

    fn render_chunk(chunk: &str) -> Result<SAMAudio, SamError> {
        let c_string = CString::new(chunk).map_err(|_| SamError::ContainsNull)?;

        unsafe {
            let result_ptr = speakText(c_string.as_ptr() as *mut i8);
            if result_ptr.is_null() {
                return Err(SamError::Code(-1));
            }

            let result = result_ptr.read();
            if result.res != 1 {
                libc::free(result_ptr as *mut c_void);
                return Err(SamError::Code(result.res));
            }

            let audio = if result.buf.is_null() || result.buf_size <= 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(result.buf as *const u8, result.buf_size as usize)
                    .to_vec()
            };

            libc::free(result_ptr as *mut c_void);
            Ok(audio)
        }
    }
}