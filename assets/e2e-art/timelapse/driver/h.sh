# helper: source h.sh ; k KEY ; t TEXT ; s (snapshot text) ; alt X
F=/Users/hippo/git_repos/Figby/assets/e2e-art/timelapse/driver/fig.py
k(){ python3 $F keypress "{\"key\":\"$1\"}" >/dev/null; }
t(){ python3 $F type_text "$(python3 -c 'import json,sys;print(json.dumps({"text":sys.argv[1]}))' "$1")" >/dev/null; }
alt(){ python3 $F type_text "{\"text\":\"\\u001b$1\"}" >/dev/null; }
s(){ sleep 0.4; python3 $F snapshot | sed -n "${1:-1},${2:-50}p"; }
bs(){ for i in $(seq 1 ${1:-6}); do python3 $F type_text "{\"text\":\"\u007f\"}" >/dev/null; done; }
