//! Whole flows against real git, with fake Prism instances in a temp folder.

use std::{fs, path::PathBuf, process::Command};

use tempfile::TempDir;

use super::{Context, fork, hook, init, new, resolve, track};
use crate::{cli::HookPhase, prism::Prism, store::MANIFEST_FILE};

/// Lines far enough apart that git merges edits to the first and the last without a conflict.
const PACK_OPTIONS: &str = "a=1\n1\n2\n3\n4\n5\n6\nz=1\n";

struct Machine {
    root: TempDir,
    context: Context,
}

impl Machine {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let instances = root.path().join("instances");
        fs::create_dir(&instances).unwrap();
        let context = Context::for_test(root.path(), Prism::at(instances));
        Self { root, context }
    }

    fn instance(&self, id: &str, playtime: u64, files: &[(&str, &str)]) {
        let dir = self.root.path().join("instances").join(id);
        fs::create_dir_all(dir.join(".minecraft")).unwrap();
        fs::write(
            dir.join("instance.cfg"),
            format!("[General]\nname={id}\ntotalTimePlayed={playtime}\n"),
        )
        .unwrap();
        for (path, text) in files {
            self.write(id, path, text);
        }
    }

    fn game(&self, id: &str) -> PathBuf {
        self.root
            .path()
            .join("instances")
            .join(id)
            .join(".minecraft")
    }

    fn write(&self, id: &str, path: &str, text: &str) {
        let path = self.game(id).join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn read(&self, id: &str, path: &str) -> String {
        fs::read_to_string(self.game(id).join(path)).unwrap()
    }

    fn cfg(&self, id: &str) -> String {
        fs::read_to_string(
            self.root
                .path()
                .join("instances")
                .join(id)
                .join("instance.cfg"),
        )
        .unwrap()
    }

    fn hook(&self, phase: HookPhase, id: &str) -> anyhow::Result<()> {
        hook::run(
            &self.context,
            phase,
            &id.parse().unwrap(),
            Some(self.game(id)),
        )
    }

    fn init(&self, remote: Option<&str>) {
        init::run(&self.context, remote).unwrap();
    }

    fn new_instance(&self, id: &str) {
        new::run(&self.context, &id.parse().unwrap(), None, Vec::new()).unwrap();
    }

    fn fork(&self, from: &str, to: &str) -> anyhow::Result<()> {
        fork::run(&self.context, &from.parse().unwrap(), &to.parse().unwrap())
    }
}

#[test]
fn fork_keeps_your_edits_and_the_packs_changes() {
    let machine = Machine::new();
    machine.init(None);
    machine.instance("old", 3_600, &[("options.txt", PACK_OPTIONS)]);
    machine.new_instance("old");
    assert!(
        machine
            .cfg("old")
            .contains("PreLaunchCommand=/usr/local/bin/migr hook pre")
    );

    // You play and change the last line; the next pack version changes the first.
    machine.hook(HookPhase::Pre, "old").unwrap();
    machine.write("old", "options.txt", &PACK_OPTIONS.replace("z=1", "z=mine"));
    machine.hook(HookPhase::Post, "old").unwrap();
    machine.instance(
        "new",
        60,
        &[("options.txt", &PACK_OPTIONS.replace("a=1", "a=pack"))],
    );

    machine.fork("old", "new").unwrap();

    assert_eq!(
        machine.read("new", "options.txt"),
        PACK_OPTIONS
            .replace("a=1", "a=pack")
            .replace("z=1", "z=mine")
    );
    assert!(machine.read("new", MANIFEST_FILE).contains("options.txt"));
    let cfg = machine.cfg("new");
    assert!(cfg.contains("totalTimePlayed=3660"), "{cfg}");
    assert!(cfg.contains("PostExitCommand=/usr/local/bin/migr hook post"));
    // The old instance stays as it was, a fallback that still works.
    assert_eq!(
        machine.read("old", "options.txt"),
        PACK_OPTIONS.replace("z=1", "z=mine")
    );
}

#[test]
fn files_the_pack_never_ships_come_along() {
    let machine = Machine::new();
    machine.init(None);
    machine.instance("old", 0, &[("options.txt", PACK_OPTIONS)]);
    machine.new_instance("old");
    machine.write("old", "servers.dat", "my server");
    machine.hook(HookPhase::Post, "old").unwrap();
    machine.instance("new", 0, &[("options.txt", PACK_OPTIONS)]);

    machine.fork("old", "new").unwrap();

    assert_eq!(machine.read("new", "servers.dat"), "my server");
}

#[test]
fn conflicts_block_the_launch_until_continue() {
    let machine = Machine::new();
    machine.init(None);
    machine.instance("old", 100, &[("options.txt", PACK_OPTIONS)]);
    machine.new_instance("old");
    machine.write("old", "options.txt", &PACK_OPTIONS.replace("a=1", "a=mine"));
    machine.instance(
        "new",
        0,
        &[("options.txt", &PACK_OPTIONS.replace("a=1", "a=pack"))],
    );

    assert!(machine.fork("old", "new").is_err());
    assert!(machine.read("new", "options.txt").contains("<<<<<<< "));
    assert!(machine.hook(HookPhase::Pre, "new").is_err());
    assert!(
        resolve::resume(&machine.context).is_err(),
        "markers are still there"
    );

    machine.write("new", "options.txt", &PACK_OPTIONS.replace("a=1", "a=both"));
    resolve::resume(&machine.context).unwrap();

    machine.hook(HookPhase::Pre, "new").unwrap();
    assert_eq!(
        machine.read("new", "options.txt"),
        PACK_OPTIONS.replace("a=1", "a=both")
    );
    assert!(machine.cfg("new").contains("totalTimePlayed=100"));
}

