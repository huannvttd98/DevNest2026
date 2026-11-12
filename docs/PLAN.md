# DevNest – Plan triển khai

> Nguồn yêu cầu: [devnest-requirements.md](../devnest-requirements.md)
> Ước lượng thời gian tính cho 1 developer full-time.
> Cập nhật lần cuối: 2026-11-09 (rev 3: hoàn thành Phase 0–2)
> Ký hiệu: `[x]` xong · `[~]` xong một phần · `[ ]` chưa làm

---

## 0. Thay đổi so với rev 1

| # | Thay đổi | Lý do |
|---|---|---|
| 1 | **Toàn bộ data lưu local** trong app data dir. Không có cloud, không có server | Yêu cầu mới |
| 2 | Thêm tính năng **Export / Import** (file `.devnest`) để chia sẻ cho user khác | Yêu cầu mới |
| 3 | ID dùng **UUID v7** thay cho integer autoincrement | Import từ máy khác phải nhận diện được bản ghi trùng hay mới |
| 4 | Secret lưu theo mô hình **vault**: 1 master key trong OS keyring, secret mã hoá trong SQLite | Windows Credential Manager giới hạn mỗi entry 2560 byte, không chứa nổi private key RSA 4096. Ngoài ra macOS không phải hỏi quyền keychain cho từng secret, và export cũng dễ hơn |
| 5 | Private key có 2 chế độ: **tham chiếu path** hoặc **lưu trong vault** | Path ở máy khác không tồn tại. Phải lưu key trong vault thì mới export được |
| 6 | Tự backup DB trước khi chạy migration | Bảo vệ dữ liệu local khi nâng version |
| 7 | Bỏ "Đồng bộ giữa thiết bị" khỏi backlog, Export/Import thay thế cho nhu cầu này | Đúng định hướng chỉ lưu local |

---

## 1. Quyết định kỹ thuật

| Vấn đề | Quyết định | Lý do |
|---|---|---|
| Truy cập SQLite | `rusqlite` (bundled) + `rusqlite_migration`, chạy qua `spawn_blocking`. **Không dùng** `tauri-plugin-sql` | Frontend không được chạy SQL trực tiếp (mục 5 của requirements) |
| Khoá chính | UUID v7 (TEXT) | Merge và import giữa các máy |
| Lưu secret | Vault: master key 256-bit trong `keyring`, secret mã hoá **XChaCha20-Poly1305** trong bảng `secrets` | Đáp ứng yêu cầu dùng OS secure storage, không bị giới hạn kích thước |
| Bảo vệ secret trong RAM | Crate `zeroize` / `secrecy` | Xoá secret khỏi bộ nhớ sau khi dùng |
| Mã hoá file export | Crate **`age`**, chế độ passphrase (scrypt) | Định dạng chuẩn, đã được audit, không tự chế crypto |
| Đóng gói file export | ZIP (`zip` crate) chứa `manifest.json`, `data.json`, `docs/` | Mở rộng được cho workspace có tài liệu |
| Đẩy output terminal | `tauri::ipc::Channel` | Throughput cao, có thứ tự |
| Editor nội dung file | CodeMirror 6 | Nhẹ hơn Monaco |
| Markdown | markdown-it (`html: false`) + DOMPurify | File `.md` từ server có thể chứa script |
| Highlight trong preview | Shiki dạng fine-grained (`shiki/core` + JS regex engine), lazy-load grammar | Bản Shiki đầy đủ rất nặng |
| Xác minh host key | TOFU, cảnh báo khi fingerprint thay đổi | Chống MITM |
| Layout | `splitpanes` hoặc tự viết resizable panel | Sidebar / Main / Info kéo giãn được |
| Giữ trạng thái tab | Unified tab store + `<KeepAlive>` | Không mất nội dung khi chuyển tab |
| Log | `tracing` + lớp redact, ghi vào `logs/` có rotate | Không ghi secret vào log |

---

