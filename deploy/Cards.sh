#!/bin/sh
# Name: Primitive Cards
# Author: Primitive AI
# DontUseFBInk

app=/mnt/us/cards/bin/cards-kindle
[ -x "$app" ] || exit 1
work=$(mktemp -d /var/tmp/primitive-cards.XXXXXX) || exit 1
trap 'rm -rf "$work"' EXIT
cp "$app" "$work/run" && chmod 755 "$work/run" || exit 1
cd / || exit 1
"$work/run"
