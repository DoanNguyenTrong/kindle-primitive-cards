#!/bin/sh
# stage.sh TARGET [STAGE_DIR]
set -eu
cd "$(dirname "$0")/.."
case "${1:-armhf}" in
    armhf|armv7-unknown-linux-musleabihf) target=armv7-unknown-linux-musleabihf ;;
    *) echo "Unsupported package architecture: $1" >&2; exit 1 ;;
esac
stage=${2:-build/stage}
target_dir=${CARGO_TARGET_DIR:-target}
bin=$target_dir/$target/release/cards-kindle
[ -x "$bin" ] || { echo "Build $bin before staging." >&2; exit 1; }
install -d "$stage/cards/bin" "$stage/documents"
install -m 755 "$bin" "$stage/cards/bin/cards-kindle"
install -m 755 deploy/Cards.sh "$stage/documents/Cards.sh"
install -d "$stage/cards/licenses"
for notice in licenses/*.txt; do install -m 644 "$notice" "$stage/cards/licenses/"; done
