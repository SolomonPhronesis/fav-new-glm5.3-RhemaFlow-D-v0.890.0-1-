// Minimal C ABI over the NVIDIA Parakeet TDT engine bundled in whisper.cpp.
// Same safe-shim pattern as rf_whisper_shim.h.
#ifndef RF_PARAKEET_SHIM_H
#define RF_PARAKEET_SHIM_H
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef struct rf_parakeet rf_parakeet;

rf_parakeet * rf_parakeet_load(const char * model_path, int use_gpu, char * err, size_t err_len);
void          rf_parakeet_free(rf_parakeet * p);

// Transcribe 16 kHz mono f32 PCM. Returns the segment count, or <0 on error.
int rf_parakeet_transcribe(rf_parakeet * p, const float * samples, int n_samples, int n_threads);

const char * rf_parakeet_segment_text(const rf_parakeet * p, int i);
long long    rf_parakeet_segment_t0_ms(const rf_parakeet * p, int i);
long long    rf_parakeet_segment_t1_ms(const rf_parakeet * p, int i);

#ifdef __cplusplus
}
#endif
#endif
