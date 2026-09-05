# OmiAI — Lộ Trình Giai Đoạn Kế Tiếp
## Từ Tám Trụ Cột Suy Luận Đến Một File `.omiai` Biết Trò Chuyện Thật

> **Tài liệu dành cho:** AI coding agent (Claude Code, Cursor, hoặc tương đương) thực thi trực tiếp trên repo `github.com/doducdanhh/OmiAI`.
> **Cơ sở của tài liệu:** được viết sau khi đã `git clone` và đọc trực tiếp toàn bộ mã nguồn trong container — không suy đoán từ tên file. Đã đọc trọn vẹn: `huongdan.txt` (14.774 byte, bản đặc tả gốc), `README.md`, `docs/architecture/README.md`, `docs/format-spec/checkpoint-v1.md`, toàn bộ 6 file trong `crates/omiai-io/src/` (chat.rs, nlp_parser.rs, tokenizer.rs, conversation.rs, action.rs, perception.rs), toàn bộ `crates/omiai-neuro/src/reservoir.rs`, toàn bộ 4 file "scaffold" (`omiai-cli`, `omiai-runtime`, `omiai-serve`, `omiai-export`), cùng `lib.rs` (bản đồ module + API công khai) của mọi crate còn lại, mọi `Cargo.toml`, và khảo sát chữ ký hàm công khai (`pub fn`/`pub struct`) của các module lõi (`knowledge::graph`, `knowledge::reasoning`, `probabilistic::bayesian`, `causal::dag`, `causal::do_calculus`, `world::world_loop`, `core::prover`). Những phần không trích dẫn dòng cụ thể (ví dụ toàn bộ `core::inference` — DPLL/CDCL, hay từng ADR) đã được kiểm tra ở mức cấu trúc: kích thước file, số lượng test, việc có/không có `todo!()`/`unimplemented!()` (kết quả: **0** trên toàn repo), để xác nhận đây là code thật chứ không phải khung rỗng.
> **Một giới hạn cần nói thẳng:** môi trường thực thi của tôi không có sẵn Rust ≥ 1.85 (chỉ có `rustc` 1.75 qua `apt`, quá cũ cho `edition = "2024"` và cú pháp *let-chains* mà chính code đang dùng), và tôi không có quyền truy cập `static.rust-lang.org`/`sh.rustup.rs` để cài bản mới. Vì vậy **tôi chưa tự tay chạy được `cargo test --workspace`** để xác nhận 100% con số "358 tests" mà README tự báo cáo — tôi chỉ xác nhận được rằng code có thật, có logic thật, có khối `#[test]` thật ở khắp nơi, và tham chiếu đúng các thuật toán/tài liệu nó tự nhận (ví dụ `reservoir.rs` trích đúng Jaeger và Sussillo & Abbott). **Việc đầu tiên AI coding agent phải làm trước khi tin bất kỳ điều gì trong tài liệu này là tự chạy `cargo test --workspace` trong môi trường thật của bạn** — đúng tinh thần "không tuyên bố gì mà không có test thật" mà chính README của dự án đã đặt ra.

---

## Mục lục

1. Tóm tắt điều hành — kết luận trước, chi tiết sau
2. Hiện trạng thực tế của repo (kiểm chứng từng dòng, không suy đoán)
3. Sự thật kỹ thuật cần thống nhất trước khi đi tiếp (Unicode, "nơ-ron nhân tạo", và "trí tuệ" nằm ở đâu)
4. Tầm nhìn sản phẩm được tái định nghĩa — OmiAI sau nâng cấp là gì
5. Kiến trúc tổng thể mới
6. Đặc tả định dạng `.omiai` (bundle-v1)
7. Lộ trình theo giai đoạn (slice-by-slice, đúng văn hoá dự án)
8. Hướng dẫn cấp mã nguồn cho từng module (để AI coding agent bám sát)
9. Ngân sách phần cứng & hiệu năng (8 GB RAM / i7-7700K)
10. Rủi ro, giới hạn, và những gì KHÔNG nên tuyên bố
11. Checklist thực thi tuần tự
12. Tài liệu tham khảo

---

## PHẦN 1 — TÓM TẮT ĐIỀU HÀNH

**Kết luận quan trọng nhất, nói trước:** bạn không cần "tái tạo bộ não con người bằng Unicode". Yêu cầu đó, viết đúng theo nghĩa đen, không tương ứng với bất kỳ cơ chế kỹ thuật thật nào — Unicode là một bảng mã ký tự, không phải chất liệu của tư duy (Phần 3 sẽ giải thích kỹ vì sao, một cách tôn trọng và cụ thể). Nhưng tin tốt là: **bạn đã đi được hơn nửa quãng đường tới một thứ ấn tượng hơn thế nhiều**, chỉ là nó không nằm ở chỗ bạn nghĩ.

Sau khi đọc toàn bộ repo, đây là bức tranh thật:

- **Phần "suy luận" (reasoning) của OmiAI là có thật và sắc bén theo đúng nghĩa kỹ thuật** — không phải một lời quảng cáo. `omiai-core` có một bộ chứng minh định lý logic vị từ bậc một hoàn chỉnh (hợp nhất Robinson, chuẩn hoá CNF, DPLL/CDCL, resolution). `omiai-probabilistic` có suy luận Bayes bằng junction-tree khớp đúng xác suất hậu nghiệm tính tay. `omiai-causal` cài đặt đúng do-calculus và phản-sự-kiện (counterfactual) kiểu Judea Pearl. `omiai-knowledge` có đồ thị tri thức, suy diễn tiến/lùi, suy luận giải thích (abduction). Đây là suy luận **có thể kiểm chứng bằng chứng minh hình thức** — một thứ mà ngay cả các mô hình ngôn ngữ lớn thương mại cũng không làm được (chúng "có vẻ" suy luận đúng nhưng không đưa ra được một proof object có thể kiểm tra lại).
- **Cái đang thiếu không phải là "trí tuệ" — mà là một bộ phiên dịch đủ tốt giữa câu nói của con người và bộ máy suy luận đó.** `crates/omiai-io` (lớp hội thoại) hiện tại chỉ có 940 dòng, dùng đúng 4 từ vựng mỗi ngôn ngữ (`human`/`người`, `mortal`/`phàm`, `capital`/`thủ đô`, `hello`/`xin chào`) và khoảng 4 khuôn mẫu câu cứng ("mọi X là Y", "X là Y", câu hỏi bắt đầu bằng "what/ai"...). Đây là một **bản demo tam đoạn luận** ("Socrates là người, mọi người đều phải chết, vậy Socrates phải chết") — chính comment trong code cũng tự nhận: *"The parser does not try to imitate a neural chatbot"*.
- **Phát hiện quan trọng nhất mang tính kiến trúc:** `ChatEngine` hiện tại — bộ não hội thoại — trong code **thật** (không phải test) chỉ import và gọi đúng một trụ cột duy nhất: `omiai_core::prover::TheoremProver`. Sáu trụ cột còn lại (`omiai-knowledge`, `omiai-probabilistic`, `omiai-causal`, `omiai-neuro`, `omiai-evolution`, `omiai-world`) xuất hiện trong `Cargo.toml` của `omiai-io` **chỉ ở mục `[dev-dependencies]`** — nghĩa là chúng chỉ được đụng tới trong test, chưa từng được nối dây vào đường đi thực tế của một câu chat. Tám trụ cột là tám hòn đảo xuất sắc, được kiểm thử tốt, nhưng **chưa phải một bộ não liền mạch**.
- **`omiai-export`, `omiai-runtime`, `omiai-serve` — ba thứ bắt buộc phải có để tạo ra và chạy một file `.omiai` — hiện là scaffold rỗng** (`#![allow(dead_code)]` và một dòng doc-comment "Scaffold — implemented in a later slice"). Không có dependency nào cho `axum`, `wasm-bindgen`, `safetensors`, `hecs` trong bất kỳ `Cargo.toml` nào. Điều này khớp với chính build order mà README tự đề ra: bước 9–12 (export → runtime → serve → cli) được đánh dấu "NEXT" / "AFTER" / "LAST", chưa làm.

Nói cách khác: **yêu cầu của bạn hôm nay — "làm ra một file `.omiai` biết chat" — chính là bước tiếp theo mà dự án đã tự vạch ra từ trước, chỉ là bạn đang mô tả nó bằng một ngôn ngữ khác** (bộ não, dây thần kinh, Unicode) so với ngôn ngữ kỹ thuật chính xác mà chính `huongdan.txt` và README đã dùng (reservoir computing, zero-training, neuro-symbolic). Tài liệu này sẽ nối hai ngôn ngữ đó lại, và đưa ra lộ trình cụ thể — có thể thực thi từng dòng — để đi từ hiện trạng tới một file `.omiai` thật sự chạy được, thật sự trò chuyện được, và **trung thực về việc nó thông minh theo kiểu gì**.

