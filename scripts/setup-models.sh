#!/usr/bin/env bash
# Downloads/builds the three offline model dependencies Hanna needs.
# Run this once from the project root: bash scripts/setup-models.sh
#
# This does NOT run automatically — read it before running, adjust paths
# for your OS, and make sure you're comfortable with what it downloads.
set -euo pipefail

APP_DIR="src-tauri"
mkdir -p "$APP_DIR/bin" "$APP_DIR/models" models

echo "== 1/3: whisper.cpp (speech-to-text) =="
if [ ! -d whisper.cpp ]; then
  git clone https://github.com/ggml-org/whisper.cpp.git
fi
cmake -B whisper.cpp/build -S whisper.cpp -DCMAKE_BUILD_TYPE=Release
cmake --build whisper.cpp/build --config Release -j
cp whisper.cpp/build/bin/whisper-cli "$APP_DIR/bin/whisper-cli"
bash whisper.cpp/models/download-ggml-model.sh base
cp whisper.cpp/models/ggml-base.bin "$APP_DIR/models/whisper-base.bin"

echo "== 2/3: llama.cpp server (local LLM) =="
if [ ! -d llama.cpp ]; then
  git clone https://github.com/ggml-org/llama.cpp.git
fi
cmake -B llama.cpp/build -S llama.cpp -DCMAKE_BUILD_TYPE=Release
cmake --build llama.cpp/build --config Release -j
cp llama.cpp/build/bin/llama-server "$APP_DIR/bin/llama-server"
echo "  -> Download a GGUF model yourself, e.g. Qwen2.5-4B-Instruct-Q4_K_M.gguf"
echo "     from https://huggingface.co and place it at $APP_DIR/models/hanna-llm.gguf"

echo "== 3/3: Piper (text-to-speech) =="
if [ ! -d piper ]; then
  git clone https://github.com/rhasspy/piper.git
fi
echo "  -> Follow piper's README to build/install the 'piper' binary into $APP_DIR/bin/"
echo "     and download a voice (.onnx + .onnx.json) into $APP_DIR/models/hanna-voice.onnx"

echo "Done. Start the LLM server separately before running the app:"
echo "  ./$APP_DIR/bin/llama-server -m $APP_DIR/models/hanna-llm.gguf --port 8080"
