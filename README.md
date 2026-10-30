# Real-Debrid Batch

[English](README.en.md) · Tiếng Việt

[![Build and release](https://github.com/coolbirdzik/realdebrid/actions/workflows/build-release.yml/badge.svg)](https://github.com/coolbirdzik/realdebrid/actions/workflows/build-release.yml)

**[Tải bản Windows x64 (.exe)](https://github.com/coolbirdzik/realdebrid/releases/latest/download/RealDebridBatch-win-x64.exe)** · [Bản ZIP](https://github.com/coolbirdzik/realdebrid/releases/latest/download/RealDebridBatch-win-x64.zip) · [Tất cả phiên bản](https://github.com/coolbirdzik/realdebrid/releases)

App desktop Windows để dán hàng loạt magnet, URL `.torrent` hoặc thêm nhiều file `.torrent`. Chọn một nơi lưu chung, bấm **Bắt đầu / Resume**; app tự chọn **toàn bộ file** bằng `files=all`, chờ Real-Debrid tải xong, lấy link và tải về máy.

## Giao diện

![Giao diện tiếng Việt của Real-Debrid Batch với hàng đợi mẫu](docs/images/app-vi.png)

Ảnh render trực tiếp từ giao diện WPF với dữ liệu minh họa, không dùng tài khoản hoặc tải torrent thật. [Xem giao diện tiếng Anh](docs/images/app-en.png).

<details>
<summary>Giao diện tối</summary>

![Real-Debrid Batch với giao diện tối](docs/images/app-vi-dark.png)

</details>

## Tính năng

| Tính năng | Hoạt động |
| --- | --- |
| Thêm hàng loạt | Magnet, URL `.torrent`, nhiều file `.torrent` hoặc kéo thả. |
| Tự chọn toàn bộ file | App chọn `files=all` trên Real-Debrid và tải lần lượt từng file. |
| Tải song song | Xử lý 1–5 torrent; hiển thị riêng tiến trình Real-Debrid và tải về máy. |
| Tiếp tục tải | Retry / Resume / Stop từng dòng hoặc nhiều dòng; giữ file `.part`. |
| Quản lý hàng đợi | Pause / Stop tất cả, xóa dòng đã chọn hoặc xóa toàn bộ; nhớ hàng đợi khi mở lại. |
| Sắp xếp file | Lưu phẳng hoặc giữ thư mục torrent; tự tránh trùng tên, không ghi đè file có sẵn. |
| Việt / Anh | Mặc định tiếng Anh; đổi ngôn ngữ ngay và nhớ lựa chọn. |
| Giao diện Fluent | Panel thiết lập riêng, hàng đợi rộng, theme sáng/tối/theo Windows và nhớ lựa chọn. |
| Lưu token | Tùy chọn nhớ token, mã hóa bằng Windows DPAPI theo tài khoản hiện tại. |
| Bản portable | Một file `.exe` cho Windows x64, kèm .NET runtime; không cần cài .NET riêng. |

## Mở app

Tải `RealDebridBatch-win-x64.exe` từ [GitHub Releases](https://github.com/coolbirdzik/realdebrid/releases/latest) rồi mở trực tiếp. Nếu dùng bản ZIP, giải nén rồi chạy `RealDebridBatch.exe`. Cần Windows x64 và tài khoản Real-Debrid Premium còn hạn; không cần cài .NET riêng.

Nếu chạy từ mã nguồn, mở `artifacts\RealDebridBatch\RealDebridBatch.exe` hoặc chạy `run.ps1` để tự build nếu chưa có exe.

1. Lấy private API token tại https://real-debrid.com/apitoken sau khi đăng nhập. Dán vào app và bấm **Kiểm tra**. Cần tài khoản Premium còn hạn.
2. Chọn nơi lưu toàn bộ. Mặc định bật **Lưu phẳng**: tất cả file nằm trực tiếp trong thư mục đã chọn. Bỏ chọn nếu muốn tạo thư mục riêng và giữ cấu trúc torrent.
3. Dán mỗi dòng một magnet hoặc URL trả về nội dung `.torrent`. Có thể chọn nhiều file `.torrent`, hoặc kéo thả vào app. Không hỗ trợ URL của trang chi tiết torrent/HTML.
4. Bấm **Thêm vào hàng đợi**, rồi **Bắt đầu / Resume**. Song song 1–5 torrent (mặc định 3); mỗi torrent tải từng file để giữ số kết nối vừa phải.

App mặc định dùng **English** khi chưa có lựa chọn ngôn ngữ. Chọn **Tiếng Việt / English** ở góc trên bên phải để đổi ngay, kể cả khi đang tải. Lựa chọn đã lưu được giữ khi cập nhật app. Bên cạnh là bộ chọn **Theo hệ thống / Sáng / Tối**; mặc định theo giao diện Windows. Tên file và phản hồi gốc từ máy chủ được giữ nguyên.

Hai cột tiến trình tách riêng phần Real-Debrid đang tải torrent và phần app tải về máy. Torrent không có seed có thể chờ lâu trên Real-Debrid.

Mỗi dòng có nút **Retry**, **Resume**, **Stop**, **Xóa**; có thể chọn nhiều dòng rồi dùng các nút tương ứng bên dưới:

- **Retry**: chạy lại ngay torrent bị lỗi, dùng lại ID Real-Debrid và tải tiếp phần `.part` nếu có. Có thể retry khi các torrent khác vẫn đang chạy; tác vụ mới tuân theo giới hạn song song đã chọn.
- **Resume**: bắt đầu torrent đang chờ hoặc tiếp tục torrent đã pause/stop. Giữ tên file và thư mục đã được gán; bỏ qua file đã hoàn tất. Nếu máy chủ không hỗ trợ tải tiếp, app tải lại file đó từ đầu.
- **Stop**: dừng tác vụ trong app và giữ file tạm để Resume sau. Torrent đang chờ lượt cũng có thể stop. Các torrent khác tiếp tục chạy. Stop chỉ dừng xử lý/tải về máy trong app; torrent trên Real-Debrid tiếp tục xử lý.
- **Xóa**: dừng và chờ tác vụ thoát trước khi xóa dòng khỏi danh sách, kể cả khi dòng đó đang tải. Giữ file đã tải, file tạm và torrent trên Real-Debrid.
- **Pause tất cả** / **Stop tất cả**: ngừng cả lượt xử lý và giữ phần đã tải. **Bắt đầu / Resume** tiếp tục các dòng đang chờ, pause hoặc stop; các dòng lỗi dùng **Retry** riêng.
- **Xóa toàn bộ**: xác nhận rồi dừng mọi tác vụ, chờ file đang ghi đóng lại và xóa toàn bộ bản ghi hàng đợi, kể cả dòng đang dừng, hoàn tất hoặc lỗi. Hủy cả bước kết nối đang chờ. Giữ nguyên file đã tải, file `.part`, metadata tải tiếp và torrent trên Real-Debrid.

## Lưu trữ

- **Lưu phẳng** (mặc định): `<nơi lưu>\<tên file>`. Không tạo thư mục torrent hoặc các thư mục con. File trùng tên được thêm hậu tố ` (2)`, ` (3)`... kể cả khi trùng giữa nhiều torrent tải song song, file đã có trên máy, hoặc file `.part` đang tải dở.
- Bỏ chọn **Lưu phẳng**: file nằm trong `<nơi lưu>\<tên torrent>_<mã hàng đợi>\...`, giữ cây thư mục khi API cung cấp ánh xạ file rõ ràng. Nếu RD trả link archive/split thì app lưu các archive/split đó, không tự giải nén ở cả hai chế độ.
- Lựa chọn được nhớ khi mở lại app. Khi tiếp tục torrent đã có file tải dở, app giữ chế độ và đường dẫn cũ. Thay đổi nơi lưu hoặc chế độ áp dụng cho torrent chưa bắt đầu lưu file; app không tự di chuyển các file cũ.
- Hàng đợi và cài đặt: `%LOCALAPPDATA%\RealDebridBatch\settings.json`. Token được mã hóa bằng Windows DPAPI theo tài khoản hiện tại khi bật **Nhớ token trên máy này**. Không đưa token vào URL, mã nguồn hoặc log.
- Bỏ dòng khỏi hàng đợi chỉ bỏ bản ghi trong app. File trên máy và torrent trên Real-Debrid vẫn giữ nguyên. App không tự xóa torrent trên tài khoản.
- App không ghi đè file hoàn tất đã tồn tại. Tải tiếp dùng HTTP Range/If-Range nếu server hỗ trợ; nếu server trả cả file, app tải lại từ đầu. Kích thước được kiểm tra trước khi đổi `.part` thành file hoàn tất.
- Đóng app sẽ hủy tác vụ và lưu hàng đợi. Mở lại rồi bấm **Bắt đầu / Resume**; không tự chạy nền khi đóng. Trạng thái Stop được nhớ khi mở lại app.

## Build và kiểm tra

Yêu cầu .NET 10 SDK để build. Không dùng thư viện NuGet bên ngoài.

```powershell
dotnet build src/RdBatch.App -c Release
dotnet run --project tests/RdBatch.Tests -c Release
dotnet run --project tests/RdBatch.App.Tests -c Release
.\build.ps1
```

`build.ps1 -FrameworkDependent` cho bản nhỏ hơn, cần .NET 10 Desktop Runtime trên máy đích. API chính thức: https://api.real-debrid.com/.

Các kiểm tra tự động dùng HTTP giả lập, không cần token và không gửi torrent thật lên tài khoản.

## CI/CD và phát hành

- **Push lên `main` / pull request:** chạy kiểm tra core và WPF trên Windows, build bản portable, lưu `.exe`, ZIP, SHA-256 và ảnh giao diện trong mục **Artifacts** của [GitHub Actions](https://github.com/coolbirdzik/realdebrid/actions). Artifact được giữ 14 ngày và tải xuống khi đăng nhập GitHub.
- **Chạy thủ công:** vào **Actions → Build and release → Run workflow** để kiểm tra và lấy artifact; thao tác này không tạo Release.
- **Đẩy tag `vMAJOR.MINOR.PATCH`:** sau khi test và build thành công, workflow tự tạo GitHub Release với ghi chú thay đổi, file `.exe`, ZIP và `SHA256SUMS.txt`. Tag có hậu tố như `v0.2.0-beta.1` tạo prerelease.
- Workflow dùng `GITHUB_TOKEN` sẵn có; chỉ job phát hành có quyền `contents: write`. Không cần thêm API token Real-Debrid, GitHub PAT hay SSH key vào secrets.

Ví dụ phát hành phiên bản tiếp theo:

```powershell
git push origin main
git tag -a v0.1.1 -m 'Release v0.1.1'
git push origin v0.1.1
```

Build bộ file phát hành ngay trên máy:

```powershell
.\tools\Build-Release.ps1 -Version 0.1.1
```

Kết quả nằm trong `artifacts/release/`. Kiểm tra SHA-256 của file tải về bằng `Get-FileHash .\RealDebridBatch-win-x64.exe -Algorithm SHA256` và đối chiếu với `SHA256SUMS.txt` trong cùng Release. Release chưa ký số Authenticode.

Kiểm tra WPF dùng thư mục cài đặt thử riêng, xác minh đổi ngôn ngữ và tạo ảnh giao diện ở `artifacts/ui-checks/`. Icon mới có nguồn vector tại `src/RdBatch.App/Assets/AppIcon.svg`; chạy `tools/Generate-Icon.ps1` để tạo lại PNG và ICO đa kích thước (16–256 px), dùng cho cửa sổ, taskbar và file exe.

Để cập nhật ảnh trong README sau khi đổi UI, chạy kiểm tra WPF ở thư mục gốc repo rồi chép `artifacts/ui-checks/vietnamese.png` sang `docs/images/app-vi.png` và `english.png` sang `docs/images/app-en.png`.
