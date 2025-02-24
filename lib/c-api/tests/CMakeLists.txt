cmake_minimum_required (VERSION 2.6)
project (WasmerRuntimeCApiTests)

add_executable(test-exported-memory test-exported-memory.c)
add_executable(test-exports test-exports.c)
add_executable(test-globals test-globals.c)
# trampoline functionality not yet implemented
#add_executable(test-import-function test-import-function.c)
add_executable(test-import-trap test-import-trap.c)
add_executable(test-imports test-imports.c)
add_executable(test-import-object test-import-object.c)
add_executable(test-instantiate test-instantiate.c)
add_executable(test-memory test-memory.c)
add_executable(test-module test-module.c)
add_executable(test-module-exports test-module-exports.c)
add_executable(test-module-imports test-module-imports.c)
add_executable(test-module-serialize test-module-serialize.c)
add_executable(test-tables test-tables.c)
add_executable(test-validate test-validate.c)
add_executable(test-context test-context.c)
add_executable(test-module-import-instantiate test-module-import-instantiate.c)


# Wasm C API tests
add_executable(wasm-c-api-hello wasm-c-api/example/hello.c)
add_executable(wasm-c-api-memory wasm-c-api/example/memory.c)
add_executable(wasm-c-api-global wasm-c-api/example/global.c)
#add_executable(wasm-c-api-table wasm-c-api/example/table.c)
add_executable(wasm-c-api-serialize wasm-c-api/example/serialize.c)
add_executable(wasm-c-api-callback wasm-c-api/example/callback.c)
#add_executable(wasm-c-api-finalize wasm-c-api/example/finalize.c)
add_executable(wasm-c-api-reflect wasm-c-api/example/reflect.c)
#add_executable(wasm-c-api-start wasm-c-api/example/start.c)

# Custom Wasm C API tests
add_executable(wasm-c-api-wasi wasm-c-api-wasi.c)

if (DEFINED WASI_TESTS)
    add_executable(test-wasi-import-object test-wasi-import-object.c)
endif()

if (DEFINED EMSCRIPTEN_TESTS)
    add_executable(test-emscripten-import-object test-emscripten-import-object.c)
endif()

include_directories(wasm-c-api/include)
include_directories(..)


find_library(
    WASMER_LIB NAMES libwasmer_c_api.dylib libwasmer_c_api.so wasmer_c_api.dll
    PATHS ${CMAKE_SOURCE_DIR}/../../../target/release/
)

if(NOT WASMER_LIB)
    message(FATAL_ERROR "wasmer library not found")
endif()

enable_testing()

set(
    COMPILER_OPTIONS
    # Clang or gcc
    $<$<OR:$<CXX_COMPILER_ID:Clang>,$<CXX_COMPILER_ID:GNU>>:
        "-Werror" >
    # MSVC
    $<$<CXX_COMPILER_ID:MSVC>:
        "/WX" >
)

target_link_libraries(test-exported-memory general ${WASMER_LIB})
target_compile_options(test-exported-memory PRIVATE ${COMPILER_OPTIONS})
add_test(test-exported-memory test-exported-memory)

target_link_libraries(test-exports general ${WASMER_LIB})
target_compile_options(test-exports PRIVATE ${COMPILER_OPTIONS})
add_test(test-exports test-exports)

target_link_libraries(test-globals general ${WASMER_LIB})
target_compile_options(test-globals PRIVATE ${COMPILER_OPTIONS})
add_test(test-globals test-globals)

# trampoline functionality not yet implemented
#target_link_libraries(test-import-function general ${WASMER_LIB})
#target_compile_options(test-import-function PRIVATE ${COMPILER_OPTIONS})
#add_test(test-import-function test-import-function)

target_link_libraries(test-import-trap general ${WASMER_LIB})
target_compile_options(test-import-trap PRIVATE ${COMPILER_OPTIONS})
add_test(test-import-trap test-import-trap)

target_link_libraries(test-imports general ${WASMER_LIB})
target_compile_options(test-imports PRIVATE ${COMPILER_OPTIONS})
add_test(test-imports test-imports)

target_link_libraries(test-import-object general ${WASMER_LIB})
target_compile_options(test-import-object PRIVATE ${COMPILER_OPTIONS})
add_test(test-import-object test-import-object)


if (DEFINED WASI_TESTS)
    target_link_libraries(test-wasi-import-object general ${WASMER_LIB})
    target_compile_options(test-wasi-import-object PRIVATE ${COMPILER_OPTIONS})
    # TODO: reenable this test
    #add_test(test-wasi-import-object test-wasi-import-object)
endif()

if (DEFINED EMSCRIPTEN_TESTS)
    target_link_libraries(test-emscripten-import-object general ${WASMER_LIB})
    target_compile_options(test-emscripten-import-object PRIVATE ${COMPILER_OPTIONS})
    add_test(test-emscripten-import-object test-emscripten-import-object)
endif()

target_link_libraries(test-instantiate general ${WASMER_LIB})
target_compile_options(test-instantiate PRIVATE ${COMPILER_OPTIONS})
add_test(test-instantiate test-instantiate)

