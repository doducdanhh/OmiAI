//! Round-trip bit-exact của World qua checkpoint-v1 — test then chốt slice 2:
//! save ở bước N → load → chạy tiếp M bước phải ra CÙNG trạng thái với
//! world chạy liền N+M bước không qua checkpoint.

use omiai_checkpoint::{traits::Checkpointable, verify_dir, FileRecord, Manifest};
use omiai_world::communication::{SignalValue, StateClass};
use omiai_world::ecology::{MIN_BENEFIT_SUPPORT, MIN_EPOCH_SUPPORT};
use omiai_world::world_loop::{World, WorldConfig};

fn config() -> WorldConfig {
    WorldConfig {
        width: 12,
        height: 12,
        n_initial_atoms: 4,
        initial_resources: 0.08,
    }
}

#[test]
fn world_save_load_resume_is_bit_exact() {
    // Thế giới chạy liền 30 bước.
    let mut continuous = World::new(config(), 123);
    for _ in 0..30 {
        continuous.step();
    }

    // Thế giới chạy 10 bước, save, load, chạy tiếp 20 bước.
    let root =
        std::env::temp_dir().join(format!("omiai-wrt-root-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000010");
    std::fs::create_dir_all(&cp).unwrap();

    let mut resumed = World::new(config(), 123);
    for _ in 0..10 {
        resumed.step();
    }
    resumed.save(&cp).expect("save world");
    verify_dir(&cp).expect("manifest + hashes ok");

    let mut loaded = World::load(&cp).expect("load world");
    // Trạng thái ngay sau load khớp trạng thái trước save.
    assert_eq!(loaded.ca.cells, resumed.ca.cells);
    assert_eq!(loaded.atoms, resumed.atoms);
    assert_eq!(loaded.step_count, resumed.step_count);
    assert_eq!(loaded.airwave, resumed.airwave, "airwave sai sau load");
    assert_eq!(loaded.vocabulary, resumed.vocabulary, "vocabulary sai sau load");
    assert_eq!(
        loaded.registry.genomes_in_order(),
        resumed.registry.genomes_in_order()
    );

    for _ in 0..20 {
        resumed.step();
        loaded.step();
    }

    // Bit-exact với thế giới chạy liên tục.
    assert_eq!(loaded.ca.cells, continuous.ca.cells, "grid sai sau resume");
    assert_eq!(loaded.atoms, continuous.atoms, "atoms sai sau resume");
    assert_eq!(loaded.step_count, continuous.step_count);
    assert_eq!(loaded.airwave, continuous.airwave, "airwave sai sau resume");
    assert_eq!(loaded.vocabulary, continuous.vocabulary, "vocabulary sai sau resume");
    assert_eq!(
        loaded.registry.genomes_in_order(),
        continuous.registry.genomes_in_order(),
        "registry sai sau resume"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn tampered_payload_detected() {
    let root =
        std::env::temp_dir().join(format!("omiai-wrt-tamper-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000003");
    std::fs::create_dir_all(&cp).unwrap();

    let mut w = World::new(config(), 7);
    w.step();
    w.save(&cp).unwrap();

    // Xoá file payload → manifest còn, hash không verify nổi.
    std::fs::remove_file(cp.join("world").join("atoms.cbor")).unwrap();
    assert!(verify_dir(&cp).is_err());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dangling_gene_reference_is_corrupt_not_silent() {
    // Atom trỏ vào slot không có trong registry: nếu load bỏ qua, atom sẽ
    // im lặng bất động mãi mãi (agent_act `continue` khi registry.get None).
    // §4 spec: resume phải dừng ồn ào, không được skip.
    let root = std::env::temp_dir()
        .join(format!("omiai-wrt-dangling-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000000");
    std::fs::create_dir_all(&cp).unwrap();

    let mut w = World::new(config(), 5);
    w.atoms[0].gene = omiai_world::registry::FormulaId::from_slot(99);
    w.save(&cp).expect("save world");
    verify_dir(&cp).expect("hash vẫn khớp — lỗi là ở mức tham chiếu");

    let err = World::load(&cp).expect_err("gene mồ côi phải bị từ chối");
    assert!(
        format!("{err}").contains("slot"),
        "lỗi phải nói rõ gene slot, nhận: {err}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn out_of_bounds_atom_is_corrupt() {
    let root =
        std::env::temp_dir().join(format!("omiai-wrt-oob-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000000");
    std::fs::create_dir_all(&cp).unwrap();

    let mut w = World::new(config(), 5);
    w.atoms[0].pos = (99, 0);
    w.save(&cp).expect("save world");

    let err = World::load(&cp).expect_err("atom ngoài lưới phải bị từ chối");
    assert!(format!("{err}").contains("pos"), "nhận: {err}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn airwave_and_vocabulary_persist_bit_exact() {
    let config = WorldConfig {
        width: 8,
        height: 8,
        n_initial_atoms: 2,
        initial_resources: 0.1,
    };

    let mut world = World::new(config, 42);
    for _ in 0..5 {
        world.step();
    }

    let root =
        std::env::temp_dir().join(format!("omiai-airvoc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000005");
    std::fs::create_dir_all(&cp).unwrap();

    world.save(&cp).expect("save world");
    verify_dir(&cp).expect("manifest + hashes ok");

    let mut loaded = World::load(&cp).expect("load world");

    // Immediate equality after load
    assert_eq!(loaded.airwave, world.airwave, "airwave sai ngay sau load");
    assert_eq!(loaded.vocabulary, world.vocabulary, "vocabulary sai ngay sau load");

    // Run both for 10 more steps
    for _ in 0..10 {
        world.step();
    }
    for _ in 0..10 {
        loaded.step();
    }

    // Bit-exact after resume
    assert_eq!(loaded.ca.cells, world.ca.cells, "grid sai sau resume");
    assert_eq!(loaded.atoms, world.atoms, "atoms sai sau resume");
    assert_eq!(loaded.step_count, world.step_count);
    assert_eq!(loaded.airwave, world.airwave, "airwave sai sau resume");
    assert_eq!(loaded.vocabulary, world.vocabulary, "vocabulary sai sau resume");
    assert_eq!(
        loaded.registry.genomes_in_order(),
        world.registry.genomes_in_order(),
        "registry sai sau resume"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn conventions_tracker_persists_bit_exact() {
    let config = WorldConfig {
        width: 8,
        height: 8,
        n_initial_atoms: 2,
        initial_resources: 0.1,
    };

    let mut world = World::new(config, 99);
    // Nạp tracker bằng tay để không phụ thuộc phase thực.
    for _ in 0..MIN_EPOCH_SUPPORT {
        world
            .conventions
            .record_signal(SignalValue::Sym(0), StateClass::East);
    }
    let mut hear = [false; omiai_world::communication::N_SYMBOLS];
    hear[0] = true;
    for i in 0..MIN_BENEFIT_SUPPORT {
        world.conventions.record_benefit(&hear, i % 2 == 0);
    }
    for _ in 0..10 {
        world
            .conventions
            .record_benefit(&[false; omiai_world::communication::N_SYMBOLS], false);
    }
    // End epoch so `epoch_vocab` đóng lại.
    world.conventions.note_step();

    let root =
        std::env::temp_dir().join(format!("omiai-cv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000001");
    std::fs::create_dir_all(&cp).unwrap();

    world.save(&cp).expect("save world");
    verify_dir(&cp).expect("manifest + hashes ok");

    let loaded = World::load(&cp).expect("load world");
    assert_eq!(loaded.conventions.epoch_index, world.conventions.epoch_index);
    assert_eq!(
        loaded.conventions.steps_in_epoch,
        world.conventions.steps_in_epoch
    );
    assert_eq!(
        loaded.conventions.epoch_vocab.total,
        world.conventions.epoch_vocab.total
    );
    assert_eq!(
        loaded.conventions.benefit.heard_steps,
        world.conventions.benefit.heard_steps
    );
    assert_eq!(
        loaded.conventions.benefit.heard_feeds,
        world.conventions.benefit.heard_feeds
    );
    assert_eq!(
        loaded.conventions.benefit.quiet_steps,
        world.conventions.benefit.quiet_steps
    );
    assert_eq!(
        loaded.conventions.benefit.quiet_feeds,
        world.conventions.benefit.quiet_feeds
    );
    assert_eq!(
        loaded.conventions.streak_len,
        world.conventions.streak_len
    );
    assert_eq!(
        loaded.conventions.promoted.len(),
        world.conventions.promoted.len()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn knowledge_graph_persists_bit_exact() {
    use omiai_knowledge::graph::{Concept, KnowledgeGraph};

    let config = WorldConfig {
        width: 8,
        height: 8,
        n_initial_atoms: 2,
        initial_resources: 0.1,
    };

    let mut world = World::new(config, 77);
    // Thêm vài node/edge bằng tay.
    let mut g = KnowledgeGraph::new();
    g.add_concept(Concept {
        id: "concept_a".into(),
        label: "Concept A".into(),
    });
    g.add_concept(Concept {
        id: "concept_b".into(),
        label: "Concept B".into(),
    });
    g.add_relation("concept_a", "concept_b", "relates_to").unwrap();
    world.knowledge = g;

    let root =
        std::env::temp_dir().join(format!("omiai-kg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000001");
    std::fs::create_dir_all(&cp).unwrap();

    world.save(&cp).expect("save world");
    verify_dir(&cp).expect("manifest + hashes ok");

    let loaded = World::load(&cp).expect("load world");

    // KnowledgeGraph không có PartialEq — so sánh concept_ids + relations đã sort.
    let mut orig_ids: Vec<String> = world.knowledge.concept_ids().map(str::to_string).collect();
    orig_ids.sort();
    let mut loaded_ids: Vec<String> =
        loaded.knowledge.concept_ids().map(str::to_string).collect();
    loaded_ids.sort();
    assert_eq!(loaded_ids, orig_ids);

    let mut orig_rels = world.knowledge.relations();
    orig_rels.sort();
    let mut loaded_rels = loaded.knowledge.relations();
    loaded_rels.sort();
    assert_eq!(loaded_rels, orig_rels);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn checkpoint_without_slice5_payloads_loads_ok() {
    // Mô phỏng checkpoint slice 3/4 (chỉ có 6 file world/*), đọc phải thành
    // công và cho tracker rỗng + graph rỗng.

    let config = WorldConfig {
        width: 8,
        height: 8,
        n_initial_atoms: 2,
        initial_resources: 0.1,
    };

    let mut world = World::new(config, 55);
    world.step(); // có airwave + vocabulary

    let root = std::env::temp_dir()
        .join(format!("omiai-oldcp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cp = root.join("step_00000001");
    std::fs::create_dir_all(&cp).unwrap();

    world.save(&cp).expect("save world");

    // Xoá hai payload mới.
    std::fs::remove_file(cp.join("communication").join("conventions.cbor")).unwrap();
    std::fs::remove_file(cp.join("knowledge_graph").join("graph.cbor")).unwrap();
    // Cập nhật manifest: ghi lại file còn lại (xài verify_dir để re-hash).
    // verify_dir sẽ đọc manifest cũ → thất bại. Ghi manifest mới bằng cách
    // load thủ công và save lại.
    let manifest = Manifest::read(&cp).expect("read manifest");
    let files: Vec<FileRecord> = manifest
        .files
        .into_iter()
        .filter(|f| {
            f.path != "communication/conventions.cbor"
                && f.path != "knowledge_graph/graph.cbor"
        })
        .collect();
    Manifest::write(&cp, &files).expect("rewrite manifest");

    // Bây giờ load phải qua, tracker rỗng, graph rỗng.
    let mut loaded = World::load(&cp).expect("load world thiếu payload mới");
    assert!(
        loaded.conventions.epoch_vocab.total == 0,
        "tracker cũ phải rỗng"
    );
    assert!(loaded.knowledge.is_empty(), "graph cũ phải rỗng");

    // Resume tiếp vài bước không panic.
    for _ in 0..5 {
        loaded.step();
    }

    let _ = std::fs::remove_dir_all(&root);
}
