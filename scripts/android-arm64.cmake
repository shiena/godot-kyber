# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

# Pin the ABI before including the NDK defaults used by bundled CMake projects.
set(ANDROID_ABI arm64-v8a CACHE STRING "Android ABI" FORCE)
set(ANDROID_PLATFORM android-24 CACHE STRING "Android platform" FORCE)
include("$ENV{ANDROID_NDK_HOME}/build/cmake/android.toolchain.cmake")