Chiến lược đề xuất, tóm tắt trong một câu: **"nối dây" 8 trụ cột đã có vào một pipeline hội thoại duy nhất (Slice 7–9, không cần nghiên cứu gì mới, thuần kỹ thuật phần mềm), sau đó — và chỉ sau đó, một cách minh bạch, có ADR riêng, tắt/bật được — cân nhắc thêm một "cái miệng" là mô hình ngôn ngữ nhỏ mã nguồn mở chạy local để diễn đạt trôi chảy những gì lõi symbolic đã chứng minh là đúng (Slice 10, tuỳ chọn), rồi mới đóng gói thành `model.omiai` và dựng runtime/serve/cli (Slice 11–15).**

---

## PHẦN 2 — HIỆN TRẠNG THỰC TẾ CỦA REPO (kiểm chứng từng dòng, không suy đoán)

### 2.1. Quy mô

Workspace Cargo gồm **15 crate** dưới `crates/*`, tổng cộng khoảng **33.740 dòng** `.rs` + `.md` + `.toml` + `.txt` (đếm bằng `wc -l`, không tính `.git`). Phân bổ số dòng `.rs` mỗi crate (gồm `src/`, `tests/`, `benches/`, `examples/` nếu có):

| Crate | Dòng .rs | Vai trò |
|---|---:|---|
| `omiai-core` | 5.531 | Logic hình thức, hợp nhất, CNF, DPLL/CDCL, CSP, LTL, modal, prover |
| `omiai-world` | 3.866 | Đời sống nhân tạo: cellular automaton, atoms/agents, giao tiếp nổi sinh |
| `omiai-probabilistic` | 3.132 | Bayes, Gibbs, HMC, junction-tree, MCTS/PUCT, Solomonoff |
| `omiai-evolution` | 2.841 | Lập trình di truyền (CGP + Formula-GP trực tiếp trên AST logic) |
| `omiai-knowledge` | 1.960 | Đồ thị tri thức, ontology, DisCoCat, triple store, SPARQL-like |
| `omiai-checkpoint` | 1.942 | Checkpoint dạng thư mục, ghi nguyên tử, BLAKE3, RNG state |
| `omiai-causal` | 1.355 | DAG nhân quả, do-calculus, SCM, phản-sự-kiện, ICP |
| `omiai-io` | 1.422 | **Lớp hội thoại — trọng tâm nâng cấp của tài liệu này** |
| `omiai-meta` | 954 | Nội quan, Active Inference, autopoiesis, hệ mục tiêu |
| `omiai-neuro` | 614 | Echo State Network (reservoir computing) + RLS |
| `omiai-memory` | 459 | Bộ nhớ làm việc / episodic / procedural / semantic |
| `omiai-runtime` | 9 | **Scaffold rỗng** |
| `omiai-export` | 7 | **Scaffold rỗng** |
| `omiai-cli` | 7 | **Scaffold rỗng** (chỉ `println!` placeholder) |
| `omiai-serve` | 6 | **Scaffold rỗng** |

Không có một `todo!()` hay `unimplemented!()` nào trong toàn bộ codebase — các phần chưa xong được để ở dạng doc-comment "Scaffold" tường minh, không phải panic ẩn. Đây là một dấu hiệu kỷ luật kỹ thuật tốt, hiếm gặp ở các dự án cá nhân quy mô này.

### 2.2. `huongdan.txt` thực sự yêu cầu gì (bản gốc, đọc trực tiếp)

`huongdan.txt` là một văn bản đặc tả kỹ thuật dày đặc, viết liền một mạch không xuống dòng (14.774 byte). Nội dung cốt lõi, diễn giải lại chính xác:

1. Xác nhận `core::logic_engine`, `core::substitution`, `core::unification` đã hoàn thiện và có test; các module còn lại tại thời điểm viết chỉ là khung.
2. **Ràng buộc phần cứng là một input thiết kế, không phải lời xin lỗi**: máy chỉ có 8 GB RAM, CPU Intel i7-7700K, không GPU rời. Đây là lý do dự án chọn *reservoir computing* (chỉ lớp readout học qua hồi quy tuyến tính RLS, không lan truyền ngược toàn mạng) thay vì một mô hình transformer cỡ lớn.
3. Yêu cầu chính của bản `huongdan.txt` này là xây `omiai-world`: atoms có gen là **con trỏ tới một `Formula` cụ thể** (không phải một vector số vô nghĩa) để "mọi hành vi sinh ra đều in ra được, đọc được, suy luận lại được — không phải hộp đen thống kê"; agent có chính sách hành vi là Formula được tiến hoá qua `evolution::genetic_programming`; giao tiếp nổi sinh kiểu trò chơi tín hiệu Lewis, đo bằng thông tin tương hỗ (mutual information); bất kỳ quy ước giao tiếp nào chứng minh lợi ích ổn định qua đủ thế hệ thì được "đề bạt" thành node có tên trong `knowledge::graph`.
4. Đặc tả chi tiết **hai định dạng lưu trữ khác nhau, không được lẫn lộn**:
   - **Checkpoint** (lúc đang mô phỏng/tiến hoá): dạng **thư mục**, không phải một file khổng lồ, ghi nguyên tử (tmp → fsync → rename), có BLAKE3 cho từng file con, giữ toàn bộ RNG state để resume đúng một quỹ đạo xác định, chỉ giữ N checkpoint gần nhất + các mốc vĩnh viễn.
   - **Bundle xuất (`model.omiai`)**: **một file archive** — về bản chất chỉ là tar nén zstd, đúng như ONNX hay safetensors cũng chỉ là archive/schema được đặt tên riêng, "không có gì huyền bí". Có `manifest.json` khai báo phiên bản schema và **những trụ cột/khả năng thực sự có mặt**, để runtime từ chối rõ ràng một bundle không tương thích thay vì crash khó hiểu.
5. `omiai-runtime` phải hoàn toàn độc lập với code huấn luyện, chỉ có `load()` + `step()`, biên dịch được sang native / `cdylib` (FFI) / `wasm32-wasi` / `wasm32-unknown-unknown`. `omiai-serve` dùng `axum` mở endpoint `/infer`. Cả hai đặc tả phải được viết chính thức tại `docs/format-spec/`.
6. **Điều khoản cuối, quan trọng nhất**: tuyệt đối không làm mọi thứ trong một lượt; phải làm tăng dần theo đúng thứ tự phụ thuộc; **không được khẳng định bất kỳ con số hiệu năng hay khả năng nào mà không có test/benchmark thật đứng sau**.

→ **Nhận xét:** `huongdan.txt` KHÔNG hề nhắc đến việc dùng "Unicode" như một cơ chế hiểu ngôn ngữ, và KHÔNG coi `omiai-neuro` (reservoir) là nơi đặt "trí tuệ hội thoại". Nó rất rõ ràng: reasoning nằm ở `core`/`knowledge`/`probabilistic`/`causal`; reservoir chỉ là một công cụ học mẫu động lực học nhẹ, CPU-friendly. Yêu cầu hôm nay của bạn, nếu muốn "tuân thủ quy tắc dự án" như bạn nói, nên tiếp nối đúng logic này — Phần 3 sẽ làm rõ vì sao, Phần 7 sẽ nói làm thế nào.

### 2.3. README hiện tại tự báo cáo gì

README (157 dòng, đọc toàn văn) tự mô tả OmiAI là *"a zero-training, self-bootstrapping reasoning system... No deep learning, no GPU, no PyTorch/TensorFlow/JAX, no training datasets"*, dựng trên 8 trụ cột. Tự báo cáo **358 test trên 52 test target, tất cả pass** (con số này tôi chưa tự chạy lại để xác minh — xem cảnh báo ở đầu tài liệu), với bảng trạng thái minh bạch: `omiai-runtime` và `omiai-serve` ghi rõ **"SCAFFOLD ONLY... no implementation yet"**; mục "Not yet implemented" liệt kê đúng: runtime, serve, export đầy đủ, đặc tả bundle-v1, và toàn bộ `benches/`, `examples/`, `scripts/`, `.github/workflows/` **ở cấp root** (phân biệt với những gì đã có trong từng crate).

Thứ tự xây dựng đề xuất trong README đánh dấu ✓ cho các bước 1–8 (core, probabilistic/causal, knowledge, neuro, evolution, world, checkpoint, memory/meta/io) và để trống bước 9–12 (export → runtime → serve → cli) với ghi chú "NEXT" / "AFTER export" / "AFTER runtime" / "LAST".

### 2.4. Lớp hội thoại (`omiai-io`) — đọc toàn văn, đây là sự thật

Đây là phần quan trọng nhất để bạn hiểu đúng vấn đề, vì nó là phần bạn muốn nâng cấp:

