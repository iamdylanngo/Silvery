# CMake settings for building the speech engine (whisper.cpp) for Windows on ARM.
# Based on whisper.cpp's own cmake/arm64-windows-llvm.cmake: use Clang (GNU-style driver) targeting
# the MSVC ABI, because Microsoft's compiler can't build it for ARM.
# Used by release/windows-common.ps1 through the CMAKE_TOOLCHAIN_FILE environment variable.
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR ARM64)

set(target aarch64-pc-windows-msvc)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)
set(CMAKE_C_COMPILER_TARGET ${target})
set(CMAKE_CXX_COMPILER_TARGET ${target})

# - armv8.2-a+dotprod+fp16: every Windows-on-ARM PC from the Snapdragon 8cx onwards (not tuned to the
#   build machine, so the program runs on other PCs too).
# - -fms-runtime-lib=static: the C/C++ runtime goes inside the .exe, matching the Rust side (+crt-static).
# Set as the Release flags, because the Rust build tool passes its own CMAKE_C_FLAGS.
set(silvery_flags "-O3 -DNDEBUG -march=armv8.2-a+dotprod+fp16 -fvectorize -fms-runtime-lib=static -Wno-format -Wno-unused-variable -Wno-unused-function -Wno-gnu-zero-variadic-macro-arguments")
set(CMAKE_C_FLAGS_RELEASE "${silvery_flags}" CACHE STRING "" FORCE)
set(CMAKE_CXX_FLAGS_RELEASE "${silvery_flags}" CACHE STRING "" FORCE)
