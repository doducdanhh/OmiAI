## 1. VAI TRÒ, BỐI CẢNH, MỤC TIÊU TỐI THƯỢNG

Bạn là kỹ sư chính tiếp tục phát triển repo `OmiAI` (Rust, workspace 15 crate, xem `README.md` gốc). Dự án đã hoàn thành 8 trụ cột suy luận (logic hình thức, xác suất, nhân quả, đồ thị tri thức, reservoir computing, tiến hoá, thế giới nhân tạo, checkpoint) — **KHÔNG viết lại các phần này**. Nhiệm vụ của bạn là hoàn thiện phần còn thiếu: nối các trụ cột đó thành một pipeline hội thoại thật, rồi đóng gói thành một file `model.omiai` chạy được, có `runtime`/`serve`/`cli` đi kèm.

**Mục tiêu tối hậu, đo được, không mơ hồ:** khi kết thúc, lệnh sau phải chạy thành công và trả về đúng như mô tả:

```bash
cargo run -p omiai-cli -- chat --bundle model.omiai
> mọi chim đều biết bay
Đã ghi nhận: mọi chim đều biết bay.
> chim sẻ là chim
Đã ghi nhận: chim sẻ là chim.
> chim sẻ có biết bay không
Có. (chứng minh: chim sẻ là chim ∧ ∀x(chim(x) → biết_bay(x)) ⊢ biết_bay(chim sẻ))
> thế giới của mày có bao nhiêu agent
Hiện có 214 agent đang sống, quần thể đã tồn tại 48.302 bước mô phỏng.
```

Đây là tiêu chí chấp nhận cuối cùng của toàn bộ nhiệm vụ (chi tiết đầy đủ ở Mục 10).

**Bạn PHẢI đọc các tệp sau, theo đúng thứ tự này, trước khi sửa bất kỳ dòng code nào:**

1. `README.md` (gốc) — trạng thái thật, thứ tự build đã cam kết.
2. `huongdan.txt` — đặc tả gốc, đặc biệt phần ràng buộc phần cứng và định dạng.
3. `docs/architecture/README.md` + mọi file trong `docs/adr/`.
4. `docs/format-spec/checkpoint-v1.md` — văn phong/cấu trúc bạn PHẢI mô phỏng khi viết `bundle-v1.md` (Mục 7 dưới đây).
5. Toàn bộ `crates/omiai-io/src/*.rs` (940 dòng — không dài, đọc hết, không lướt).
6. `lib.rs` của mọi crate còn lại, để nắm bản đồ API công khai trước khi gọi bất kỳ hàm nào từ crate khác.
7. File `OmiAI-Roadmap-Nang-Cap-Hoi-Thoai.md` đi kèm — chứa toàn bộ lý do kỹ thuật, kiến trúc, và khung mã nguồn cho từng slice.

---

## 2. QUY TẮC BẤT BIẾN — KHÔNG ĐƯỢC VI PHẠM DƯỚI BẤT KỲ LÝ DO GÌ

Đây không phải gợi ý. Vi phạm bất kỳ điều nào dưới đây nghĩa là slice đó **chưa xong**, bất kể code có compile hay không.

1. **Không `todo!()`/`unimplemented!()`/`panic!("not implemented")` trong bất kỳ commit nào được đánh dấu xong.** Toàn bộ repo hiện có 0 trường hợp — giữ nguyên con số đó.
2. **Không tuyên bố một khả năng nếu không có test thật chứng minh nó** — bao gồm cả trong docstring, README, commit message. Nếu chưa có test, viết "scaffold — chưa implement", đúng văn hoá đã có.
3. **Không tăng phạm vi (scope) của một slice đang làm dở.** Nếu phát hiện việc phát sinh, ghi vào một mục "Phát hiện thêm — để slice sau" trong ghi chú tiến độ (Mục 8), không tự ý làm luôn.
4. **Không được để lớp diễn đạt ngôn ngữ (khuôn mẫu HAY mô hình ngôn ngữ cục bộ) tự quyết định một sự kiện/xác suất/quan hệ nhân quả mới.** Mọi khẳng định sự kiện trong câu trả lời phải truy ngược được về một `ReasoningResult` cụ thể (xem file roadmap, Phần 8.1). Vi phạm điều này là vi phạm nghiêm trọng nhất trong toàn bộ danh sách.
5. **Không thêm dependency ngoài (crate mới, mô hình ngôn ngữ, v.v.) mà không ghi lại lý do trong một ADR mới**, đúng số thứ tự tiếp theo sau ADR gần nhất đang có trong `docs/adr/`.
6. **Không đổi định dạng đã "implemented and tested" (ví dụ checkpoint-v1) trừ khi bắt buộc** — nếu bắt buộc đổi, phải viết migration path tường minh (Mục 7.6) và test round-trip cho cả định dạng cũ lẫn mới.
7. **Luôn chạy đủ ba lệnh sau trước khi coi bất kỳ thay đổi nào là "xong"**, không được bỏ qua bất kỳ lệnh nào:
   ```bash
   cargo test --workspace
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo fmt --all -- --check
   ```
8. **Môi trường mục tiêu là CPU-only, 8 GB RAM (i7-7700K).** Bất kỳ thay đổi nào làm tăng đáng kể RAM/CPU footprint phải đi kèm số đo thật (không ước lượng) đối chiếu bảng ngân sách ở file roadmap Phần 9.

---

## 3. GIAO THỨC VÒNG LẶP THỰC THI — "KIÊN TRÌ ĐẾN KHI THÀNH CÔNG", ĐÚNG NGHĨA KỸ THUẬT

Yêu cầu gốc là "vòng lặp vô tận cho đến khi thành công". Đây là bản dịch đúng nghĩa kỹ thuật của yêu cầu đó — **kiên trì không giới hạn số lần thử, nhưng mỗi lần thử phải khác lần trước** (không lặp lại y nguyên một hành động đã thất bại — đó không phải "kiên trì", đó là treo máy). Một vòng lặp thực sự hữu ích trông như sau:

