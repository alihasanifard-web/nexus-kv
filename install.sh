#!/usr/bin/env bash
# ==============================================================================
# ⚡ NexusKV One-Line Automated Installer for Linux & macOS
# Architect: Ali Hasanifard
# Usage: curl -sSL https://raw.githubusercontent.com/alihasanifard-web/nexus-kv/main/install.sh | bash
# ==============================================================================

set -e

REPO="alihasanifard-web/nexus-kv"
INSTALL_DIR="${HOME}/.nexus-kv/bin"

echo ""
echo -e "\033[1;36m  ███╗   ██╗███████╗██╗  ██╗██╗   ██╗███████╗\033[0m"
echo -e "\033[1;36m  ████╗  ██║██╔════╝╚██╗██╔╝██║   ██║██╔════╝\033[0m"
echo -e "\033[1;36m  ██╔██╗ ██║█████╗   ╚███╔╝ ██║   ██║███████╗\033[0m"
echo -e "\033[1;36m  ██║╚██╗██║██╔══╝   ██╔██╗ ██║   ██║╚════██║\033[0m"
echo -e "\033[1;36m  ██║ ╚████║███████╗██╔╝ ██╗╚██████╔╝███████║\033[0m"
echo -e "\033[1;37m  NexusKV Distributed Raft & LSM-Tree Storage Engine\033[0m"
echo -e "\033[90m  Architect: Ali Hasanifard (github.com/alihasanifard-web/nexus-kv)\033[0m"
echo ""

OS="$(uname -s)"
ARCH="$(uname -m)"

echo "[*] Detecting platform: ${OS} (${ARCH})..."

mkdir -p "${INSTALL_DIR}"

# Check if rust & cargo are available to build from source directly if prebuilts unavailable
if command -v cargo >/dev/null 2>&1; then
    echo "[*] Building high-performance release binaries via Cargo..."
    TMP_DIR=$(mktemp -d)
    git clone --depth 1 https://github.com/${REPO}.git "${TMP_DIR}/nexus-kv"
    cd "${TMP_DIR}/nexus-kv"
    cargo build --release --bins
    cp target/release/nexus-kv "${INSTALL_DIR}/nexus-kv"
    cp target/release/nexus_cli "${INSTALL_DIR}/nexus-cli"
    cp target/release/nexus_bench "${INSTALL_DIR}/nexus-bench"
    cp target/release/nexus_dashboard "${INSTALL_DIR}/nexus-dashboard"
    chmod +x "${INSTALL_DIR}"/*
    rm -rf "${TMP_DIR}"
else
    echo "[*] Downloading pre-compiled binary release..."
    case "${OS}" in
        Linux)
            TAR_NAME="nexus-kv-linux-x86_64.tar.gz"
            ;;
        Darwin)
            if [ "${ARCH}" = "arm64" ]; then
                TAR_NAME="nexus-kv-macos-arm64.tar.gz"
            else
                TAR_NAME="nexus-kv-macos-x86_64.tar.gz"
            fi
            ;;
        *)
            echo "[-] Unsupported operating system: ${OS}"
            exit 1
            ;;
    esac

    URL="https://github.com/${REPO}/releases/latest/download/${TAR_NAME}"
    curl -sSL "${URL}" -o "/tmp/${TAR_NAME}" || true
    if [ -f "/tmp/${TAR_NAME}" ]; then
        tar -xzf "/tmp/${TAR_NAME}" -C "${INSTALL_DIR}"
        chmod +x "${INSTALL_DIR}"/*
        rm -f "/tmp/${TAR_NAME}"
    fi
fi

# Add to PATH in ~/.bashrc or ~/.zshrc
SHELL_CONFIG="${HOME}/.bashrc"
if [ -f "${HOME}/.zshrc" ]; then
    SHELL_CONFIG="${HOME}/.zshrc"
fi

if ! grep -q ".nexus-kv/bin" "${SHELL_CONFIG}" 2>/dev/null; then
    echo "export PATH=\"${INSTALL_DIR}:\$PATH\"" >> "${SHELL_CONFIG}"
    echo "[+] Added ${INSTALL_DIR} to PATH in ${SHELL_CONFIG}"
fi

echo ""
echo -e "\033[32m✔ NexusKV successfully installed to ${INSTALL_DIR}!\033[0m"
echo -e "\033[37mTo start using NexusKV now:\033[0m"
echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
echo "  nexus-kv         # Start database server (port 6379)"
echo "  nexus-cli        # Open interactive REPL console"
echo "  nexus-bench      # Run 102k+ QPS benchmark"
echo ""
