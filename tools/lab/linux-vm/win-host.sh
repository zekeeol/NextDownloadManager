#!/bin/bash
# Host side (Git Bash on Windows) of win.sh: run one of its steps in the lab VM and, when the step
# printed a frame and an output file is given, crop VirtualBox's screenshot to it.
#   win-host.sh <vm> <out.png|-> <win.sh step and its arguments>
#   e.g. win-host.sh ndm-ubuntu - start pr-199 ar dark adwaita
#        win-host.sh ndm-ubuntu shots/options.png open options
# Environment for the guest: NDM_STATE, NDM_SET, NDM_SETTLE, NDM_NATIVE_ACCENT, NDM_NATIVE_CONTRAST. The guest account
# from NDM_VM_USER (default mdm) and NDM_VM_PASSWORD (required).
V="${VBOXMANAGE:-/c/Program Files/Oracle/VirtualBox/VBoxManage.exe}"
user=${NDM_VM_USER:-mdm}
pass=${NDM_VM_PASSWORD:?set NDM_VM_PASSWORD to the lab account password}
# The password goes to VBoxManage in a file only this user reads (--passwordfile), removed on
# exit: never on a command line, where any process listing would show it.
pwf=$(mktemp); trap 'rm -f "$pwf"' EXIT; printf '%s' "$pass" > "$pwf"
vm=$1; out=$2; shift 2
env="NDM_STATE=${NDM_STATE:-} NDM_SET='${NDM_SET:-}' NDM_SETTLE=${NDM_SETTLE:-} NDM_ON=${NDM_ON:-} NDM_NATIVE_ACCENT=${NDM_NATIVE_ACCENT:-} NDM_NATIVE_CONTRAST=${NDM_NATIVE_CONTRAST:-}"
geo=$(MSYS_NO_PATHCONV=1 "$V" guestcontrol "$vm" run --exe /bin/bash --username "$user" --passwordfile "$(cygpath -w "$pwf")" --timeout 150000 --wait-stdout -- \
  -c "$env bash /home/$user/lab/win.sh $*" 2>/dev/null | tail -1)
[ "$out" = - ] && { echo "$geo"; exit 0; }
case "$geo" in *[0-9]\ *[0-9]\ *[0-9]\ *[0-9]*) ;; *) echo "no frame from win.sh $* ($geo)" >&2; exit 1 ;; esac
mkdir -p "$(dirname "$out")"
full="${out%.png}.full.png"
"$V" controlvm "$vm" screenshotpng "$(cygpath -aw "$full")"
NDM_GROW=${NDM_GROW:-0} python - "$full" "$out" $geo <<'PY'
import os, sys
from PIL import Image
full, out, x, y, w, h = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:7])
im = Image.open(full)
# NDM_GROW: pixels more on every side (a menu that opens past the window's edge).
g = int(os.environ.get("NDM_GROW") or 0); x, y, w, h = x - g, y - g, w + 2 * g, h + 2 * g
# A frame partly off the screen is clipped to it.
box = (max(x, 0), max(y, 0), min(x + w, im.width), min(y + h, im.height))
im.crop(box).save(out, optimize=True)
print(out, *box)
PY
rm -f -- "$full"
