#!/usr/bin/env python3
"""Fail when native core or service acquires a GUI dependency."""
import json
import subprocess
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked"]))
packages = {p["id"]: p["name"] for p in metadata["packages"]}
nodes = {n["id"]: n["dependencies"] for n in metadata["resolve"]["nodes"]}
for root in ("ndm-core", "ndm-backend"):
    todo = [key for key, value in packages.items() if value == root]
    visited = set()
    while todo:
        key = todo.pop()
        if key in visited:
            continue
        visited.add(key)
        name = packages[key]
        assert not name.startswith(("gpui", "egui", "eframe", "winit", "wgpu")), f"{root} depends on GUI package {name}"
        todo.extend(nodes[key])
    print(f"{root}: no GUI dependencies ({len(visited)} packages checked)")
