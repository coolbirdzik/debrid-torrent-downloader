"""Writes a labeled sample library and queue so the UI can be looked at (and screenshotted) without any account.

Usage: python scripts/seed-sample-data.py [data-directory]
The default directory is the app's own data directory on Windows. An existing data.json is never overwritten.
"""
import json
import os
import sys
from datetime import datetime, timedelta, timezone

directory = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.environ["APPDATA"], "dev.rdbatch.desktop")
os.makedirs(directory, exist_ok=True)
target = os.path.join(directory, "data.json")
if os.path.exists(target):
    sys.exit(f"{target} already exists; refusing to overwrite it")

now = datetime.now(timezone.utc)
iso = lambda minutes: (now - timedelta(minutes=minutes)).strftime("%Y-%m-%dT%H:%M:%SZ")
f = lambda i, p, b: {"id": i, "path": p, "bytes": b, "selected": 1}

def draft(name, key, minutes, files, downloaded=(), partial=()):
    return {"addedAt": iso(minutes), "provider": "realDebrid", "source": f"magnet:?xt=urn:btih:{key}", "sourceKey": f"btih:{key}", "name": name,
            "torrentId": None, "remoteAccount": None, "files": files, "excludedFileIds": [], "downloadedFileIds": list(downloaded), "partialFileIds": list(partial),
            "isHidden": False, "inspectionAttempts": 0, "nextInspectionAt": None, "inspectionError": None}

GB = 1024 ** 3
MB = 1024 ** 2
library = [
    draft("Example Series", "A" * 40, 5, [f(1, "/Example Series/Season 01/S01E01.mkv", 700 * MB), f(2, "/Example Series/Season 01/S01E02.mp4", 665 * MB),
          f(3, "/Example Series/Season 01/S01E03.mkv", 712 * MB), f(4, "/Example Series/Season 02/S02E01.mkv", 690 * MB), f(5, "/Example Series/Extras/trailer.mov", 50 * MB),
          f(6, "/Example Series/notes.txt", 2048), f(7, "/Example Series/cover.jpg", 400 * 1024)], downloaded=[1], partial=[2]),
    draft("Sample Album", "B" * 40, 90, [f(1, "/Sample Album/01 Intro.flac", 31 * MB), f(2, "/Sample Album/02 Theme.flac", 44 * MB), f(3, "/Sample Album/booklet.pdf", 6 * MB)]),
    draft("Sample Archive", "C" * 40, 600, [f(1, "/Sample Archive/data.zip", 1 * GB), f(2, "/Sample Archive/readme.nfo", 900)]),
]

def job(name, state, detail, remote, local, total, key=None, selected=None, active=False, args=()):
    return {"id": os.urandom(16).hex(), "provider": "realDebrid", "source": "", "sourceKey": key or "", "torrentId": None, "remoteAccount": None, "selectedFileIds": selected,
            "outputDirectory": None, "flattenFiles": None, "files": [], "name": name, "state": state, "detail": {"key": detail, "args": list(args)}, "remoteProgress": remote,
            "localProgress": local, "totalBytes": total, "isActive": active, "isStopping": False, "isRemoving": False}

jobs = [
    job("Sample documents", "completed", "Đã lưu {0} file · {1}", 100, 100, 12 * MB, args=("2", "C:\\Downloads\\DebridDock")),
    job("Sample videos", "stopped", "Đã dừng · giữ file tạm · bấm Resume để tải tiếp", 100, 52, 256 * MB),
    job("Linux image", "queued", "Sẵn sàng", 0, 0, 2 * GB),
    job("Unavailable torrent", "failed", "Torrent không còn trên dịch vụ. Xóa dòng khỏi hàng đợi rồi thêm lại.", 0, 0, 1024),
]

data = {"settings": {"provider": "realDebrid", "language": "en", "appearance": "light", "browserView": "list", "browserSort": "name",
                     "outputDirectory": "C:\\Downloads\\DebridDock", "flattenFiles": True, "concurrency": 3, "rememberToken": True, "libraryImportPaused": True},
        "jobs": jobs, "library": library, "remoteTorrents": []}
json.dump(data, open(target, "w", encoding="utf-8"), indent=2)
print("wrote", target)
