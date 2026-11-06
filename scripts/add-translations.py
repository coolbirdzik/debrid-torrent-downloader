"""One-off helper: append English translations for strings introduced by the Tauri UI to src/lib/en.json."""
import json

path = "src/lib/en.json"
catalog = json.load(open(path, encoding="utf-8"))
catalog.update({
    "Đã kết nối: {0}": "Connected: {0}",
    "Đang đọc torrent…": "Reading torrents…",
    "Không tìm thấy file. Thử từ khóa khác hoặc bỏ lọc.": "No files found. Try another search or clear the filters.",
    "Không đọc được clipboard.": "Could not read the clipboard.",
    "Xóa toàn bộ hàng đợi?": "Clear the whole queue?",
    "{0} torrent trên tài khoản · {1} đã chọn": "{0} torrents on the account · {1} selected",
    "Xóa {0} torrent ID liệt kê bên dưới khỏi {1}. Dừng các tác vụ liên quan và bỏ dòng trong hàng đợi; giữ file trên máy. Xóa từ xa không thể hoàn tác.":
        "Delete the {0} torrent IDs listed below from {1}. Linked tasks are stopped and their queue rows removed; local files are kept. Remote deletion cannot be undone.",
    "Đang tải danh sách…": "Loading the list…",
    "Không có torrent nào trên tài khoản.": "There are no torrents on the account.",
    "Đã xóa {0} torrent trên {1}.": "Deleted {0} torrents on {1}.",
    "Đã thêm {0} torrent vào hàng đợi.": "Added {0} torrents to the queue.",
    "{0} torrent đã có trong thư viện.": "{0} torrents were already in the library.",
    "Không lưu được token vào kho khóa của hệ điều hành; token chỉ được giữ trong phiên này.":
        "Could not save the token in the operating system keychain; it is kept for this session only.",
    "Token hiện tại thuộc tài khoản {0}, không phải {1}. Tải lại danh sách trước khi xóa.":
        "The current token belongs to account {0}, not {1}. Reload the list before deleting.",
    "Torrent này thuộc tài khoản khác.": "This torrent belongs to another account.",
    "Thư mục không tồn tại.": "The folder does not exist.",
})
json.dump(catalog, open(path, "w", encoding="utf-8", newline=""), ensure_ascii=False, indent=2)
open(path, "a", encoding="utf-8").write("\n")
print(len(catalog), "entries")
