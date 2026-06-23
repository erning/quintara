cmake_minimum_required(VERSION 3.22.1)

project(quintara_rapfi_android LANGUAGES CXX)

set(RAPFI_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../../../bots/rapfi/vendor/rapfi/Rapfi")

add_library(rapfi SHARED
    rapfi_android.cpp
    ${RAPFI_ROOT}/command/command.cpp
    ${RAPFI_ROOT}/core/compressor.cpp
    ${RAPFI_ROOT}/core/hash.cpp
    ${RAPFI_ROOT}/core/iohelper.cpp
    ${RAPFI_ROOT}/core/platform.cpp
    ${RAPFI_ROOT}/core/utils.cpp
    ${RAPFI_ROOT}/core/version.cpp
    ${RAPFI_ROOT}/database/dbclient.cpp
    ${RAPFI_ROOT}/database/dbtypes.cpp
    ${RAPFI_ROOT}/database/dbutils.cpp
    ${RAPFI_ROOT}/database/yxdbstorage.cpp
    ${RAPFI_ROOT}/eval/eval.cpp
    ${RAPFI_ROOT}/eval/evaluator.cpp
    ${RAPFI_ROOT}/eval/mix10nnue.cpp
    ${RAPFI_ROOT}/eval/mix9svqnnue.cpp
    ${RAPFI_ROOT}/game/board.cpp
    ${RAPFI_ROOT}/game/movegen.cpp
    ${RAPFI_ROOT}/game/pattern.cpp
    ${RAPFI_ROOT}/search/ab/history.cpp
    ${RAPFI_ROOT}/search/ab/search.cpp
    ${RAPFI_ROOT}/search/hashtable.cpp
    ${RAPFI_ROOT}/search/mcts/node.cpp
    ${RAPFI_ROOT}/search/mcts/search.cpp
    ${RAPFI_ROOT}/search/movepick.cpp
    ${RAPFI_ROOT}/search/opening.cpp
    ${RAPFI_ROOT}/search/searchcommon.cpp
    ${RAPFI_ROOT}/search/searchoutput.cpp
    ${RAPFI_ROOT}/search/searchthread.cpp
    ${RAPFI_ROOT}/search/timecontrol.cpp
    ${RAPFI_ROOT}/config.cpp
    ${RAPFI_ROOT}/internalConfig.cpp
)

target_compile_features(rapfi PRIVATE cxx_std_17)
target_include_directories(rapfi PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${RAPFI_ROOT}
)

add_subdirectory(${RAPFI_ROOT}/external/cpptoml ${CMAKE_CURRENT_BINARY_DIR}/cpptoml)
add_subdirectory(${RAPFI_ROOT}/external/lz4 ${CMAKE_CURRENT_BINARY_DIR}/lz4)
add_subdirectory(${RAPFI_ROOT}/external/simde ${CMAKE_CURRENT_BINARY_DIR}/simde)

target_compile_definitions(rapfi PRIVATE
    NO_PREFETCH
    USE_NEON
)

target_link_libraries(rapfi PRIVATE
    cpptoml
    lz4
    simde
)
