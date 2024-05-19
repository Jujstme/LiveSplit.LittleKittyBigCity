#![no_std]
#![warn(
    clippy::complexity,
    clippy::correctness,
    clippy::perf,
    clippy::style,
    clippy::undocumented_unsafe_blocks,
    rust_2018_idioms
)]

extern crate alloc;

#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

use asr::{
    future::{next_tick, retry},
    settings::Gui,
    timer::{self, TimerState},
    Process,
};

// Stuff imported
mod autosplitting;
mod memory;
mod mono;
mod process;
mod scene_manager;
mod settings;
mod csharp;

asr::panic_handler!();
asr::async_main!(stable);

async fn main() {
    // When the autosplitter is loaded, it loads the settings
    let mut settings = settings::Settings::register();

    loop {
        // First thing to do in the autosplitter logic is to hook to the target process.
        // This needs to stay inside the loop as the autosplitter must re-try to hook
        // to the target process once it is exited.
        let (process, process_name) = retry(|| {
            process::PROCESS_NAMES.iter().find_map(|&name| {
                let mut proc = Process::attach(name);
                if proc.is_none() && process::USE_LINUX_WORKAROUND && name.len() > 15 {
                    proc = Process::attach(&name[0..15])
                }

                Some((proc?, name))
            })
        })
        .await;

        process
            .until_closes(async {
                // Once the target process has been found and attached to,
                // the autosplitter can set up some default watchers.
                // In select cases it might be useful to set the watchers at the
                // beginning of the autosplitter logic, eg. immediately after
                // loading the settings. In the majority of cases, however,
                // this is not necessary.
                let mut watchers = autosplitting::Watchers::default();

                // Perform memory scanning to look for the addresses we need.
                // Depending on the game and the logic, we can either define fixed
                // memory offsets, or perform more advanced stuff (eg. sigscanning).
                // The name of the executable is passed here in order to easily allow
                // to query for the process' main module.
                let addresses = memory::Memory::init(&process, process_name).await;

                loop {
                    // Splitting logic. Adapted from OG LiveSplit:
                    // Order of execution
                    // 1. update() will always be run first. There are no conditions on the execution of this action.
                    // 2. If the timer is currently either running or paused, then the isLoading, gameTime, and reset actions will be run.
                    // 3. If reset does not return true, then the split action will be run.
                    // 4. If the timer is currently not running (and not paused), then the start action will be run.
                    settings.update();
                    autosplitting::update(&process, &addresses, &mut watchers);

                    if [TimerState::Running, TimerState::Paused].contains(&timer::state()) {
                        if let Some(val) = autosplitting::is_loading(&watchers, &settings) {
                            match val {
                                true => timer::pause_game_time(),
                                false => timer::resume_game_time(),
                            };
                        }

                        if let Some(game_time) =
                            autosplitting::game_time(&watchers, &settings, &addresses)
                        {
                            timer::set_game_time(game_time);
                        }

                        match autosplitting::reset(&watchers, &settings) {
                            true => timer::reset(),
                            false => {
                                if autosplitting::split(&watchers, &settings) {
                                    timer::split();
                                }
                            }
                        }
                    }

                    if timer::state().eq(&TimerState::NotRunning)
                        && autosplitting::start(&watchers, &settings)
                    {
                        timer::start();
                        timer::pause_game_time();

                        if let Some(val) = autosplitting::is_loading(&watchers, &settings) {
                            match val {
                                true => timer::pause_game_time(),
                                false => timer::resume_game_time(),
                            };
                        }
                    }

                    next_tick().await;
                }
            })
            .await;
    }
}
