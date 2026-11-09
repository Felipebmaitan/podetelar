use super::{Application, Choice};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Identity {
    pub pid: u32,
    pub created: u64,
}

#[derive(Clone)]
pub struct Process {
    pub identity: Identity,
    pub parent: u32,
    pub name: String,
    pub image: Option<String>,
}

pub struct Inventory {
    pub processes: HashMap<u32, Process>,
    pub sessions: HashSet<u32>,
}

impl Inventory {
    // Check birth order at every parent edge so a recycled parent PID cannot
    // turn an unrelated application into part of a captured/protected tree.
    fn belongs_to(&self, child: &Process, root: Identity) -> bool {
        let mut current = child;
        for _ in 0..self.processes.len() {
            if current.identity == root {
                return true;
            }
            let Some(parent) = self.processes.get(&current.parent) else {
                break;
            };
            if parent.identity.created != 0
                && current.identity.created != 0
                && parent.identity.created > current.identity.created
            {
                break;
            }
            current = parent;
        }
        false
    }

    fn protected(&self, process: &Process, owner: Identity, exclusions: &[String]) -> bool {
        let mut current = process;
        for _ in 0..self.processes.len() {
            if current.identity == owner
                || discord(current)
                || current
                    .image
                    .as_ref()
                    .is_some_and(|image| exclusions.contains(image))
            {
                return true;
            }
            let Some(parent) = self.processes.get(&current.parent) else {
                break;
            };
            if parent.identity.created != 0
                && current.identity.created != 0
                && parent.identity.created > current.identity.created
            {
                break;
            }
            current = parent;
        }
        false
    }

    pub fn applications(&self, owner: Identity) -> Vec<Application> {
        let mut applications: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|pid| {
                let process = self.processes.get(pid)?;
                if self.protected(process, owner, &[]) {
                    return None;
                }
                Some(Application {
                    id: process.image.clone()?,
                    label: process.name.clone(),
                })
            })
            .collect();
        applications.sort_by(|a, b| a.label.cmp(&b.label).then(a.id.cmp(&b.id)));
        applications.dedup_by(|a, b| a.id == b.id);
        applications
    }

    pub fn roots(&self, choice: &Choice, exclusions: &[String], owner: Identity) -> Vec<Identity> {
        let candidates: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|pid| {
                let process = self.processes.get(pid)?;
                let image = process.image.as_ref()?;
                if process.identity.created == 0 || self.protected(process, owner, exclusions) {
                    return None;
                }
                if !match choice {
                    Choice::Off => false,
                    Choice::Desktop => true,
                    Choice::Application(selected) => image == selected,
                } {
                    return None;
                }
                // Include-process-tree cannot remove a protected descendant. Omit
                // this whole root instead of accidentally including that descendant.
                if self.processes.values().any(|child| {
                    self.belongs_to(child, process.identity)
                        && self.protected(child, owner, exclusions)
                }) {
                    return None;
                }
                Some(process.identity)
            })
            .collect();
        let mut roots: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|candidate| {
                let process = &self.processes[&candidate.pid];
                !candidates
                    .iter()
                    .any(|other| other != candidate && self.belongs_to(process, *other))
            })
            .collect();
        roots.sort_by_key(|root| (root.pid, root.created));
        roots
    }
}

fn discord(process: &Process) -> bool {
    matches!(
        process.name.to_ascii_lowercase().as_str(),
        "discord.exe" | "discordptb.exe" | "discordcanary.exe"
    ) || process.image.as_ref().is_some_and(|path| {
        path.split(['\\', '/'])
            .any(|part| matches!(part, "discord" | "discordptb" | "discordcanary"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn process(pid: u32, parent: u32, name: &str) -> Process {
        Process {
            identity: Identity {
                pid,
                created: pid as u64,
            },
            parent,
            name: name.into(),
            image: Some(format!("c:/{name}").to_lowercase()),
        }
    }
    fn inventory(processes: Vec<Process>, sessions: &[u32]) -> Inventory {
        Inventory {
            processes: processes
                .into_iter()
                .map(|process| (process.identity.pid, process))
                .collect(),
            sessions: sessions.iter().copied().collect(),
        }
    }
    fn owner() -> Identity {
        Identity { pid: 1, created: 1 }
    }
    #[test]
    fn desktop_excludes_discord_viewer_and_their_descendants_without_double_capture() {
        let state = inventory(
            vec![
                process(1, 0, "publisher.exe"),
                process(2, 1, "chrome.exe"),
                process(3, 2, "chrome.exe"),
                process(4, 0, "Discord.exe"),
                process(5, 4, "audio.exe"),
                process(6, 0, "game.exe"),
                process(7, 6, "game.exe"),
            ],
            &[2, 3, 4, 5, 6, 7],
        );
        assert_eq!(
            state.roots(&Choice::Desktop, &[], owner()),
            vec![Identity { pid: 6, created: 6 }]
        );
    }
    #[test]
    fn restart_new_pid_is_filtered_by_image_and_unknown_processes_are_never_captured() {
        let mut state = inventory(
            vec![process(8, 0, "game.exe"), process(9, 0, "Discord.exe")],
            &[8, 9, 10],
        );
        let choice = Choice::Application("c:/game.exe".into());
        assert_eq!(state.roots(&choice, &[], owner())[0].pid, 8);
        state.processes.remove(&8);
        state.processes.insert(12, process(12, 0, "game.exe"));
        state.sessions = [9, 10, 12].into_iter().collect();
        assert_eq!(state.roots(&choice, &[], owner())[0].pid, 12);
        assert!(
            state
                .roots(&choice, &["c:/game.exe".into()], owner())
                .is_empty()
        );
    }
    #[test]
    fn pid_reuse_does_not_protect_or_capture_an_unrelated_old_child() {
        let state = inventory(
            vec![
                process(1, 0, "publisher.exe"),
                process(5, 9, "game.exe"),
                process(9, 0, "Discord.exe"),
            ],
            &[5],
        );
        assert_eq!(state.roots(&Choice::Desktop, &[], owner())[0].pid, 5);
    }
    #[test]
    fn a_game_with_a_protected_descendant_is_omitted_instead_of_leaking_it() {
        let state = inventory(
            vec![process(6, 0, "game.exe"), process(7, 6, "Discord.exe")],
            &[6, 7],
        );
        assert!(state.roots(&Choice::Desktop, &[], owner()).is_empty());
    }
    #[test]
    fn unavailable_birth_times_keep_protected_parent_and_descendant_checks_conservative() {
        let mut protected = process(7, 6, "Discord.exe");
        protected.identity.created = 0;
        protected.image = None;
        let state = inventory(vec![process(6, 0, "game.exe"), protected], &[6]);
        assert!(state.roots(&Choice::Desktop, &[], owner()).is_empty());
        let mut protected_parent = process(7, 0, "Discord.exe");
        protected_parent.identity.created = 0;
        let state = inventory(vec![protected_parent, process(8, 7, "audio.exe")], &[8]);
        assert!(state.roots(&Choice::Desktop, &[], owner()).is_empty());
    }
}
