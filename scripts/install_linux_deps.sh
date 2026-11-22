#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

# Build and test prerequisites for Ubuntu 24.04. Invoke with sudo.
set -euo pipefail
apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential cmake pkg-config clang libclang-dev libssl-dev python3 git curl unzip \
    libudev-dev libevdev-dev libinput-dev libx11-dev libxcb1-dev \
    libxcb-render0-dev libxcb-xfixes0-dev libxcb-xtest0-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad gstreamer1.0-libav ffmpeg