```
VÒNG LẶP CHÍNH (áp dụng cho MỌI slice, MỌI lần sửa lỗi):

  lần_thử = 0
  cách_đã_thử = []

  LẶP:
    lần_thử += 1
    chạy: cargo test --workspace && cargo clippy ... && cargo fmt --check

    NẾU tất cả lệnh trên exit code 0:
        → THÀNH CÔNG. Thoát vòng lặp. Sang bước "dọn dẹp & ghi chú" (Mục 4, bước E).

    NẾU thất bại:
        đọc kỹ TOÀN BỘ output lỗi (không chỉ dòng cuối)
        xác định: đây có phải CÙNG một lỗi với lần thử trước không?

        NẾU cùng lỗi VÀ cách sửa lần này giống hệt cách đã thử:
            → DỪNG LẠI. Đây không phải kiên trì, đây là vòng lặp vô ích.
            → Viết chẩn đoán vào docs/blockers/<tên-slice>.md:
              - Lỗi chính xác (copy nguyên văn)
              - Danh sách MỌI cách đã thử và vì sao mỗi cách thất bại
              - Giả thuyết về nguyên nhân gốc (root cause), dù chưa chắc chắn
            → Nếu có slice ĐỘC LẬP khác chưa làm, chuyển sang làm slice đó
              trong lúc chờ, thay vì đứng yên.
            → Nếu KHÔNG có slice độc lập nào khác, dừng và báo cáo người
              dùng theo đúng Mục 9 (đừng tự đoán mò thêm nữa — im lặng thử
              random không phải "kiên trì", nó là lãng phí).

        NẾU khác lỗi, hoặc cùng lỗi nhưng có giả thuyết sửa MỚI chưa thử:
            ghi cách_đã_thử.append(cách_vừa_thử)
            áp dụng cách sửa mới, có chủ đích, dựa trên việc ĐỌC KỸ thông
            báo lỗi (số dòng, tên kiểu, trait bound...) — không đoán mò
            → LẶP LẠI từ đầu vòng lặp
```

**Vì sao thiết kế thế này, không phải "lặp lại vô hạn không điều kiện":** một vòng lặp thật sự vô hạn, không có cơ chế nhận biết "mình đang giẫm chân tại chỗ", sẽ (a) đốt tài nguyên/thời gian vô ích trên một lỗi cần con người quyết định (ví dụ: chọn mô hình ngôn ngữ nào — đây là quyết định sản phẩm/pháp lý về giấy phép, không phải lỗi kỹ thuật để "thử tiếp"), và (b) có rủi ro agent tự đánh lừa chính nó rằng đã "xong" để thoát vòng lặp (ví dụ: xoá test đang fail thay vì sửa code — **TUYỆT ĐỐI CẤM**: không được xoá, comment-out, hay làm yếu đi (`#[ignore]`) một test đang thất bại để vòng lặp "qua được" bước kiểm tra). Kiên trì thật sự nghĩa là **không bỏ cuộc sau một lần thử**, và **biết phân biệt "cần thử cách khác" với "cần một quyết định mà chỉ con người mới đưa ra được"** — không phải lặp mù không suy nghĩ.

### 3.1. Việc gì được lặp "gần như vô hạn" một cách hợp lý

- Lỗi compile (type mismatch, trait bound thiếu, lifetime) — luôn có thể sửa được bằng cách đọc kỹ thông báo lỗi của `rustc`, hầu như không bao giờ cần dừng lại hỏi người dùng.
- Test logic sai (assertion fail vì thuật toán viết sai) — tiếp tục debug bằng cách thêm `dbg!()`/`println!()` tạm thời, thu hẹp phạm vi lỗi, sửa, chạy lại. Loại bỏ mọi debug print tạm trước khi commit.
- Test flaky (thất bại ngẫu nhiên) — thường do RNG không seed cố định; sửa tận gốc (seed cố định mọi test dùng ngẫu nhiên, đúng pattern `rand_chacha`/`ChaCha8Rng` đã dùng trong `omiai-world`), không phải chạy lại tới khi may mắn pass.
- Clippy warning — luôn sửa được, không có lý do hợp lệ để dừng lại hỏi vì việc này.

### 3.2. Việc gì KHÔNG được tự lặp/tự quyết — phải dừng và hỏi (chi tiết đầy đủ ở Mục 9)

- Chọn mô hình ngôn ngữ cụ thể + chấp nhận giấy phép của nó (Slice 10).
- Bất kỳ thay đổi nào phá vỡ khả năng tương thích ngược của checkpoint-v1 đã "implemented and tested".
- Bất kỳ lúc nào bảng ngân sách RAM (file roadmap, Phần 9) bị vượt quá sau khi đo thật.

---

## 4. QUY TRÌNH LÀM VIỆC CHO MỖI SLICE (áp dụng tuần tự cho Slice 7 → 15, xem Mục 5)

Mỗi slice — không ngoại lệ — đi qua đúng 5 bước theo thứ tự này. Không được nhảy bước B (viết test trước) để đi thẳng vào bước C, kể cả khi bạn "chắc chắn" cách implement.

**A. ĐỌC.** Đọc lại các file liên quan trực tiếp đến slice này (đã liệt kê tương ứng ở Mục 5) + phần lý giải kỹ thuật tương ứng trong file roadmap. Không giả định API của một crate khác — mở file, đọc chữ ký hàm thật.

**B. VIẾT TEST TRƯỚC (test-first).** Viết trước danh sách các trường hợp test sẽ định nghĩa "slice này xong" — bao gồm ít nhất: 1 test hạnh phúc (happy path), 1 test biên (edge case: input rỗng, giá trị 0, danh sách rỗng), 1 test lỗi được xử lý đúng cách (không phải panic). Với bất kỳ định dạng serialize nào (đặc biệt `.omiai` — xem Mục 7), bắt buộc thêm test round-trip. Viết các test này TRƯỚC, để chúng đỏ (fail vì chưa có implementation) — đây là bằng chứng test thật sự kiểm tra được điều gì đó, không phải test viết sau để khớp với code.

**C. TRIỂN KHAI.** Viết implementation tối thiểu để test ở bước B chuyển xanh. Không thêm tính năng ngoài phạm vi test đã viết (tránh over-engineering làm chậm vòng lặp).

**D. VÒNG LẶP KIỂM TRA — SỬA LỖI.** Áp dụng đúng giao thức Mục 3 cho tới khi cả ba lệnh ở Quy tắc bất biến #7 đều xanh.

