#include "rf_whisper_shim.h"
#include "whisper.h"

#include <cstdio>
#include <exception>

struct rf_whisper {
    whisper_context * ctx = nullptr;
};

static void set_err(char * err, size_t n, const char * msg) {
    if (err && n) std::snprintf(err, n, "%s", msg);
}

extern "C" {

rf_whisper * rf_whisper_load(const char * model_path, int use_gpu, char * err, size_t err_len) {
    try {
        whisper_context_params cp = whisper_context_default_params();
        cp.use_gpu = use_gpu != 0;
        whisper_context * ctx = whisper_init_from_file_with_params(model_path, cp);
        if (!ctx) {
            set_err(err, err_len, "whisper.cpp could not load the model file (missing, corrupt or wrong format)");
            return nullptr;
        }
        rf_whisper * w = new rf_whisper();
        w->ctx = ctx;
        return w;
    } catch (const std::exception & e) {
        set_err(err, err_len, e.what());
    } catch (...) {
        set_err(err, err_len, "unknown C++ exception while loading whisper model");
    }
    return nullptr;
}

void rf_whisper_free(rf_whisper * w) {
    if (!w) return;
    if (w->ctx) whisper_free(w->ctx);
    delete w;
}

int rf_whisper_transcribe(rf_whisper * w, const float * samples, int n_samples,
                          const char * initial_prompt, int beam_size, int n_threads) {
    if (!w || !w->ctx || !samples || n_samples <= 0) return -1;
    try {
        whisper_full_params p = whisper_full_default_params(
            beam_size > 1 ? WHISPER_SAMPLING_BEAM_SEARCH : WHISPER_SAMPLING_GREEDY);
        p.language          = "en";
        p.translate         = false;
        p.no_context        = true;
        p.n_threads         = n_threads > 0 ? n_threads : 4;
        p.print_progress    = false;
        p.print_special     = false;
        p.print_realtime    = false;
        p.print_timestamps  = false;
        p.suppress_blank    = true;
        p.suppress_nst      = true;   // non-speech tokens (was suppress_non_speech_tokens)
        p.temperature       = 0.0f;   // deterministic first pass
        p.initial_prompt    = initial_prompt;
        if (beam_size > 1) {
            p.beam_search.beam_size = beam_size;
            p.beam_search.patience  = -1.0f;
            // v0.854: rescue decode for hard audio. When a window fails the
            // quality gates (compression ratio / mean logprob), whisper.cpp
            // re-decodes it at a higher temperature (0.2 -> 0.4 -> 0.6).
            // This is what pulls music- and noise-damaged windows back from
            // hallucinated repetition. The fast interim path (beam == 1) stays
            // fully deterministic so partials remain cheap and stable.
            p.temperature_inc             = 0.2f;
            p.entropy_thold               = 2.4f;  // this whisper.cpp pin's compression-ratio gate
            p.logprob_thold               = -1.0f;
        }
        if (whisper_full(w->ctx, p, samples, n_samples) != 0) return -2;
        return whisper_full_n_segments(w->ctx);
    } catch (...) {
        return -3;
    }
}

const char * rf_whisper_segment_text(const rf_whisper * w, int i) {
    return (w && w->ctx) ? whisper_full_get_segment_text(w->ctx, i) : "";
}
// whisper reports segment times in 10 ms units.
long long rf_whisper_segment_t0_ms(const rf_whisper * w, int i) {
    return (w && w->ctx) ? (long long) whisper_full_get_segment_t0(w->ctx, i) * 10 : 0;
}
long long rf_whisper_segment_t1_ms(const rf_whisper * w, int i) {
    return (w && w->ctx) ? (long long) whisper_full_get_segment_t1(w->ctx, i) * 10 : 0;
}

} // extern "C"