- **`tokenizer.rs`** (114 dòng): một lexer dùng combinator `nom`, tách chuỗi thành `Ident`/`Number`/`StringLit`/dấu câu/`Arrow`/`Op`. Đây là nơi Unicode của Rust thực sự phát huy tác dụng — hàm `satisfy(|c: char| c.is_alphabetic())` dùng đúng phân loại Unicode nên nhận diện chữ cái tiếng Việt có dấu (`ư`, `ơ`, `ế`...) là chữ cái hợp lệ. Nhưng nó được thiết kế để tách một **ngôn ngữ hình thức gần giống Prolog** (ví dụ `Human(socrates) -> Mortal(socrates)`), không phải để hiểu câu tiếng Việt tự nhiên.
- **`nlp_parser.rs`** (366 dòng): từ điển (`lexicon_vi`/`lexicon_en`) có đúng **4 mục mỗi ngôn ngữ**. Phát hiện câu hỏi bằng cách tìm từ trong danh sách cố định (`"what" | "why" | "how" | "ai" | "gì" | "tại"`). Dựng công thức logic bằng cách đếm số từ và vị trí từ (`words.len() == 4 && words[0] ∈ {"every","mọi"} && words[2] ∈ {"is","là"}` → sinh ra `∀x (Human(x) → Mortal(x))`). Chính doc-comment đầu file viết: *"The parser does not try to imitate a neural chatbot. Instead it turns user text into a compact semantic form..."* — dự án đã tự nhận thức đúng bản chất của module này.
- **`chat.rs`** (229 dòng): `ChatEngine::handle()` chỉ phụ thuộc `NlpParser` và `TheoremProver`. Với câu khẳng định → lưu `Formula` vào bộ nhớ hội thoại; với câu hỏi → gọi `TheoremProver::prove(&premises, &query)`, nếu không chứng minh được thì thử chứng minh phủ định, nếu cả hai đều thất bại thì trả lời "chưa đủ dữ kiện". Đây thực sự là một pipeline suy luận **đúng**, chỉ là phạm vi đầu vào cực hẹp.
- **`conversation.rs`** (88 dòng): bộ nhớ hội thoại giữ danh sách lượt nói, các `Formula` đã biết (khử trùng lặp), và "thực thể đang được chú ý" (để giải quyết đại từ sau này — hiện chưa dùng).
- **`action.rs`** / **`perception.rs`** (61 + 50 dòng): tiện ích rời rạc cho một vòng lặp agent giả định (hành động, ngưỡng hoá vector thành atom biểu tượng) — chưa nối vào `chat.rs`.

**Cargo.toml của `omiai-io` là bằng chứng kiến trúc rõ nhất**: mục `[dependencies]` (code thật) chỉ có `omiai-core`, `serde`, `nom`. Mục `[dev-dependencies]` (chỉ dùng trong test) mới có `omiai-world`, `omiai-evolution`, `omiai-meta`, `omiai-neuro`, `omiai-probabilistic`, `omiai-causal`, `omiai-knowledge`. Nói thẳng: **khi bạn chat với OmiAI hôm nay, bạn đang nói chuyện với đúng một trong tám trụ cột (`core`), bảy trụ cột kia đang ngồi ở phòng bên cạnh, đã xây xong, có cửa, nhưng chưa ai mở cửa nối hành lang.**

### 2.5. "Nơ-ron nhân tạo" đã tồn tại thật — `omiai-neuro/reservoir.rs` (đọc toàn văn)

Đây là điều bạn cần biết trước khi nghĩ mình phải "xây dây thần kinh nhân tạo" từ đầu: **nó đã có rồi, và cài đặt đúng**. `Reservoir` là một Echo State Network (ESN) chuẩn: ma trận trọng số hồi quy ngẫu nhiên cố định (`sparse_random_matrix`, không bao giờ lan truyền ngược), chuẩn hoá bán kính phổ về gần "rìa hỗn loạn" (`normalize_spectral_radius`, thường 0.9–1.0), cập nhật trạng thái `x ← (1-α)x + α·tanh(Wx + W_in·u)`, và **chỉ lớp đọc ra (`w_out`) được huấn luyện** — bằng Recursive Least Squares (`rls_update`, đúng công thức RLS chuẩn với ma trận nghịch đảo tương quan `P`) hoặc hồi quy ridge dạng đóng (`train_readout_ridge`). Có cả ước lượng số mũ Lyapunov lớn nhất để đo độ hỗn loạn. Trích dẫn trong code đúng và có thật: Jaeger (echo state networks) và Sussillo & Abbott (thuật toán FORCE).

Đây LÀ mạng nơ-ron nhân tạo thật — theo đúng nghĩa kỹ thuật (đơn vị phi tuyến kết nối tái hồi, học từ dữ liệu). Nhưng vai trò của nó trong repo hiện tại là **dự đoán chuỗi thời gian số** (test hiện có: học một sóng sin, dự đoán giá trị tiếp theo) — không phải xử lý ngôn ngữ. Phần 3.2 sẽ giải thích chính xác vì sao reservoir computing mạnh ở việc này nhưng không phải công cụ đúng cho việc hiểu ngôn ngữ mở, và Phần 7 sẽ đề xuất một vai trò thật, trung thực, hữu ích cho nó trong pipeline hội thoại — thay vì bỏ nó đi hoặc gán cho nó việc nó không làm được.

---

## PHẦN 3 — SỰ THẬT KỸ THUẬT CẦN THỐNG NHẤT TRƯỚC KHI ĐI TIẾP

Phần này không phải để nói "không thể làm được". Nó để đảm bảo cái mà AI coding agent sắp xây **thực sự hoạt động như nó được mô tả** — vì tài liệu này sẽ được một agent thực thi gần như theo nghĩa đen, và nếu khung tham chiếu ban đầu sai, agent sẽ xây ra một thứ *trông* đúng mô tả nhưng *không thật sự làm* điều mô tả đó (một ELIZA phức tạp đội lốt "bộ não"). Đây chính xác là điều `huongdan.txt` và README đang cố tránh khi họ nhấn mạnh "không tuyên bố gì không có test thật đứng sau".

### 3.1. Unicode là gì, và vì sao nó không phải — và không thể là — "cái hiểu ngôn ngữ"

Unicode là một **bảng ánh xạ**: mỗi ký tự (chữ cái, dấu, biểu tượng) được gán một số nguyên (code point). Rust hỗ trợ Unicode rất tốt — `String`/`&str` luôn là UTF-8 hợp lệ, `char` là một scalar value Unicode, và các hàm như `char::is_alphabetic()` dùng đúng bảng phân loại Unicode nên nhận diện chữ cái có dấu của tiếng Việt chính xác. Đây là hạ tầng **cần thiết và có thật** — nó là lý do `tokenizer.rs` tách được từ tiếng Việt đúng ranh giới.

Nhưng hãy xét ví dụ cụ thể: hai từ **"chào"** (lời chào) và **"cháo"** (món ăn) khác nhau đúng một dấu thanh. Unicode cho máy biết chính xác đây là hai chuỗi code point khác nhau (`à` là U+00E0, `á` là U+00E1). Nhưng Unicode **không hề biết, và về nguyên lý không thể biết**, một chuỗi nghĩa là lời chào còn chuỗi kia là món ăn sáng. Khoảng cách từ "chuỗi ký tự khác nhau" đến "nghĩa khác nhau" — đó chính là toàn bộ bài toán "hiểu ngôn ngữ tự nhiên" (natural language understanding). Bài toán đó chỉ có hai lối giải được biết đến trong khoa học máy tính hiện nay, không có lối thứ ba:

1. **Lối biểu tượng (symbolic)** — đúng thứ `omiai-core` + `omiai-knowledge` đang làm: định nghĩa tường minh mỗi từ/khái niệm ánh xạ tới một hằng/vị từ trong một hệ hình thức (`Formula`/`Term`), rồi suy luận bằng các luật suy diễn đã được chứng minh đúng đắn (soundness) toán học. Đây là hướng OmiAI đã chọn, và nó thật.
2. **Lối thống kê (statistical/learned)** — cách các mô hình ngôn ngữ lớn hoạt động: học hàng tỷ trọng số từ hàng nghìn tỷ token văn bản, sao cho mạng tự "khám phá" ra cấu trúc nghĩa mà không ai lập trình tường minh. Đây là hướng đòi hỏi dữ liệu và compute ở quy mô mà một dự án cá nhân, CPU-only, 8 GB RAM không thể tự tái tạo từ số 0 (Mục 3.4 sẽ định lượng rõ khoảng cách này).

Unicode không phải là lối thứ ba. Nó là *lớp mã hoá đầu vào chung* mà cả hai lối trên đều cần dùng để đọc được văn bản — giống như việc một cuốn sách được in bằng phông chữ rõ ràng không khiến người mù chữ đọc hiểu được nó. **Bảng mã tốt là điều kiện cần cho cả hai lối, nhưng không phải là bất kỳ lối nào trong hai lối đó.** Vì vậy: giữ nguyên việc Rust xử lý Unicode đúng trong tokenizer (đó là việc đúng, đã làm đúng) — nhưng đặt "trí tuệ" vào đúng chỗ của nó: `core`, `knowledge`, `probabilistic`, `causal`. Đó là khuyến nghị cốt lõi của Phần 7.

