# DevNest – Yêu cầu dự án

## 1. Tổng quan

**Tên dự án:** DevNest  
**Tên ứng dụng:** DevNest  
**Tên repository đề xuất:** `devnest`  
**Package đề xuất:** `devnest-desktop`  
**Mô tả:** All-in-one Desktop Workspace for Developers

DevNest là ứng dụng desktop tập trung các công cụ phục vụ công việc phát triển phần mềm và quản trị server, bao gồm quản lý kết nối SSH, terminal, duyệt và quản lý file, đọc/chỉnh sửa tài liệu Markdown.

### Mục tiêu

- Chỉ phát triển ứng dụng desktop, không xây dựng Web App.
- Không yêu cầu backend server riêng.
- Ưu tiên ứng dụng nhẹ, ổn định, giao diện hiện đại.
- Định hướng hỗ trợ macOS và Windows.
- Có kiến trúc dễ mở rộng thêm các module trong tương lai.

## 2. Công nghệ đề xuất

| Thành phần | Công nghệ |
|---|---|
| Desktop framework | Tauri 2 |
| Frontend | Vue 3 |
| Ngôn ngữ frontend | TypeScript |
| Backend desktop | Rust |
| UI | Tailwind CSS |
| State management | Pinia |
| Terminal | xterm.js |
| SSH | russh |
| SFTP | russh-sftp |
| Markdown | markdown-it |
| Code highlighting | Shiki |
| Local database | SQLite |
| Icons | Lucide |

Các thư viện cần được kiểm tra khả năng tương thích và mức độ bảo trì tại thời điểm triển khai. Có thể thay thế nếu phát hiện lựa chọn phù hợp hơn.

## 3. Yêu cầu chức năng

### 3.1. SSH Connection Manager

Quản lý nhiều kết nối SSH đến các server.

- Thêm, sửa và xóa kết nối.
- Phân nhóm kết nối theo dự án hoặc mục đích.
- Lưu các thông tin: tên kết nối, hostname/IP, port, username.
- Hỗ trợ xác thực bằng SSH private key và password.
- Cho phép cấu hình đường dẫn private key và passphrase nếu cần.
- Kiểm tra kết nối và hiển thị trạng thái.
- Cho phép mở nhiều kết nối/phiên làm việc.
- Có khả năng ngắt kết nối và kết nối lại.

### 3.2. SSH Terminal

Terminal được tích hợp trực tiếp trong ứng dụng.

- Mở terminal tương tác sau khi kết nối SSH.
- Nhập và thực thi lệnh trên server.
- Hiển thị output, lỗi và trạng thái tiến trình.
- Hỗ trợ nhiều terminal tab.
- Đóng, đổi tên và chuyển đổi giữa các tab.
- Hỗ trợ resize terminal.
- Xử lý mất kết nối và cung cấp thao tác reconnect.
- Đồng bộ kích thước terminal với cửa sổ hiển thị.

### 3.3. File Explorer

Cung cấp giao diện cây thư mục để quản lý file và folder.

**Local File Explorer**
- Duyệt thư mục và file trên máy tính.
- Mở rộng/thu gọn folder.
- Tạo, đổi tên và xóa file/folder.
- Mở file bằng editor phù hợp trong ứng dụng.

**Remote File Explorer (SFTP)**
- Duyệt thư mục và file trên server thông qua SFTP.
- Mở rộng/thu gọn cây thư mục.
- Tạo, đổi tên và xóa file/folder.
- Upload và download file.
- Mở và chỉnh sửa file remote.
- Hiển thị lỗi và trạng thái thao tác file rõ ràng.

### 3.4. Markdown Viewer / Editor

Hỗ trợ tài liệu Markdown (`.md`).

- Mở và đọc file Markdown.
- Render heading, list, table, link, image, blockquote và code block.
- Chỉnh sửa nội dung Markdown.
- Preview nội dung sau khi render.
- Hỗ trợ chế độ editor, preview và chia đôi màn hình.
- Syntax highlighting cho code block.
- Hỗ trợ mở nhiều tài liệu bằng tab.
- Lưu file local và hỗ trợ lưu file remote khi được mở qua SFTP.
- Cảnh báo khi đóng tab có thay đổi chưa lưu.

### 3.5. Workspace / Project Manager

Quản lý tài nguyên theo từng dự án.

Mỗi workspace/project có thể chứa:
- Danh sách SSH connections liên quan.
- Tài liệu Markdown.
- Ghi chú.
- Thư mục/file liên quan.

Ví dụ cấu trúc logic:

```text
My Workspace
├── AI Mama
│   ├── SSH Connections
│   ├── Documentation
│   │   ├── deployment.md
│   │   └── database.md
│   └── Notes
├── eBay Solution
│   ├── SSH Connections
│   ├── Documentation
│   └── Notes
└── Yahoo Auction
    ├── SSH Connections
    ├── Documentation
    └── Notes
```

Workspace Manager là chức năng có thể triển khai sau khi các chức năng cốt lõi đã ổn định.

## 4. Yêu cầu giao diện

Định hướng giao diện desktop chia thành các khu vực:

### 4.1. Sidebar bên trái

- Danh sách workspace/project.
- Danh sách SSH connections.
- File Explorer dạng cây.
- Nhóm và thu gọn/mở rộng các mục.

### 4.2. Main Workspace

- Hiển thị SSH Terminal.
- Hiển thị Markdown Editor/Preview.
- Hiển thị nội dung file.
- Hỗ trợ nhiều tab và chuyển đổi giữa các tab.