**E. DỌN DẸP, GHI CHÚ, BÁO CÁO.**
   - Xoá mọi `dbg!()`/`println!()` debug tạm.
   - Cập nhật bảng trạng thái trong `README.md` gốc — CHỈ đánh dấu "tested" nếu bước D vừa xanh thật.
   - Nếu slice này tạo ra một quyết định kiến trúc đáng ghi nhớ (không phải chi tiết vụn vặt), viết một ADR mới, số thứ tự tiếp theo.
   - Nếu dùng git: commit với message dạng `slice-N: <mô tả ngắn> (+X test, Y dòng)`.
   - Viết báo cáo tiến độ theo đúng mẫu ở Mục 8.
   - **Chỉ sau khi hoàn tất mọi ý trên** mới được chuyển sang slice tiếp theo.

---

## 5. PHÂN RÃ NHIỆM VỤ CHI TIẾT THEO SLICE

Thực thi ĐÚNG THỨ TỰ. Không bắt đầu slice N+1 khi slice N chưa qua bước E ở Mục 4. Với lý do đầy đủ của mỗi slice, xem file roadmap Phần 7 — dưới đây là bản mệnh lệnh hoá, dạng việc-cần-làm cụ thể theo file.

### SLICE 7 — Nối dây 8 trụ cột vào ChatEngine
- [ ] Sửa `crates/omiai-io/Cargo.toml`: chuyển `omiai-knowledge`, `omiai-probabilistic`, `omiai-causal`, `omiai-neuro`, `omiai-world` từ `[dev-dependencies]` sang `[dependencies]`.
- [ ] Chạy `cargo tree -p omiai-io -e no-dev` NGAY sau khi sửa — nếu thấy vòng phụ thuộc, dừng, đọc `docs/adr/0005-io-meta-cycle.md` (đã giải quyết vấn đề tương tự trước đây), áp dụng pattern tương tự.
- [ ] Tạo `crates/omiai-io/src/router.rs` với `enum ReasoningResult` + `struct DialogueRouter` (khung sườn đầy đủ ở file roadmap Phần 8.1 — copy khung, KHÔNG copy các thân hàm còn đánh dấu placeholder, tự viết chúng).
- [ ] Viết hàm trích subject/predicate thật từ `Formula::Atom` (KHÔNG dùng `format!("{q:?}")` — xem cảnh báo rõ trong roadmap Phần 8.1) — test riêng hàm này trước khi nối vào `KnowledgeGraph::query_path`.
- [ ] Test bắt buộc (tối thiểu 4, một cho mỗi pillar mới): gọi `ChatEngine::handle` với một câu hỏi cần `knowledge::graph`, một câu cần `probabilistic::bayesian`, một câu cần `causal`, và một truy vấn world — assert rằng `ReasoningResult` trả về đúng biến thể tương ứng (không chỉ assert "có trả lời", phải assert ĐÚNG pillar nào trả lời).

### SLICE 8 — Mở rộng semantic parser
- [ ] Sửa `nlp_parser.rs`: khi gặp khái niệm chưa có trong `lexicon_vi`/`lexicon_en`, gọi `KnowledgeGraph::add_concept` thay vì chỉ viết hoa chữ cái đầu (`capitalize()`) rồi bỏ qua.
- [ ] Viết một tầng ngữ pháp dùng `nom` (đã có) cho ít nhất các cấu trúc: phủ định ("X không phải Y"/"X is not Y"), câu hỏi có/không ("có phải X là Y"/"is X a Y"), câu ghép hai mệnh đề nối bằng "và"/"and". Mỗi cấu trúc mới = ít nhất 2 test (một tiếng Việt, một tiếng Anh).
- [ ] Tạo `crates/omiai-io/data/seed_sentences.jsonl` (hoặc định dạng tương đương) chứa 100–150 cặp (câu, Formula mục tiêu ở dạng có thể `assert_eq!` được) viết tay, phủ các mẫu câu ở trên.
- [ ] Viết script/hàm tổ hợp: nhân bản 100–150 cặp gốc lên 300–800 cặp bằng cách thay danh từ/tên riêng bằng các giá trị khác đã có trong ontology — CHIA rõ 80% làm tập train, 20% tập test, KHÔNG để lẫn.
- [ ] (Nếu chọn làm phần tiến hoá luật): dùng `evolution::formula_gp` tối ưu trên tập train, báo cáo % khớp trên tập TEST (chưa từng thấy) trong README — đây là con số bắt buộc phải xuất hiện, không được bỏ qua.

