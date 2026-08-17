#!/bin/sh

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache --force --quiet /usr/share/icons/hicolor || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database --quiet /usr/share/applications || true
fi
