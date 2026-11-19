//! `fRandomWalk`.

use super::*;

/// `ZTAIMGR_F_RANDOM_WALK`: port only, over a fake unit, manager, map and recording stand-ins. Covers each
/// phase of `fRandomWalk`: queued targets, a standing destination, the random-box candidate walk, the neighbour
/// walk with its path-cost limit, the fall-back to the unit's own position, the guard byte and the deferrals.
pub(crate) fn run_ztaimgr_f_random_walk_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::{collections::VecDeque, sync::Mutex};

    use crate::ztaimgr::{f_random_walk, RandomWalkOps};
    let test_name = "ZTAIMGR_F_RANDOM_WALK";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static RAND: Mutex<VecDeque<u32>> = Mutex::new(VecDeque::new());
    static PATH_RESULTS: Mutex<VecDeque<bool>> = Mutex::new(VecDeque::new());
    static WALKABLE: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static NEIGHBORS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static HAS_DEST: AtomicU32 = AtomicU32::new(0);
    static DEST: Mutex<[i32; 3]> = Mutex::new([0; 3]);
    static ADD_TARGET: AtomicU32 = AtomicU32::new(0);
    static RING: AtomicU32 = AtomicU32::new(0);
    static RANGE: AtomicU32 = AtomicU32::new(1);
    static WORLD: AtomicU32 = AtomicU32::new(0);
    static TILE_BASE: AtomicU32 = AtomicU32::new(0);
    static COST_LIMIT: AtomicU32 = AtomicU32::new(10);
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    fn index(tile: u32) -> u32 {
        (tile - TILE_BASE.load(Ordering::Relaxed)) / 0x8c
    }
    fn triple(ptr: u32) -> [i32; 3] {
        [unsafe { *(ptr as *const i32) }, unsafe { *((ptr + 4) as *const i32) }, unsafe { *((ptr + 8) as *const i32) }]
    }
    unsafe fn rand() -> u32 {
        RAND.lock().unwrap().pop_front().expect("rand queue exhausted")
    }
    unsafe fn world() -> u32 {
        WORLD.load(Ordering::Relaxed)
    }
    unsafe fn neighbors(_: u32, tile: u32) -> Vec<u32> {
        log(format!("neighbors({})", index(tile)));
        NEIGHBORS.lock().unwrap().iter().map(|i| TILE_BASE.load(Ordering::Relaxed) + i * 0x8c).collect()
    }
    unsafe fn path_cost(_: u32, from: u32, to: u32) -> i32 {
        log(format!("cost({},{})", index(from), index(to)));
        (index(to) % 100) as i32
    }
    unsafe fn cost_limit() -> i32 {
        COST_LIMIT.load(Ordering::Relaxed) as i32
    }
    unsafe fn no_destination() -> [i32; 3] {
        [-1, -1, -1]
    }
    extern "thiscall" fn v_walk(_: u32, tile: u32, flag: u32) -> u32 {
        log(format!("walk({},{flag})", index(tile)));
        u32::from(WALKABLE.lock().unwrap().contains(&index(tile)))
    }
    extern "thiscall" fn v_get_dest(_: u32, out: u32) -> u32 {
        log("get_dest".into());
        if ADD_TARGET.load(Ordering::Relaxed) != 0 {
            let ring = RING.load(Ordering::Relaxed);
            unsafe {
                *(ring as *mut u32) = ring + 8;
                *((ring + 8) as *mut u32) = ring;
            }
        }
        if HAS_DEST.load(Ordering::Relaxed) != 0 {
            let dest = *DEST.lock().unwrap();
            for (i, v) in dest.iter().enumerate() {
                unsafe { *((out + i as u32 * 4) as *mut i32) = *v };
            }
        }
        HAS_DEST.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_set_dest(_: u32, goal: u32) -> u32 {
        log(format!("set_dest({:?})", triple(goal)));
        0
    }
    extern "thiscall" fn m_get_path(_: u32, start: u32, goal: u32, _: u32, flag: u32) -> u32 {
        log(format!("path({:?},{:?},{flag})", triple(start), triple(goal)));
        u32::from(PATH_RESULTS.lock().unwrap().pop_front().unwrap_or(false))
    }
    extern "thiscall" fn m_range(_: u32, _: u32) -> u32 {
        RANGE.load(Ordering::Relaxed)
    }
    let ops = RandomWalkOps { rand, world, neighbors, path_cost, path_cost_limit: cost_limit, no_destination };

    let mut unit_vtable = Box::new([0u32; 0x100]);
    unit_vtable[0x168 / 4] = v_walk as *const () as usize as u32;
    unit_vtable[0x1f4 / 4] = v_get_dest as *const () as usize as u32;
    unit_vtable[0x1f8 / 4] = v_set_dest as *const () as usize as u32;
    let mut mgr_vtable = Box::new([0u32; 0x20]);
    mgr_vtable[0x18 / 4] = m_get_path as *const () as usize as u32;
    mgr_vtable[0x20 / 4] = m_range as *const () as usize as u32;
    mgr_vtable[0x24 / 4] = m_range as *const () as usize as u32;
    let mut mgr = Box::new([0u32; 0x80]);
    mgr[0] = mgr_vtable.as_ptr() as u32;
    let this = mgr.as_ptr() as u32;

    let mut tiles = vec![0u8; 20 * 20 * 0x8c];
    for i in 0..400usize {
        let base = i * 0x8c + 0x34;
        for (k, v) in [(i % 20) as i32, (i / 20) as i32, 0].iter().enumerate() {
            tiles[base + k * 4..base + k * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    TILE_BASE.store(tiles.as_ptr() as u32, Ordering::Relaxed);
    let mut world_block = Box::new([0u32; 0x40]);
    world_block[(8 + 0x2c) / 4] = 20;
    world_block[(8 + 0x30) / 4] = 20;
    world_block[(8 + 0x38) / 4] = tiles.as_ptr() as u32;
    WORLD.store(world_block.as_ptr() as u32, Ordering::Relaxed);
    let mut ring = vec![[0u32; 2]; 2];
    RING.store(ring.as_ptr() as u32, Ordering::Relaxed);

    let mut block = Box::new([0u32; 0x200]);
    block[0] = unit_vtable.as_ptr() as u32;
    block[0x154 / 4] = ring.as_ptr() as u32;
    let unit = block.as_ptr() as u32;
    // Unit at tile (5, 5), height cell 3.
    block[0x114 / 4] = 5 * 64 + 10;
    block[0x118 / 4] = 5 * 64;
    block[0x11c / 4] = 3 * 16 + 2;
    let guard = |block: &Box<[u32; 0x200]>| unsafe { *((block.as_ptr() as usize + 0x20f) as *const u8) };
    let start = [5, 5, 3];
    let tile_of = |x: i32, y: i32| (y * 20 + x) as u32;

    struct Case {
        label: &'static str,
        add_target: bool,
        dest: Option<[i32; 3]>,
        paths: Vec<bool>,
        walkable: Vec<u32>,
        neighbors: Vec<u32>,
        rands: Vec<u32>,
        expect_result: Option<bool>,
        expect_log: Vec<String>,
    }
    let case = |label, add_target, dest, paths: &[bool], walkable: Vec<u32>, neighbors: Vec<u32>, rands: &[u32], expect_result, expect_log: Vec<String>| Case {
        label, add_target, dest, paths: paths.to_vec(), walkable, neighbors, rands: rands.to_vec(), expect_result, expect_log,
    };
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    // Radius 1 box (rand 0 -> half extent 1): perimeter order dx -1..1, dy -1..1, skipping the centre.
    let ring_tiles: Vec<u32> = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)].iter().map(|(dx, dy)| tile_of(5 + dx, 5 + dy)).collect();
    let walk_log = |tiles: &[u32]| tiles.iter().map(|t| format!("walk({t},1)")).collect::<Vec<_>>();
    let goal_of = |t: u32| [(t % 20) as i32, (t / 20) as i32, 0];
    let mut radius_log = vec!["get_dest".to_string()];
    radius_log.extend(walk_log(&ring_tiles));
    let pick_goal = goal_of(ring_tiles[2]);
    let mut radius_ok = radius_log.clone();
    radius_ok.push(format!("set_dest({pick_goal:?})"));
    radius_ok.push(format!("path({start:?},{pick_goal:?},0)"));
    let mut fallback = radius_log.clone();
    fallback.extend(s(&["neighbors(105)"]));
    let cases = vec![
        case("queued after get_dest", true, None, &[], vec![], vec![], &[], Some(true), s(&["get_dest"])),
        case("standing destination", false, Some([9, 9, 9]), &[true], vec![], vec![], &[], Some(true), vec!["get_dest".to_string(), format!("path({start:?},[9, 9, 9],1)")]),
        case("destination is own tile", false, Some(start), &[true], vec![tile_of(4, 6)], vec![], &[0, 0, 0], Some(true), {
            let mut v = vec!["get_dest".to_string()];
            v.extend(walk_log(&ring_tiles));
            v.push(format!("set_dest({:?})", goal_of(tile_of(4, 6))));
            v.push(format!("path({start:?},{:?},0)", goal_of(tile_of(4, 6))));
            v
        }),
        case("radius walk picks a candidate", false, None, &[true], vec![ring_tiles[2], ring_tiles[5]], vec![], &[0, 0, 0], Some(true), radius_ok.clone()),
        case("radius path fails, neighbours", false, None, &[false, true], vec![ring_tiles[2], 102], vec![101, 102], &[0, 0, 0, 1], Some(true), {
            let mut v = radius_ok.clone();
            v.push("neighbors(105)".into());
            v.push("walk(102,1)".into());
            v.push("cost(105,102)".into());
            v.push(format!("set_dest({:?})", goal_of(102)));
            v.push(format!("path({start:?},{:?},0)", goal_of(102)));
            v
        }),
        case("neighbour over cost limit", false, None, &[false], vec![150, 160], vec![150, 160], &[0, 0, 0, 0], Some(false), {
            let mut v = radius_log.clone();
            v.push("neighbors(105)".into());
            v.push("walk(150,1)".into());
            v.push("cost(105,150)".into());
            v.push("walk(160,1)".into());
            v.push("cost(105,160)".into());
            v.push(format!("path({start:?},{start:?},0)"));
            v
        }),
        case("neighbour walk starts at the random index and does not wrap", false, None, &[false, false], vec![101, 102], vec![101, 102], &[0, 0, 1], Some(false), {
            let mut v = radius_log.clone();
            v.push("neighbors(105)".into());
            v.push("walk(102,1)".into());
            v.push("cost(105,102)".into());
            v.push(format!("set_dest({:?})", goal_of(102)));
            v.push(format!("path({start:?},{:?},0)", goal_of(102)));
            v.push(format!("path({start:?},{start:?},0)"));
            v
        }),
    ];
    let _ = fallback;
    for case in &cases {
        let ring_ptr = ring.as_ptr() as u32;
        ring[0][0] = ring_ptr;
        ring[1][0] = 0;
        *RAND.lock().unwrap() = case.rands.iter().copied().collect();
        *PATH_RESULTS.lock().unwrap() = case.paths.iter().copied().collect();
        *WALKABLE.lock().unwrap() = case.walkable.clone();
        *NEIGHBORS.lock().unwrap() = case.neighbors.clone();
        HAS_DEST.store(u32::from(case.dest.is_some()), Ordering::Relaxed);
        *DEST.lock().unwrap() = case.dest.unwrap_or([0; 3]);
        ADD_TARGET.store(u32::from(case.add_target), Ordering::Relaxed);
        RANGE.store(1, Ordering::Relaxed);
        LOG.lock().unwrap().clear();
        let result = unsafe { f_random_walk(this, unit, &ops) };
        if result != case.expect_result || *LOG.lock().unwrap() != case.expect_log || guard(&block) != 0 {
            failures.push(format!("{}: result {result:?}, log {:?}, guard {}; expected {:?}, {:?}", case.label, LOG.lock().unwrap(), guard(&block), case.expect_result, case.expect_log));
        }
    }

    // Gates and deferrals.
    ring[0][0] = ring.as_ptr() as u32;
    HAS_DEST.store(0, Ordering::Relaxed);
    ADD_TARGET.store(0, Ordering::Relaxed);
    LOG.lock().unwrap().clear();
    if unsafe { f_random_walk(this, 0, &ops) } != Some(false) {
        failures.push("null unit".into());
    }
    RANGE.store(0, Ordering::Relaxed);
    *RAND.lock().unwrap() = VecDeque::new();
    if unsafe { f_random_walk(this, unit, &ops) }.is_some() {
        failures.push("zero random extent should defer".into());
    }
    RANGE.store(1, Ordering::Relaxed);
    block[0x114 / 4] = 100 * 64;
    if unsafe { f_random_walk(this, unit, &ops) }.is_some() {
        failures.push("unit off the map should defer".into());
    }
    block[0x114 / 4] = 5 * 64 + 10;
    *RAND.lock().unwrap() = VecDeque::from([0, 0, 0]);
    *WALKABLE.lock().unwrap() = vec![];
    *NEIGHBORS.lock().unwrap() = vec![];
    if unsafe { f_random_walk(this, unit, &ops) }.is_some() {
        failures.push("empty neighbour list should defer".into());
    }
    let _ = (&mut ring, &tiles, &world_block);
    finish_test(test_name, failures, failure_log)
}