## 2. Lưu trữ dữ liệu local

### 2.1. Vị trí

Thư mục do Tauri `app_data_dir()` quản lý:

- Windows: `%APPDATA%\<identifier>\`
- macOS: `~/Library/Application Support/<identifier>/`

```text
<app_data_dir>/
├── devnest.db          # SQLite (WAL mode)
├── backups/            # devnest-<timestamp>.db, tạo trước migration, giữ 5 bản gần nhất
└── logs/               # log đã redact, rotate theo ngày
```

> ⚠️ Phải chốt `identifier` trong `tauri.conf.json` ngay từ Phase 0, ví dụ `com.devnest.app`. Đổi identifier về sau thì app sẽ trỏ sang thư mục data mới và user mất data.

### 2.2. Vault (lưu secret)

```text
OS Keyring (Keychain / Credential Manager)
└── service "devnest", account "master-key"  ->  32 byte random (tạo ở lần chạy đầu)

SQLite: secrets(owner_id, kind, nonce, ciphertext)
        kind ∈ {password, passphrase, private_key}
        ciphertext = XChaCha20-Poly1305(master_key, nonce, secret, aad = owner_id||kind)
```

- Chỉ backend Rust giải mã secret, và chỉ khi cần kết nối. Secret **không bao giờ** được gửi lên frontend.
- Frontend chỉ biết các cờ `has_password`, `has_passphrase`, `has_stored_key`.
- Nếu đọc keyring thất bại (bị xoá, hoặc đổi máy mà copy nguyên DB sang): báo "Vault không mở được", cho phép giữ data không chứa secret và nhập lại secret.
- (Sau MVP) Tuỳ chọn master password thay cho keyring, dành cho máy bị khoá keyring.

### 2.3. Schema SQLite

```sql
connection_groups(id TEXT PK, name, sort_order, created_at, updated_at)

connections(id TEXT PK, group_id, name, host, port, username,
            auth_type   TEXT CHECK (auth_type IN ('password','key')),
            key_source  TEXT CHECK (key_source IN ('path','vault')),  -- khi auth_type = key
            key_path,                                                 -- khi key_source = path
            created_at, updated_at, last_connected_at)

secrets(owner_id TEXT, kind TEXT, nonce BLOB, ciphertext BLOB, created_at,
        PRIMARY KEY (owner_id, kind))

known_hosts(host, port, key_type, fingerprint, added_at,
            PRIMARY KEY (host, port, key_type))

settings(key TEXT PK, value)

-- Phase 5
workspaces(id TEXT PK, name, sort_order, created_at, updated_at)
workspace_connections(workspace_id, connection_id)
workspace_items(id TEXT PK, workspace_id, type, path, source, connection_id)
notes(id TEXT PK, workspace_id, title, content, created_at, updated_at)
```

Migration: `rusqlite_migration` + `PRAGMA user_version`. Trước mỗi lần migrate, copy DB vào `backups/` bằng SQLite Online Backup API.

---

## 3. Export / Import

### 3.1. Phạm vi export

| Dữ liệu | Export | Ghi chú |
|---|---|---|
| Connection groups, connections (host, port, username, auth type) | ✅ | |
| Known hosts (fingerprint) | ✅ tuỳ chọn | Người nhận tin luôn host key của người gửi |
| Password, passphrase | ⚠️ tuỳ chọn, **mặc định tắt** | Bắt buộc đặt mật khẩu cho file |
| Private key lưu trong vault | ⚠️ tuỳ chọn, **mặc định tắt** | Bắt buộc mật khẩu, có cảnh báo mạnh |
| Private key dạng path | ❌ chỉ xuất tên file | Người nhận map lại key của họ khi import |
| Workspaces, notes, tài liệu Markdown (Phase 5) | ✅ | Tài liệu local được nhúng vào `docs/`, tài liệu remote chỉ lưu tham chiếu |
| `last_connected_at`, settings của máy (kích thước cửa sổ...) | ❌ | Chỉ có ý nghĩa trên máy này |

Có thể chọn export **tất cả**, **theo group**, **theo connection** hoặc **theo workspace**.

### 3.2. Định dạng file `.devnest`

```text
export.devnest
├─ [nếu có mật khẩu] age-encrypted(passphrase) của ZIP bên dưới
└─ ZIP
   ├── manifest.json
   ├── data.json
   └── docs/…            (Phase 5)