### 3.2. Vai trò thật của "nơ-ron nhân tạo" (reservoir computing) trong một hệ thống ngôn ngữ

Reservoir computing / Echo State Network, đúng như `omiai-neuro` cài đặt, có một điểm mạnh rất cụ thể: nó nắm bắt **động lực học thời gian ngắn hạn** (fading memory) của một chuỗi tín hiệu, với chi phí huấn luyện cực rẻ (chỉ hồi quy tuyến tính, không lan truyền ngược) — tuyệt vời cho dự đoán chuỗi thời gian hỗn loạn, tín hiệu điều khiển robot, phân loại mẫu động lực học đơn giản.

Điểm yếu — nói thẳng vì nó quan trọng cho quyết định thiết kế — là: các trọng số hồi quy bên trong reservoir là **ngẫu nhiên và cố định vĩnh viễn**; toàn bộ "sự học" nằm ở lớp đọc-ra tuyến tính duy nhất phía sau. Điều này về mặt lý thuyết biểu đạt (expressivity) tương đương một mô hình kernel/random-feature: rất tốt cho các ánh xạ đầu vào–đầu ra "trơn" và có bộ nhớ ngắn, nhưng **không có cơ chế học biểu diễn phân cấp, hợp thành** (hierarchical, compositional representation learning) mà một mạng transformer nhiều lớp huấn luyện bằng backprop trên dữ liệu khổng lồ đạt được — và chính cơ chế đó mới là thứ cho phép hiểu cú pháp lồng nhau, tham chiếu xa, ẩn dụ, ngữ cảnh nhiều câu. Dùng một reservoir làm "bộ não hiểu ngôn ngữ" trung tâm là dùng sai công cụ cho đúng việc — không phải vì nó "chưa đủ mạnh", mà vì đó không phải bài toán nó được thiết kế để giải, kể cả về mặt lý thuyết.

**Vai trò trung thực, hữu ích, và thật sự tận dụng được reservoir trong OmiAI (đề xuất trong Phần 7):**
- Dự đoán **quỹ đạo động lực học của `omiai-world`** (mật độ agent, năng lượng trung bình, tốc độ hội tụ ngôn ngữ nổi sinh) — đúng bài toán chuỗi thời gian nó vốn giỏi.
- Cung cấp một **nguồn biến thiên có kiểm soát** (không phải ngẫu nhiên thuần, mà là hỗn loạn tất định — tái lập được với cùng seed) để lớp diễn đạt ngôn ngữ chọn giữa nhiều cách nói cùng một sự thật, tránh việc OmiAI luôn trả lời đúng một câu y hệt cho cùng một loại câu hỏi — một use-case nhỏ nhưng thật, trung thực về việc nó làm gì (không giả vờ nó "hiểu" câu).
- Một tín hiệu "trực giác nhanh" bổ sung cho vòng lặp Active Inference ở `meta::self_improvement` — phản ứng tức thời trước khi bộ suy luận symbolic chạy xong (giống trực giác nhanh vs tư duy chậm, nhưng không đánh tráo vai trò của hai hệ thống).

### 3.3. "Trí tuệ sắc bén" thực ra đã tồn tại trong repo — chỉ là bị nhốt sau một cánh cửa hẹp

Đây là điều đáng nói với bạn một cách rõ ràng, vì nó thay đổi hoàn toàn cách nhìn vấn đề: bạn **đã có** một cỗ máy suy luận sắc bén, theo nghĩa kỹ thuật nghiêm ngặt nhất của từ này —

- `core::prover` + `core::inference` chứng minh định lý bằng resolution/DPLL/CDCL — mỗi kết luận đi kèm một **chứng minh có thể in ra, kiểm tra lại từng bước**. Không một chatbot thống kê phổ biến nào (kể cả các mô hình thương mại lớn) đưa ra được một proof object thật sự kiểm chứng được cho mỗi câu trả lời — chúng "trông có vẻ" suy luận đúng bằng cách sinh văn bản giống suy luận, nhưng không có cơ chế kiểm tra hình thức đứng sau.
- `probabilistic::bayesian` tính đúng xác suất hậu nghiệm bằng thuật toán khớp nối cây (junction tree/Hugin propagation) — README ghi rõ nó khớp với giá trị tính tay trên một mạng Bayes kinh điển (P(Rain|Wet) = 0.7396). Đây là suy luận có thể kiểm chứng bằng toán, không phải "cảm giác đúng".
- `causal::do_calculus` + `causal::dag` làm đúng tiêu chuẩn cửa sau (back-door criterion) và phản-sự-kiện kiểu Pearl (abduction khôi phục nhiễu, intervention truyền nhiễu đó tiếp) — nghĩa là OmiAI, nếu được nối dây, có thể trả lời "tại sao" và "nếu X thì Y có xảy ra không" bằng suy luận nhân quả thật, không phải tương quan nguỵ biện.

**Nút thắt cổ chai không nằm ở suy luận. Nó nằm 100% ở bản dịch từ câu người nói sang `Formula`/`Term` mà các bộ máy trên tiêu thụ được.** Đây là tin tốt về mặt kỹ thuật: dịch ngôn ngữ tự nhiên sang biểu diễn hình thức là một bài toán khó nhưng **đã được nghiên cứu kỹ, có nhiều kỹ thuật không cần deep learning** (ngữ pháp phụ thuộc, CCG, ngữ pháp chuyển đổi, và — thú vị nhất với riêng repo này — dùng chính bộ máy `evolution::genetic_programming` sẵn có để *tiến hoá* luật phân tích ngữ nghĩa, xem Slice 8 ở Phần 7). Nó dễ đạt tiến bộ thật hơn nhiều so với việc cố "tạo ra ý thức".

### 3.4. Vì sao "y hệt con người, hiểu mọi thứ" là sai khung tham chiếu — định lượng cụ thể, không nói suông

Để không lặp lại đúng lỗi mà `huongdan.txt` cảnh báo ("không tuyên bố gì không có bằng chứng"), đây là con số thật, không phải cảm tính: các mô hình ngôn ngữ đạt được sự trôi chảy/hiểu ngôn ngữ mở như con người ngày nay được huấn luyện trên **hàng nghìn tỷ token** văn bản (tương đương hàng chục triệu cuốn sách), bằng **hàng nghìn GPU** chạy liên tục nhiều tháng, sau đó tinh chỉnh thêm bằng phản hồi con người trên quy mô lớn (RLHF). Máy mục tiêu của OmiAI — một CPU i7-7700K, 8 GB RAM, không GPU — có công suất tính toán nhỏ hơn cụm huấn luyện đó nhiều bậc độ lớn (order of magnitude), và không có kho dữ liệu tương đương. Đây không phải vấn đề "cố gắng chưa đủ" — đây là khoảng cách tài nguyên vật lý, giống như so sánh một xưởng cơ khí tại nhà với một nhà máy ô tô: khác biệt về quy mô, không phải về ý chí.

**Điều này không có nghĩa là bỏ cuộc — nó có nghĩa là chọn đúng mục tiêu để "xuất sắc" thật sự đạt được:**
- Nếu mục tiêu là *"trả lời đúng và giải thích được vì sao đúng, trong phạm vi tri thức đã dạy cho nó"* → **hoàn toàn khả thi, đã có 80% hạ tầng, chỉ cần nối dây (Slice 7–9).**
- Nếu mục tiêu là *"có một xã hội nhân tạo với ngôn ngữ tự sinh, đo lường được, quan sát được"* → **đã gần xong** (`omiai-world` với 99+1 test, giao tiếp Lewis-signaling đã cài) — đây thực ra là phần **độc đáo và ấn tượng nhất** của cả dự án, hiếm dự án cá nhân nào có được, và nó KHÔNG cần bất kỳ mô hình ngôn ngữ lớn nào để có giá trị thật.
- Nếu mục tiêu là *"nói chuyện trôi chảy như ChatGPT về bất kỳ chủ đề gì"* → **không thể đạt được bằng cách tự huấn luyện từ số 0 trên phần cứng này**, chỉ có một con đường thật: **tái sử dụng có ý thức trọng số của một mô hình mã nguồn mở đã được huấn luyện sẵn** (Slice 10, tuỳ chọn, minh bạch, có ADR riêng) — không phải "tự dạy nó từ đầu bằng Unicode", mà là mượn một "cái miệng" đã biết nói, gắn vào một "cái đầu" (lõi symbolic) đã biết suy luận đúng.

Ba mục tiêu trên **không loại trừ nhau** — bạn có thể có cả ba trong cùng một file `.omiai`, mỗi cái được xây bằng đúng công cụ của nó, và mỗi cái được mô tả trung thực đúng bằng khả năng thật của nó. Đó là điều Phần 4–7 sẽ trình bày cụ thể.

