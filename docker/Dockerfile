# Build machine for Silvery's Linux and Windows versions.
# Linux: built natively for x86_64 or arm64 (PLATFORM build arg). Windows: cross-built with MinGW-w64 (paused).
ARG PLATFORM=linux/amd64
FROM --platform=$PLATFORM rust:1-trixie

RUN apt-get update && apt-get install -y --no-install-recommends \
        cmake clang libclang-dev pkg-config \
        libasound2-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
        libxkbcommon-dev libwayland-dev libgl1-mesa-dev \
        mingw-w64 \
        xvfb libgl1-mesa-dri xauth libxkbcommon-x11-0 libegl1 fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/* \
    && update-alternatives --set x86_64-w64-mingw32-gcc /usr/bin/x86_64-w64-mingw32-gcc-posix \
    && update-alternatives --set x86_64-w64-mingw32-g++ /usr/bin/x86_64-w64-mingw32-g++-posix \
    && rustup target add x86_64-pc-windows-gnu

# Cross-compiler settings for the Windows build
ENV CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
    CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++ \
    AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar \
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

WORKDIR /src
