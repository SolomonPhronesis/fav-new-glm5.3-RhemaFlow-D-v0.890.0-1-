// Minimal C ABI over whisper.cpp. Only pointers, ints and C strings cross the
// boundary — whisper_full_params (~40 fields, callbacks) is never redeclared in
// Rust, so its layout can never silently drift out of sync.
#ifndef RF_WHISPER_SHIM_H
#define RF_WHISPER_SHIM_H
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef struct rf_whisper rf_whisper;

// NULL on failure; a message is written to err (if err_len > 0).
rf_whisper * rf_whisper_load(const char * model_path, int use_gpu, char * err, size_t err_len);
void         rf_whisper_free(rf_whisper * w);

// Transcribe 16 kHz mono f32 PCM. Returns the segment count, or <0 on error.
int rf_whisper_transcribe(rf_whisper * w, const float * samples, int n_samples,
                          const char * initial_prompt, int beam_size, int n_threads);

// Valid until the next transcribe/free. Times are milliseconds.
const char * rf_whisper_segment_text(const rf_whisper * w, int i);
long long    rf_whisper_segment_t0_ms(const rf_whisper * w, int i);
long long    rf_whisper_segment_t1_ms(const rf_whisper * w, int i);

#ifdef __cplusplus
}
#endif
#endif