target_link_libraries(test-memory general ${WASMER_LIB})
target_compile_options(test-memory PRIVATE ${COMPILER_OPTIONS})
add_test(test-memory test-memory)

target_link_libraries(test-module general ${WASMER_LIB})
target_compile_options(test-module PRIVATE ${COMPILER_OPTIONS})
add_test(test-module test-module)

target_link_libraries(test-module-exports general ${WASMER_LIB})
target_compile_options(test-module-exports PRIVATE ${COMPILER_OPTIONS})
add_test(test-module-exports test-module-exports)

target_link_libraries(test-module-imports general ${WASMER_LIB})
target_compile_options(test-module-imports PRIVATE ${COMPILER_OPTIONS})
add_test(test-module-imports test-module-imports)

target_link_libraries(test-module-serialize general ${WASMER_LIB})
target_compile_options(test-module-serialize PRIVATE ${COMPILER_OPTIONS})
add_test(test-module-serialize test-module-serialize)

target_link_libraries(test-tables general ${WASMER_LIB})
target_compile_options(test-tables PRIVATE ${COMPILER_OPTIONS})
add_test(test-tables test-tables)

target_link_libraries(test-validate general ${WASMER_LIB})
target_compile_options(test-validate PRIVATE ${COMPILER_OPTIONS})
add_test(test-validate test-validate)

target_link_libraries(test-context general ${WASMER_LIB})
target_compile_options(test-context PRIVATE ${COMPILER_OPTIONS})
add_test(test-context test-context)

target_link_libraries(test-module-import-instantiate general ${WASMER_LIB})
target_compile_options(test-module-import-instantiate PRIVATE ${COMPILER_OPTIONS})
add_test(test-module-import-instantiate test-module-import-instantiate)

target_link_libraries(wasm-c-api-hello general ${WASMER_LIB})
target_compile_options(wasm-c-api-hello PRIVATE ${COMPILER_OPTIONS})
add_test(NAME wasm-c-api-hello
         COMMAND wasm-c-api-hello
         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/wasm-c-api/example
)

target_link_libraries(wasm-c-api-memory general ${WASMER_LIB})
target_compile_options(wasm-c-api-memory PRIVATE ${COMPILER_OPTIONS})
add_test(NAME wasm-c-api-memory
         COMMAND wasm-c-api-memory
         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/wasm-c-api/example
)

target_link_libraries(wasm-c-api-global general ${WASMER_LIB})
target_compile_options(wasm-c-api-global PRIVATE ${COMPILER_OPTIONS})
add_test(NAME wasm-c-api-global
         COMMAND wasm-c-api-global
         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/wasm-c-api/example
)

#target_link_libraries(wasm-c-api-table general ${WASMER_LIB})
#target_compile_options(wasm-c-api-table PRIVATE ${COMPILER_OPTIONS})
#add_test(NAME wasm-c-api-table
#         COMMAND wasm-c-api-table
#         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/wasm-c-api/example
#)

target_link_libraries(wasm-c-api-serialize general ${WASMER_LIB})
target_compile_options(wasm-c-api-serialize PRIVATE ${COMPILER_OPTIONS})
add_test(NAME wasm-c-api-serialize
         COMMAND wasm-c-api-serialize
         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/wasm-c-api/example
)

target_link_libraries(wasm-c-api-callback general ${WASMER_LIB})
target_compile_options(wasm-c-api-callback PRIVATE ${COMPILER_OPTIONS})
add_test(NAME wasm-c-api-callback
         COMMAND wasm-c-api-callback
         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/wasm-c-api/example
)

#target_link_libraries(wasm-c-api-finalize general ${WASMER_LIB})
#target_compile_options(wasm-c-api-finalize PRIVATE ${COMPILER_OPTIONS})
#add_test(NAME wasm-c-api-finalize
#         COMMAND wasm-c-api-finalize
#         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/wasm-c-api/example
#)

target_link_libraries(wasm-c-api-reflect general ${WASMER_LIB})
target_compile_options(wasm-c-api-reflect PRIVATE ${COMPILER_OPTIONS})
add_test(NAME wasm-c-api-reflect
         COMMAND wasm-c-api-reflect
         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/wasm-c-api/example
)

#target_link_libraries(wasm-c-api-start general ${WASMER_LIB})
#target_compile_options(wasm-c-api-start PRIVATE ${COMPILER_OPTIONS})
#add_test(NAME wasm-c-api-start
#         COMMAND wasm-c-api-start
#         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/wasm-c-api/example
#)


set_property(TARGET wasm-c-api-wasi PROPERTY C_STANDARD 11)
target_link_libraries(wasm-c-api-wasi general ${WASMER_LIB})
target_compile_options(wasm-c-api-wasi PRIVATE ${COMPILER_OPTIONS})
add_test(NAME wasm-c-api-wasi
         COMMAND wasm-c-api-wasi
         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR} #/wasm-c-api/example
)

