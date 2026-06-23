#include "rapfi_c_api.h"

#include "command/command.h"
#include "core/types.h"
#include "game/board.h"
#include "search/searchcommon.h"
#include "search/searchthread.h"

#include <algorithm>
#include <filesystem>
#include <fstream>
#include <memory>
#include <mutex>
#include <string>

struct rapfi_handle {
    std::string last_error;
    std::unique_ptr<Board> board;
    Search::SearchOptions options;
};

namespace {

std::mutex rapfi_mutex;

GameRule map_rule(int rule) {
    switch (rule) {
        case 1:
            return {Rule::STANDARD, GameRule::FREEOPEN};
        case 4:
            return {Rule::RENJU, GameRule::FREEOPEN};
        default:
            return {Rule::FREESTYLE, GameRule::FREEOPEN};
    }
}

void set_error(rapfi_handle* handle, const std::string& message) {
    if (handle != nullptr) {
        handle->last_error = message;
    }
}

int fail(rapfi_handle* handle, const std::string& message) {
    set_error(handle, message);
    return -1;
}

bool ensure_board(rapfi_handle* handle) {
    return handle != nullptr && handle->board != nullptr;
}

} // namespace

rapfi_handle* rapfi_create(const char* config_path, const char* weights_dir) {
    std::lock_guard<std::mutex> lock(rapfi_mutex);

    auto* handle = new rapfi_handle();
    if (config_path == nullptr || config_path[0] == '\0') {
        handle->last_error = "Rapfi config path is empty";
        return handle;
    }

    try {
        Command::configPath = std::filesystem::u8path(config_path);
        Command::allowInternalConfig = false;
        if (weights_dir != nullptr && weights_dir[0] != '\0') {
            Command::CommandLine::binaryDirectory = std::filesystem::u8path(weights_dir);
        }

        if (!Command::loadConfig()) {
            handle->last_error = "failed to load Rapfi config";
        }
    } catch (const std::exception& e) {
        handle->last_error = e.what();
    }

    return handle;
}

void rapfi_destroy(rapfi_handle* handle) {
    delete handle;
}

int rapfi_new_game(rapfi_handle* handle, int board_size, int rule) {
    std::lock_guard<std::mutex> lock(rapfi_mutex);
    if (handle == nullptr) {
        return -1;
    }
    if (board_size <= 0 || board_size > MAX_BOARD_SIZE) {
        return fail(handle, "board size is unsupported by Rapfi");
    }

    try {
        handle->options = Search::SearchOptions {};
        handle->options.rule = map_rule(rule);
        handle->board = std::make_unique<Board>(board_size);
        handle->board->newGame(handle->options.rule.rule);
        handle->last_error.clear();
        return 0;
    } catch (const std::exception& e) {
        return fail(handle, e.what());
    }
}

int rapfi_set_position(rapfi_handle* handle, const int* xs, const int* ys, int move_count) {
    std::lock_guard<std::mutex> lock(rapfi_mutex);
    if (!ensure_board(handle)) {
        return fail(handle, "Rapfi board is not initialized");
    }
    if (move_count < 0) {
        return fail(handle, "move count is negative");
    }
    if (move_count > 0 && (xs == nullptr || ys == nullptr)) {
        return fail(handle, "move arrays are null");
    }

    const int board_size = handle->board->size();
    const GameRule rule = handle->options.rule;
    try {
        handle->board = std::make_unique<Board>(board_size);
        handle->board->newGame(rule.rule);
        for (int i = 0; i < move_count; ++i) {
            Pos pos {xs[i], ys[i]};
            if (!handle->board->isLegal(pos)) {
                return fail(handle, "move history contains an illegal point");
            }
            handle->board->move(rule, pos);
        }
        handle->last_error.clear();
        return 0;
    } catch (const std::exception& e) {
        return fail(handle, e.what());
    }
}

int rapfi_think(rapfi_handle* handle, int time_ms, int* out_x, int* out_y) {
    std::lock_guard<std::mutex> lock(rapfi_mutex);
    if (!ensure_board(handle)) {
        return fail(handle, "Rapfi board is not initialized");
    }
    if (out_x == nullptr || out_y == nullptr) {
        return fail(handle, "output pointers are null");
    }

    try {
        handle->options.setTimeControl(std::max(time_ms, 1), 0);
        handle->options.multiPV = 1;
        handle->options.disableOpeningQuery = true;
        Search::Threads.clear(false);
        Search::Threads.startThinking(*handle->board, handle->options);
        Search::Threads.waitForIdle();

        Pos best = Search::Threads.main()->bestMove;
        if (!best.valid() || best == Pos::PASS || !best.isInBoard(handle->board->size(), handle->board->size())) {
            return fail(handle, "Rapfi did not return a board move");
        }
        *out_x = best.x();
        *out_y = best.y();
        handle->board->move(handle->options.rule, best);
        handle->last_error.clear();
        return 0;
    } catch (const std::exception& e) {
        return fail(handle, e.what());
    }
}

void rapfi_stop(rapfi_handle* /*handle*/) {
    Search::Threads.stopThinking();
}

const char* rapfi_last_error(rapfi_handle* handle) {
    if (handle == nullptr) {
        return "Rapfi handle is null";
    }
    return handle->last_error.c_str();
}

int rapfi_is_available(void) {
    return 1;
}