### SLICE 9 — Diễn đạt có bằng chứng
- [ ] Sửa các hàm `realize_*` trong `nlp_parser.rs` để nhận `&ReasoningResult` thay vì các tham số rời rạc hiện tại.
- [ ] Với `ReasoningResult::Proved`, thêm câu liệt kê premise đã dùng (lấy từ trường tương ứng trong `ProofReport`).
- [ ] Với `ReasoningResult::Probabilistic`, LUÔN in số phần trăm cụ thể — viết một test cố ý assert rằng chuỗi trả lời chứa một pattern số (regex `\d+%` hoặc tương đương) để tự động hoá việc kiểm tra "không được nói mơ hồ".
- [ ] Thêm biến thể `ParseIntent::AskWorld`; nối vào `omiai_world::registry`/`communication::vocabulary` (read-only — không được gọi bất kỳ hàm `&mut self` nào của `World` từ đường dẫn chat).
- [ ] Test đa dạng hoá: gọi cùng một input 20 lần, assert ≥ 3 chuỗi output khác nhau NHƯNG mọi con số/tên riêng trích xuất được từ mỗi chuỗi phải giống hệt nhau qua cả 20 lần (viết một hàm trích số/tên riêng dùng chung cho test này và cho Slice 10's grounding test).

### SLICE 10 — (TUỲ CHỌN — xác nhận với người dùng trước khi bắt đầu, xem Mục 9) Pillar ngôn ngữ cục bộ
- [ ] Viết `docs/adr/000X-optional-local-llm-surface-layer.md` (số thứ tự = ADR gần nhất + 1 tại thời điểm thực thi) — dùng mẫu ở roadmap Phần 8.5.
- [ ] Thêm dependency (`llama-cpp-2` HOẶC `candle` + `candle-transformers` — chọn một, ghi lý do trong ADR).
- [ ] Tải một mô hình cụ thể, đã lượng tử hoá GGUF, giấy phép Apache-2.0/MIT — ghi rõ tên/nguồn/sha256/giấy phép vào `language_model_info` ngay từ đầu, không để trống rồi bổ sung sau.
- [ ] Viết prompt template ràng buộc chặt (mẫu cụ thể ở roadmap Slice 10, mục 5) — prompt PHẢI được review lại xem có rò rỉ khả năng "tự bịa" hay không trước khi coi là xong.
- [ ] **Test bắt buộc, không thương lượng: bài test grounding** — 50 câu hỏi mẫu, viết một hàm parse output của mô hình ngôn ngữ để trích mọi con số và tên riêng xuất hiện, so khớp với đúng tập con số/tên riêng có trong `ReasoningResult` đã đưa vào prompt — bất kỳ số/tên nào KHÔNG khớp = test đỏ. Slice này chỉ "xong" khi bài test này chạy 50/50 xanh nhiều lần liên tiếp (không phải một lần ăn may).
- [ ] Đo RAM thật (không ước lượng) bằng `/usr/bin/time -v` khi mô hình đang nạp + chạy song song với world simulation; đối chiếu bảng ngân sách roadmap Phần 9.

### SLICE 11 — `omiai-export`
- [ ] Viết `docs/format-spec/bundle-v1.md` — dùng ĐÚNG đặc tả ở Mục 7 dưới đây (không viết khác đi).
- [ ] Implement theo đúng thuật toán đóng gói tất định ở Mục 7.2 (thứ tự file, mtime=0, v.v. — đây không phải chi tiết tuỳ chọn).
- [ ] Test bắt buộc: xem đầy đủ chiến lược test ở Mục 7.5 (golden fixture, round-trip, property-based, corruption tests theo từng biến thể lỗi).

### SLICE 12 — `omiai-runtime`
- [ ] Implement `OmiaiModel::load()` theo đúng thuật toán xác thực ở Mục 7.3 — implement đúng thứ tự kiểm tra đã liệt kê, không đảo thứ tự (thứ tự đó được thiết kế để fail sớm, tránh giải mã payload nặng trước khi xác nhận file toàn vẹn).
- [ ] Implement `step()` gọi `DialogueRouter` (Slice 7) rồi lớp diễn đạt (Slice 9/10).
- [ ] Build thành công cho cả 3 đích: native, `cdylib`, `wasm32-unknown-unknown` (dùng `cargo build --target wasm32-unknown-unknown`). `wasm32-wasi` build riêng nếu thời gian cho phép — ghi rõ trong báo cáo tiến độ nếu tạm hoãn.

### SLICE 13 — `omiai-serve`
- [ ] Implement `axum` server theo khung ở roadmap Phần 8.4.
- [ ] Test: dùng `reqwest` hoặc tương đương trong `tests/` để gọi `POST /infer` thật qua HTTP (không chỉ gọi hàm Rust trực tiếp) — đây là integration test thật, chứng minh server thật sự chạy được, không chỉ compile được.

### SLICE 14 — `omiai-cli`
- [ ] Subcommand `train`, `resume`, `export`, `bench`, `chat` (REPL), `serve` — dùng `clap` (đã có sẵn dependency).
- [ ] `chat` REPL PHẢI hoạt động với ví dụ ở Mục 1 (mục tiêu tối hậu) — chạy thử chính xác kịch bản đó, copy output thật vào báo cáo tiến độ cuối cùng.

### SLICE 15 — Kiểm thử đầu-cuối & demo
- [ ] `tests/` ở root: kịch bản (a)-(f) đã liệt kê ở roadmap Phần 7 Slice 15 — viết như MỘT test dài duy nhất mô phỏng một phiên hội thoại thật, không phải nhiều test rời rạc.
- [ ] `examples/world_demo.rs`, `examples/communication_demo.rs`.
- [ ] Cập nhật README: bảng trạng thái, số lượng test mới, build order (đánh dấu ✓ cho 9-14 nếu xong).

---

## 6. TIÊU CHUẨN "TEST HIỆU QUẢ" — QUY TẮC CHỌN LOẠI TEST ĐÚNG

Viết test không phải để đạt độ phủ (coverage) cao — viết test để **một thay đổi sai trong tương lai chắc chắn bị bắt lỗi**. Dùng đúng loại test cho đúng loại rủi ro:

| Loại rủi ro | Loại test bắt buộc | Ví dụ cụ thể trong repo này |
|---|---|---|
| Một pillar có API đúng nhưng KHÔNG được gọi từ đường dẫn thật (đúng lỗi đã phát hiện ở Slice 7 trước nâng cấp) | **Integration test end-to-end**, không phải unit test nội bộ pillar | `ChatEngine::handle()` → assert `ReasoningResult` đến từ đúng pillar mong đợi |
| Bất biến toán học phải đúng với MỌI input hợp lệ, không chỉ vài ví dụ tay | **Property-based test** (`proptest`, đã là dependency sẵn có) | CNF giữ nguyên giá trị chân lý; export→import bundle giữ nguyên trạng thái |
| Định dạng serialize sẽ được đọc lại bởi code viết sau, có thể khác version | **Golden-file / fixture test**: một file cố định commit vào repo, KHÔNG generate lại mỗi lần chạy test | `tests/fixtures/minimal_v1.omiai` (chi tiết Mục 7.5) |
| Input đến từ người dùng, không kiểm soát được nội dung | **Fuzz test** (khuyến nghị `cargo-fuzz`) trên `tokenizer::tokenize` và `nlp_parser::parse_message` — mục tiêu: không panic với BẤT KỲ chuỗi UTF-8 nào, kể cả chuỗi rỗng, chỉ toàn dấu câu, hay input cực dài |
| Một con số "độ chính xác X%" được tuyên bố công khai (Slice 8, Slice 10) | **Regression test in với số cụ thể**, tách biệt tập train/test, chạy lại được bởi người khác | Test parser trên tập 20% chưa từng thấy; test grounding 50 câu |
| Hành vi ngẫu nhiên (RNG, reservoir, world simulation) | **Seed cố định, không bao giờ dùng `thread_rng()` trong test** | Theo đúng pattern `ChaCha8Rng` đã dùng trong `omiai-world` (ADR-0006) |
| Lỗi có thể xảy ra khi nạp dữ liệu KHÔNG đáng tin (file bundle bị hỏng/chỉnh tay) | **Một test riêng cho MỖI biến thể lỗi** trong enum error, không gộp nhiều trường hợp vào một test | Mỗi variant của `BundleError` (Mục 7.4) = ít nhất 1 test cố ý tạo ra đúng lỗi đó |

**Quy tắc vàng khi review lại test của chính mình trước khi coi slice là xong:** đọc từng test, tự hỏi "nếu tôi cố tình phá code ở chỗ test này đang kiểm tra, test có đỏ không?". Nếu câu trả lời là "không chắc" — test đó chưa đủ tốt, viết lại.

---

## 7. ĐẶC TẢ FILE `.omiai` — PHIÊN BẢN CHÍNH XÁC TUYỆT ĐỐI

Đây là phần quan trọng nhất của toàn bộ chỉ thị này. Mọi con số, thứ tự, và tên trường dưới đây là **bắt buộc, không phải gợi ý** — nếu implementation khác đi dù chỉ một chi tiết (ví dụ thứ tự file trong tar, hay tên trường JSON), nó không còn là "bundle-v1" nữa. Nội dung này CHÍNH LÀ nội dung cần chép vào `docs/format-spec/bundle-v1.md`.

### 7.1. Quan hệ với checkpoint — không được nhầm lẫn

`.omiai` **không phải** một dạng khác của checkpoint. Checkpoint là thư mục nhiều file, dùng để tiếp tục huấn luyện/mô phỏng, giữ đầy đủ RNG state để resume đúng quỹ đạo. Bundle `.omiai` là **một file archive duy nhất**, dùng để triển khai suy luận, đã cắt tỉa, không cần RNG state của quá trình tiến hoá. Một `omiai-export` nhận đường dẫn tới một checkpoint (thư mục) làm đầu vào và sinh ra một file `.omiai` làm đầu ra — không bao giờ đi ngược lại.

### 7.2. Thuật toán đóng gói tất định (deterministic construction) — thực thi ĐÚNG thứ tự

```
HÀM export_bundle(checkpoint_dir, output_path, capabilities_muốn_bật):

  BƯỚC 1 — Thu thập payload:
    Với mỗi pillar có cờ = true trong capabilities_muốn_bật:
      đọc dữ liệu tương ứng từ checkpoint_dir, CẮT TỈA theo đúng bảng ở
      §7.1 (bỏ lịch sử tiến hoá, chỉ giữ top-N + tóm tắt cho evolution;
      bỏ toàn bộ lịch sử checkpoint, chỉ giữ trạng thái hiện tại cho world).
      Nếu một pillar có cờ = true nhưng KHÔNG có dữ liệu thật để xuất →
      DỪNG NGAY, lỗi (không được xuất bundle với cờ nói dối).

  BƯỚC 2 — Với mỗi file payload thu được ở Bước 1:
      tính BLAKE3(nội dung bytes thô) → chuỗi hex 64 ký tự thường.

  BƯỚC 3 — Sắp xếp:
      sắp danh sách (đường_dẫn, blake3) THEO THỨ TỰ TỪ ĐIỂN byte-wise
      trên đường_dẫn (dùng so sánh &str mặc định của Rust — ổn định,
      không phụ thuộc locale). Thứ tự này dùng LẠI y hệt cho thứ tự
      entry trong tar ở Bước 5 — đây là điều kiện để cùng một tập file
      luôn sinh ra tar byte-for-byte giống nhau bất kể thứ tự đọc từ
      hệ điều hành.

  BƯỚC 4 — Dựng manifest.json:
      Điền đúng schema ở §7.4. Trường "files" = danh sách đã sắp ở Bước 3.
      Serialize bằng serde với struct có field theo ĐÚNG thứ tự khai báo
      trong §7.4 (không dùng HashMap cho phần cấu trúc cố định — dùng
      struct thường hoặc IndexMap nếu cần phần động), 2-space indent,
      UTF-8 không BOM, kết thúc bằng một dòng trống.
      *** manifest.json KHÔNG tự liệt kê chính nó trong mảng "files" ***
      (đúng tiền lệ đã có ở checkpoint-v1.md — không tạo ngoại lệ mới).

  BƯỚC 5 — Dựng tar (định dạng ustar/POSIX, không cần PAX vì mọi
            đường dẫn đều ngắn):
      Entry ĐẦU TIÊN, luôn luôn: "manifest.json".
      Sau đó, các entry theo ĐÚNG thứ tự đã sắp ở Bước 3.
      Mọi tar header: mtime=0, uid=0, gid=0, uname="", gname="",
      mode=0o644. KHÔNG tạo entry thư mục riêng (ví dụ không có entry
      "reservoir/") — dùng tên entry có "/" như một chuỗi phẳng, ví dụ
      "reservoir/weights.safetensors" là MỘT entry, không phải một
      thư mục chứa entry con.

  BƯỚC 6 — Nén:
      Nén toàn bộ byte stream tar bằng zstd. Mức nén mặc định = 19 (cân
      bằng kích thước/thời gian cho bundle phát hành); expose cờ
      `--fast` dùng mức 3 cho vòng lặp phát triển cục bộ.
      (Lưu ý: file kết quả sẽ tự nhiên có magic bytes chuẩn của zstd
      — 28 B5 2F FD ở 4 byte đầu. KHÔNG thêm magic byte tuỳ biến riêng
      lên trước — làm vậy phá khả năng các công cụ zstd chuẩn (`zstd`,
      `file`, thư viện zstd của ngôn ngữ khác) nhận diện đúng file.
      `manifest.json` bên trong đã đủ vai trò "khai báo schema".)

  BƯỚC 7 — Ghi ra output_path.

  BƯỚC 8 — TỰ KIỂM CHỨNG (bắt buộc, không phải tuỳ chọn):
      Gọi NGAY hàm load_bundle(output_path) (§7.3) trên chính file vừa
      ghi. Nếu load thất bại vì BẤT KỲ lý do gì → export_bundle() PHẢI
      trả lỗi, KHÔNG được coi export là thành công. Đây là cách duy
      nhất đảm bảo "cầm đèn soi chính đường mình vừa đi qua" thay vì
      chỉ tin thuật toán viết đúng trên lý thuyết.

  BƯỚC 9 — Trả về BundleManifest đã dựng (để caller — ví dụ omiai-cli
      export — in ra tóm tắt cho người dùng: kích thước file, các
      capability đã bật).
```

### 7.3. Thuật toán xác thực & nạp (validation/load) — thực thi ĐÚNG thứ tự, dừng ở lỗi ĐẦU TIÊN gặp phải

```
HÀM load_bundle(path) -> Result<OmiaiModel, BundleError>:

  BƯỚC 1 — Mở file, giải nén zstd dạng streaming.
      Lỗi framing zstd → BundleError::NotZstdData.

  BƯỚC 2 — Đọc entry tar ĐẦU TIÊN.
      Tên khác "manifest.json" → BundleError::ManifestNotFirstEntry(tên_thật).
      (Cố ý nghiêm ngặt vị trí — cho phép một reader dạng streaming xác
      thực capability TRƯỚC khi phải đọc hết toàn bộ file, quan trọng
      khi bundle có mô hình ngôn ngữ nặng hàng trăm MB.)

  BƯỚC 3 — Parse JSON, đối chiếu schema §7.4.
      Thiếu trường bắt buộc / sai kiểu → BundleError::ManifestSchemaError(chi tiết).

  BƯỚC 4 — Kiểm tra format_version.
      Không phải 1 (hoặc lớn hơn version runtime này biết) →
      BundleError::UnsupportedFormatVersion(số_tìm_thấy).
      KHÔNG được cố "đoán" parse tiếp một version không nhận ra.

  BƯỚC 5 — Với MỖI entry tar còn lại:
      5a. Kiểm tra đường dẫn: không chứa "..", không bắt đầu bằng "/",
          không escape ra ngoài thư mục gốc bundle (chống path
          traversal / "tar-slip") → vi phạm: BundleError::UnsafePath(đường_dẫn).
      5b. Tính BLAKE3(bytes đọc được), so với giá trị khai trong
          manifest.files cho đường dẫn này → không khớp:
          BundleError::HashMismatch { path }.
      5c. Đánh dấu đường dẫn này là "đã thấy".

  BƯỚC 6 — Đối chiếu tập hợp:
      Đường dẫn có trong manifest.files nhưng KHÔNG thấy trong tar →
      BundleError::MissingDeclaredFile(đường_dẫn).
      Đường dẫn thấy trong tar HAI LẦN → BundleError::DuplicateFile(đường_dẫn).
      Đường dẫn thấy trong tar nhưng KHÔNG có trong manifest.files (và
      khác "manifest.json") → BundleError::UndeclaredFile(đường_dẫn).

  BƯỚC 7 — Đối chiếu capabilities với payload thật:
      capabilities.X = false NHƯNG tồn tại entry có tiền tố "X/" →
      BundleError::CapabilityMismatch { flag: "X", reason: "false nhưng có payload" }.
      capabilities.X = true NHƯNG thiếu entry bắt buộc của X (theo bảng
      cố định — xem danh sách file bắt buộc mỗi pillar ở §7.4) →
      BundleError::CapabilityMismatch { flag: "X", reason: "true nhưng thiếu payload" }.
      capabilities.language_model = true NHƯNG language_model_info = null →
      BundleError::ManifestSchemaError("language_model bật nhưng thiếu language_model_info").

  BƯỚC 8 — CHỈ SAU KHI Bước 1–7 đều qua: giải mã từng payload CBOR/
      safetensors thành cấu trúc trong bộ nhớ (KnowledgeGraph,
      BayesianNetwork, CausalDag, Reservoir, ...).
      Lỗi giải mã → BundleError::PayloadDeserializeError { path, chi tiết }.

  BƯỚC 9 — Trả về OmiaiModel đã dựng đầy đủ.
```

**Vì sao thứ tự này quan trọng, không được đảo:** Bước 5–7 (xác thực toàn vẹn + khớp capability) chạy TRƯỚC Bước 8 (giải mã payload nặng) — một file bị hỏng hay bị chỉnh tay sai sẽ bị từ chối ngay, KHÔNG lãng phí thời gian/RAM giải mã một mô hình ngôn ngữ hàng trăm MB rồi mới phát hiện file đó không hợp lệ.

### 7.4. JSON Schema đầy đủ cho `manifest.json` (chép nguyên vào `docs/format-spec/bundle-v1.md`)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://omiai.local/schema/bundle-manifest-v1.json",
  "title": "OmiAI bundle manifest v1",
  "type": "object",
  "additionalProperties": false,
  "required": ["format_version", "schema", "created_utc", "capabilities", "entrypoint", "files"],
  "properties": {
    "format_version": { "const": 1 },
    "schema": { "const": "omiai-bundle" },
    "created_utc": { "type": "string", "format": "date-time" },
    "source_checkpoint_step": { "type": ["integer", "null"], "minimum": 0 },
    "git_commit": { "type": ["string", "null"], "pattern": "^[0-9a-f]{40}$" },
    "capabilities": {
      "type": "object",
      "additionalProperties": false,
      "required": ["logic", "knowledge_graph", "probabilistic", "causal", "reservoir", "world_query", "language_model"],
      "properties": {
        "logic": { "type": "boolean" },
        "knowledge_graph": { "type": "boolean" },
        "probabilistic": { "type": "boolean" },
        "causal": { "type": "boolean" },
        "reservoir": { "type": "boolean" },
        "world_query": { "type": "boolean" },
        "language_model": { "type": "boolean" }
      }
    },
    "language_model_info": {
      "type": ["object", "null"],
      "required": ["name", "quantization", "license", "source_url", "sha256", "role", "may_assert_unverified_facts"],
      "properties": {
        "name": { "type": "string", "minLength": 1 },
        "quantization": { "type": "string" },
        "license": { "type": "string" },
        "source_url": { "type": "string", "format": "uri" },
        "sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
        "role": { "const": "surface_realization_only" },
        "may_assert_unverified_facts": { "const": false }
      }
    },
    "entrypoint": {
      "type": "object",
      "required": ["function", "input_schema", "output_schema"],
      "properties": {
        "function": { "const": "step" },
        "input_schema": { "const": "InferInput_v1" },
        "output_schema": { "const": "InferOutput_v1" }
      }
    },
    "files": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "blake3"],
        "additionalProperties": false,
        "properties": {
          "path": { "type": "string", "pattern": "^[^/\\\\][^\\\\]*[^/\\\\]$|^[^/\\\\]$" },
          "blake3": { "type": "string", "pattern": "^[0-9a-f]{64}$" }
        }
      }
    }
  }
}
```

**Bảng file bắt buộc theo từng capability** (dùng cho kiểm tra ở Bước 7, §7.3 — mọi đường dẫn tương đối từ gốc bundle, LUÔN dùng `/`, kể cả khi build trên Windows):

| capability | file/thư mục bắt buộc nếu = true |
|---|---|
| `logic` | `logic/seed_facts.cbor` |
| `knowledge_graph` | `knowledge_graph/graph.cbor` |
| `probabilistic` | `probabilistic/networks.cbor` |
| `causal` | `causal/dag.cbor` |
| `reservoir` | `reservoir/weights.safetensors` |
| `world_query` | `world_snapshot/grid.bin`, `world_snapshot/agents.cbor`, `world_snapshot/vocabulary.cbor` |
| `language_model` | `language_model/model.gguf`, `language_model/tokenizer.json`, `language_model/model_card.json` |

### 7.5. Bảng lỗi đầy đủ + chiến lược test bắt buộc cho mỗi lỗi

```rust
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("không phải dữ liệu zstd hợp lệ")]
    NotZstdData,
    #[error("entry đầu tiên phải là manifest.json, thấy '{0}'")]
    ManifestNotFirstEntry(String),
    #[error("manifest.json không khớp schema: {0}")]
    ManifestSchemaError(String),
    #[error("format_version {0} không được runtime này hỗ trợ")]
    UnsupportedFormatVersion(u32),
    #[error("đường dẫn không an toàn trong archive: {0}")]
    UnsafePath(String),
    #[error("hash BLAKE3 không khớp cho file: {path}")]
    HashMismatch { path: String },
    #[error("file khai báo trong manifest nhưng thiếu trong archive: {0}")]
    MissingDeclaredFile(String),
    #[error("file xuất hiện hai lần trong archive: {0}")]
    DuplicateFile(String),
    #[error("file có trong archive nhưng không khai báo trong manifest: {0}")]
    UndeclaredFile(String),
    #[error("capability '{flag}' không khớp dữ liệu thật: {reason}")]
    CapabilityMismatch { flag: &'static str, reason: String },
    #[error("lỗi giải mã payload tại {path}: {detail}")]
    PayloadDeserializeError { path: String, detail: String },
    #[error("lỗi I/O: {0}")]
    Io(#[from] std::io::Error),
}
```

**Với MỖI biến thể trên, viết ít nhất một test cố ý tạo ra đúng lỗi đó** (ví dụ: lấy fixture hợp lệ, sửa 1 byte trong một payload → phải nhận đúng `HashMismatch`, không phải panic hay lỗi khác; xoá một entry khai báo trong manifest → phải nhận đúng `MissingDeclaredFile`; đặt `capabilities.language_model = false` nhưng vẫn nhét thư mục `language_model/` vào tar → phải nhận đúng `CapabilityMismatch`). Đây là danh sách test tối thiểu cho Slice 11 — không được coi Slice 11 xong nếu thiếu bất kỳ dòng nào trong bảng này.

### 7.6. Chiến lược test tổng thể cho định dạng bundle (bắt buộc cả 4 lớp, không chỉ chọn một)

1. **Golden fixture test:** tạo MỘT file `tests/fixtures/minimal_v1.omiai` thật nhỏ (chỉ `capabilities.logic = true`, 2-3 fact), **commit thẳng vào repo dưới dạng binary**, không sinh lại mỗi lần chạy test. Viết test `load_bundle()` trên đúng file tĩnh này — tách biệt hoàn toàn khỏi code `export_bundle()`, để một lỗi ở writer không thể vô tình che giấu lỗi ở reader (và ngược lại).
2. **Round-trip test cụ thể:** `export_bundle()` một checkpoint mẫu tự tạo trong test → `load_bundle()` lại → so sánh từng trường trạng thái trong bộ nhớ với trạng thái nguồn, dùng `assert_eq!` trên từng pillar, không so sánh gộp bằng Debug string.
3. **Property-based test** (dùng `proptest`, đã có sẵn dependency trong workspace): sinh ngẫu nhiên (nhưng hợp lệ) các trạng thái checkpoint nhỏ, khẳng định export→import luôn bảo toàn trạng thái, với ít nhất 256 case ngẫu nhiên mỗi lần chạy CI.
4. **Corruption test theo từng biến thể lỗi:** xem bảng ở §7.5 — mỗi dòng, một test.

### 7.7. Chính sách versioning

Khi cần `format_version = 2` trong tương lai: định nghĩa `ManifestV2` là **struct RIÊNG**, không sửa `ManifestV1` thành optional-field chồng chất. Viết `fn upgrade_v1_to_v2(v1: ManifestV1) -> ManifestV2` thuần (pure function), có test round-trip riêng cho chính hàm nâng cấp này. `load_bundle()` phiên bản mới đọc `format_version` trước (chỉ parse đúng trường đó, chưa parse toàn bộ), rẽ nhánh sang parser đúng version, rồi gọi chuỗi hàm nâng cấp nếu cần — loader mới VẪN đọc được bundle v1 cũ, không bao giờ được phép "bỏ hỗ trợ" version cũ mà không thông báo rõ trong changelog.

---

## 8. ĐỊNH DẠNG BÁO CÁO TIẾN ĐỘ (dùng sau MỖI slice, đúng bước E ở Mục 4)

```markdown
### Slice N — <tên> — [XONG / ĐANG LÀM / TẮC NGHẼN]

- Test: +X test mới, X/X pass. Lệnh đã chạy: cargo test --workspace (kèm dòng cuối output thật)
- Clippy: sạch / N warning đã sửa
- Số liệu công khai (nếu có): <ví dụ: độ chính xác parser 87% trên tập test 160 câu chưa từng thấy>
- Quyết định kiến trúc mới (nếu có): ADR-000X — <tên>
- Phát hiện thêm, để dành slice sau: <liệt kê hoặc "không có">
- Nếu TẮC NGHẼN: dán nguyên văn lỗi + mọi cách đã thử (xem docs/blockers/<slice>.md)
```

Không dùng ngôn ngữ mơ hồ như "có vẻ hoạt động tốt" — luôn thay bằng con số/lệnh cụ thể để người đọc tự chạy lại kiểm chứng, đúng văn hoá đã có của dự án.

---

## 9. KHI NÀO PHẢI DỪNG LẠI VÀ HỎI NGƯỜI DÙNG (không tự quyết, không tự lặp thêm)

Dừng NGAY và hỏi rõ ràng, cụ thể (không hỏi chung chung "tôi nên làm gì tiếp") trong các trường hợp:

1. **Trước khi bắt đầu Slice 10** — đây là quyết định sản phẩm (có muốn giữ 100% triết lý "zero-training" hay chấp nhận tái sử dụng trọng số ngoài?), không phải quyết định kỹ thuật thuần tuý. Hỏi rõ: "Bạn có muốn bật pillar ngôn ngữ cục bộ (Slice 10)? Nếu có, ưu tiên mô hình nhỏ nhất (tiết kiệm RAM, kém trôi chảy hơn) hay mô hình lớn hơn trong ngân sách 8GB (trôi chảy hơn, ít RAM dư hơn cho world simulation)?"
2. **Khi chọn mô hình ngôn ngữ cụ thể** — trình bày 2-3 lựa chọn cụ thể kèm đánh đổi (kích thước/RAM, giấy phép, chất lượng đa ngôn ngữ Việt-Anh) thay vì tự chọn một cái.
3. **Khi vòng lặp Mục 3 gặp cùng một lỗi 2 lần với cách sửa khác nhau mà vẫn thất bại** — đây là tín hiệu có thể vấn đề nằm ở giả định thiết kế (ví dụ một API không đúng như tài liệu này giả định), không phải lỗi cú pháp đơn thuần.
4. **Khi một thay đổi sẽ phá vỡ khả năng đọc checkpoint-v1/bundle-v1 cũ** — không bao giờ tự quyết định "phá vỡ tương thích ngược thì ổn" mà không hỏi.
5. **Khi đo RAM thật vượt ngân sách ở bảng roadmap Phần 9** — hỏi có nên giảm kích thước mô hình ngôn ngữ, giảm kích thước reservoir, hay giảm phạm vi world simulation.

---

## 10. TIÊU CHÍ HOÀN THÀNH TOÀN BỘ NHIỆM VỤ (definition of done, cấp toàn dự án)

Nhiệm vụ CHỈ được coi là hoàn thành xuất sắc khi TẤT CẢ các điều sau đều đúng, đồng thời, có thể kiểm chứng bằng lệnh cụ thể:

```
[ ] cargo test --workspace  → 100% pass, số lượng test ≥ số ban đầu (358) + toàn
    bộ test mới của Slice 7-15 (ước tính hợp lý: +80-150 test mới)
[ ] cargo clippy --workspace --all-targets --all-features -- -D warnings  → sạch
[ ] cargo fmt --all -- --check  → sạch
[ ] Một file model.omiai tồn tại thật trên đĩa, tạo bằng
    `cargo run -p omiai-cli -- export`, có thể nạp lại bằng
    `cargo run -p omiai-cli -- chat --bundle model.omiai` và tái hiện ĐÚNG
    kịch bản ví dụ ở Mục 1 (copy-paste output thật vào báo cáo cuối cùng,
    không mô tả bằng lời)
[ ] docs/format-spec/bundle-v1.md tồn tại, khớp § 7 byte-for-byte về mặt
    cấu trúc/schema
[ ] README.md gốc: bảng trạng thái cập nhật trung thực, build order 9-14
    được đánh dấu ✓ đúng những gì thật sự xong (không đánh dấu trước)
[ ] Mọi ADR mới (nếu có, đặc biệt Slice 10) đã được viết, đánh số đúng
    thứ tự tiếp nối
[ ] omiai-runtime build thành công cho native + cdylib + ít nhất một
    đích WASM
[ ] omiai-serve trả lời đúng qua HTTP thật (curl/reqwest), không chỉ
    qua lời gọi hàm nội bộ
[ ] KHÔNG còn bất kỳ crate nào ở trạng thái "SCAFFOLD ONLY" trong README
```

---

## 11. CHECKLIST TỔNG — DÁN VÀO ĐẦU PHIÊN LÀM VIỆC CỦA AGENT

```
[ ] Đọc 7 file bắt buộc ở Mục 1
[ ] Chạy cargo test --workspace để xác nhận baseline 358 test (hoặc số
    thật tại thời điểm bắt đầu) — ghi lại con số CHÍNH XÁC trước khi
    sửa bất cứ gì
[ ] Slice 7 → bước A-E Mục 4 → báo cáo Mục 8
[ ] Slice 8 → bước A-E Mục 4 → báo cáo Mục 8
[ ] Slice 9 → bước A-E Mục 4 → báo cáo Mục 8
[ ] DỪNG, hỏi người dùng về Slice 10 (Mục 9, điểm 1-2)
[ ] Slice 10 (nếu người dùng đồng ý) → bước A-E Mục 4 → báo cáo Mục 8
[ ] Slice 11 → bước A-E Mục 4, đặc biệt tuân thủ Mục 7 byte-for-byte
[ ] Slice 12 → bước A-E Mục 4
[ ] Slice 13 → bước A-E Mục 4
[ ] Slice 14 → bước A-E Mục 4
[ ] Slice 15 → bước A-E Mục 4
[ ] Kiểm tra toàn bộ Mục 10 (definition of done) — từng dòng, không bỏ sót
[ ] Báo cáo tổng kết cuối cùng: dán nguyên văn output của kịch bản demo
    ở Mục 1, kèm bảng trạng thái README cuối cùng
```

---

*Hết chỉ thị. File này cố ý cô đọng và mệnh lệnh hoá để một AI coding agent bám theo cơ học, từng bước — mọi lý do "vì sao" đầy đủ nằm ở file `OmiAI-Roadmap-Nang-Cap-Hoi-Thoai.md` đi kèm; agent nên `view` file đó bất cứ khi nào một chỉ thị ở đây chưa đủ rõ để quyết định.*