#[test]
fn abort_puts_the_new_instance_back() {
    let machine = Machine::new();
    machine.init(None);
    machine.instance("old", 0, &[("options.txt", PACK_OPTIONS)]);
    machine.new_instance("old");
    machine.write("old", "options.txt", &PACK_OPTIONS.replace("a=1", "a=mine"));
    let pack = PACK_OPTIONS.replace("a=1", "a=pack");
    machine.instance("new", 0, &[("options.txt", &pack)]);
    let cfg_before = machine.cfg("new");

    assert!(machine.fork("old", "new").is_err());
    resolve::abort(&machine.context).unwrap();

    assert_eq!(machine.read("new", "options.txt"), pack);
    assert!(!machine.game("new").join(MANIFEST_FILE).exists());
    assert_eq!(
        machine.cfg("new"),
        cfg_before.replace(
            "totalTimePlayed=0\n",
            "totalTimePlayed=0\nOverrideCommands=false\nPreLaunchCommand=\nPostExitCommand=\n",
        )
    );
    // Nothing left behind: the same fork can start over.
    assert!(machine.fork("old", "new").is_err());
}

#[test]
fn track_takes_a_forgotten_file_from_the_old_instance() {
    let machine = Machine::new();
    machine.init(None);
    machine.instance("old", 0, &[("options.txt", PACK_OPTIONS)]);
    machine.new_instance("old");
    machine.instance("new", 0, &[("options.txt", PACK_OPTIONS)]);
    machine.fork("old", "new").unwrap();
    machine.write("old", "config/NEI/client.cfg", "theme=mine\n");
    machine.write("new", "config/NEI/client.cfg", "theme=pack\n");

    track::run(
        &machine.context,
        &"new".parse().unwrap(),
        vec!["config/NEI".parse().unwrap()],
        Some(&"old".parse().unwrap()),
    )
    .unwrap();

    assert_eq!(machine.read("new", "config/NEI/client.cfg"), "theme=mine\n");
    assert!(machine.read("new", MANIFEST_FILE).contains("config/NEI"));

    // The pack's version is the base for the next fork, so a pack change there merges in.
    machine.instance(
        "newer",
        0,
        &[
            ("options.txt", PACK_OPTIONS),
            ("config/NEI/client.cfg", "theme=pack\n"),
        ],
    );
    machine.fork("new", "newer").unwrap();
    assert_eq!(
        machine.read("newer", "config/NEI/client.cfg"),
        "theme=mine\n"
    );
}

#[test]
fn second_machine_picks_up_files_on_first_launch() {
    let remote = tempfile::tempdir().unwrap();
    let remote_url = remote.path().join("configs.git");
    let status = Command::new("git")
        .args(["init", "--quiet", "--bare"])
        .arg(&remote_url)
        .status()
        .unwrap();
    assert!(status.success());
    let remote_url = remote_url.to_str().unwrap();

    let desk = Machine::new();
    desk.init(Some(remote_url));
    desk.instance("gtnh", 0, &[("options.txt", PACK_OPTIONS)]);
    desk.new_instance("gtnh");
    desk.write("gtnh", "options.txt", "from desk\n");
    desk.hook(HookPhase::Post, "gtnh").unwrap();

    let laptop = Machine::new();
    laptop.instance("gtnh", 0, &[("options.txt", PACK_OPTIONS)]);
    laptop.init(Some(remote_url));
    assert!(laptop.cfg("gtnh").contains("PreLaunchCommand="));
    laptop.hook(HookPhase::Pre, "gtnh").unwrap();
    assert_eq!(laptop.read("gtnh", "options.txt"), "from desk\n");

    laptop.write("gtnh", "options.txt", "from laptop\n");
    laptop.hook(HookPhase::Post, "gtnh").unwrap();
    desk.hook(HookPhase::Pre, "gtnh").unwrap();
    assert_eq!(desk.read("gtnh", "options.txt"), "from laptop\n");
}

#[test]
fn offline_never_blocks_the_game() {
    let machine = Machine::new();
    machine.init(None);
    machine.instance("gtnh", 0, &[("options.txt", PACK_OPTIONS)]);
    machine.new_instance("gtnh");
    let store = machine.context.store().unwrap();
    store
        .git()
        .run(["remote", "add", "origin", "/nonexistent/remote.git"])
        .unwrap();

    machine.hook(HookPhase::Pre, "gtnh").unwrap();
    machine.write("gtnh", "options.txt", "offline edit\n");
    machine.hook(HookPhase::Post, "gtnh").unwrap();
}

#[test]
fn unknown_instances_cannot_launch_with_hooks() {
    let machine = Machine::new();
    machine.init(None);
    machine.instance("stray", 0, &[]);
    assert!(machine.hook(HookPhase::Pre, "stray").is_err());
    machine.hook(HookPhase::Post, "stray").unwrap();
}
