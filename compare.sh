#!/usr/bin/env bash
# Same audio, same scorer, every engine, three conditions.  Answers "is ours as good as R2T2 on MY voice?"
#   ./compare.sh my-reading.wav "the exact words I read"  [ws://127.0.0.1:8272/asr_stream_api_v1]
# Needs: ./target/release/rhemaflow-d built (./run.sh --yes), the Whisper model, and
# (optional) the R2T2 sidecar running (sidecar/setup_mac.sh, then ~/RhemaFlow-R2T2/run_sidecar.sh).
set -euo pipefail
WAV="${1:?usage: compare.sh AUDIO.wav \"reference words\" [stream-url]}"
REF="${2:?give the exact words that were spoken}"
URL="${3:-}"
BIN=./target/release/rhemaflow-d
[[ -x "$BIN" ]] || { echo "build first:  ./run.sh --yes"; exit 1; }
STREAM=(); [[ -n "$URL" ]] && STREAM=(--stream-url "$URL")
run() { echo; echo "=== $1 ==="; shift; "$BIN" benchmark "$WAV" --ref-text "$REF" "${STREAM[@]}" "$@" 2>/dev/null | sed -n '/^ENGINE/,/^RTF/p;/first word/p'; }
run "clean"
run "steady noise, 10 dB below your voice"   --mix-noise 10
run "worship-band music bed, 10 dB below"    --mix-music 10
echo; echo "WER = word errors. Accuracy = 100% - WER.  'first word' / 'last word' = how fast text appears (streaming only)."
