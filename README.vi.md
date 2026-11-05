# Real-Debrid Torrent Multiple Downloader

[English](README.md) · Tiếng Việt

[![Build and release](https://github.com/coolbirdzik/realdebrid/actions/workflows/build-release.yml/badge.svg)](https://github.com/coolbirdzik/realdebrid/actions/workflows/build-release.yml)

**[Tải bản Windows x64 (.exe)](https://github.com/coolbirdzik/realdebrid/releases/latest/download/RealDebridBatch-win-x64.exe)** · [Bản ZIP](https://github.com/coolbirdzik/realdebrid/releases/latest/download/RealDebridBatch-win-x64.zip) · [Tất cả phiên bản](https://github.com/coolbirdzik/realdebrid/releases)

App desktop Windows để duyệt và tải các file đã chọn trong torrent qua Real-Debrid. Dán magnet, URL `.torrent` hoặc thêm nhiều file `.torrent`; xem cây thư mục, tìm tên file, lọc video rồi đưa đúng các kết quả đã chọn vào hàng đợi.

## Giao diện

![Cây thư mục và kết quả tìm video trong torrent](docs/images/app-vi.png)

Ảnh render trực tiếp từ giao diện WPF với dữ liệu minh họa, không dùng tài khoản hoặc tải torrent thật. [Xem giao diện tiếng Anh](docs/images/app-en.png).

<details>
<summary>Giao diện tối</summary>

![Real-Debrid Batch với giao diện tối](docs/images/app-vi-dark.png)

</details>

## Tính năng

| Tính năng | Hoạt động |
| --- | --- |
| Thêm hàng loạt | Magnet, URL `.torrent`, nhiều file `.torrent` hoặc kéo thả. |
| Duyệt torrent | Cây thư mục, chế độ Danh sách/Lưới; tìm theo tên/đường dẫn trong cả thư mục con. |
| Tìm và chọn video | Lọc định dạng video, tick từng file hoặc chọn tất cả kết quả rồi thêm vào hàng đợi. |
| Dọn thư viện ảo | Tìm và bỏ file không cần, hoàn tác, rồi thêm toàn bộ file còn lại vào queue. Nhớ file đã bỏ và kiểu xem khi mở lại. |
| Batch mới | Xóa cả thư viện ảo, ô nhập và bộ lọc bằng **Batch mới**; giữ công việc đã đưa vào queue. |
| Xóa torrent trên Real-Debrid | Xem lại đúng các ID đã chọn rồi xác nhận xóa từ xa; dừng tác vụ liên quan trước, giữ file trên máy và báo lỗi từng torrent. |
| Tải song song | Xử lý 1–5 torrent; hiển thị riêng tiến trình Real-Debrid và tải về máy. |
| Tiếp tục tải | Retry / Resume / Stop từng dòng hoặc nhiều dòng; giữ file `.part`. |
| Quản lý hàng đợi | Pause / Stop tất cả, xóa dòng đã chọn hoặc xóa toàn bộ; nhớ hàng đợi khi mở lại. |
| Sắp xếp file | Lưu phẳng hoặc giữ thư mục torrent; tự tránh trùng tên, không ghi đè file có sẵn. |
| Việt / Anh | Mặc định tiếng Anh; đổi ngôn ngữ ngay và nhớ lựa chọn. |
| Giao diện phẳng | Ô nhập luôn hiện, dùng đường phân cách, chế độ Danh sách/Lưới và nhớ theme sáng/tối/theo Windows. |
| Lưu token | Tùy chọn nhớ token, mã hóa bằng Windows DPAPI theo tài khoản hiện tại. |
| Bản portable | Một file `.exe` cho Windows x64, kèm .NET runtime; không cần cài .NET riêng. |

## Mở app

Tải `RealDebridBatch-win-x64.exe` từ [GitHub Releases](https://github.com/coolbirdzik/realdebrid/releases/latest) rồi mở trực tiếp. Nếu dùng bản ZIP, giải nén rồi chạy `RealDebridBatch.exe`. Cần Windows x64 và tài khoản Real-Debrid Premium còn hạn; không cần cài .NET riêng.

Nếu chạy từ mã nguồn, mở `artifacts\RealDebridBatch\RealDebridBatch.exe` hoặc chạy `run.ps1` để tự build nếu chưa có exe.

1. Lấy private API token tại https://real-debrid.com/apitoken sau khi đăng nhập. Dán vào app và bấm **Kiểm tra**. Cần tài khoản Premium còn hạn.
2. Chọn nơi lưu toàn bộ. Mặc định bật **Lưu phẳng**: tất cả file nằm trực tiếp trong thư mục đã chọn. Bỏ chọn nếu muốn tạo thư mục riêng và giữ cấu trúc torrent.
3. Dán mỗi dòng một magnet hoặc URL trả về nội dung `.torrent`, hoặc chọn/kéo thả nhiều file `.torrent`. Các thao tác này chỉ điền ô nhập; bấm **Thêm vào thư viện** mới đọc cả loạt torrent. Không hỗ trợ URL trang chi tiết torrent/HTML. Ô nhập luôn hiện trên mọi tab, vẫn gõ/paste được khi queue hoặc bước đọc metadata đang chạy. Nội dung nhập thêm trong lúc đọc được giữ lại để bạn bấm thêm batch tiếp theo.
4. Trong tab **File trong torrent**, chọn **Danh sách** hoặc **Lưới**, duyệt cây thư mục hoặc tìm theo tên/đường dẫn. Tìm kiếm gồm thư mục hiện tại và thư mục con; chọn **Tất cả torrent** để tìm trên cả thư viện. Bật **Chỉ video** để bỏ phụ đề và các file khác.
5. Tìm file không muốn tải, tick từng file hoặc **Chọn / bỏ chọn kết quả**, rồi **Bỏ khỏi thư viện** (hoặc Delete khi đang thao tác ở phần file). Chỉ bỏ bản ghi ảo và thư mục đã trống; giữ file trên máy, torrent trên Real-Debrid và queue đã tạo. **Hoàn tác** khôi phục lượt bỏ gần nhất trong phiên hiện tại; file đã bỏ vẫn được nhớ khi mở lại app. Ctrl+A chọn các kết quả đang thấy, Ctrl+Z hoàn tác trong phần file.
6. Sau khi dọn, bấm **Thêm tất cả còn lại (N)** để đưa toàn bộ file còn lại trong cả thư viện vào queue, kể cả ngoài thư mục/từ khóa hiện tại; file đã có trong queue được bỏ qua. Hoặc chọn riêng kết quả cần tải rồi **Thêm vào hàng đợi**. Đổi bộ lọc sẽ bỏ chọn; đổi Danh sách/Lưới giữ lựa chọn. File đã có trong queue vẫn có thể bỏ khỏi thư viện ảo nhưng không bị thêm queue trùng.
7. Mở tab **Hàng đợi tải**, bấm **Bắt đầu / Resume**. Song song 1–5 torrent (mặc định 3); mỗi dòng chỉ tải lần lượt các file đã chọn.

Đọc metadata cần thêm nguồn lên Real-Debrid, nhưng chưa chọn file và chưa tải về máy. **Dừng đọc** hủy bước chờ; **Thêm vào thư viện** thử lại bằng ID đã lưu. Thư viện, file đã bỏ, kiểu xem và ID file đã chọn được nhớ khi mở lại app. Nếu thêm các file khác từ cùng torrent, app tạo dòng queue riêng và thêm torrent mới trên Real-Debrid lúc bắt đầu để giữ lựa chọn của tác vụ cũ.

Để bỏ cả thư mục, chọn nó trong cây bên trái rồi bấm nút thùng rác **Bỏ thư mục** phía trên cây, nhấn Delete hoặc chuột phải → **Bỏ thư mục khỏi thư viện**. Thao tác bỏ toàn bộ file và thư mục con, kể cả file bị bộ lọc tìm kiếm/video ẩn. Bỏ thư mục gốc torrent sẽ ẩn cả gốc, kể cả khi chưa có metadata. **Hoàn tác** khôi phục thư mục cùng các lựa chọn bỏ file trước đó; giữ file trên máy, queue và danh sách ID từ xa. Thư mục gốc tự biến mất khi file cuối cùng bị bỏ; các thư mục đã bỏ không xuất hiện lại sau khi mở app.

**Shift+click** chọn một dải từ mốc chọn trước đó trong Danh sách/Lưới và bảng Real-Debrid. Shift+click tiếp giữ nguyên mốc; **Ctrl+Shift+click** thêm dải và **Ctrl+click** bật/tắt từng mục. Checkbox và ô lưới vẫn bật/tắt riêng được khi không giữ Ctrl. Dải chọn theo đúng thứ tự đang sort/lọc. Đổi bộ lọc sẽ đặt lại mốc; đổi Danh sách/Lưới giữ mốc. Hộp thoại xác nhận và thông báo của app dùng đúng theme sáng/tối, gồm cả thanh tiêu đề. Xác nhận xóa mặc định chọn **Hủy**.

App mặc định dùng **English** khi chưa có lựa chọn ngôn ngữ. Chọn **Tiếng Việt / English** ở góc trên bên phải để đổi ngay, kể cả khi đang tải. Lựa chọn đã lưu được giữ khi cập nhật app. Bên cạnh là bộ chọn **Theo hệ thống / Sáng / Tối**; mặc định theo giao diện Windows. Tên file và phản hồi gốc từ máy chủ được giữ nguyên.

Hai cột tiến trình tách riêng phần Real-Debrid đang tải torrent và phần app tải về máy. Torrent không có seed có thể chờ lâu trên Real-Debrid.

Khi Real-Debrid đang tải, queue hiển thị số seed và tốc độ máy chủ báo về. **0 seed / 0 B/s** nghĩa là đang chờ nguồn tải và có thể chưa có link để tải về máy. App giữ nguyên ID file đã chọn và vẫn cho thêm các torrent khác trong lúc chờ.

## Batch mới và xóa torrent từ xa

Bấm **Batch mới** trong tab **File trong torrent** để xóa cả thư viện ảo, lịch sử bỏ file, ô nhập và bộ lọc tìm kiếm/video. Ô nhập vẫn hiện sẵn cho batch tiếp theo. Giữ hàng đợi, file đã tải và torrent trên Real-Debrid. ID từ xa vẫn được nhớ sau khi xóa thư viện hoặc queue, kể cả torrent mới đọc metadata chưa đưa vào queue.

Mở tab **Torrent trên Real-Debrid** để xem toàn bộ torrent trong tài khoản Real-Debrid của token, kể cả torrent thêm trên web. Danh sách tự tải khi mở tab (hoặc bấm **Tải lại**) và hiện dung lượng, trạng thái/tiến độ, ngày thêm, kèm nhãn **Trong hàng đợi** cho torrent đang gắn với hàng đợi tải. Đổi token sẽ xóa danh sách cũ. Tick từng torrent hoặc **Chọn / bỏ chọn tất cả**, bấm **Xóa torrent đã chọn…**, xem lại đúng tên/ID trong bảng rồi **Xác nhận xóa trên Real-Debrid**. Bấm **Hủy** trước khi bắt đầu sẽ giữ nguyên. Khi đang xóa, Hủy dừng các yêu cầu còn lại; các torrent đã xóa từ xa không thể Hoàn tác.

Trước khi xóa, app kiểm tra token vẫn thuộc đúng tài khoản đã tải danh sách; nếu khác thì không xóa gì. Sau đó app chờ tác vụ gắn với từng ID đã chọn dừng rồi mới xóa ID đó. Torrent đã xóa thành công/đã không còn biến khỏi danh sách, các dòng queue liên quan được bỏ khỏi dữ liệu lưu. Torrent bị lỗi vẫn hiện lý do và giữ dòng queue đã dừng để thử lại. Giữ file đã tải và `.part` trên máy. Metadata ảo vẫn duyệt được; nếu đưa các file này vào queue lần nữa, app sẽ thêm torrent mới khi bắt đầu tải. [Ảnh màn hình xem lại trước khi xóa](docs/images/remote-delete-vi.png) dùng dữ liệu minh họa.

Mỗi dòng có nút **Retry**, **Resume**, **Stop**, **Xóa**; có thể chọn nhiều dòng rồi dùng các nút tương ứng bên dưới:

- **Retry**: chạy lại ngay torrent bị lỗi, dùng lại ID Real-Debrid và tải tiếp phần `.part` nếu có. Có thể retry khi các torrent khác vẫn đang chạy; tác vụ mới tuân theo giới hạn song song đã chọn.
- **Resume**: bắt đầu torrent đang chờ hoặc tiếp tục torrent đã pause/stop. Giữ tên file và thư mục đã được gán; bỏ qua file đã hoàn tất. Nếu máy chủ không hỗ trợ tải tiếp, app tải lại file đó từ đầu.
- **Stop**: dừng tác vụ trong app và giữ file tạm để Resume sau. Torrent đang chờ lượt cũng có thể stop. Các torrent khác tiếp tục chạy. Stop chỉ dừng xử lý/tải về máy trong app; torrent trên Real-Debrid tiếp tục xử lý.
- **Xóa**: dừng và chờ tác vụ thoát trước khi xóa dòng khỏi danh sách, kể cả khi dòng đó đang tải. Giữ file đã tải, file tạm và torrent trên Real-Debrid.
- **Pause tất cả** / **Stop tất cả**: ngừng cả lượt xử lý và giữ phần đã tải. **Bắt đầu / Resume** tiếp tục các dòng đang chờ, pause hoặc stop; các dòng lỗi dùng **Retry** riêng.
- **Xóa toàn bộ**: xác nhận rồi dừng mọi tác vụ, chờ file đang ghi đóng lại và xóa toàn bộ bản ghi hàng đợi, kể cả dòng đang dừng, hoàn tất hoặc lỗi. Hủy cả bước kết nối đang chờ. Giữ nguyên file đã tải, file `.part`, metadata tải tiếp và torrent trên Real-Debrid.

## Lưu trữ

- **Lưu phẳng** (mặc định): `<nơi lưu>\<tên file>`. Không tạo thư mục torrent hoặc các thư mục con. File trùng tên được thêm hậu tố ` (2)`, ` (3)`... kể cả khi trùng giữa nhiều torrent tải song song, file đã có trên máy, hoặc file `.part` đang tải dở.
- Bỏ chọn **Lưu phẳng**: file nằm trong `<nơi lưu>\<tên torrent>_<mã hàng đợi>\...`, giữ cây thư mục khi API cung cấp ánh xạ file rõ ràng. Tải theo lựa chọn cần link riêng khớp từng file; nếu RD trả link gộp, thiếu file hoặc tên/kích thước không khớp, app báo lỗi trước khi tải file đó để tránh lưu nhầm. Queue từ phiên bản cũ giữ hành vi tải toàn bộ, kể cả archive/split.
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

Các kiểm tra tự động dùng HTTP giả lập, không cần token và không gửi torrent thật lên tài khoản. Bao gồm đọc metadata, tìm thư mục con, lọc video, gửi đúng ID file, chỉ tải lựa chọn từ torrent đã cache, khôi phục/Resume và từ chối link gộp. Kiểm tra WPF thao tác bộ duyệt file và render hai ngôn ngữ, hai theme, cửa sổ thường/nhỏ.

## CI/CD và phát hành

- **Push lên `main` / pull request:** chạy kiểm tra core và WPF trên Windows, build bản portable, lưu `.exe`, ZIP, SHA-256 và ảnh giao diện trong mục **Artifacts** của [GitHub Actions](https://github.com/coolbirdzik/realdebrid/actions). Artifact được giữ 14 ngày và tải xuống khi đăng nhập GitHub.
- **Chạy thủ công:** vào **Actions → Build and release → Run workflow** để kiểm tra và lấy artifact; thao tác này không tạo Release.
- **Đẩy tag `vMAJOR.MINOR.PATCH`:** sau khi test và build thành công, workflow tự tạo GitHub Release với ghi chú thay đổi, file `.exe`, ZIP và `SHA256SUMS.txt`. Tag có hậu tố như `v0.2.0-beta.1` tạo prerelease.
- Workflow dùng `GITHUB_TOKEN` sẵn có; chỉ job phát hành có quyền `contents: write`. Không cần thêm API token Real-Debrid, GitHub PAT hay SSH key vào secrets.

Ví dụ phát hành phiên bản tiếp theo:

```powershell
git push origin main
git tag -a v0.2.1 -m 'Release v0.2.1'
git push origin v0.2.1
```

Build bộ file phát hành ngay trên máy:

```powershell
.\tools\Build-Release.ps1 -Version 0.2.1
```

Kết quả nằm trong `artifacts/release/`. Kiểm tra SHA-256 của file tải về bằng `Get-FileHash .\RealDebridBatch-win-x64.exe -Algorithm SHA256` và đối chiếu với `SHA256SUMS.txt` trong cùng Release. Release chưa ký số Authenticode.

Kiểm tra WPF dùng thư mục cài đặt thử riêng, xác minh đổi ngôn ngữ và tạo ảnh giao diện ở `artifacts/ui-checks/`. Icon mới có nguồn vector tại `src/RdBatch.App/Assets/AppIcon.svg`; chạy `tools/Generate-Icon.ps1` để tạo lại PNG và ICO đa kích thước (16–256 px), dùng cho cửa sổ, taskbar và file exe.

Để cập nhật ảnh trong README sau khi đổi UI, chạy kiểm tra WPF ở thư mục gốc repo rồi chép `artifacts/ui-checks/browser-grid-vietnamese.png` sang `docs/images/app-vi.png` và `browser-grid-english.png` sang `docs/images/app-en.png`. Ảnh tối là `browser-vietnamese-dark.png` và `browser-english-dark.png`.