---

## PHẦN 4 — TẦM NHÌN SẢN PHẨM ĐƯỢC TÁI ĐỊNH NGHĨA: OmiAI SAU NÂNG CẤP LÀ GÌ

Sau khi hoàn thành lộ trình ở Phần 7, một file `model.omiai` xuất ra sẽ là một hệ **neuro-symbolic lai**, có thể mô tả trung thực bằng đúng một đoạn sau (dùng để viết `README.md`/tài liệu quảng bá về sau, mỗi câu đều phải đúng nghĩa đen):

> *"OmiAI là một tác nhân hội thoại suy luận được: mọi câu trả lời liên quan đến sự kiện đều đi kèm một chứng minh logic hình thức có thể kiểm tra lại (`ProofResult`), một mức tin cậy xác suất khi có bất định (Bayesian inference), và một lời giải thích nhân quả khi được hỏi 'tại sao' (do-calculus). Nó vận hành một xã hội nhân tạo nội tại gồm các agent tiến hoá, có thể tự phát triển quy ước giao tiếp riêng đo lường được bằng thông tin tương hỗ — bạn có thể hỏi thẳng OmiAI 'thế giới của mày đang nói từ gì cho nguy hiểm' và nhận câu trả lời từ dữ liệu mô phỏng thật, không phải văn mẫu. Diễn đạt câu trả lời có thể ở một trong hai chế độ, tuỳ cấu hình: chế độ thuần logic (câu văn dựng từ khuôn mẫu, không phụ thuộc mô hình ngoài, 100% giải thích được), hoặc chế độ trôi chảy (một mô hình ngôn ngữ nhỏ mã nguồn mở, chạy hoàn toàn local, chỉ dùng để đặt câu cho đúng sự thật mà lõi symbolic đã xác nhận — không được phép tự bịa sự kiện mới)."*

Đây là một tuyên bố **có thể chứng minh từng vế bằng test thật** — đúng tinh thần của chính dự án. Nó khác về chất so với "trí tuệ nhân tạo hiểu ngôn ngữ như con người" (một tuyên bố không thể kiểm chứng và, như Phần 3 đã chỉ ra, không đúng với bất kỳ hệ thống nào chạy trên phần cứng này) — nhưng nó **thật, độc đáo, và trong một số khía cạnh (khả năng giải thích, khả năng kiểm chứng) còn vượt trội hơn một chatbot thống kê thuần tuý**.

### 4.1. Ba trụ giá trị thật, không phóng đại

| Trụ giá trị | Đã sẵn sàng bao nhiêu % | Việc còn lại |
|---|---:|---|
| **Suy luận có thể kiểm chứng** (logic + xác suất + nhân quả) | ~85% (thuật toán đã có, đã test) | Nối dây vào `ChatEngine` (Slice 7) |
| **Xã hội nhân tạo có ngôn ngữ nổi sinh quan sát được** | ~90% (đã cài, đã test 100 test) | Thêm API truy vấn read-only cho chat (Slice 9) |
| **Diễn đạt trôi chảy tự nhiên đa dạng chủ đề** | ~15% (chỉ khuôn mẫu cứng) | Slice 8 (mở NLU) bắt buộc; Slice 10 (LLM cục bộ) tuỳ chọn nếu muốn trôi chảy thật sự |

### 4.2. Nguyên tắc thiết kế bất biến (áp dụng cho mọi slice ở Phần 7)

1. **Không trụ cột nào được phép "nói dối thay cho" một trụ cột khác.** Nếu lớp diễn đạt ngôn ngữ (dù là khuôn mẫu hay LLM cục bộ) đưa ra một khẳng định sự kiện, khẳng định đó phải truy được về một `ProofResult`, một xác suất từ `BayesianNetwork`, hoặc một suy diễn từ `KnowledgeGraph` — không có ngoại lệ. Nếu không truy được, câu trả lời phải được gắn cờ rõ ràng là "chưa xác minh" (`grounded: false`), không được trình bày với giọng điệu chắc chắn.
2. **Mọi khả năng mới đều được khai báo trong `manifest.json` của bundle** (Phần 6) — runtime đọc bundle nào phải biết chính xác bundle đó có/không có trụ cột nào, không suy đoán.
3. **Mọi lựa chọn có tái sử dụng trọng số/mã nguồn ngoài dự án đều phải minh bạch qua ADR riêng** (như Slice 10) — kể cả khi lựa chọn đó là hợp lý và nên làm. Đây là khác biệt giữa "âm thầm phá vỡ triết lý zero-training" và "có ý thức mở rộng triết lý một cách trung thực".
4. **Không có benchmark thật, không có tuyên bố hiệu năng.** Không có round-trip test, trụ cột đó chưa "xong". Giữ nguyên văn hoá đã có của dự án.

---

## PHẦN 5 — KIẾN TRÚC TỔNG THỂ MỚI

Sơ đồ hiện tại của `docs/architecture/README.md` là:

```
                 ┌─────────────┐
   text ──► io   │  omiai-io   │  NLP → logic formulas
                 └──────┬──────┘
                        ▼
   ┌────────── core ──────────┐   knowledge / probabilistic /
   │ logic · CNF · unification│   causal / neuro / memory
   │ resolution · DPLL/CDCL   │   (pillars, siblings over core)
   │ CSP · prover · LTL       │
   └──────────────────────────┘
                        ▼
              evolution → meta          world (substrate)
                        ▼                   ▼
                   checkpoint ◄────────────┘
                        ▼
           export · runtime · serve · cli
```

Sơ đồ đề xuất cho giai đoạn tiếp theo — thay đúng hai chỗ: (a) `io` trở thành một **bộ định tuyến thật** gọi tới mọi pillar chứ không chỉ `core`; (b) thêm một **pillar thứ 9 tuỳ chọn** (`language`, tắt theo mặc định) song song với, không thay thế, lõi symbolic:

```
                         ┌───────────────────────────┐
   văn bản người dùng ──►│   omiai-io (v2)            │
   (UTF-8, mọi ngôn ngữ) │   tokenizer ─► semantic     │
                         │   parser (rule + evolved)   │
                         └──────────────┬──────────────┘
                                        │  Formula / Query / Intent
                                        ▼
                     ┌──────────────────────────────────────┐
                     │           DIALOGUE ROUTER              │
                     │  (mới — trái tim của Slice 7)          │
                     └───┬─────────┬─────────┬─────────┬─────┘
                         ▼         ▼         ▼         ▼
                  core::prover  knowledge  probabilistic  causal
                 (chứng minh)   ::graph    ::bayesian    ::do_calculus
                         │         │         │         │
                         └─────────┴────┬────┴─────────┘
                                        ▼
                         ┌──────────────────────────┐
                         │   PROOF / EVIDENCE OBJECT  │  ← sự thật đã kiểm chứng
                         │   (ProofResult, xác suất,   │     nằm ở đây, bất biến
                         │    DAG nhân quả, nguồn gốc) │     qua mọi lớp diễn đạt
                         └──────────────┬─────────────┘
                                        ▼
                      ┌─────────────────────────────────┐
                      │      SURFACE REALIZATION          │
                      │  chế độ A: khuôn mẫu mở rộng        │
                      │            + reservoir (đa dạng hoá) │
                      │  chế độ B (tuỳ chọn, ADR-0008):     │
                      │      LLM cục bộ nhỏ = "cái miệng"   │
                      │      (không được thêm sự kiện mới)  │
                      └──────────────┬───────────────────┘
                                        ▼
                                 văn bản trả lời
                                        ▲
                      omiai-world (đọc song song, read-only):
                      trạng thái xã hội agent, từ vựng nổi sinh
                                        ▲
                      omiai-neuro (đọc song song, read-only):
                      seed đa dạng hoá diễn đạt, dự báo quỹ đạo world
```

`evolution → meta → checkpoint → export → runtime → serve → cli` giữ nguyên như sơ đồ gốc — không đổi, vì chúng đã đúng thiết kế.

---

## PHẦN 6 — ĐẶC TẢ ĐỊNH DẠNG `.omiai` (bundle-v1)

`huongdan.txt` đã mô tả đúng ý tưởng (tar+zstd, có manifest khai báo khả năng) nhưng **`docs/format-spec/bundle-v1.md` vẫn đang là "TODO"** theo README. Đây là đặc tả đầy đủ, viết theo đúng văn phong và cấu trúc của `docs/format-spec/checkpoint-v1.md` đã có (status table, layout, JSON schema có bảng field, chính sách tương thích ngược) — AI coding agent nên tạo file này tại đúng đường dẫn đó.

### 6.1. Nội dung cần đưa vào `docs/format-spec/bundle-v1.md`

**Status:** implemented and tested / hoặc scaffold, tuỳ giai đoạn thực thi — cập nhật trung thực khi code xong, không viết trước.

