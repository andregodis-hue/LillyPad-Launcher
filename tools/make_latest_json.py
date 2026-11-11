#!/usr/bin/env python3
"""Prints the Tauri updater manifest (latest.json) for a signed installer.
usage: make_latest_json.py <installer.exe> <tag>   (expects <installer.exe>.sig next to it)
The launcher downloads from DOWNLOAD_BASE, the file must be published there under the same name."""
import json, os, sys, datetime
exe, tag = sys.argv[1], sys.argv[2]
DOWNLOAD_BASE = "https://lillypadnetwork.se/launcher/"
sig = open(exe + ".sig").read().strip()
name = os.path.basename(exe).replace(" ", "%20")
print(json.dumps({
    "version": tag.lstrip("v"),
    "notes": "",
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {"windows-x86_64": {"signature": sig, "url": DOWNLOAD_BASE + name}},
}, indent=2))
