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
use core::slice;

use alloc::vec::Vec;
use asr::{
    future::{next_tick, retry},
    game_engine::unity::{
        get_scene_name,
        mono::{Image, Module, UnityPointer, Version},
        SceneManager,
    },
    settings::{gui::Title, Gui},
    time::Duration,
    timer::{self, TimerState},
    watcher::Watcher,
    Address64, Process,
};
use bytemuck::CheckedBitPattern;

#[cfg(target_arch = "wasm32")]
use lol_alloc::{FreeListAllocator, LockedAllocator};

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: LockedAllocator<FreeListAllocator> = LockedAllocator::new(FreeListAllocator::new());

asr::panic_handler!();
asr::async_main!(stable);

const PROCESS_NAMES: &[&str] = &["Little Kitty, Big City.exe"];

async fn main() {
    let mut settings = Settings::register();

    loop {
        // Hook to the target process
        let process = retry(|| PROCESS_NAMES.iter().find_map(|&name| Process::attach(name))).await;

        process
            .until_closes(async {
                // Once the target has been found and attached to, set up some default watchers
                let mut watchers = Watchers::default();

                // Perform memory scanning to look for the addresses we need
                let addresses = Memory::init(&process).await;

                loop {
                    // Splitting logic. Adapted from OG LiveSplit:
                    // Order of execution
                    // 1. update() will always be run first. There are no conditions on the execution of this action.
                    // 2. If the timer is currently either running or paused, then the isLoading, gameTime, and reset actions will be run.
                    // 3. If reset does not return true, then the split action will be run.
                    // 4. If the timer is currently not running (and not paused), then the start action will be run.
                    settings.update();
                    update_loop(&process, &addresses, &mut watchers);

                    if [TimerState::Running, TimerState::Paused].contains(&timer::state()) {
                        match is_loading(&watchers, &settings) {
                            Some(true) => timer::pause_game_time(),
                            Some(false) => timer::resume_game_time(),
                            _ => (),
                        }

                        match game_time(&watchers, &settings, &addresses) {
                            Some(x) => timer::set_game_time(x),
                            _ => (),
                        }

                        match reset(&watchers, &settings) {
                            true => timer::reset(),
                            _ => match split(&watchers, &settings) {
                                true => timer::split(),
                                _ => (),
                            },
                        }
                    }

                    if timer::state().eq(&TimerState::NotRunning) && start(&watchers, &settings) {
                        timer::start();
                        timer::pause_game_time();

                        match is_loading(&watchers, &settings) {
                            Some(true) => timer::pause_game_time(),
                            Some(false) => timer::resume_game_time(),
                            _ => (),
                        }
                    }

                    next_tick().await;
                }
            })
            .await;
    }
}

#[derive(Gui)]
struct Settings {
    /// General settings
    general: Title,
    #[default = true]
    /// Enable auto start
    start: bool,
    /// Splitting settings
    split: Title,
    /// Split after eating fish
    #[default = true]
    eat_fish: bool,
    /// Split on game end
    #[default = true]
    got_home: bool,
    /// Quest list
    quests: Title,
    /// Split on completion of any quest
    #[default = false]
    any_quest: bool,
}

#[derive(Default)]
struct Watchers {
    start_trigger: Watcher<bool>,
    end_trigger: Watcher<bool>,
    is_loading: Watcher<bool>,
    quest_list: Watcher<Vec<QuestData>>,
    //quest_doggo: Watcher<bool>,
    //quest_mayor: Watcher<bool>,
    is_post_eating: Watcher<bool>,
    allow_player_shake: Watcher<bool>,
}

#[derive(Copy, Clone)]
struct QuestData {
    quest_id: u32,
    complete: bool,
}

struct Memory {
    mono_module: Module,
    mono_image: Image,
    scene_manager: SceneManager,
    trashcan_allow_shake: UnityPointer<3>,
    is_loading_save: UnityPointer<2>,
    is_teleporting: UnityPointer<2>,
    is_outro: UnityPointer<2>,
    quest_list: UnityPointer<2>,
    post_eat: UnityPointer<2>,
}

impl Memory {
    async fn init(game: &Process) -> Self {
        let mono_module = Module::wait_attach(game, Version::V3).await;
        let mono_image = mono_module.wait_get_default_image(game).await;
        let scene_manager = SceneManager::wait_attach(game).await;

        let trashcan_allow_shake = UnityPointer::new(
            "CatPlayer",
            0,
            &["_instance", "trashDive_TrashCan", "allowPlayerShake"],
        );
        let is_loading_save =
            UnityPointer::new("CatSaveSystemManager", 0, &["_instance", "_isLoading"]);
        let is_teleporting = UnityPointer::new("CatPlayer", 0, &["_instance", "isTeleporting"]);
        let is_outro = UnityPointer::new("CatGameManager", 0, &["_instance", "isInOutro"]);
        let quest_list = UnityPointer::new("Journal", 0, &["achievementMaster", "0"]);
        let post_eat = UnityPointer::new("CatPlayer", 0, &["_instance", "isPostEating"]);

        asr::print_limited::<24>(&"  => Autosplitter ready!");

        Self {
            mono_module,
            mono_image,
            scene_manager,
            trashcan_allow_shake,
            is_loading_save,
            is_teleporting,
            is_outro,
            quest_list,
            post_eat,
        }
    }
}

