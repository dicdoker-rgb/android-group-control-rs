#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  cat <<'EOF'
Usage:
  scripts/scrcpy-whep-bridge.sh start <adb-serial>
  scripts/scrcpy-whep-bridge.sh stop <adb-serial>
  scripts/scrcpy-whep-bridge.sh status <adb-serial>

Environment:
  MEDIAMTX_RTMP_BASE   Default: rtmp://192.168.31.9:1935
  MEDIAMTX_WHEP_BASE   Default: http://192.168.31.9:8889
  SCRCPY_VIDEO_ENCODER Default: c2.rk.avc.encoder
  TMUX_BIN             Default: first tmux found in PATH
EOF
  exit 1
fi

ACTION="$1"
SERIAL="$2"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_DIR="${SCRCPY_WHEP_RUNTIME_DIR:-/tmp/android-group-control-scrcpy-whep}"
SERIAL_KEY="$(printf '%s' "$SERIAL" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//')"
PATH_KEY="scrcpy-${SERIAL_KEY}"
INSTANCE_DIR="$RUNTIME_DIR/$PATH_KEY"
FIFO_PATH="$INSTANCE_DIR/stream.mkv"
LOG_PATH="$INSTANCE_DIR/bridge.log"
SUPERVISOR_PID_FILE="$INSTANCE_DIR/supervisor.pid"
FFMPEG_PID_FILE="$INSTANCE_DIR/ffmpeg.pid"
SCRCPY_PID_FILE="$INSTANCE_DIR/scrcpy.pid"
MEDIAMTX_WHEP_BASE="${MEDIAMTX_WHEP_BASE:-http://192.168.31.9:8889}"
MEDIAMTX_RTMP_BASE="${MEDIAMTX_RTMP_BASE:-rtmp://192.168.31.9:1935}"
SCRCPY_VIDEO_ENCODER="${SCRCPY_VIDEO_ENCODER:-c2.rk.avc.encoder}"
TMUX_BIN="${TMUX_BIN:-$(command -v tmux || true)}"
WHEP_URL="${MEDIAMTX_WHEP_BASE}/${PATH_KEY}/whep"
INGEST_URL="${MEDIAMTX_RTMP_BASE}/${PATH_KEY}"
SESSION_NAME="scrcpy-whep-${SERIAL_KEY}"

mkdir -p "$INSTANCE_DIR"

is_running() {
  local pid_file="$1"
  [[ -f "$pid_file" ]] || return 1
  local pid
  pid="$(cat "$pid_file" 2>/dev/null || true)"
  [[ -n "$pid" ]] || return 1
  kill -0 "$pid" 2>/dev/null
}

cleanup_stale() {
  for pid_file in "$SUPERVISOR_PID_FILE" "$FFMPEG_PID_FILE" "$SCRCPY_PID_FILE"; do
    if [[ -f "$pid_file" ]] && ! is_running "$pid_file"; then
      rm -f "$pid_file"
    fi
  done
  if [[ -p "$FIFO_PATH" ]] && ! is_running "$SUPERVISOR_PID_FILE"; then
    rm -f "$FIFO_PATH"
  fi
}

session_exists() {
  [[ -n "$TMUX_BIN" ]] || return 1
  "$TMUX_BIN" has-session -t "$SESSION_NAME" 2>/dev/null
}

require_tmux() {
  if [[ -z "$TMUX_BIN" ]]; then
    echo "tmux not found in PATH; set TMUX_BIN or install tmux" >&2
    exit 1
  fi
}

start_bridge() {
  require_tmux
  cleanup_stale
  if session_exists; then
    echo "bridge already running for $SERIAL"
    echo "WHEP: $WHEP_URL"
    exit 0
  fi

  : > "$LOG_PATH"
  local supervisor_cmd=(
    python3 -u "$SCRIPT_DIR/scrcpy_whep_bridge_supervisor.py"
    --serial "$SERIAL"
    --instance-dir "$INSTANCE_DIR"
    --ingest-url "$INGEST_URL"
    --video-encoder "$SCRCPY_VIDEO_ENCODER"
    --supervisor-pid-file "$SUPERVISOR_PID_FILE"
  )
  local shell_cmd
  printf -v shell_cmd '%q ' "${supervisor_cmd[@]}"
  shell_cmd="${shell_cmd}>>$(printf '%q' "$LOG_PATH") 2>&1"
  "$TMUX_BIN" new-session -d -s "$SESSION_NAME" "$shell_cmd"
  sleep 1.5

  echo "started bridge for $SERIAL"
  echo "SESSION: $SESSION_NAME"
  echo "INGEST: $INGEST_URL"
  echo "WHEP: $WHEP_URL"
  echo "LOG:  $LOG_PATH"
}

stop_bridge() {
  require_tmux
  cleanup_stale
  if session_exists; then
    "$TMUX_BIN" kill-session -t "$SESSION_NAME" 2>/dev/null || true
    sleep 0.8
  fi
  for pid_file in "$SUPERVISOR_PID_FILE" "$SCRCPY_PID_FILE" "$FFMPEG_PID_FILE"; do
    if is_running "$pid_file"; then
      kill "$(cat "$pid_file")" 2>/dev/null || true
      sleep 0.2
    fi
    if is_running "$pid_file"; then
      kill -9 "$(cat "$pid_file")" 2>/dev/null || true
    fi
    rm -f "$pid_file"
  done
  rm -f "$FIFO_PATH"
  echo "stopped bridge for $SERIAL"
}

status_bridge() {
  cleanup_stale
  local session_state="stopped"
  local supervisor_state="stopped"
  local ffmpeg_state="stopped"
  local scrcpy_state="stopped"
  if session_exists; then
    session_state="running ($SESSION_NAME)"
  fi
  if is_running "$SUPERVISOR_PID_FILE"; then
    supervisor_state="running ($(cat "$SUPERVISOR_PID_FILE"))"
  fi
  if is_running "$FFMPEG_PID_FILE"; then
    ffmpeg_state="running ($(cat "$FFMPEG_PID_FILE"))"
  fi
  if is_running "$SCRCPY_PID_FILE"; then
    scrcpy_state="running ($(cat "$SCRCPY_PID_FILE"))"
  fi
  echo "serial: $SERIAL"
  echo "path:   $PATH_KEY"
  echo "tmux:   $session_state"
  echo "super:  $supervisor_state"
  echo "ffmpeg: $ffmpeg_state"
  echo "scrcpy: $scrcpy_state"
  echo "WHEP:   $WHEP_URL"
  echo "LOG:    $LOG_PATH"
}

case "$ACTION" in
  start)
    start_bridge
    ;;
  stop)
    stop_bridge
    ;;
  status)
    status_bridge
    ;;
  *)
    echo "unknown action: $ACTION" >&2
    exit 1
    ;;
esac
