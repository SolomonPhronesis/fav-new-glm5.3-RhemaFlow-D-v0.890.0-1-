#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# RhemaFlow v0.855.0 — one-command setup + build + launch for Apple Silicon.
#
#   ./run.sh                 everything (first run: installs toolchain + model)
#   ./run.sh --demo          no model download — built-in demo ASR
#   ./run.sh --no-asr        no speech recognition — simulator + mic meter only
#   ./run.sh --church NAME   church name shown on the stage window
#   ./run.sh --yes           never prompt (non-interactive)
#   ./run.sh --engine parakeet   use NVIDIA Parakeet instead of Whisper
#   ./run.sh model-parakeet      download the Parakeet model (about 1.2 GB) and exit
#   ./run.sh model-turbo         download Whisper large-v3-turbo (about 574 MB) and exit
#
# ClearSpeech (v0.852.0): while the streaming engine shows the live draft, the
# best local model (Parakeet TDT v3 → Whisper large-v3-turbo → base/tiny) also
# re-decodes every finished sentence and corrects the transcript. See
# docs/CLEARSPEECH.md for the before/after numbers; docs/EVALSET.md for scoring.
#   ./run.sh --metal         build with the Apple GPU (Metal) backend (opt-in)
#   ./run.sh benchmark A.wav [--ref transcript.txt]   compare engines on one file
#
# Everything needed for a live service runs OFFLINE after setup:
# ASR (whisper), parsing, stage display and recording are all local.
# ---------------------------------------------------------------------------
set -euo pipefail

ORIG_PWD="$(pwd)"
cd "$(dirname "$0")"
ROOT="$(pwd)"

