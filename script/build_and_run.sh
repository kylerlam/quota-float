#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
APP_NAME="Quota Float"
APP_PROCESS="quota-float"
BUNDLE_ID="app.quotafloat.desktop"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_APP="$ROOT_DIR/src-tauri/target/release/bundle/macos/$APP_NAME.app"
INSTALLED_APP="/Applications/$APP_NAME.app"
TRASH_DIR="${HOME}/.Trash"

case "$MODE" in
  run|--debug|debug|--logs|logs|--telemetry|telemetry|--verify|verify) ;;
  *)
    echo "usage: $0 [run|--debug|--logs|--telemetry|--verify]" >&2
    exit 2
    ;;
esac

cd "$ROOT_DIR"
BUILD_STARTED="$(date +%s)"
BUILD_STATUS=0
npm run tauri -- build --bundles app || BUILD_STATUS=$?

if [[ ! -d "$BUILD_APP" || ! -x "$BUILD_APP/Contents/MacOS/$APP_PROCESS" ]]; then
  echo "Tauri did not produce $BUILD_APP (exit $BUILD_STATUS)." >&2
  (( BUILD_STATUS == 0 )) && BUILD_STATUS=1
  exit "$BUILD_STATUS"
fi

BINARY_MTIME="$(stat -f %m "$BUILD_APP/Contents/MacOS/$APP_PROCESS")"
if (( BINARY_MTIME < BUILD_STARTED )); then
  echo "The app bundle was not refreshed by this build (exit $BUILD_STATUS)." >&2
  (( BUILD_STATUS == 0 )) && BUILD_STATUS=1
  exit "$BUILD_STATUS"
fi

/usr/bin/osascript -e 'tell application "Quota Float" to quit' >/dev/null 2>&1 || true
for _ in 1 2 3 4 5; do
  pgrep -x "$APP_PROCESS" >/dev/null 2>&1 || break
  sleep 1
done
pkill -x "$APP_PROCESS" >/dev/null 2>&1 || true

if [[ -d "$INSTALLED_APP" ]]; then
  BACKUP_APP="$TRASH_DIR/$APP_NAME-old-$(date +%Y%m%d-%H%M%S).app"
  mv "$INSTALLED_APP" "$BACKUP_APP"
  echo "Previous app moved to $BACKUP_APP"
fi
/usr/bin/ditto "$BUILD_APP" "$INSTALLED_APP"

open_app() {
  /usr/bin/open -n "$INSTALLED_APP"
}

case "$MODE" in
  run)
    open_app
    ;;
  --debug|debug)
    lldb -- "$INSTALLED_APP/Contents/MacOS/$APP_PROCESS"
    ;;
  --logs|logs)
    open_app
    /usr/bin/log stream --info --style compact --predicate "process == \"$APP_PROCESS\""
    ;;
  --telemetry|telemetry)
    open_app
    /usr/bin/log stream --info --style compact --predicate "subsystem == \"$BUNDLE_ID\""
    ;;
  --verify|verify)
    open_app
    sleep 2
    pgrep -x "$APP_PROCESS" >/dev/null
    ;;
esac