fn update_loop(game: &Process, memory: &Memory, watchers: &mut Watchers) {
    let current_scene = memory.scene_manager.get_current_scene_path::<128>(game);

    watchers.is_post_eating.update_infallible(
        memory
            .post_eat
            .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
            .unwrap_or_default()
            != 0,
    );
    watchers.allow_player_shake.update_infallible(
        memory
            .trashcan_allow_shake
            .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
            .unwrap_or_default()
            != 0,
    );

    watchers.start_trigger.update_infallible(
        current_scene
            .as_ref()
            .is_ok_and(|scene| get_scene_name(&scene) == b"Level_X")
            && watchers
                .allow_player_shake
                .pair
                .is_some_and(|val| val.changed_to(&true)),
    );

    watchers.end_trigger.update_infallible(
        memory
            .is_outro
            .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
            .is_ok_and(|val| val != 0),
    );

    watchers.is_loading.update_infallible(
        current_scene.as_ref().is_ok_and(|scene| {
            let scene_name = get_scene_name(&scene);
            scene_name == b"Loading" || scene_name == b"MainMenu_LKBC"
        }) || memory
            .is_loading_save
            .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
            .is_ok_and(|val| val != 0)
            || memory
                .is_teleporting
                .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
                .is_ok_and(|val| val != 0),
    );

    watchers.quest_list.update_infallible({
        #[derive(Copy, Clone, CheckedBitPattern)]
        #[repr(C)]
        struct List {
            _padding: [u8; 0x10],
            items: Address64,
            size: u32,
        }

        #[derive(Copy, Clone, CheckedBitPattern)]
        #[repr(C)]
        struct QuestInternal {
            _padding: [u8; 0x48],
            id: u32,
            _padding1: [u8; 17],
            completed: u8,
        }

        match memory
            .quest_list
            .deref::<List>(game, &memory.mono_module, &memory.mono_image)
        {
            Ok(list_data) => {
                let mut items = Vec::<Address64>::with_capacity(list_data.size as usize);

                let slice =
                    unsafe { slice::from_raw_parts_mut(items.as_mut_ptr(), items.capacity()) };

                unsafe { items.set_len(items.capacity()) };

                match game.read_into_slice(list_data.items + 0x20, slice) {
                    Ok(_) => {
                        let mut data = Vec::with_capacity(list_data.size as usize);

                        for i in items {
                            if let Ok(internal_data) = game.read::<QuestInternal>(i) {
                                data.push(QuestData {
                                    quest_id: internal_data.id,
                                    complete: internal_data.completed != 0,
                                });
                            }
                        }

                        data
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    });
}

fn start(watchers: &Watchers, settings: &Settings) -> bool {
    settings.start
        && watchers
            .start_trigger
            .pair
            .is_some_and(|val| val.changed_to(&true))
}

fn split(watchers: &Watchers, settings: &Settings) -> bool {
    let end_trigger = settings.got_home
        && watchers
            .end_trigger
            .pair
            .is_some_and(|val| val.changed_to(&true));

    let quest_list = settings.any_quest && {
        let mut value = false;

        if let Some(quest) = &watchers.quest_list.pair {
            let current: &Vec<QuestData> = &quest.current;
            let old = &quest.old;

            /*
            let quest_ids: [u8; 5] = [58, 15, 3, 4, 5];

            for i in quest_ids {
                let current = current
                    .iter()
                    .find(|&val| val.quest_id.eq(&i))
                    .map(|val| val.complete);
                let old = old
                    .iter()
                    .find(|&val| val.quest_id.eq(&i))
                    .map(|val| val.complete);

                if old.is_some_and(|val| !val) && current.is_some_and(|val| val) {
                    value = true;
                    break;
                }
            }
            */

            for i in current {
                let quest_id = i.quest_id;

                let current = current
                    .iter()
                    .find(|&val| val.quest_id.eq(&quest_id))
                    .map(|val| val.complete);

                let old = old
                    .iter()
                    .find(|&val| val.quest_id.eq(&quest_id))
                    .map(|val| val.complete);

                if old.is_some_and(|val| !val) && current.is_some_and(|val| val) {
                    value = true;
                    break;
                }
            }
        }

        value
    };

    let post_eating = settings.eat_fish
        && watchers
            .is_post_eating
            .pair
            .is_some_and(|val| val.changed_to(&true));

    end_trigger || quest_list || post_eating
}

fn reset(_watchers: &Watchers, _settings: &Settings) -> bool {
    false
}

fn is_loading(watchers: &Watchers, _settings: &Settings) -> Option<bool> {
    Some(watchers.is_loading.pair.is_some_and(|val| val.eq(&true)))
}

fn game_time(_watchers: &Watchers, _settings: &Settings, _addresses: &Memory) -> Option<Duration> {
    None
}
