# How Silvery works

Notes for developers: what each part does, how data flows, and the switches used for testing.

## Overview

```
 microphone ──cpal──► audio thread ──16 kHz mono──► sample buffer ──12 s chunks──► Whisper worker
                                                         │                              │
                                                         ▼                              ▼
                                               live sound wave (UI)            transcript segments
                                                                                        │
 your notes ───────────────────────────────────────────────────────────────────────────┤
                                                                                        ▼
                                                              Ollama /api/chat (streamed) ──► summary / answers
```

Silvery is one Rust program using **eframe/egui** for the window. Slow work (audio, speech-to-text,
AI, downloads) runs on background threads, which send messages back to the UI over a channel
(`app::Msg`). The UI never blocks.

## Source files

| File | Responsibility |
|---|---|
| `src/main.rs` | Start-up, window options, `--screenshot` and the test switches, logo/window icon |
| `src/app.rs` | The whole UI: sidebar, record bar, tabs, Settings, dialogs; message handling; auto-save |
| `src/model.rs` | `Meeting`, `Segment`, `Settings`, the on-disk `Store`, the example meeting |
| `src/audio.rs` | `Recorder`: microphone capture via cpal, resampling to 16 kHz mono, loudness, quiet-point cutting, WAV writing |
| `src/transcribe.rs` | Whisper worker thread (whisper-rs), model list and one-time model download |
| `src/llm.rs` | Ollama client: model list, streaming chat, summary styles (prompts), Ask prompt, `<think>` stripping |
| `src/markdown.rs` | Small Markdown renderer for summaries (headings, bullets, tickable checkboxes, bold/italic) |
| `src/mic.rs` | Microphone list, macOS microphone permission, packaging as `Silvery.app` |
| `src/platform.rs` | Per-system differences: wording, opening folders, microphone settings, starting Ollama |
| `src/theme.rs` | Colours, spacing and fonts |

## Recording and transcription

1. `Recorder::start` opens the chosen input with cpal on its own thread. That thread owns the stream,
   because streams can't move between threads on macOS. Each callback mixes to mono, resamples to
   16 kHz (linear) and appends to a shared buffer.
2. Every frame, the UI checks the buffer. Once 12 s of new audio has built up, it cuts the chunk at the
   quietest 100 ms in the last 3 s (`audio::quiet_cut`), so words aren't split, and sends it to the
   Whisper worker with its time offset.
3. The worker loads the model once and transcribes chunks in order. It skips near-silent chunks and
   filters common Whisper "hallucinations" (`[music]`, lone "you", and so on). Chunks that arrive before the
   model is downloaded wait for `Cmd::ModelReady`.
4. On **Stop**, the remainder is sent, the full recording is written to `recordings/<id>-<n>.wav`,
   and once every chunk is transcribed the summary is started automatically.

Speech models (`ggml-*.bin`) are downloaded from the whisper.cpp model repository on Hugging Face into
`models/`. On macOS, whisper.cpp uses Metal. Elsewhere it uses the processor, unless built with
`--features cuda` or `--features vulkan`.

## AI (Ollama)

- `GET /api/tags` lists models (embedding models are hidden). A sensible default is picked
  (`qwen3.5` › `qwen3:8b` › `qwen3` › `gpt-oss` › `llama3` › `mistral` › `gemma`).
- `POST /api/chat` with `stream: true`. The reply arrives as one JSON object per line, and each piece is
  shown as it arrives. For Qwen 3 / DeepSeek-R1, `think: false` skips the reasoning phase (it's retried
  without it if the server refuses). Any `<think>…</think>` text is removed.
- **Summary prompt:** a system prompt with the chosen style's sections, plus the date, your notes and the
  timestamped transcript. The first line of the reply (`# Title`) names untitled meetings.
- **Ask prompt:** the meeting material plus the summary as context, then the conversation so far.
- Options: `temperature 0.3`, `num_ctx 16384`.

## Data files

All under the data folder (see the user guide), or `SILVERY_DATA_DIR` if set.

- `meetings/<id>.json`: written atomically (temporary file + rename) about a second after a change:
  ```json
  { "id": "20261125-153700-123", "title": "…", "created": "2026-11-25T15:37:00+00:00",
    "duration_ms": 39000, "notes": "…", "transcript": [{ "start_ms": 0, "end_ms": 6000, "text": "…" }],
    "summary": "## Overview\n…", "template": 0, "chat": [{ "role": "user", "content": "…" }],
    "recordings": ["20261125-153700-123-1.wav"] }
  ```
- `settings.json`: Ollama address, model, speech model, language, microphone, default style.
- `recordings/*.wav`: 16-bit, 16 kHz, mono.

## Test and developer switches

| Switch | Effect |
|---|---|
| `--screenshot <file.png>` | Opens without taking focus, draws ~120 frames, saves the window as PNG and exits. Data is read-only in this mode. |
| `SILVERY_VIEW=transcript\|notes\|ask\|settings\|recording` | With `--screenshot`: picture a specific screen. `recording` uses made-up audio. |
| `SILVERY_DATA_DIR=<folder>` | Use another data folder. An empty one shows only the example meeting. |
| `--self-test [speech.wav]` | Without a window: lists Ollama models, writes a summary of the example meeting, and transcribes the WAV (16 kHz mono 16-bit). |
| `--mic-check [--ask]` | Prints microphone permission and inputs, listens for 3 s and reports whether sound arrived. |
| `--install-app` | Writes `Silvery.app` (Info.plist with the microphone description, icon, ad-hoc signature) to `~/Applications`, or to `SILVERY_APP_DIR`. |

Example, as used for the pictures in these docs:

```sh
SILVERY_DATA_DIR=/tmp/silvery-demo SILVERY_VIEW=transcript \
  dist/Silvery.app/Contents/MacOS/silvery --screenshot docs/images/screenshot-transcript.png
```

## Why Silvery.app matters on macOS

macOS only gives an app microphone access after the person allows it. A program started inside
another app (a terminal or an IDE) is judged by *that* app's permission. If that app can't ask, macOS
silently hands over silence. Packaging as `Silvery.app` with `NSMicrophoneUsageDescription` makes
Silvery responsible for itself, so macOS shows its own "allow microphone" prompt. Ad-hoc signatures
change with every build, so macOS may ask again after an update.

## Logo

`assets/logo.svg` is the source. `logo-1024.png`, `logo-256.png`, `logo-64.png`, `logo-tile-128.png`
(the sidebar) and `Silvery.icns` (the macOS app icon) are made from it.
