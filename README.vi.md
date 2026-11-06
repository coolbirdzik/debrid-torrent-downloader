# DebridDock: ứng dụng all-in-one cho Real-Debrid, TorBox và AllDebrid

[English](README.md) · Tiếng Việt

[![Build and release](https://github.com/coolbirdzik/realdebrid/actions/workflows/build.yml/badge.svg)](https://github.com/coolbirdzik/realdebrid/actions/workflows/build.yml)
![Nền tảng: Windows, macOS, Linux](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Dịch vụ debrid: Real-Debrid, TorBox, AllDebrid](https://img.shields.io/badge/debrid-Real--Debrid%20%7C%20TorBox%20%7C%20AllDebrid-blueviolet)

**DebridDock là ứng dụng debrid miễn phí cho máy tính: một app duy nhất để thêm torrent vào Real-Debrid, TorBox hoặc
AllDebrid, duyệt chúng trong thư viện ảo, chọn đúng file cần lấy rồi tải về máy qua một hàng đợi tải nhanh, có thể
tiếp tục (resume).** Không còn phải mở nhiều tab web, copy từng link và dùng thêm một trình quản lý tải riêng cho mỗi
dịch vụ debrid. Chạy trên Windows, macOS và Linux.

![DebridDock: thư viện torrent ảo cho Real-Debrid, TorBox và AllDebrid, giao diện sáng](docs/images/app-vi.png)

## Mục lục

- [Vì sao dùng DebridDock](#vì-sao-dùng-debriddock)
- [Tính năng](#tính-năng)
- [Cách hoạt động](#cách-hoạt-động)
- [Ảnh chụp màn hình](#ảnh-chụp-màn-hình)
- [Tải về và cài đặt](#tải-về-và-cài-đặt)
- [Câu hỏi thường gặp](#câu-hỏi-thường-gặp)
- [Dành cho lập trình viên](#dành-cho-lập-trình-viên)

## Vì sao dùng DebridDock

Dịch vụ debrid tải torrent trên máy chủ tốc độ cao của họ rồi đưa cho bạn link tải HTTPS trực tiếp. Làm tay cho cả
một mùa phim hay một loạt torrent nghĩa là phải thêm từng magnet trên web, chờ, chọn file, unrestrict từng link rồi
đưa link vào trình quản lý tải. DebridDock làm trọn chuỗi đó trong một cửa sổ:

- **Một giao diện cho mọi dịch vụ debrid.** Real-Debrid, TorBox và AllDebrid dùng chung thư viện, hàng đợi và cách
  thao tác. Đổi dịch vụ trong Cài đặt; mỗi dịch vụ giữ token và torrent riêng.
- **Thư viện ảo thay cho một đống link.** Torrent được đọc trên dịch vụ và hiện thành thư mục, file như trình quản lý
  file, trước khi có bất kỳ thứ gì ghi xuống ổ đĩa.
- **Chỉ tải những gì bạn chọn.** Chọn từng video, cả thư mục hoặc toàn bộ phần còn lại; chỉ những file đó được chọn
  trên dịch vụ và tải về.
- **Hàng đợi tải cứ để đó mà chạy.** Thêm vào là tải ngay, tiếp tục được sau khi dừng hay app bị tắt, tự thử lại khi
  rớt mạng.

## Tính năng

### Ứng dụng debrid all-in-one

- Hỗ trợ **Real-Debrid, TorBox và AllDebrid**. Mỗi dịch vụ là một adapter riêng sau một giao diện chung, nên có thể
  thêm dịch vụ khác.
- Thêm **link magnet**, **URL `.torrent`** hoặc **file `.torrent` trên máy** (dán cả danh sách, hoặc kéo thả file vào
  cửa sổ). Torrent trùng được phát hiện.
- Xem và dọn **torrent trên tài khoản debrid**: liệt kê mọi torrent và xóa những cái không cần nữa, có bước xem lại
  trước khi xóa.

### Thư viện torrent ảo

- Duyệt mọi torrent dưới dạng **thư mục và file**, có lùi, tiến, lên một cấp (dùng được nút bên hông chuột, Alt+←/→
  và Backspace), breadcrumb và chế độ **Tất cả file** dạng phẳng.
- **Xem dạng danh sách hoặc lưới**, sắp xếp theo tên hoặc ngày thêm, bấm tiêu đề cột để sắp xếp (tên, dung lượng,
  trạng thái).
- **Tìm kiếm** trong thư mục và mọi thư mục con, bộ lọc **Chỉ video** hiện mọi video trong cây thư mục hiện tại.
- Mỗi file cho biết **đã tải xong, tải một phần hay đã có trong hàng đợi**, và giữ dấu đó kể cả khi xóa hàng đợi
  hay mở lại app.
- Bỏ file hoặc thư mục khỏi thư viện mà không đụng tới ổ đĩa hay dịch vụ, có hoàn tác (Ctrl+Z).

### Tải torrent từ debrid

- **Thêm file đã chọn, hoặc mọi file còn lại** trong thư viện vào hàng đợi chỉ với một cú bấm. Tải bắt đầu ngay.
- **Tải có resume** (`Range` + `If-Range`): dừng, tạm dừng hoặc tắt app, lần sau tải tiếp đúng từ byte đã dừng.
- **Tự động thử lại:** rớt mạng, quá thời gian chờ hay link tải hết hạn đều được thử lại với thời gian chờ tăng dần
  và tải tiếp phần dở. Nút **Retry tất cả** dành cho những gì vẫn lỗi.
- Chọn **thư mục lưu**, giữ cấu trúc thư mục của torrent hoặc lưu phẳng, chạy **1 đến 5 torrent cùng lúc**.
- Tiến độ hiện cả hai phía: tiến độ trên dịch vụ và tiến độ tải về máy, kèm tốc độ và số file.

### Nhanh, riêng tư, đa nền tảng

- Viết bằng **Rust** (engine tải) và **Tauri 2** (vỏ ứng dụng native) nên nhẹ và tốn ít RAM.
- Vẫn mượt với **hơn 30.000 file** trong thư viện và **hàng nghìn file** trong một torrent.
- API token nằm trong **keychain của hệ điều hành** (Windows Credential Manager, macOS Keychain, Linux Secret
  Service), không bao giờ ghi vào file cài đặt dạng thường, không bao giờ hiện trong thông báo lỗi.
- Bộ cài cho Windows, macOS và Linux, cùng **bản portable cho Windows** giữ toàn bộ dữ liệu cạnh file chạy.
- Giao diện tiếng Việt và tiếng Anh, sáng và tối.

## Cách hoạt động

1. **Kết nối dịch vụ.** Chọn Real-Debrid, TorBox hoặc AllDebrid trong Cài đặt và dán API token (app có nút mở trang
   lấy token của từng dịch vụ).
2. **Thêm torrent.** Dán link magnet hoặc URL `.torrent`, hoặc thả file `.torrent` vào. DebridDock thêm chúng vào tài
   khoản debrid và đọc danh sách file.
3. **Duyệt và chọn.** Mở thư mục trong thư viện ảo, tìm kiếm, lọc video và chọn những gì cần.
4. **Tải về.** Bấm **Thêm vào hàng đợi**. DebridDock chọn các file đó trên dịch vụ, chờ dịch vụ xử lý xong, lấy link
   trực tiếp và tải về thư mục của bạn, có resume và tự thử lại.

DebridDock không tự kết nối tới peer torrent nào: máy bạn chỉ tải qua HTTPS từ dịch vụ debrid.

## Ảnh chụp màn hình

![Hàng đợi tải của DebridDock: tải có resume, Retry tất cả và nút thao tác trên từng dòng](docs/images/queue-vi.png)

<details>
<summary>Giao diện tối và bản tiếng Anh</summary>

![Thư viện ảo của DebridDock, giao diện tối](docs/images/app-vi-dark.png)

![DebridDock bản tiếng Anh](docs/images/app-en.png)

</details>

Ảnh chụp từ app thật với dữ liệu mẫu (`scripts/seed-sample-data.py`), không dùng tài khoản hay tải thật.

## Tải về và cài đặt

Tải bộ cài cho hệ điều hành của bạn ở trang [Releases](https://github.com/coolbirdzik/realdebrid/releases): `.msi`
hoặc `.exe` cho Windows, `.dmg` cho macOS, `.deb`, `.rpm` hoặc `.AppImage` cho Linux. Bản portable cho Windows build
được từ mã nguồn bằng `pnpm build:portable` (xem [Build và test](#build-và-test)).

Bản build **chưa được ký số**, nên lần đầu mở hệ điều hành sẽ cảnh báo:

- **Windows:** SmartScreen hiện "Windows protected your PC". Bấm *More info*, rồi *Run anyway*.
- **macOS:** chuột phải vào app, chọn *Open*, hoặc chạy `xattr -dr com.apple.quarantine "/Applications/DebridDock.app"`.
- **Linux:** AppImage cần `chmod +x`. Để nhớ API token, gói `.deb` và `.rpm` cần keyring Secret Service (GNOME Keyring
  hoặc KWallet).

## Câu hỏi thường gặp

**Dịch vụ debrid là gì?**
Là dịch vụ trả phí như Real-Debrid, TorBox hay AllDebrid, tải torrent (và file từ các trang lưu trữ) trên máy chủ của
họ rồi cho bạn link tải trực tiếp tốc độ cao. DebridDock là ứng dụng khách cho các dịch vụ đó; bạn cần tài khoản và
API token của riêng mình.

**DebridDock có miễn phí không?**
Có. DebridDock miễn phí và mã nguồn mở (MIT). Bạn chỉ trả tiền gói debrid của mình.

**DebridDock có tải torrent bằng BitTorrent trên máy tôi không?**
Không. Dịch vụ debrid lấy torrent. DebridDock chỉ tải file đã xong qua HTTPS, nên không peer nào thấy IP của bạn.

**Có tải được chỉ vài file trong torrent, ví dụ một tập phim, không?**
Được. Thư viện ảo sinh ra để làm việc đó: mở torrent, chọn file hoặc thư mục cần lấy rồi thêm vào hàng đợi. Chỉ những
file đó được chọn trên dịch vụ và tải về.

**Rớt mạng hoặc tắt app giữa chừng thì sao?**
File dở được giữ lại. Lỗi kết nối được tự thử lại, và sau khi mở lại app, **Bắt đầu / Resume** tải tiếp mọi file từ
đúng byte đã dừng.

**Dùng được nhiều tài khoản debrid không?**
Bạn giữ được token cho từng dịch vụ và chuyển giữa Real-Debrid, TorBox, AllDebrid trong Cài đặt. Mỗi torrent trong
hàng đợi nhớ tài khoản của nó và không bao giờ được tiếp tục bằng tài khoản khác.

**Token và dữ liệu được lưu ở đâu?**
Token nằm trong keychain của hệ điều hành. Thư viện, hàng đợi và cài đặt nằm trong một file `data.json` ở thư mục dữ
liệu của app (hoặc cạnh file chạy với bản portable).

Vui lòng chỉ tải nội dung bạn có quyền tải.

## Dành cho lập trình viên

DebridDock gồm lõi và engine tải viết bằng Rust, vỏ Tauri 2 và giao diện Svelte 5.

### Cấu trúc thư mục

| Đường dẫn | Nội dung |
| --- | --- |
| `crates/core` | `rdbatch-core`: adapter cho từng dịch vụ, downloader có resume, engine hàng đợi, lưu trữ. Không phụ thuộc UI hay Tauri nên `cargo test` chạy ở đâu cũng được. |
| `src-tauri` | `rdbatch-desktop`: trạng thái ứng dụng, lệnh IPC, sự kiện gộp, truy cập keychain. |
| `src` | Giao diện Svelte. `src/lib/browser.ts` chứa logic duyệt thư viện dưới dạng hàm thuần, có test. |
| `scripts` | `check-i18n.mjs` (mọi chuỗi UI đều có bản dịch tiếng Anh), `seed-sample-data.py` (thư viện mẫu để chụp ảnh), `fake-realdebrid.py` (server giả để chạy end-to-end). |
| `.github/workflows/build.yml` | Test trên cả ba hệ điều hành, build bộ cài và tạo draft release. |

### Build và test

Yêu cầu: Rust (stable), Node 22+, pnpm 10. Trên Linux cần thêm
`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev patchelf`.

```sh
pnpm install
pnpm tauri dev            # chạy app
cargo test --workspace
pnpm check && pnpm test && node scripts/check-i18n.mjs
pnpm tauri build          # bộ cài cho hệ điều hành hiện tại, trong target/release/bundle
pnpm build:portable       # bản portable, trong dist/portable và dist/DebridDock-portable-win-x64.zip (Windows)
```

`RDBATCH_DATA_DIR=<thư mục>` gom toàn bộ dữ liệu (hàng đợi, thư viện, cài đặt) vào một thư mục: dùng portable, test,
chụp ảnh. Bản portable có sẵn file `portable` cạnh file chạy, làm điều tương tự với thư mục `data` ngay bên cạnh.
API token không bao giờ ghi vào đó; token nằm trong keychain của hệ điều hành và tạm giữ trong bộ nhớ nếu máy không có
keychain.

### Vì sao nhanh

- Tiến độ không bao giờ gửi theo từng chunk. Engine cập nhật hàng đợi; một task gửi bản mới nhất của mỗi job thay đổi
  khoảng 250 ms một lần. UI bận không làm chậm việc tải, việc tải nhanh không làm ngập UI.
- Khi resume, mọi file của torrent được ghép trong một lượt, nên torrent hàng nghìn file cũng tiếp tục ngay.
- `data.json` được ghi nguyên tử và gộp lần ghi, trừ khi vừa tạo ID torrent từ xa: lúc đó ghi ngay để lỡ crash cũng
  không để lại torrent mồ côi trên dịch vụ.
- Danh sách được ảo hóa; thư viện lọc 30.000 file trong vài mili giây (`browser.test.ts` kiểm tra).

### Các quy tắc an toàn được test

Resume bằng `Range` + `If-Range`, tải lại từ đầu khi server bỏ qua `Range`, từ chối `Content-Range` sai, không coi
body bị cụt là đã xong, không bao giờ ghi đè file đã hoàn tất, giữ file dở khi dừng, từ chối path traversal và thư mục
symlink, tự thử lại lỗi kết nối nhưng không bao giờ gửi lại upload hoặc xóa chưa rõ kết quả, không để token lọt vào
bất kỳ thông báo lỗi nào (kể cả URL tải của TorBox), từ chối torrent thuộc tài khoản hoặc dịch vụ khác.

### Phát hành

`.github/workflows/build.yml` test và build bộ cài trên mỗi lần push lên `main` và mỗi pull request (lưu dạng
artifact 14 ngày). Với tag `vX.Y.Z` khớp `version` trong `src-tauri/tauri.conf.json`, workflow còn tạo draft release
kèm checksum:

```sh
git tag -a v0.3.0 -m "Release v0.3.0"
git push origin v0.3.0
```

### Lịch sử

DebridDock trước đây tên Real-Debrid Batch, đổi tên khi hỗ trợ nhiều dịch vụ. Dự án bắt đầu là app WPF/.NET chỉ chạy
trên Windows; phần code đó đã bị xóa khi app Tauri thay thế và vẫn xem được trong lịch sử git đến commit `8e3a5d1`.

## Giấy phép

MIT.