**1. Layout.** Một bundle `model.omiai` là **một file duy nhất** = `tar` rồi nén bằng `zstd` (khác hẳn checkpoint — checkpoint là thư mục nhiều file, bundle là một file archive để phân phối/nạp):

```
model.omiai  (= .tar.zst, đổi tên đuôi)
├── manifest.json
├── logic/
│   └── seed_facts.cbor          # tri thức nền được "dạy" trước khi xuất, tối giản
├── knowledge_graph/
│   └── graph.cbor               # cắt tỉa: bỏ lịch sử suy luận, giữ đồ thị hiện tại
├── probabilistic/
│   └── networks.cbor            # các BayesianNetwork đã định nghĩa (nếu có)
├── causal/
│   └── dag.cbor                 # CausalDag đã định nghĩa (nếu có)
├── reservoir/
│   └── weights.safetensors      # W, W_in, W_out đã huấn luyện (mmap được)
├── world_snapshot/               # tuỳ chọn — chỉ nếu muốn cho phép "hỏi thế giới"
│   ├── grid.bin
│   ├── agents.cbor
│   └── vocabulary.cbor          # từ vựng nổi sinh đã đề bạt, để tra cứu nhanh
├── language_model/                # CHỈ tồn tại nếu pillar 9 (Slice 10) được bật
│   ├── model.gguf                # trọng số đã lượng tử hoá (không train trong bundle)
│   ├── tokenizer.json
│   └── model_card.json           # tên, phiên bản, giấy phép, checksum nguồn gốc
└── io/
    └── lexicon.cbor              # từ điển đã học qua evolved semantic parser (Slice 8)
```

**2. `manifest.json` — trường bắt buộc:**

```json
{
  "format_version": 1,
  "schema": "omiai-bundle",
  "created_utc": "2026-09-05T00:00:00Z",
  "source_checkpoint_step": 1234567,
  "git_commit": null,
  "capabilities": {
    "logic": true,
    "knowledge_graph": true,
    "probabilistic": true,
    "causal": true,
    "reservoir": true,
    "world_query": false,
    "language_model": false
  },
  "language_model_info": null,
  "entrypoint": {
    "function": "step",
    "input_schema": "InferInput_v1",
    "output_schema": "InferOutput_v1"
  },
  "files": [
    { "path": "manifest.json", "blake3": null },
    { "path": "logic/seed_facts.cbor", "blake3": "<64 hex chars>" }
  ]
}
```

Khi `capabilities.language_model = true`, `language_model_info` **bắt buộc** khác `null` và phải có dạng:

```json
{
  "name": "ví dụ: Qwen2.5-1.5B-Instruct",
  "quantization": "Q4_K_M",
  "license": "Apache-2.0",
  "source_url": "https://huggingface.co/...",
  "sha256": "<checksum của file .gguf gốc>",
  "role": "surface_realization_only",
  "may_assert_unverified_facts": false
}
```

Trường `role` và `may_assert_unverified_facts` không phải trang trí — `omiai-runtime` (Phần 7, Slice 12) phải đọc và **thực thi** ràng buộc này ở tầng code (từ chối để mô hình ngôn ngữ trả lời trực tiếp một câu hỏi sự kiện nếu không có `ProofResult`/xác suất đi kèm), không chỉ ghi trong tài liệu.

**3. Quy tắc nạp (load-time contract), bắt buộc với `omiai-runtime`:**

- `format_version` không khớp → lỗi tường minh, không cố "đoán" đọc.
- Thiếu file được khai báo trong `files[]`, hoặc BLAKE3 không khớp → từ chối nạp, báo rõ file nào hỏng.
- `capabilities.X = false` → runtime không được gọi bất kỳ hàm nào của pillar `X`, kể cả khi thư mục `X/` tình cờ tồn tại trong archive (phòng bundle bị chỉnh tay sai).
- Không có `language_model` → runtime chạy 100% chế độ khuôn mẫu, không bao giờ cố tải mô hình ngoài.

**4. Chính sách tương thích ngược:** `format_version = 1` là đặc tả đầu tiên. Khi có `v2`, loader `v2` bắt buộc đọc được bundle `v1` cũ (giữ struct `ManifestV1` riêng, có hàm `upgrade_v1_to_v2()` tường minh) — y hệt nguyên tắc đã áp dụng cho checkpoint.

**5. Khác biệt cố ý với checkpoint (nêu rõ để không nhầm lẫn khi review code):**

| | Checkpoint | Bundle (`.omiai`) |
|---|---|---|
| Dạng | Thư mục | Một file archive (tar+zstd) |
| Mục đích | Tiếp tục huấn luyện/mô phỏng | Triển khai để suy luận |
| Nội dung | Đầy đủ, gồm lịch sử tiến hoá, RNG state | Đã cắt tỉa — chỉ phần cần cho `step()` |
| RNG state | Bắt buộc (để resume đúng quỹ đạo) | Không cần (suy luận không cần resume quỹ đạo tiến hoá) |
| Tần suất ghi | Định kỳ, tự động, giữ cửa sổ trượt | Thủ công, mỗi khi "release" một phiên bản |

---

## PHẦN 7 — LỘ TRÌNH THEO GIAI ĐOẠN (slice-by-slice, đúng văn hoá dự án)

Đặt tên slice tiếp nối đúng quy ước đã có ở `docs/superpowers/plans/YYYY-MM-DD-<tên-slice>.md` (dự án đã dùng slice 1, 2, 3, 5 cho checkpoint/world/communication/knowledge-promotion). Đề xuất **Slice 6 = trạng thái hiện tại** (đã xong theo README), và bắt đầu từ **Slice 7**. Mỗi slice bên dưới có: mục tiêu, phạm vi, tiêu chí "xong" (test/benchmark cụ thể), và rủi ro cần canh. **Agent tuyệt đối không nhảy cóc slice — đúng điều khoản cuối của `huongdan.txt`.**

### Slice 7 — "Nối dây": biến 8 hòn đảo thành một bộ não

**Mục tiêu:** `ChatEngine` gọi được tất cả các pillar đã có, không chỉ `core::prover`. Đây là slice giá trị-trên-rủi ro cao nhất trong toàn bộ lộ trình: **0% nghiên cứu mới, 100% kỹ thuật phần mềm thuần túy**, vì mọi pillar đích đã có API công khai, đã test.

**Phạm vi cụ thể:**
1. Chuyển `omiai-knowledge`, `omiai-probabilistic`, `omiai-causal`, `omiai-neuro`, `omiai-world` từ `[dev-dependencies]` sang `[dependencies]` thật trong `crates/omiai-io/Cargo.toml`.
2. Thêm một struct `DialogueRouter` mới trong `omiai-io` (chi tiết mã nguồn ở Phần 8.1) nhận một `Formula`/`ParseIntent` đã phân tích và quyết định pillar nào xử lý:
   - Câu khẳng định phổ quát/đơn lẻ đã biết chắc (như hiện tại) → `core::prover`.
   - Câu hỏi có từ khoá bất định ("có lẽ", "khả năng", "probably", "likely") → nếu có `BayesianNetwork` phù hợp đã nạp, gọi `infer_exact`/`infer_mcmc`; trả lời kèm số phần trăm thật, không bịa.
   - Câu hỏi "tại sao" / "why" / "nếu ... thì" / "what if" → nếu có `CausalDag` phù hợp, gọi `backdoor_criterion` + do-calculus; trả lời bằng một giải thích nhân quả có cấu trúc (X ảnh hưởng Y qua Z), không chỉ tương quan.
   - Câu hỏi liên quan quan hệ đã biết nhưng không có trong bộ facts hội thoại hiện tại → thử `knowledge::graph::query_path` / `reasoning::forward_chain` trước khi báo "chưa đủ dữ kiện".
3. Chuẩn hoá một `enum ReasoningResult` chung bọc quanh cả bốn loại kết quả trên (proof / xác suất / giải thích nhân quả / đường đi tri thức) — đây chính là "PROOF / EVIDENCE OBJECT" ở sơ đồ Phần 5 — để lớp diễn đạt (Slice 9) chỉ cần xử lý một kiểu dữ liệu duy nhất.
4. **Test bắt buộc trước khi coi slice này là xong:** ít nhất một integration test trong `crates/omiai-io/tests/` gọi `ChatEngine` end-to-end đi qua **mỗi một trong bốn** pillar mới (không chỉ unit test nội bộ từng pillar — pillar đã có unit test riêng rồi, cái thiếu là chứng minh chúng *thật sự được gọi từ chat*).

**Rủi ro cần canh:** vòng phụ thuộc (`omiai-io` → `omiai-world` → ... → có thể quay lại `omiai-io`?) — kiểm tra bằng `cargo tree` trước khi đổi `Cargo.toml`; nếu có vòng, dùng lại đúng pattern ADR-0005 (io/meta cycle) đã áp dụng trước đó cho vấn đề tương tự.

### Slice 8 — Mở rộng "tai và miệng": semantic parser diện rộng, không cần deep learning

