#include "rf_parakeet_shim.h"
#include "parakeet.h"

#include <cstdio>
#include <exception>

struct rf_parakeet {
    parakeet_context * ctx = nullptr;
};

static void set_err(char * err, size_t n, const char * msg) {
    if (err && n) std::snprintf(err, n, "%s", msg);
}

extern "C" {

rf_parakeet * rf_parakeet_load(const char * model_path, int use_gpu, char * err, size_t err_len) {
    try {
        parakeet_context_params cp = parakeet_context_default_params();
        cp.use_gpu = use_gpu != 0;
        parakeet_context * ctx = parakeet_init_from_file_with_params(model_path, cp);
        if (!ctx) {
            set_err(err, err_len, "parakeet could not load the model file (missing, corrupt or not a Parakeet GGML file)");
            return nullptr;
        }
        rf_parakeet * p = new rf_parakeet();
        p->ctx = ctx;
        return p;
    } catch (const std::exception & e) {
        set_err(err, err_len, e.what());
    } catch (...) {
        set_err(err, err_len, "unknown C++ exception while loading parakeet model");
    }
    return nullptr;
}

void rf_parakeet_free(rf_parakeet * p) {
    if (!p) return;
    if (p->ctx) parakeet_free(p->ctx);
    delete p;
}

int rf_parakeet_transcribe(rf_parakeet * p, const float * samples, int n_samples, int n_threads) {
    if (!p || !p->ctx || !samples || n_samples <= 0) return -1;
    try {
        parakeet_full_params fp = parakeet_full_default_params(PARAKEET_SAMPLING_GREEDY);
        fp.n_threads  = n_threads > 0 ? n_threads : 4;
        fp.no_context = true;
        if (parakeet_full(p->ctx, fp, samples, n_samples) != 0) return -2;
        return parakeet_full_n_segments(p->ctx);
    } catch (...) {
        return -3;
    }
}

const char * rf_parakeet_segment_text(const rf_parakeet * p, int i) {
    return (p && p->ctx) ? parakeet_full_get_segment_text(p->ctx, i) : "";
}
// Segment times are in 10 ms frames (see parakeet-cli: 11 s of audio = 1101 frames).
long long rf_parakeet_segment_t0_ms(const rf_parakeet * p, int i) {
    return (p && p->ctx) ? (long long) parakeet_full_get_segment_t0(p->ctx, i) * 10 : 0;
}
long long rf_parakeet_segment_t1_ms(const rf_parakeet * p, int i) {
    return (p && p->ctx) ? (long long) parakeet_full_get_segment_t1(p->ctx, i) * 10 : 0;
}

} // extern "C"
