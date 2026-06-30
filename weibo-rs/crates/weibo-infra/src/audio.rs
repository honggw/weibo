//! Simple audio playback for new message notification using rodio.

use rodio::Source;
use std::io::Cursor;
use std::sync::LazyLock;

/// Pre-generated 440Hz sine wave beep (0.15s, mono, 44100Hz sample rate).
/// Generated at compile time to avoid runtime synthesis overhead.
static BEEP_WAV: LazyLock<Vec<u8>> = LazyLock::new(|| {
    generate_wav_beep(440.0, 0.15, 44100)
});

/// Generate a simple WAV file (PCM, 16-bit, mono) with a sine wave.
fn generate_wav_beep(freq: f32, duration_secs: f32, sample_rate: u32) -> Vec<u8> {
    let num_samples = (sample_rate as f32 * duration_secs) as u32;
    let data_size = num_samples * 2; // 16-bit = 2 bytes per sample
    let file_size = 44 + data_size;

    let mut buf = Vec::with_capacity(file_size as usize);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(file_size - 8).to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes());  // PCM format
    buf.extend_from_slice(&1u16.to_le_bytes());  // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 2; // sample_rate * channels * bytes_per_sample
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());  // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());

    // PCM samples (sine wave with fade-in/out to avoid clicks)
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let amplitude = if t < 0.01 {
            t / 0.01 // fade in (10ms)
        } else if t > duration_secs - 0.02 {
            (duration_secs - t) / 0.02 // fade out (20ms)
        } else {
            1.0
        };
        let sample = (amplitude * 0.3 * (2.0 * std::f32::consts::PI * freq * t).sin() * 32767.0) as i16;
        buf.extend_from_slice(&sample.to_le_bytes());
    }

    buf
}

/// 播放新消息提示音。收到非自己的消息时调用。
/// 使用 rodio 播放合成的 beep 音效。
pub fn play_notification() {
    std::thread::spawn(|| {
        match rodio::OutputStream::try_default() {
            Ok((_stream, handle)) => {
                let cursor = Cursor::new(BEEP_WAV.clone());
                match rodio::Decoder::new(cursor) {
                    Ok(source) => {
                        if handle.play_raw(source.convert_samples()).is_ok() {
                            // Hold stream open for the duration of playback
                            std::thread::sleep(std::time::Duration::from_millis(200));
                        }
                    }
                    Err(e) => {
                        crate::logger::info(&format!("[audio] 解码提示音失败: {}", e));
                    }
                }
            }
            Err(e) => {
                crate::logger::info(&format!("[audio] 无法打开音频输出: {}", e));
            }
        }
    });
}
