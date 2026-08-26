//! Round-trip bit-exact của World qua checkpoint-v1 — test then chốt slice 2:
//! save ở bước N → load → chạy tiếp M bước phải ra CÙNG trạng thái với
//! world chạy liền N+M bước không qua checkpoint.

use omiai_checkpoint::{traits::Checkpointable, verify_dir};
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
