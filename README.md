# migr

**M**inecraft **I**nstance **G**it **R**ust. Moves your configs from one Prism Launcher instance to the next whenever your modpack updates, without redoing them by hand.

Each pack update means a fresh instance and an afternoon of copying configs over. migr keeps your files in a git repository and merges them into the new instance for you. Your edits stay, and so does whatever the pack changed in the new version. When you and the pack changed the same line, you sort it out by hand, the way you would in any git merge.

It was made for GT New Horizons, but it works with any pack in Prism.

## How it works

- Every instance gets a branch, and the pack's own files for that instance live on `pack/<branch>`.
- `migr fork old new` saves the new instance's untouched files as a child of the old pack's branch, then merges your old branch in. The old pack's branch becomes the merge base, so git can tell what you changed from what the pack changed.
- Prism runs migr before launch and after exit. The post-exit hook commits your changes, and with a remote it also pushes them. The pre-launch hook pulls newer files and stops the launch while a fork still has conflicts.
- Only the paths listed in `.migr.toml` are tracked. That file sits in the game folder and in each branch, so a fork inherits the list.
- Playtime adds up: a fork sets the new instance's playtime to old plus new.

Old instances are never touched, so they still work as a fallback.

## Install

```sh
brew install fopwoc/tap/migr
```

The Homebrew build is for Apple Silicon Macs. Anywhere else, build it with `cargo install --path crates/migr`. You also need `git`.

## Setup

```sh
migr init                      # or: migr init git@github.com:you/gtnh-configs.git
migr new GT_New_Horizons_2.8.3_Java_17-25
```

Prism must be closed while `new`, `fork`, `link`, `continue` and `abort` run. They edit `instance.cfg`, and Prism would overwrite those edits when it saves.

`new` takes the instance's current files as the pack's own. If you already changed them, pass a pristine `.minecraft` of the same pack version with `--pack <dir>`. You can get one by unzipping the pack.

By default migr tracks `options.txt`, `optionsnf.txt`, `localconfig.cfg` and `servers.dat`. Add more with `--track <path>` on `new`, or with `migr track` later. Folders work too.

## Updating the pack

1. Import the new pack version as a new Prism instance. Don't launch it yet.
2. Run `migr fork <old instance> <new instance>`.
3. Launch the new instance.

If the fork stops on conflicts, edit the files it lists, then run `migr continue`. To undo the fork entirely, run `migr abort`.

Forgot to track something? From the new instance, take your version of it from the old one:

```sh
migr track <new instance> config/NEI --from <old instance>
```

For paths you already changed, or added yourself (a mod jar, a resource pack), pass `--pack <a fresh .minecraft>`. Otherwise the current files count as the pack's own, and the next fork would drop files the new pack doesn't ship.

## Another machine

```sh
migr init git@github.com:you/gtnh-configs.git
```

Instances whose folder name matches a branch get hooked, and their first launch writes your files. An instance with a different folder name gets bound by hand with `migr link <instance> <branch>`.

Sync is best effort. Being offline never stops the game; the push is retried after the next session. If two machines change the same branch, migr launches with the local files and leaves the sorting to `migr git -i <instance> -- ...`. Playtime is not synced between machines.

## Commands

| Command | |
|---|---|
| `init [remote]` | Create the repository, or clone it |
| `new <instance>` | Start tracking an instance |
| `fork <from> <to>` | Carry your files into a new pack version |
| `track <instance> <path>... [--from <instance>] [--pack <dir>]` | Track more paths |
| `link <instance> [branch]` | Bind an instance to an existing branch |
| `status` | Tracked instances, playtime, a fork in progress |
| `continue` / `abort` | Finish or undo a fork with conflicts |
| `git [-i <instance>] -- <args>` | Plain git on the repository |

## Files

- Repository: `~/.local/share/migr/repo.git`
- Machine config: `~/.config/migr/config.toml`, with `prism_dir`, `repo` and `links`, all optional
- `LOG_LEVEL=debug` shows every git command migr runs

> [!NOTE]
> This project contains AI-generated code. See [AI_USAGE.md](AI_USAGE.md) for details.