**Mục tiêu:** thay từ điển 4-từ + 4 khuôn mẫu cứng bằng một bộ phân tích ngữ nghĩa có độ phủ thật, **vẫn hoàn toàn symbolic, vẫn giải thích được từng luật, không có "hộp đen"**.

**Phạm vi, theo ba bước tăng dần (đừng làm gộp — đúng nguyên tắc "tăng dần" của dự án):**

1. **Gộp từ điển vào `knowledge::graph`/`ontology` thay vì `HashMap<String,String>` cô lập trong `nlp_parser`.** Khi người dùng nói một câu dạng "X là Y" với Y chưa từng biết, thay vì chỉ tạo một `Formula::Atom` tạm, hệ thống **thêm một `Concept` mới vào `KnowledgeGraph`** (nếu là khái niệm mới) — nghĩa là "dạy" OmiAI một từ mới thật sự **được nhớ lâu dài, tra cứu lại được, tham gia suy luận tiếp** (qua `forward_chain`/`query_path`), không chỉ tồn tại trong bộ nhớ hội thoại của phiên hiện tại.

2. **Thay việc "đếm số từ và vị trí" bằng ngữ pháp phụ thuộc tối giản.** Vẫn dùng `nom` (đã có, không cần crate mới), nhưng viết một tầng ngữ pháp thật với các loại cụm từ (danh ngữ, động ngữ, mệnh đề quan hệ) thay vì so khớp `words[0] == "every"`. Điều này tự nhiên mở rộng số câu xử lý được từ ~4 dạng lên hàng chục dạng, mà vẫn 100% tường minh, review được từng luật.

3. **Ý tưởng cốt lõi, đúng tinh thần dự án nhất, đề xuất mạnh:** dùng chính `evolution::formula_gp` (đã tồn tại, đã dùng để tiến hoá Formula làm chính sách hành vi agent trong `omiai-world`) để **tiến hoá luật ánh xạ câu→Formula** thay vì viết tay mãi mãi. Cách làm:
   - Xây một tập dữ liệu nhỏ, tự tạo, **hoàn toàn nội bộ, không gọi dịch vụ ngoài**: khoảng 300–800 cặp `(câu tiếng Việt hoặc tiếng Anh, Formula mục tiêu)`. Tự viết tay ~100–150 cặp lõi, sau đó **nhân bản bằng tổ hợp từ vựng** (thay `"Socrates"` bằng các tên khác, thay `"người"` bằng các danh từ khác đã có trong ontology) — đây là tổ hợp thuần tuý trên dữ liệu của chính bạn, không phải gọi một mô hình ngoài để "sinh dữ liệu hộ".
   - Định nghĩa fitness = tỉ lệ khớp đúng `Formula` sinh ra so với `Formula` mục tiêu trên tập dữ liệu này (có thể dùng chính hàm so sánh cấu trúc AST đã có sẵn cho `Formula`/`LtlFormula` trong `evolution::formula_gp`/`ltl_formula_gp`).
   - Chạy `genetic_programming` (đảo hoá — đã có "async island model") để tiến hoá một tập luật ánh xạ, biểu diễn dưới dạng cây cú pháp có thể **in ra đọc được** (không phải trọng số ẩn) — mỗi "gen" ở đây vẫn là một cấu trúc symbolic tường minh, đúng triết lý ADR-0004 ("gene = FormulaId, không phải con số vô nghĩa") áp dụng sang cho chính lớp ngôn ngữ.
   - Đây **là một hình thức "huấn luyện" thật (có vòng lặp tối ưu trên dữ liệu)** — nói thẳng điều này ra trong tài liệu dự án thay vì né tránh — nhưng nó **không phải deep learning/backprop**, và kết quả luôn ở dạng luật đọc được, đúng ràng buộc "không hộp đen" gốc của dự án. Nên ghi một ADR ngắn (ADR-0008 hoặc số tiếp theo tại thời điểm thực thi) nêu rõ điểm này để tài liệu hoá quyết định minh bạch.

**Tiêu chí xong:** một bộ test hồi quy (regression test) chạy toàn bộ tập dữ liệu nhãn, báo cáo % khớp đúng — con số này **phải được in trong README**, đúng văn hoá "không tuyên bố không có số".

### Slice 9 — Trả lời có bằng chứng, có xác suất, có nhân quả, có biến thiên tự nhiên

**Mục tiêu:** lớp `realize_*` (hiện đang là các khối `match` cứng sinh đúng một câu cho mỗi tình huống) trở thành một bộ sinh câu **từ `ReasoningResult`** (Slice 7) với:

1. **Trình bày chứng minh:** khi `ProofResult::Proved`, không chỉ nói "đúng" — nói được *dựa trên các sự kiện nào* (liệt kê premises đã dùng, lấy từ `ProofReport` của `prove_timed`). Đây là điểm khác biệt lớn nhất so với chatbot thống kê: **câu trả lời có nguồn gốc kiểm chứng được.**
2. **Trình bày xác suất trung thực:** khi câu trả lời đến từ `BayesianNetwork`, luôn kèm số (`"khoảng 74%"` không phải `"có lẽ"` mơ hồ) — và nếu không đủ evidence để tính, nói rõ thay vì đoán.
3. **Trình bày nhân quả:** khi câu trả lời đến từ `causal::do_calculus`, phân biệt rõ ràng "A và B thường đi cùng nhau" (tương quan) với "A gây ra B" (đã qua kiểm định back-door) trong chính câu trả lời — đây là một điểm giáo dục tốt, hiếm chatbot phổ thông làm đúng.
4. **Đa dạng hoá diễn đạt bằng reservoir (vai trò trung thực từ Mục 3.2):** với mỗi (loại-kết-quả, mức-tin-cậy) có một **tập nhiều cách diễn đạt hợp lệ** thay vì một câu cố định; dùng trạng thái hiện tại của `Reservoir` (đã chạy nền, nhận input là embedding thô của lượt hội thoại) làm chỉ số chọn giữa các cách diễn đạt — tái lập được (cùng seed → cùng lựa chọn), không phải giả vờ "cảm xúc", chỉ là tránh sự đơn điệu máy móc.
5. **Truy vấn thế giới mô phỏng (read-only) như một tính năng hội thoại thật:** thêm intent mới `ParseIntent::AskWorld` — cho phép câu như "thế giới của mày có bao nhiêu agent" / "từ nào agent dùng để báo nguy hiểm" được trả lời bằng dữ liệu thật từ `world::registry`/`communication::vocabulary` (đã có, đã test) thay vì bịa. Đây là tính năng **không chatbot nào khác có** — nó xứng đáng là điểm nhấn khi trình bày dự án.

**Tiêu chí xong:** với cùng một câu hỏi lặp lại 20 lần, hệ thống trả về ít nhất 3 cách diễn đạt khác nhau (chứng minh Mục 4 hoạt động) nhưng **không bao giờ** đổi nội dung sự thật/con số giữa các lần (chứng minh diễn đạt tách biệt khỏi nội dung — bất biến thiết kế then chốt).

### Slice 10 (TUỲ CHỌN, minh bạch qua ADR riêng) — Pillar thứ 9: diễn đạt trôi chảy bằng mô hình ngôn ngữ cục bộ

**Đây là slice duy nhất trong toàn bộ lộ trình có tái sử dụng trọng số huấn luyện từ bên ngoài dự án — nói thẳng, không giấu.** Nếu bạn muốn giữ 100% triết lý "zero-training" nguyên vẹn, có thể bỏ qua hoàn toàn slice này — Slice 7–9 đã tạo ra một hệ thống hoàn chỉnh, thật, hữu ích, không cần nó. Nhưng nếu mục tiêu thật sự là "nói chuyện trôi chảy như con người" (đúng như yêu cầu ban đầu của bạn), đây là con đường **duy nhất** đạt được điều đó một cách trung thực trên phần cứng 8 GB/CPU-only — không có cách nào tự huấn luyện một mô hình đủ trôi chảy từ số 0 trên máy này (Mục 3.4).

**Nguyên tắc bất di bất dịch của slice này (thực thi bằng code ở `omiai-runtime`, không chỉ ghi trong doc):**

> Mô hình ngôn ngữ cục bộ **chỉ được phép diễn đạt lại** một `ReasoningResult` đã có sẵn từ lõi symbolic. Nó **không bao giờ** được gọi để tự quyết định một sự kiện đúng/sai, một xác suất, hay một quan hệ nhân quả mới. Nếu người dùng hỏi điều gì đó lõi symbolic không trả lời được, câu trả lời **phải** nói rõ "đây là suy đoán của một mô hình phụ trợ, chưa được lõi logic xác minh" (`grounded: false` trong response, xem Phần 6) — không được trộn lẫn với các câu trả lời có `grounded: true`.

**Việc cần làm:**

