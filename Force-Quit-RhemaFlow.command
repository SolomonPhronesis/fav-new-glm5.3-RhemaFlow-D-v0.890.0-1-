#!/bin/sh
# Double-click this if RhemaFlow is frozen ("not responding"). It closes the app and stops any sound it left playing.
echo "Closing RhemaFlow..."
pkill -TERM -x rhemaflow-d 2>/dev/null          # polite first: the app stops its own players
sleep 2
pkill -KILL -x rhemaflow-d 2>/dev/null          # then force it if it is still there
PIDS="${TMPDIR:-/tmp}/rhemaflow-players.pid"
[ -f "$PIDS" ] && for p in $(cat "$PIDS"); do
  case "$(ps -p "$p" -o comm= 2>/dev/null)" in *afplay*|*ffmpeg*) kill "$p" 2>/dev/null;; esac
done
rm -f "$PIDS"
echo "Done. You can open RhemaFlow again."
sleep 2
