"""A tiny fake Real-Debrid server for end-to-end runs of the desktop app (debug builds only).

    python scripts/fake-realdebrid.py 8765
    RDBATCH_API_BASE=http://127.0.0.1:8765/ RDBATCH_TOKEN=test RDBATCH_FAST=1 RDBATCH_DATA_DIR=<dir> rdbatch-desktop

Serves one torrent (SMK1) with one file (smoke.bin). Every request is printed so a run can be audited.
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
BODY = (b"real-debrid-batch smoke test\n" * 4000)[:100_000]
state = {"selected": False}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(self.command, self.path, flush=True)

    def reply(self, status, payload=None, raw=None, headers=None):
        data = raw if raw is not None else (json.dumps(payload).encode() if payload is not None else b"")
        self.send_response(status)
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def body(self):
        return self.rfile.read(int(self.headers.get("Content-Length", 0) or 0)).decode()

    def do_GET(self):
        if self.path == "/user":
            return self.reply(200, {"username": "smoke", "type": "premium", "premium": 3600})
        if self.path.startswith("/torrents/info/SMK1"):
            ready = state["selected"]
            return self.reply(200, {
                "id": "SMK1", "filename": "Smoke torrent", "status": "downloaded" if ready else "waiting_files_selection", "progress": 100 if ready else 0,
                "bytes": len(BODY), "files": [{"id": 1, "path": "/Smoke/smoke.bin", "bytes": len(BODY), "selected": int(ready)}], "links": ["LINK-1"] if ready else []})
        if self.path.startswith("/dl/smoke"):
            return self.reply(200, raw=BODY)
        self.reply(404, {"error": "unknown"})

    def do_POST(self):
        body = self.body()
        if self.path == "/torrents/addMagnet":
            return self.reply(201, {"id": "SMK1"})
        if self.path.startswith("/torrents/selectFiles/"):
            state["selected"] = True
            return self.reply(204)
        if self.path == "/unrestrict/link":
            return self.reply(200, {"filename": "smoke.bin", "filesize": len(BODY), "download": f"http://127.0.0.1:{PORT}/dl/smoke"})
        self.reply(404, {"error": "unknown " + body})


ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
