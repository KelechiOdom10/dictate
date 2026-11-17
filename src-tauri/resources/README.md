# Bundled resources

## ggml-silero-v5.1.2.bin

Silero VAD v5.1.2 in ggml format, compiled into the app by `src/vad.rs` with
`include_bytes!`. dictate uses it to tell a take that holds speech from one that
is only room tone, before the take reaches the speech model.

- Source: https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v5.1.2.bin
- SHA-256: 29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf
- Size: 885,098 bytes
- Licence: MIT (Silero VAD, https://github.com/snakers4/silero-vad; ggml conversion
  by ggml-org, MIT)
