#!/usr/bin/env python3
"""Long-lived driver: loads scripts/figby-mcp.py and serves do_tool() calls over a unix socket.
Used when the Figby MCP server is not attached to the Claude session."""
import importlib.util, json, os, socket, base64, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location("figby_mcp", ROOT / "scripts/figby-mcp.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
SOCK = os.environ.get("FIGBY_DRIVER_SOCK", "/tmp/figby-driver.sock")
OUT = Path(os.environ.get("FIGBY_SHOTS", "/tmp/figby-shots"))
OUT.mkdir(parents=True, exist_ok=True)
if os.path.exists(SOCK): os.unlink(SOCK)
s = socket.socket(socket.AF_UNIX); s.bind(SOCK); s.listen(1)
n = 0
while True:
    c, _ = s.accept()
    data = b""
    while not data.endswith(b"\n"):
        chunk = c.recv(65536)
        if not chunk: break
        data += chunk
    try:
        req = json.loads(data)
        res = m.do_tool(req["name"], req.get("args", {}))
        if "png_base64" in res:
            n += 1
            p = OUT / f"snap{n:04d}.png"; p.write_bytes(base64.b64decode(res.pop("png_base64"))); res["png"] = str(p)
    except Exception as e:
        res = {"error": f"{type(e).__name__}: {e}"}
    c.sendall((json.dumps(res, ensure_ascii=False) + "\n").encode()); c.close()
