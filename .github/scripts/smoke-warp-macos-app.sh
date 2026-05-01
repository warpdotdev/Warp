#!/usr/bin/env bash
set -euxo pipefail

app="${1:?usage: smoke-warp-macos-app.sh <WarpOss.app> [label]}"
label="${2:-warp-oss}"
slug="$(printf '%s' "$label" | tr -c '[:alnum:]._-' '-')"
binary="$app/Contents/MacOS/warp-oss"
stdout="${RUNNER_TEMP:?}/${slug}.stdout"
stderr="${RUNNER_TEMP:?}/${slug}.stderr"
log="${HOME:?}/Library/Logs/warp-oss.log"

test -d "$app"
test -x "$binary"
test -s "$app/Contents/Info.plist"
test -s "$app/Contents/Resources/settings_schema.json"

mkdir -p "$HOME/Library/Logs"
"$binary" >"$stdout" 2>"$stderr" &
pid="$!"
echo "Started $label with pid $pid"

runtime_ok=0
for _ in $(seq 1 20); do
  if ! kill -0 "$pid" 2>/dev/null; then
    break
  fi

  runtime_ok=1
  if [ -s "$log" ]; then
    break
  fi
  sleep 1
done

if kill -0 "$pid" 2>/dev/null; then
  kill "$pid" || true
fi
wait "$pid" || true

echo "=== $label stdout ==="
sed -n '1,200p' "$stdout" || true
echo "=== $label stderr ==="
sed -n '1,200p' "$stderr" || true
echo "=== $label log ==="
if [ -s "$log" ]; then
  tail -n 200 "$log"
else
  echo "No warp-oss.log was written."
fi

if [ "$runtime_ok" -ne 1 ] && [ ! -s "$log" ]; then
  echo "$label neither stayed alive briefly nor wrote a log file" >&2
  exit 1
fi