### 4.3. Information Panel

Có thể hiển thị:
- Thông tin server đang kết nối.
- Host/IP, port, username.
- Trạng thái kết nối.
- Thông tin file đang mở.

Panel này có thể bật/tắt để tối ưu diện tích làm việc.

### 4.4. Nguyên tắc UX

- Giao diện hiện đại, rõ ràng, ưu tiên hiệu quả làm việc.
- Có thể hỗ trợ dark mode và light mode.
- Có trạng thái loading, empty state và error state.
- Các thao tác xóa hoặc ghi đè cần có xác nhận phù hợp.
- Không làm mất nội dung đang chỉnh sửa khi chuyển tab hoặc mất kết nối.

## 5. Bảo mật và độ ổn định

- Không lưu SSH password dưới dạng plaintext.
- Sử dụng cơ chế lưu trữ thông tin xác thực an toàn của hệ điều hành khi có thể (macOS Keychain, Windows Credential Manager).
- Bảo vệ private key và passphrase.
- Không ghi password, passphrase hoặc nội dung private key vào log.
- Quản lý vòng đời SSH connection và SFTP session rõ ràng.
- Xử lý timeout, mất kết nối và reconnect.
- Cảnh báo trước khi ghi đè file remote.
- Khuyến nghị tạo bản backup trước khi sửa file cấu hình quan trọng trên server.
- Phân tách frontend và backend; các thao tác hệ thống nhạy cảm phải được kiểm soát ở backend Rust.

## 6. Kiến trúc source code tham khảo

```text
devnest/
├── src/
│   ├── components/
│   │   ├── sidebar/
│   │   ├── terminal/
│   │   ├── file-explorer/
│   │   ├── markdown/
│   │   └── tabs/
│   ├── modules/
│   │   ├── ssh/
│   │   ├── sftp/
│   │   ├── file-manager/
│   │   ├── markdown/
│   │   └── workspace/
│   ├── stores/
│   ├── layouts/
│   └── App.vue
├── src-tauri/
│   ├── src/
│   │   ├── ssh/
│   │   ├── sftp/
│   │   ├── filesystem/
│   │   ├── database/
│   │   └── main.rs
│   └── tauri.conf.json
├── package.json
└── README.md
```

Đây là cấu trúc tham khảo ban đầu; cần điều chỉnh theo conventions của Tauri và quy mô thực tế khi triển khai.

## 7. Lộ trình phát triển

### Phase 1 – Core Desktop & SSH Manager

- Khởi tạo Tauri 2 + Vue 3 + TypeScript.
- Xây dựng layout và sidebar.
- Xây dựng SSH Connection Manager.
- Lưu và đọc cấu hình kết nối.
- Kiểm tra kết nối SSH.

### Phase 2 – SSH Terminal

- Tích hợp xterm.js.
- Kết nối terminal với SSH shell.
- Hỗ trợ nhiều terminal tab.
- Xử lý resize, disconnect và reconnect.

### Phase 3 – File Management

- Xây dựng Local File Explorer.
- Xây dựng Remote SFTP Explorer.
- Upload/download.
- Tạo, đổi tên, xóa file/folder.
- Mở và chỉnh sửa file.

### Phase 4 – Markdown

- Markdown Viewer.
- Markdown Editor.
- Preview realtime.
- Syntax highlighting.
- Lưu file và cảnh báo thay đổi chưa lưu.

### Phase 5 – Workspace & hoàn thiện

- Quản lý workspace/project.
- Liên kết SSH connections với project.
- Quản lý tài liệu và ghi chú.
- Hoàn thiện bảo mật, UX và xử lý lỗi.
- Build và kiểm thử trên macOS, Windows.

## 8. Phạm vi MVP

MVP nên tập trung vào các chức năng cốt lõi sau:

1. SSH Connection Manager.
2. SSH Terminal.
3. Local File Explorer.
4. Remote File Explorer qua SFTP.
5. Markdown Viewer cơ bản.

Các chức năng như Workspace Manager nâng cao, Markdown Editor đầy đủ và tiện ích bổ sung có thể triển khai ở các phiên bản tiếp theo.

## 9. Tiêu chí nghiệm thu MVP

- Người dùng có thể tạo, sửa, xóa và lưu SSH connection.
- Có thể kết nối SSH bằng phương thức xác thực được hỗ trợ.
- Có thể mở terminal và thực thi lệnh tương tác.
- Có thể duyệt cây thư mục local và remote.
- Có thể upload/download file qua SFTP.
- Có thể mở và xem nội dung file Markdown.
- Có thông báo phù hợp khi kết nối hoặc thao tác thất bại.
- Thông tin xác thực không bị lưu hoặc ghi log dưới dạng plaintext.
- Ứng dụng có thể build và chạy trên các hệ điều hành mục tiêu.

## 10. Định hướng mở rộng

Sau MVP, có thể cân nhắc:
- Git Manager.
- Database Manager.
- Docker Manager.
- Quản lý log và tìm kiếm log.
- Lưu command snippets thường dùng.
- Đồng bộ workspace và cấu hình giữa các thiết bị (nếu có nhu cầu).

---

**Định hướng sản phẩm:** DevNest là một desktop workspace dành cho developer, giúp tập trung quản lý SSH server, terminal, file và tài liệu kỹ thuật trong một ứng dụng duy nhất.
