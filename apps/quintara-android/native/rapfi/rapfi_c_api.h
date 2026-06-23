#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef struct rapfi_handle rapfi_handle;

rapfi_handle* rapfi_create(const char* config_path, const char* weights_dir);
void rapfi_destroy(rapfi_handle* handle);

int rapfi_new_game(rapfi_handle* handle, int board_size, int rule);
int rapfi_set_position(rapfi_handle* handle, const int* xs, const int* ys, int move_count);
int rapfi_think(rapfi_handle* handle, int time_ms, int* out_x, int* out_y);
void rapfi_stop(rapfi_handle* handle);

const char* rapfi_last_error(rapfi_handle* handle);
int rapfi_is_available(void);

#ifdef __cplusplus
}
#endif
