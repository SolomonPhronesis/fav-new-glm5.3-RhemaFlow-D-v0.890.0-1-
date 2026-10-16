#!/usr/bin/env bash
# One-time setup of the Confucius4-R2T2 streaming sidecar on Apple Silicon.
# STATUS: written from reading the R2T2 repo; NOT yet run on a Mac. Expect to fix a step or two.
# Re-runnable. Everything lives in $R2T2_HOME (default ~/RhemaFlow-R2T2); nothing system-wide.
set -euo pipefail
say() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
die() { printf '\033[31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]] || die "Apple Silicon Mac required (M1 or later)."
command -v git >/dev/null || die "git missing (xcode-select --install)"
command -v cmake >/dev/null || die "cmake missing:  brew install cmake"
command -v python3 >/dev/null || die "python3 missing:  brew install python@3.12"

HOME_DIR="${R2T2_HOME:-$HOME/RhemaFlow-R2T2}"
HERE="$(cd "$(dirname "$0")" && pwd)"
LLAMA_COMMIT="ad6c66839af3c5646fba8c6c2e2087a1e4e38948"   # pinned by the R2T2 repo (2026-09-14)
mkdir -p "$HOME_DIR"; cd "$HOME_DIR"

say "1/6  R2T2 source (Apache-2.0 code)"
[[ -d repo/.git ]] || git clone --depth 1 https://github.com/netease-youdao/Confucius4-R2T2.git repo

say "2/6  Python environment (no vLLM: it cannot install on macOS)"
python3 -m venv venv
# shellcheck disable=SC1091
source venv/bin/activate
pip install -q -U pip
pip install -q torch qwen-asr websockets numpy pybind11 librosa soundfile huggingface_hub
pip install -q --no-deps -e repo      # NOT `pip install -e repo`: that pulls qwen-asr[vllm]

say "3/6  llama.cpp at the commit R2T2 was built against"
[[ -d repo/third_party/llama.cpp/.git ]] || { mkdir -p repo/third_party; git clone https://github.com/ggml-org/llama.cpp repo/third_party/llama.cpp; }
git -C repo/third_party/llama.cpp fetch -q origin "$LLAMA_COMMIT" || true
git -C repo/third_party/llama.cpp checkout -q "$LLAMA_COMMIT"

say "4/6  Build the native extension with Metal"
# The shipped .so files are Linux/CUDA. Rebuild; use @loader_path where the repo uses $ORIGIN.
sed -i.bak 's|\$ORIGIN/../bin|@loader_path/../bin|' repo/r2t2_llama/CMakeLists.txt
cmake -S repo/r2t2_llama -B build \
  -DLLAMA_CPP_DIR="$PWD/repo/third_party/llama.cpp" \
  -DGGML_METAL=ON -DCMAKE_BUILD_TYPE=Release \
  -Dpybind11_DIR="$(python -m pybind11 --cmakedir)" \
  -DPython_EXECUTABLE="$(which python)"
cmake --build build -j"$(sysctl -n hw.ncpu)"
mkdir -p repo/r2t2_llama/bin && cp -a build/bin/lib*.dylib repo/r2t2_llama/bin/ 2>/dev/null || true
python -c "import sys; sys.path.insert(0,'repo'); from r2t2_llama.native.qwen3asr_native import Qwen3ASRNative; print('native extension OK')"

say "5/6  Model files (you must accept the NetEase Youdao Model Use License first)"
cat <<'MSG'
Read:  https://github.com/netease-youdao/Confucius4-R2T2/blob/master/MODEL_LICENSE
Then download, into  $R2T2_HOME/models :
  (a) the HF processor files   : huggingface-cli download netease-youdao/Confucius4-R2T2 \
          --include "*.json" "*.txt" "*.model" --local-dir models/processor
  (b) GGUF language model + audio projector (a file named mmproj*.gguf plus one other .gguf)
      from NetEase's official GGUF release or the unmodified mirror t8star/Confucius-R2t2-Comfy
      -> put both files in models/gguf/
MSG
mkdir -p models/processor models/gguf

say "6/6  Launch script"
cat > run_sidecar.sh <<RUN
#!/usr/bin/env bash
cd "$HOME_DIR" && source venv/bin/activate
export R2T2_REPO="$HOME_DIR/repo"
exec python "$HERE/r2t2_server.py" --backend llama \\
  --processor-path "$HOME_DIR/models/processor" --gguf-dir "$HOME_DIR/models/gguf" \\
  --language English --chunk-ms 160 --threads 4 --context "\$(cat "$HERE/context_books.txt")" "\$@"
RUN
chmod +x run_sidecar.sh
echo
echo "Done. Start the service:   $HOME_DIR/run_sidecar.sh"
echo "Then run RhemaFlow:        ./run.sh --stream-url ws://127.0.0.1:8272/asr_stream_api_v1"
