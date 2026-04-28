#!/bin/bash

# These env vars must be set:
# SENTRY_PROJECT
# SENTRY_AUTH_TOKEN
# SENTRY_ORG
# DEBUG_FILE_OR_FOLDER_PATH

set -x

if which sentry-cli >/dev/null; then
  ERROR="$(sentry-cli upload-dif "$DEBUG_FILE_OR_FOLDER_PATH")"
  if [ ! $? -eq 0 ]; then
    echo "warning: sentry-cli - $ERROR"
  fi
else
  echo "warning: sentry-cli not installed, download from https://github.com/getsentry/sentry-cli/releases"
fi