CYAN='\033[0;36m'; GOLD='\033[0;33m'; RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
step() { echo -e "${CYAN}▸${NC} $*"; }
ok()   { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${GOLD}!${NC} $*"; }
die()  { echo -e "${RED}✗${NC} $*"; exit 1; }

ASSUME_YES=1
METAL=0
ENGINE=whisper
SUBCMD=""
PASSTHROUGH=()
args=("$@")
i=0
while (( i < ${#args[@]} )); do
  a="${args[$i]}"
  case "$a" in
    --yes)   ASSUME_YES=1 ;;
    --ask)   ASSUME_YES=0 ;;
    --metal) METAL=1 ;;
    --engine)
      i=$((i + 1)); ENGINE="${args[$i]:-whisper}"
      PASSTHROUGH+=("--engine" "$ENGINE") ;;
    benchmark|model-parakeet|model-turbo)
      # subcommands are only recognised as the first argument
      if (( ${#PASSTHROUGH[@]} == 0 )); then SUBCMD="$a"; fi
      PASSTHROUGH+=("$a") ;;
    *) PASSTHROUGH+=("$a") ;;
  esac
  i=$((i + 1))
done
# Homebrew now asks its own "Do you want to proceed? [y/n]" before installing. That question comes from
# Homebrew, not from this script, so we answer it here (unless ./run.sh --ask was used).
export HOMEBREW_NO_ENV_HINTS=1
export HOMEBREW_NO_INSTALL_CLEANUP=1
brew_install() { # brew_install ffmpeg  -> returns 0 on success; never stops the whole script
  local brew_bin
  brew_bin="$(command -v brew 2>/dev/null || true)"
  [[ -z "$brew_bin" && -x /opt/homebrew/bin/brew ]] && brew_bin=/opt/homebrew/bin/brew
  [[ -z "$brew_bin" && -x /usr/local/bin/brew ]] && brew_bin=/usr/local/bin/brew
  [[ -z "$brew_bin" ]] && return 1
  if [[ $ASSUME_YES == 1 ]]; then
    # feed "y" to every question; ignore the harmless "broken pipe" yes gets when brew finishes
    ( set +o pipefail; yes y 2>/dev/null | NONINTERACTIVE=1 "$brew_bin" install "$@" )
  else
    "$brew_bin" install "$@"
  fi
}
ask() { # ask "question" -> exits if non-interactive and declined
  if [[ $ASSUME_YES == 1 ]]; then return 0; fi
  read -r -p "$(echo -e "${GOLD}?${NC} $1 [Y/n] ")" ans
  [[ ! "$ans" =~ ^[Nn] ]]
}

echo ""
echo -e "${GOLD}═══════════════════════════════════════════════════════${NC}"
echo -e "${GOLD}   RhemaFlow v0.855.0 — the preacher speaks,         ${NC}"
echo -e "${GOLD}   RhemaFlow understands.                              ${NC}"
echo -e "${GOLD}═══════════════════════════════════════════════════════${NC}"
echo ""

# --- 1. platform -----------------------------------------------------------
if [[ "$(uname -s)" == "Darwin" ]]; then
  if [[ "$(uname -m)" == "arm64" ]]; then
    ok "Apple Silicon detected (aarch64-apple-darwin) — the supported baseline."
  else
    warn "macOS on Intel — it should build, but M1 is the tested baseline."
  fi
  # Xcode Command Line Tools (clang for whisper.cpp)
  if xcode-select -p > /dev/null 2>&1; then
    ok "Xcode Command Line Tools present."
  else
    warn "Xcode Command Line Tools are missing."
    echo "  macOS will show an installer dialog now — after it finishes, re-run ./run.sh"
    xcode-select --install || true
    die "Install the Command Line Tools, then re-run ./run.sh"
  fi
else
  warn "Not macOS ($(uname -s)) — this build targets macOS on M1."
  warn "Continuing anyway (useful for development on Linux/Windows)."
fi

# --- 2. Rust toolchain -----------------------------------------------------
if command -v cargo > /dev/null 2>&1 || [[ -x "$HOME/.cargo/bin/cargo" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
  ok "Rust toolchain: $(cargo --version)."
else
  step "Rust is not installed."
  if ask "Install rustup + stable Rust now (~1 minute)?"; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
    export PATH="$HOME/.cargo/bin:$PATH"
    ok "Rust installed: $(cargo --version)"
  else
    die "Rust is required. Install from https://rustup.rs and re-run ./run.sh"
  fi
fi

# --- 2b. ffmpeg (fixes iPhone / .mov / odd video and sound files by itself) -------
if command -v ffmpeg > /dev/null 2>&1 || [[ -x /opt/homebrew/bin/ffmpeg || -x /usr/local/bin/ffmpeg ]]; then
  ok "ffmpeg present: odd videos and sounds are converted automatically."
elif command -v brew > /dev/null 2>&1 || [[ -x /opt/homebrew/bin/brew ]]; then
  step "ffmpeg missing: installing via Homebrew (one time, a few minutes)."
  brew_install ffmpeg || warn "ffmpeg did not install; RhemaFlow still runs, but odd video files will not convert."
else
  warn "Homebrew not found, so ffmpeg was not installed. RhemaFlow still runs; plain MP4/MP3 files work without it."
fi

# --- 2c. poppler (sharper, faster PDF pages; the Mac's own PDF engine is the fallback) ---
if command -v pdftoppm > /dev/null 2>&1 || [[ -x /opt/homebrew/bin/pdftoppm || -x /usr/local/bin/pdftoppm ]]; then
  ok "poppler present: PDF pages render at full quality."
elif command -v brew > /dev/null 2>&1 || [[ -x /opt/homebrew/bin/brew ]]; then
  step "poppler missing: installing via Homebrew (one time) for the best PDF pages."
  brew_install poppler || warn "poppler did not install; PDFs still open using the Mac's built-in engine."
fi

# --- 3. cmake (whisper.cpp build dependency) -------------------------------
if command -v cmake > /dev/null 2>&1; then
  ok "cmake: $(cmake --version | head -1 | cut -d' ' -f3)."
elif [[ "$(uname -s)" == "Darwin" ]] && command -v brew > /dev/null 2>&1; then
  step "cmake missing — installing via Homebrew (one-time)."
  brew_install cmake || die "cmake did not install. Run: brew install cmake   then re-run ./run.sh"
  ok "cmake installed."
else
  warn "cmake is missing and Homebrew was not found."
  echo "  whisper.cpp needs cmake to compile. Install it with either:"
  echo "    brew install cmake          (install Homebrew from https://brew.sh)"
  echo "    pip3 install --user cmake"
  die "Install cmake, then re-run ./run.sh"
fi

# --- 4. whisper model ------------------------------------------------------
MODEL="$ROOT/models/ggml-tiny.en.bin"
MODEL_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin"
MODEL_SHA256="921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f"
PK_MODEL="$ROOT/models/ggml-parakeet-tdt-0.6b-v3-f16.bin"
PK_URL="https://huggingface.co/ggml-org/parakeet-GGUF/resolve/main/ggml-parakeet-tdt-0.6b-v3-f16.bin"

fetch_parakeet() {
  if [[ -f "$PK_MODEL" ]]; then
    ok "Parakeet model found ($(du -h "$PK_MODEL" | cut -f1))."
    return 0
  fi
  step "Parakeet TDT 0.6B v3 model not found (about 1.2 GB, downloaded once, offline afterwards)."
  if ask "Download the Parakeet model now?"; then
    mkdir -p "$ROOT/models"
    curl -L -C - --retry 3 -o "$PK_MODEL" "$PK_URL"
    # No checksum is published to compare against, so only sanity-check the size.
    local bytes; bytes="$(wc -c < "$PK_MODEL" | tr -d ' ')"
    if (( bytes < 100000000 )); then
      rm -f "$PK_MODEL"
      die "Parakeet download looks truncated (${bytes} bytes). Re-run to retry."
    fi
    warn "Parakeet model downloaded; no published checksum was available to verify it."
    return 0
  fi
  return 1
}

TURBO_MODEL="$ROOT/models/ggml-large-v3-turbo-q5_0.bin"
TURBO_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin"

fetch_turbo() {
  if [[ -f "$TURBO_MODEL" ]]; then
    ok "Whisper large-v3-turbo model found ($(du -h "$TURBO_MODEL" | cut -f1))."
    return 0
  fi
  step "Whisper large-v3-turbo model not found (about 574 MB, downloaded once, fully offline afterwards)."
  if ask "Download the ClearSpeech turbo model now?"; then
    mkdir -p "$ROOT/models"
    curl -L -C - --retry 3 -o "$TURBO_MODEL" "$TURBO_URL"
    local bytes; bytes="$(wc -c < "$TURBO_MODEL" | tr -d ' ')"
    if (( bytes < 400000000 )); then
      rm -f "$TURBO_MODEL"
      die "Turbo download looks truncated (${bytes} bytes). Re-run to retry."
    fi
    warn "Turbo model downloaded; no published checksum was available to verify it."
    return 0
  fi
  return 1
}

SKIP_MODEL=0
ACTIVE_MODEL="$MODEL"
for a in ${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}; do
  [[ "$a" == "--demo" || "$a" == "--no-asr" ]] && SKIP_MODEL=1
done

if [[ "$SUBCMD" == "model-parakeet" ]]; then
  fetch_parakeet || die "Parakeet model not downloaded."
  ok "Done. Run ./run.sh --engine parakeet to use it."
  exit 0
elif [[ "$SUBCMD" == "model-turbo" ]]; then
  fetch_turbo || die "Turbo model not downloaded."
  ok "Done. The ClearSpeech final pass picks it up automatically (Parakeet stays first if installed)."
  exit 0
elif [[ "$SUBCMD" == "benchmark" ]]; then
  SKIP_MODEL=1   # benchmark finds whichever models are installed and skips the rest
elif [[ $SKIP_MODEL == 1 ]]; then
  warn "Demo/no-ASR mode — skipping model download."
elif [[ "$ENGINE" == "parakeet" ]]; then
  if fetch_parakeet; then
    ACTIVE_MODEL="$PK_MODEL"
  else
    warn "Skipping model — the app will start in simulator-only mode."
    SKIP_MODEL=1
  fi
elif [[ -f "$MODEL" ]]; then
  ok "Whisper model found ($(du -h "$MODEL" | cut -f1))."
else
  step "Whisper tiny.en model not found (77 MB, downloaded once, fully offline afterwards)."
  if ask "Download the English speech model now?"; then
    mkdir -p "$ROOT/models"
    curl -L -C - --retry 3 -o "$MODEL" "$MODEL_URL"
    if command -v shasum > /dev/null 2>&1; then
      got="$(shasum -a 256 "$MODEL" | cut -d' ' -f1)"
      if [[ "$got" != "$MODEL_SHA256" ]]; then
        rm -f "$MODEL"
        die "Checksum mismatch ($got) — download corrupted. Re-run ./run.sh"
      fi
      ok "Model checksum verified."
    fi
  else
    warn "Skipping model — the app will start in simulator-only mode."
    SKIP_MODEL=1
  fi
fi

# --- 4b. disk space ---------------------------------------------------------
# RhemaFlow only needs about 2 GB free to carry on (v0.844.0: this used to ask for 6 GB, which was
# more than a Mac that already holds a finished build needs). If the disk is below that, OLD
# RhemaFlow builds are cleared first (they are only build files; they are made again on demand and
# nothing of yours is in them). To use another limit:  RF_NEED_MB=3000 ./run.sh
NEED_MB="${RF_NEED_MB:-2000}"
free_mb() { df -k "$ROOT" 2>/dev/null | awk 'NR==2 {printf "%d", $4/1024}'; }
FREE_MB="$(free_mb)"; FREE_MB="${FREE_MB:-999999}"
if [[ "$SUBCMD" != "model-parakeet" && "$SUBCMD" != "model-turbo" ]] && (( FREE_MB < NEED_MB )); then
  warn "Only $((FREE_MB / 1024)) GB free on this disk; RhemaFlow likes at least $((NEED_MB / 1000)) GB."
  PARENT="$(dirname "$ROOT")"
  for old in "$PARENT"/RhemaFlow-D-v*/target; do
    [[ -d "$old" && "$old" != "$ROOT/target" ]] || continue
    step "Removing the old build files in $old"
    rm -rf "$old"
  done
  FREE_MB="$(free_mb)"
  if (( FREE_MB < NEED_MB )) && [[ -d "$HOME/.cargo/registry/cache" ]]; then
    step "Clearing Cargo's download cache (it downloads again when needed)."
    rm -rf "$HOME/.cargo/registry/cache" "$HOME/.cargo/git/checkouts" 2>/dev/null || true
    FREE_MB="$(free_mb)"
  fi
  if (( FREE_MB < NEED_MB )); then
    echo ""
    die "Still only $((FREE_MB / 1024)) GB free (need $((NEED_MB / 1000)) GB). Free up about $(( (NEED_MB - FREE_MB) / 1000 + 1 )) GB, then run ./run.sh again. Quick wins: empty the Trash; delete old RhemaFlow-D-v0.8* folders and old .zip files in Downloads; Apple menu > About This Mac > Storage > Manage."
  fi
  ok "Freed space: $((FREE_MB / 1024)) GB now free."
fi

# --- 5. build --------------------------------------------------------------
if [[ $METAL == 1 ]]; then
  step "Building RhemaFlow-D with the Metal GPU backend (opt-in; first build takes ~5–10 minutes)…"
  cargo build --release -p rf-app --features metal
else
  step "Building RhemaFlow-D (first build takes ~5–10 minutes; later builds are seconds)…"
  cargo build --release -p rf-app
fi
ok "Built: target/release/rhemaflow-d"

if [[ "$SUBCMD" == "benchmark" ]]; then
  cd "$ORIG_PWD"   # so relative audio / transcript paths mean what the caller meant
  exec "$ROOT/target/release/rhemaflow-d" ${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}
fi

# --- 6. launch -------------------------------------------------------------
echo ""
step "Starting RhemaFlow-D…"
echo ""
echo -e "  ${GOLD}Microphone permission${NC}"
echo "  macOS will ask ONCE: “<your terminal> would like to access the microphone”"
echo "  → click OK. If you missed it:"
echo "    System Settings → Privacy & Security → Microphone → enable your terminal app."
echo ""
echo -e "  ${GOLD}10-second demo${NC}"
echo "  1. Click “Start service”  (recording starts automatically)"
echo "  2. Say:  “Let's turn to John chapter 3 verse 16.”"
echo "  3. The verse appears on stage within a couple of seconds."
echo "  4. Say:  “Next.”  ·  “Previous.”  ·  “Read it.”  ·  “Full screen.”  ·  “Verse 18.”"
echo "  5. Press F to open the congregation stage window."
echo ""
# Wrap the program in a tiny .app so the macOS menu says "About RhemaFlow-D" with the version (and the
# microphone question names RhemaFlow-D). If anything here fails the plain program is used instead.
APP_BIN="$ROOT/target/release/rhemaflow-d"
VER="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
APP="$ROOT/target/RhemaFlow.app"
if rm -rf "$APP" && mkdir -p "$APP/Contents/MacOS" && cp "$ROOT/target/release/rhemaflow-d" "$APP/Contents/MacOS/RhemaFlow" \
   && cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>RhemaFlow</string>
<key>CFBundleDisplayName</key><string>RhemaFlow</string>
<key>CFBundleIdentifier</key><string>app.rhemaflow.d</string>
<key>CFBundleExecutable</key><string>RhemaFlow</string>
<key>CFBundleShortVersionString</key><string>${VER}</string>
<key>CFBundleVersion</key><string>${VER}</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSAppNapIsDisabled</key><true/>
<key>NSSupportsAutomaticTermination</key><false/>
<key>NSSupportsSuddenTermination</key><false/>
<key>NSMicrophoneUsageDescription</key><string>RhemaFlow listens to the preacher to find Scripture references.</string>
<key>NSHumanReadableCopyright</key><string>RhemaFlow ${VER}</string>
</dict></plist>
PLIST
then
  defaults write app.rhemaflow.d NSAppSleepDisabled -bool YES >/dev/null 2>&1 || true   # belt and braces: no App Nap
  codesign --force --sign - "$APP" >/dev/null 2>&1 || true
  APP_BIN="$APP/Contents/MacOS/RhemaFlow"
fi
cd "$ROOT"   # the program looks for ./models next to where it starts
if [[ $SKIP_MODEL == 1 ]]; then
  exec "$APP_BIN" ${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}
else
  exec "$APP_BIN" --model "$ACTIVE_MODEL" ${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}
fi
