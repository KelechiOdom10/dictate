//! Short system sounds when a take starts and stops (Advanced → Sound feedback).

#[derive(Clone, Copy, Debug)]
pub enum Cue {
    Start,
    Stop,
}

fn path(cue: Cue) -> &'static str {
    match cue {
        Cue::Start => "/System/Library/Sounds/Tink.aiff",
        Cue::Stop => "/System/Library/Sounds/Pop.aiff",
    }
}

/// Plays `cue` without waiting for it. A missing sound is logged, never fatal.
pub fn play(cue: Cue) {
    let spawned = std::process::Command::new("/usr/bin/afplay")
        .args(["-v", "0.5", path(cue)])
        .spawn();
    match spawned {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(error) => log::warn!("[sound] {cue:?}: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn cues_use_sounds_that_ship_with_macos() {
        for cue in [Cue::Start, Cue::Stop] {
            assert!(std::path::Path::new(path(cue)).is_file(), "{cue:?}");
        }
        assert!(std::path::Path::new("/usr/bin/afplay").is_file());
    }
}
