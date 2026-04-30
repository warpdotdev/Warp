#!/usr/bin/env bash
# Check for Node.js and Yarn 4+ (required by command-signatures-v2 crate).
# Source this script from bootstrap/install scripts:
#   source "$PWD/script/check_node_yarn_deps.sh"

REQUIRED_NODE_VERSION="18.14.1"

# Check Node.js is installed
if ! command -v node >/dev/null 2>&1; then
    echo "Error: Node.js is not installed."
    echo "Node.js $REQUIRED_NODE_VERSION+ is required to build the command-signatures-v2 crate."
    echo "Install it via: https://nodejs.org/en/download"
    echo "  or use a version manager like nvm, fnm, or volta."
    return 1 2>/dev/null || exit 1
fi

# Check Node.js version
NODE_VERSION="$(node --version | sed 's/^v//')"
node_major="$(echo "$NODE_VERSION" | cut -d. -f1)"
node_minor="$(echo "$NODE_VERSION" | cut -d. -f2)"
node_patch="$(echo "$NODE_VERSION" | cut -d. -f3)"
req_major="$(echo "$REQUIRED_NODE_VERSION" | cut -d. -f1)"
req_minor="$(echo "$REQUIRED_NODE_VERSION" | cut -d. -f2)"
req_patch="$(echo "$REQUIRED_NODE_VERSION" | cut -d. -f3)"

if [ "$node_major" -lt "$req_major" ] || \
   { [ "$node_major" -eq "$req_major" ] && [ "$node_minor" -lt "$req_minor" ]; } || \
   { [ "$node_major" -eq "$req_major" ] && [ "$node_minor" -eq "$req_minor" ] && [ "$node_patch" -lt "$req_patch" ]; }; then
    echo "Error: Node.js $NODE_VERSION is too old."
    echo "Node.js $REQUIRED_NODE_VERSION+ is required."
    echo "Upgrade via your system package manager or version manager."
    return 1 2>/dev/null || exit 1
fi

# Check Yarn is installed
if ! command -v yarn >/dev/null 2>&1; then
    echo "Error: yarn is not installed."
    echo "yarn is required to build the command-signatures-v2 crate."
    echo "Enable it via corepack (ships with Node.js):"
    echo "  corepack enable"
    return 1 2>/dev/null || exit 1
fi

# Check Yarn version is 4+ (Corepack-managed, not Yarn 1.x from Homebrew/apt)
JS_CRATE_DIR="$PWD/crates/command-signatures-v2/js"
if ! YARN_VERSION="$(cd "$JS_CRATE_DIR" && yarn --version 2>/dev/null)"; then
    echo "Error: Could not determine yarn version from $JS_CRATE_DIR."
    echo "Ensure yarn is installed and Corepack is enabled:"
    echo "  corepack enable"
    return 1 2>/dev/null || exit 1
fi

YARN_MAJOR="$(echo "$YARN_VERSION" | cut -d. -f1)"
if [ "$YARN_MAJOR" -lt 4 ] 2>/dev/null; then
    echo "Error: yarn $YARN_VERSION detected, but Yarn 4+ is required."
    echo "The command-signatures-v2 crate requires Yarn 4 via Corepack."
    echo "If you have yarn installed via a system package, remove it first."
    echo "Then enable Corepack:"
    echo "  corepack enable"
    return 1 2>/dev/null || exit 1
fi

echo "✅ Node.js $(node --version) and yarn $YARN_VERSION detected."