```

```json
// manifest.json
{
  "format": "devnest-export",
  "format_version": 1,
  "app_version": "0.1.0",
  "created_at": "2026-11-09T10:00:00Z",
  "exported_by": "tên hiển thị (tuỳ chọn)",
  "contains_secrets": false,
  "scope": { "type": "groups", "ids": ["..."] },
  "counts": { "groups": 2, "connections": 8, "known_hosts": 8 }
}
```

- Khi đọc file, kiểm tra header `age-encryption.org/v1` để biết file có mã hoá hay không.
- `format_version` cho phép app mới import file cũ (có chuyển đổi), và từ chối rõ ràng khi gặp file mới hơn app.
- **Quy tắc:** file có `contains_secrets = true` thì bắt buộc phải mã hoá. App không bao giờ tạo file chứa secret ở dạng plaintext.

### 3.3. Luồng Export

1. Chọn phạm vi (all / group / connection / workspace).
2. Chọn tuỳ chọn: kèm known hosts, kèm secret.
3. Nếu kèm secret: nhập mật khẩu 2 lần (tối thiểu 12 ký tự, có thanh đo độ mạnh) và xác nhận cảnh báo.
4. Rust build ZIP **trong bộ nhớ**, mã hoá nếu cần, ghi file qua save dialog. Không tạo file tạm plaintext.
5. Hiện kết quả: số bản ghi đã export, có chứa secret hay không.

### 3.4. Luồng Import

1. Chọn file `.devnest`. Nếu file mã hoá thì nhập mật khẩu.
2. Validate manifest và schema của `data.json` (dùng `serde`, kiểm tra `deny_unknown_fields` theo từng version).
3. **Màn preview**, phân loại từng bản ghi:
   - **Mới**: chưa có trong DB.
   - **Trùng ID**: cùng UUID. Hiện diff các trường khác nhau.
   - **Nghi trùng**: khác UUID nhưng cùng `host + port + username`.
4. Chọn cách xử lý (cho tất cả hoặc từng bản ghi): **Bỏ qua** / **Ghi đè** / **Tạo bản sao**.
5. Cho phép sửa `username` trước khi import, vì người nhận thường dùng user riêng.
6. Connection dùng key dạng path: yêu cầu chọn key trên máy người nhận, hoặc để trống rồi cấu hình sau.
7. Ghi toàn bộ trong **1 transaction**. Có lỗi thì rollback hết. Secret được mã hoá lại bằng master key của máy người nhận.
8. Báo cáo kết quả: số bản ghi thêm / ghi đè / bỏ qua, và danh sách connection còn thiếu credential.

### 3.5. Bảo mật Export/Import

- Mặc định **không** export secret. Khuyến nghị mỗi người dùng private key riêng.
- Secret chỉ tồn tại trong RAM phía Rust và được `zeroize` sau khi dùng.
- Không log nội dung file export/import, không log mật khẩu file.
- Giới hạn kích thước file import (ví dụ 50MB) và kích thước khi giải nén, để chống zip bomb.
- Kiểm tra path trong ZIP (chặn `../` và path tuyệt đối) để chống zip slip.
- Kiểm tra lại dữ liệu import như dữ liệu nhập tay (port hợp lệ, host không rỗng...).

### 3.6. Backup / Restore cho chính mình

Dùng cùng cơ chế: **Export all + kèm secret + mật khẩu** để backup, và **Import** để restore sang máy mới. Không cần làm thêm tính năng riêng.

---

## 4. Kiến trúc source

### 4.1. Rust backend (`src-tauri/src/`)

```text
src-tauri/src/
├── main.rs / lib.rs        # đăng ký commands, state, plugins
├── error.rs                # AppError thống nhất -> serialize {code, message}
├── database/               # kết nối, migrations, backup, repositories
├── vault/                  # master key (keyring), encrypt/decrypt secret
├── ssh/
│   ├── session_manager.rs  # HashMap<ConnectionId, Arc<SshConnection>>
│   ├── connection.rs       # connect, auth, keep-alive, host key check
│   └── terminal.rs         # PTY, input, window_change, output qua Channel
├── sftp/                   # list, stat, read, write, mkdir, rename, remove, transfer
├── filesystem/             # thao tác file local, giới hạn kích thước file
└── transfer/               # export / import (.devnest): zip, age, manifest, merge
```

Nguyên tắc:
- Một SSH connection mở được nhiều channel (nhiều shell tab dùng chung một SFTP session).
- Mọi thao tác hệ thống đều đi qua Tauri command. Frontend chỉ gọi command.
- Chặn mở file lớn bằng editor (mặc định 5MB, cấu hình được).

### 4.2. Frontend (`src/`)

- Pinia stores: `connections`, `sessions`, `tabs`, `fileExplorer`, `settings`.
- `modules/*/api.ts` gói toàn bộ lệnh `invoke`.
- Interface `FileProvider` có hai bản `LocalFileProvider` và `SftpFileProvider`.
- Tab type: `terminal`, `markdown`, `file`.
- `modules/transfer/`: wizard Export và Import (bước chọn phạm vi → tuỳ chọn → preview → kết quả).

---

## 5. Lộ trình chi tiết

### Phase 0 – Setup (2–3 ngày)

- [x] Cài toolchain: Rust stable, Node LTS (dùng npm thay pnpm)
- [x] Scaffold Tauri 2 + Vue 3 + TypeScript, **chốt `identifier`** = `com.devnest.app`
- [x] Tailwind CSS, Pinia, Lucide
- [x] ESLint, Prettier, `cargo clippy`, `rustfmt`, Vitest
- [x] Tauri capabilities ở mức tối thiểu, CSP chặt
- [~] CI GitHub Actions build song song Windows + macOS (đã viết `.github/workflows/ci.yml`, chưa chạy vì chưa có repo)

**Kết quả:** app trống chạy được, CI xanh.

### Phase 1 – Nền tảng data & SSH Connection Manager (~2.5 tuần)

- [~] **Spike `russh`**: đã test ed25519 có passphrase, password, key dạng file và dạng nội dung. Còn RSA cũ / ECDSA / server thuật toán cũ
- [x] Database: UUID v7, migrations, auto backup trước migrate, WAL
- [x] Vault: tạo/đọc master key, encrypt/decrypt, xử lý lỗi keyring, zeroize
- [x] Layout 3 vùng + dark/light mode
- [x] CRUD connection group và connection
- [x] Form connection: auth password hoặc key (chọn **path** hoặc **lưu vào vault**), passphrase
- [x] Test connection: timeout, báo lỗi cụ thể
- [x] Xác minh host key (TOFU) + hộp thoại cảnh báo
- [x] Logging có redact
- [x] Unit test cho vault và repository

**Kết quả:** CRUD và test được connection. Secret không xuất hiện ở dạng plaintext trong DB, log hay frontend.

### Phase 2 – SSH Terminal (~1.5–2 tuần)

- [x] xterm.js với các addon fit, web-links, search
- [x] Shell qua session manager, stream output qua Channel (gom output thành batch)
- [x] Nhiều tab: mở, đóng, đổi tên, chuyển tab
- [x] Resize bằng ResizeObserver, gọi fit rồi gửi `window_change` (debounce)
- [x] Keep-alive, phát hiện mất kết nối, overlay Reconnect, giữ scrollback
- [x] Dọn channel và connection khi đóng tab hoặc tắt app
- [ ] Kiểm tra IME (Việt/Nhật) và copy/paste trên Windows (chưa chạy được trên máy Windows: Smart App Control)

**Kết quả:** chạy được lệnh tương tác (`top`, `vim`, `htop`) ổn định với nhiều tab.

### Phase 3 – File Management (~2.5–3 tuần)

> Đã có cửa sổ SFTP riêng để duyệt file server và kéo file về thư mục local; upload và các thao tác quản lý vẫn nằm trong phase này.

**Local File Explorer**
- [ ] Cây lazy-load, mở rộng/thu gọn
- [ ] Tạo, đổi tên, xoá file/folder (có xác nhận)
- [ ] Mở và lưu file bằng CodeMirror 6

**Remote File Explorer (SFTP)**
- [ ] Dùng chung component cây qua `SftpFileProvider`
- [ ] Tạo, đổi tên, xoá file/folder
- [ ] Upload/download có progress, huỷ được, có hàng đợi
- [ ] Xác nhận khi ghi đè; so `mtime`/`size` trước khi lưu file remote
- [ ] Tuỳ chọn tạo `.bak`; ghi atomic (file tạm rồi rename) khi server hỗ trợ
- [ ] Hiển thị lỗi và trạng thái thao tác rõ ràng

### Phase 3.5 – Export / Import connections (~1–1.5 tuần) 🆕

- [ ] Module `transfer`: build/parse ZIP, manifest, `format_version`
- [ ] Mã hoá/giải mã bằng `age` passphrase
- [ ] Wizard Export: chọn phạm vi, tuỳ chọn secret và known hosts, đặt mật khẩu
- [ ] Wizard Import: mật khẩu → validate → preview (mới / trùng / nghi trùng) → chọn cách xử lý → map key path → transaction
- [ ] Chống zip bomb, zip slip; giới hạn kích thước file
- [ ] Test round-trip: export rồi import trên DB sạch, so sánh dữ liệu
- [ ] Test import file cũ, file hỏng, sai mật khẩu, file version mới hơn app

### Phase 4a – Markdown Viewer, thuộc MVP (~1 tuần)

- [ ] markdown-it: heading, list, table, link, image, blockquote, code block
- [ ] Shiki fine-grained, lazy-load ngôn ngữ
- [ ] Sanitize bằng DOMPurify
- [ ] Ảnh tương đối: local dùng asset protocol; remote chỉ hiện placeholder
- [ ] Nhiều tài liệu bằng tab

> ### 🎯 Mốc MVP: khoảng 9–11 tuần
> Đối chiếu checklist nghiệm thu (mục 6), build bản cài đặt Windows + macOS.

### Phase 4b – Markdown Editor (~1 tuần)

- [ ] Ba chế độ editor / preview / split, cuộn đồng bộ
- [ ] Preview realtime (debounce)
- [ ] Lưu file local và remote
- [ ] Đánh dấu dirty, cảnh báo khi đóng tab hoặc tắt app mà chưa lưu
- [ ] Ảnh remote trong preview (đọc qua SFTP, chuyển thành blob URL)

### Phase 5 – Workspace & hoàn thiện (~2.5–3 tuần)

- [ ] Quản lý workspace/project, liên kết connection
- [ ] Notes lưu trong DB, tài liệu và thư mục theo project
- [ ] **Export/Import workspace**: nhúng tài liệu local vào `docs/`, kèm notes
- [ ] Rà soát bảo mật: log, vault, CSP, capabilities
- [ ] Hoàn thiện loading / empty / error state
- [ ] Ký code Windows; ký + notarize macOS
- [ ] (Tuỳ chọn) Auto-update qua `tauri-plugin-updater`
- [ ] Kiểm thử trên macOS và Windows

---

## 6. Checklist nghiệm thu MVP

| # | Tiêu chí | Phase |
|---|---|---|
| 1 | Tạo, sửa, xoá, lưu SSH connection | 1 |
| 2 | Kết nối SSH bằng password và private key | 1 |
| 3 | Mở terminal, thực thi lệnh tương tác | 2 |
| 4 | Duyệt cây thư mục local và remote | 3 |
| 5 | Upload/download file qua SFTP | 3 |
| 6 | Mở và xem nội dung file Markdown | 4a |
| 7 | Có thông báo phù hợp khi kết nối/thao tác thất bại | 1–4a |
| 8 | Credential không lưu và không ghi log dưới dạng plaintext | 1 |
| 9 | Build và chạy được trên Windows và macOS | 0, MVP |
| 10 🆕 | Toàn bộ data lưu local, app hoạt động không cần mạng (trừ khi kết nối SSH) | 1 |
| 11 🆕 | Export connection ra file `.devnest`, import được trên máy khác | 3.5 |
| 12 🆕 | File export chứa secret luôn được mã hoá; sai mật khẩu thì không import được | 3.5 |
| 13 🆕 | Import có preview và xử lý trùng lặp, không làm hỏng data hiện có | 3.5 |

---

## 7. Rủi ro

| Rủi ro | Ảnh hưởng | Giảm thiểu |
|---|---|---|
| `russh` không hỗ trợ đủ định dạng key / thuật toán cũ | Không kết nối được một số server | Spike đầu Phase 1, dự phòng `ssh2` (libssh2) |
| Mất master key trong keyring (reset OS, copy DB sang máy khác) | Không giải mã được secret | Báo lỗi rõ ràng, data không chứa secret vẫn dùng được, khuyến nghị backup bằng Export |
| macOS hỏi quyền keychain liên tục với bản build chưa ký | Khó chịu khi dev/test | Chỉ có 1 entry keychain; ký code sớm |
| Người dùng chia sẻ file chứa secret với mật khẩu yếu | Lộ credential | Mặc định tắt secret, bắt buộc mật khẩu đủ mạnh, cảnh báo rõ |
| Định dạng export thay đổi giữa các version | Import thất bại | `format_version` + bộ chuyển đổi, test với file cũ |
| Terminal chậm khi output lớn | Treo UI | Gom output phía Rust, giới hạn scrollback |
| Tốc độ upload/download của `russh-sftp` với file lớn | Chuyển file chậm | Benchmark ở Phase 3, ghi pipeline nhiều request song song |
| Ký code macOS cần Apple Developer account | Trễ phát hành | Chuẩn bị từ Phase 0 |
| IME và phím tắt trong xterm trên Windows | Gõ tiếng Việt/Nhật lỗi | Test sớm ở Phase 2 |
| Ghi đè file remote khi file đã bị sửa từ nơi khác | Mất dữ liệu trên server | So `mtime`/`size`, xác nhận, tạo backup |

---

## 8. Câu hỏi cần chốt

1. Passphrase của private key: **lưu vào vault** hay **hỏi mỗi lần kết nối**?
2. Có hỗ trợ **ssh-agent** / **import `~/.ssh/config`** trong MVP không? (Đề xuất: làm sau MVP)
3. Có cần **jump host / bastion** không?
4. Ngôn ngữ giao diện: chỉ tiếng Anh hay cần i18n (Việt/Nhật)?
5. Có yêu cầu **ký code và auto-update** ngay ở MVP không?
6. 🆕 Có cho phép export **kèm secret** không, hay cấm hẳn và chỉ cho chia sẻ cấu hình?
7. 🆕 Có cần import từ nguồn khác (`~/.ssh/config`, PuTTY, Termius, MobaXterm) không?
8. 🆕 Giá trị `identifier` của app (quyết định thư mục lưu data) là gì?

---

## 9. Sau MVP (backlog)

- Git Manager
- Database Manager
- Docker Manager
- Quản lý log và tìm kiếm log
- Command snippets (có trong Export/Import)
- Master password thay cho keyring (tuỳ chọn)
- Import từ `~/.ssh/config` / PuTTY / các tool khác