1. **Viết ADR mới** (đúng thứ tự tiếp theo sau ADR-0007 hiện có, ví dụ `docs/adr/0008-optional-local-llm-surface-layer.md`), nêu: bối cảnh (Phần 3.4 của tài liệu này), quyết định, hệ quả (bundle lớn hơn, cần quản lý giấy phép mô hình), và ranh giới trách nhiệm (chỉ diễn đạt, không quyết định sự thật).
2. **Chọn thư viện suy luận Rust.** Hai lựa chọn thật, đang được duy trì tích cực tại thời điểm viết tài liệu này (kiểm tra lại phiên bản mới nhất khi thực thi, vì hệ sinh thái này đổi nhanh):
   - **`candle`** (do Hugging Face duy trì) — framework ML thuần Rust, có `candle-transformers` hỗ trợ sẵn kiến trúc Llama/Qwen/Mistral/Phi, chạy CPU tốt, biên dịch nhẹ, gọn để nhúng.
   - **`llama-cpp-2`** (từ `utilityai/llama-cpp-rs`) — binding Rust cho `llama.cpp`, đọc thẳng định dạng GGUF đã lượng tử hoá, tối ưu CPU rất tốt (SIMD AVX2 — đúng tập lệnh i7-7700K có hỗ trợ), hệ sinh thái GGUF cực lớn trên Hugging Face.
   
   Khuyến nghị: bắt đầu với `llama-cpp-2` vì GGUF + AVX2 là con đường ít rủi ro hiệu năng nhất trên đúng phần cứng mục tiêu; cân nhắc `candle` sau nếu cần kiểm soát sâu hơn ở tầng Rust thuần (không cần `clang`/`bindgen`).
   
3. **Chọn mô hình — ưu tiên nhỏ nhất còn dùng được, giấy phép rộng.** Tính đến giữa năm 2026, các lựa chọn nhỏ, giấy phép permissive (Apache-2.0/MIT), phù hợp CPU/RAM hạn chế: **Phi-4-mini** (3,8B tham số, MIT, ngữ cảnh 128K, khoảng 2,5 GB ở lượng tử hoá Q4 — điểm mạnh: suy luận/toán tốt cho kích thước nhỏ, điểm yếu: kiến thức thực tế mỏng — chấp nhận được vì vai trò của nó chỉ là diễn đạt, không phải nguồn kiến thức); các biến thể nhỏ của **Qwen3** (Apache-2.0, đa ngôn ngữ tốt — quan trọng vì bạn cần cả tiếng Việt); hoặc các mô hình nhỏ hơn nữa (lớp 0,5B–1,5B) nếu cần chừa nhiều RAM hơn cho `omiai-world`/`omiai-neuro` chạy song song. **Tuyệt đối kiểm tra lại giấy phép trước khi đóng gói vào bundle phân phối** — một số mô hình mạnh (như dòng Llama) dùng giấy phép cộng đồng có điều kiện, không phải OSI-approved thuần tuý; đọc kỹ điều khoản trước khi chọn nếu bundle sẽ được chia sẻ công khai.
4. **Ngân sách RAM cụ thể** — xem bảng đầy đủ ở Phần 9, nhưng nguyên tắc: mô hình ngôn ngữ + reservoir + world grid + hệ điều hành + chính runtime Rust phải cộng lại dưới 8 GB với biên an toàn; bắt đầu từ mô hình nhỏ nhất chấp nhận được, chỉ tăng lên nếu đo đạc thật (không phải ước lượng) còn dư RAM.
5. **Prompt ràng buộc chặt, không phải "chat tự do":** prompt gửi cho mô hình ngôn ngữ **luôn** có cấu trúc "Đây là sự thật đã được xác minh: {ProofResult hoặc xác suất hoặc giải thích nhân quả}. Hãy diễn đạt lại thành một câu {ngôn ngữ} tự nhiên, không thêm thông tin nào ngoài nội dung trên." — đây là kỹ thuật "data-to-text generation", giảm mạnh rủi ro mô hình tự bịa so với để nó trả lời tự do.

**Tiêu chí xong:** một bộ test "grounding" — với 50 câu hỏi mẫu, kiểm tra bằng chương trình (không phải mắt thường) rằng **mọi con số/sự kiện xuất hiện trong câu trả lời của LLM đều khớp với `ReasoningResult` đã đưa vào prompt** (so khớp chuỗi con số, tên riêng) — nếu LLM thêm bất kỳ con số/tên riêng nào không có trong input, test này phải fail. Đây là bài test quan trọng nhất của toàn bộ Slice 10.

### Slice 11 — `omiai-export`: đóng gói `model.omiai` thật

Thực thi đúng đặc tả Phần 6: đọc một checkpoint (thư mục), cắt tỉa (bỏ lịch sử tiến hoá, giữ top-N + thống kê tóm tắt — logic này đã được mô tả sẵn cho `evolution/population.cbor` trong `huongdan.txt`, chỉ cần tái dùng), viết `manifest.json` với `capabilities` khớp thật với dữ liệu có trong checkpoint nguồn (không khai `true` cho pillar không có dữ liệu), nén bằng `zstd`, đóng gói bằng `tar`. Test bắt buộc: **round-trip xuất rồi nạp lại phải cho cùng hành vi `step()` với cùng input** — tiêu chí y hệt logic round-trip test đã áp dụng cho checkpoint.

### Slice 12 — `omiai-runtime`: `load()` + `step()`, ba đích biên dịch

Crate này **không được phép** phụ thuộc bất kỳ crate huấn luyện/tiến hoá nào ở dạng cho phép ghi trạng thái tiến hoá — chỉ đọc bundle, chạy suy luận. Ba đích biên dịch đúng theo `huongdan.txt`:
- Thư viện Rust gốc (dùng trực tiếp từ `omiai-serve`/`omiai-cli`).
- `crate-type = ["cdylib"]` để C/C++/ngôn ngữ khác gọi qua FFI.
- `wasm32-wasi` (chạy độc lập qua `wasmtime`, gọi được từ Python qua `wasmtime-py`) và `wasm32-unknown-unknown` + `wasm-bindgen` (nhúng thẳng vào JavaScript/trình duyệt) — hai đích WASM tách biệt vì mục đích khác nhau (server-side độc lập vs. nhúng trình duyệt), không gộp chung.

Chi tiết khung mã nguồn ở Phần 8.2.

### Slice 13 — `omiai-serve`: HTTP `/infer` bằng `axum`

Server tối giản: nạp một `model.omiai` khi khởi động (đường dẫn qua biến môi trường/tham số dòng lệnh), mở `POST /infer` nhận JSON `{ "input": "...", "session_id": "..." }`, trả JSON chứa văn bản trả lời **cùng với** `ReasoningResult` gốc (proof, xác suất, nguồn) và cờ `grounded` — để bất kỳ client nào (kể cả không phải Rust) đều thấy được phần "bằng chứng" đứng sau câu trả lời, không chỉ nhận văn bản suông. Chi tiết khung mã nguồn ở Phần 8.3.

### Slice 14 — `omiai-cli`: ghép nối tất cả thành trải nghiệm dùng được

Dùng `clap` (đã có trong dependency) cho các subcommand: `train` (chạy `world::World::step()` liên tục, tự động checkpoint theo chu kỳ cấu hình), `resume` (đọc `checkpoints/index.json`, tiếp tục), `export` (gọi Slice 11), `bench` (chạy các benchmark `criterion` đã khai trong README), `chat` (một REPL cục bộ — nạp trực tiếp một bundle hoặc checkpoint, cho phép thử hội thoại ngay trên terminal **không cần** dựng HTTP server, cực kỳ hữu ích để agent tự kiểm tra công việc của chính mình trong lúc phát triển), `serve` (gọi Slice 13).

### Slice 15 — Kiểm thử tổng thể, benchmark, demo kịch bản đầu-cuối

Viết `tests/` ở cấp root (hiện còn thiếu theo README) với một kịch bản đầu-cuối duy nhất chạy được bằng `cargo test`: (a) dạy hệ thống vài sự kiện mới bằng câu tự nhiên, (b) hỏi một câu hỏi logic và kiểm tra proof đi kèm, (c) hỏi một câu xác suất và kiểm tra số trả về đúng khoảng, (d) hỏi một câu "tại sao" và kiểm tra cấu trúc giải thích nhân quả, (e) hỏi về thế giới mô phỏng và kiểm tra dữ liệu trả về khớp trạng thái `World` thật, (f) nếu Slice 10 được bật, kiểm tra bài test "grounding" ở Slice 10 chạy qua toàn bộ pipeline `omiai-serve`. Bổ sung `examples/world_demo.rs` và `examples/communication_demo.rs` như `huongdan.txt` đã đề cập nhưng chưa thấy trong repo hiện tại — đây là hai ví dụ trình diễn giá trị độc đáo nhất của dự án (thế giới nhân tạo + ngôn ngữ nổi sinh), nên ưu tiên làm đẹp phần này khi demo cho người khác xem.