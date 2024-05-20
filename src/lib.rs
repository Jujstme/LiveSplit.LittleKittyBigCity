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
use alloc::vec::Vec;
use asr::{
    future::{next_tick, retry},
    game_engine::unity::get_scene_name,
    settings::{gui::Title, Gui},
    time::Duration,
    timer::{self, TimerState},
    watcher::Watcher,
    Process,
};
use bytemuck::Zeroable;
use csharp::CSharpList;
use mono::{Image, Module, UnityPointer};
use scene_manager::SceneManager;

mod csharp;
mod mono;
mod scene_manager;

asr::panic_handler!();
asr::async_main!(stable);

const PROCESS_NAMES: &[&str] = &["Little Kitty, Big City.exe"];
const USE_LINUX_WORKAROUND: bool = true;

#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

async fn main() {
    // When the autosplitter is loaded, it loads the settings
    let mut settings = Settings::register();

    loop {
        // First thing to do in the autosplitter logic is to hook to the target process.
        // This needs to stay inside the loop as the autosplitter must re-try to hook
        // to the target process once it is exited.
        let (process, process_name) = retry(|| {
            PROCESS_NAMES.iter().find_map(|&name| {
                let mut proc = Process::attach(name);
                if proc.is_none() && USE_LINUX_WORKAROUND && name.len() > 15 {
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
                let mut watchers = Watchers::default();

                // Perform memory scanning to look for the addresses we need.
                // Depending on the game and the logic, we can either define fixed
                // memory offsets, or perform more advanced stuff (eg. sigscanning).
                // The name of the executable is passed here in order to easily allow
                // to query for the process' main module.
                let addresses = Memory::init(&process, process_name).await;

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
                        if let Some(val) = is_loading(&watchers, &settings) {
                            match val {
                                true => timer::pause_game_time(),
                                false => timer::resume_game_time(),
                            };
                        }

                        if let Some(game_time) = game_time(&watchers, &settings, &addresses) {
                            timer::set_game_time(game_time);
                        }

                        match reset(&watchers, &settings) {
                            true => timer::reset(),
                            false => {
                                if split(&watchers, &settings) {
                                    timer::split();
                                }
                            }
                        }
                    }

                    if timer::state().eq(&TimerState::NotRunning) && start(&watchers, &settings) {
                        timer::start();
                        timer::pause_game_time();

                        if let Some(val) = is_loading(&watchers, &settings) {
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
    /// Find the crow
    #[default = true]
    find_crow: bool,
    /// Bring crow 25 shinies
    #[default = true]
    bring_crow_25_shinies: bool,
    /// Become an artist
    #[default = true]
    become_artist: bool,
    /// Catch a bird
    #[default = true]
    catch_a_bird: bool,
    /// Help the Mayor get some sleep
    #[default = true]
    help_mayor: bool,
    /// Rescue the tanuki from the pipe
    #[default = true]
    rescue_tanuki: bool,
    /// Reunite the duckling family
    #[default = true]
    reunite_the_family: bool,
    /// Fetch 3 feathers for the tanuki
    #[default = true]
    fetch_3_feathers: bool,
    /// Pose for Beetle
    #[default = true]
    pose_for_beetle: bool,
    /// Fetch the dog's balls
    #[default = true]
    fetch_dog_balls: bool,
    /// Find Chameleon
    #[default = true]
    find_chameleon_1: bool,
    /// Find Chameleon... again!
    #[default = true]
    find_chameleon_2: bool,
    /// Find Chameleon, part III
    #[default = true]
    find_chameleon_3: bool,
    /// Find Chameleon: Episode 4
    #[default = true]
    find_chameleon_4: bool,
    /// Find Chameleon: 5IVE!
    #[default = true]
    find_chameleon_5: bool,
    /// Chameleon 6: Find and Furious
    #[default = true]
    find_chameleon_6: bool,
    /// Find Chameleon: Chapter 7
    #[default = true]
    find_chameleon_7: bool,
    /// Find Chameleon: The Return of Chaml
    #[default = true]
    find_chameleon_8: bool,
    /// Steal the gardener's lunch
    #[default = true]
    steal_lunch: bool,
    /// Boss Cat vs. Ramune!
    #[default = true]
    catch_yellow_bird: bool,
    /// Waiting on a sunbeam
    #[default = true]
    sunbeam: bool,
    /// Cat-chievements
    catchievements: Title,
    /// Hello Everyone! (meet all characters)
    #[default = false]
    hello_everyone: bool,
    /// Quack Troops! (collect all ducklings)
    #[default = false]
    quack_troops: bool,
    /// Snap Happy! (got photo mode)
    #[default = false]
    snap_happy: bool,
    /// Capped Crusader (collect all hats)
    #[default = false]
    capped_crusader: bool,
    /// World Traveler (open all portals)
    #[default = false]
    world_traveler: bool,
    /// Cat Napper (nap in all spots)
    #[default = false]
    cat_napper: bool,
    /// Bird Botherer (catch 20 birds)
    #[default = false]
    bird_botherer: bool,
    /// If I Fits, I Sits (climb in 5 boxes)
    #[default = false]
    if_i_fits_i_sits: bool,
    /// Litter Picker (recycle 100 items)
    #[default = false]
    litter_picker: bool,
    /// Smash Hit (break 100 objects)
    #[default = false]
    smash_hit: bool,
    /// Sticky Business (bust all bird nests)
    #[default = false]
    sticky_business: bool,
    /// Give A Dog A Bone (bring bone to all dogs)
    #[default = false]
    give_a_dog_a_bone: bool,
    /// Cult of Purr-sonality (be pet 10 times)
    #[default = false]
    cult_of_purrsonality: bool,
    /// Local Celebrity (be photographed 20 times)
    #[default = false]
    local_celebrity: bool,
    /// Papa-cat-zi (take 20 photos)
    #[default = false]
    papa_cat_zi: bool,
    /// Cat-Like Reflexes (catch a bid in mid-air)
    #[default = false]
    cat_like_reflexes: bool,
    /// Back Of The Net (score all soccer goals)
    #[default = false]
    back_of_the_net: bool,
    /// Surprise! (knock over a human)
    #[default = false]
    surprise: bool,
    /// Fruit Fall (make a human slip on a banana)
    #[default = false]
    fruit_fall: bool,
    /// Industrial Artist (concrete artist)
    #[default = false]
    industrial_artist: bool,
    /// Checkmate!
    #[default = false]
    checkmate: bool,
    /// To Me, To You (human kick ball to you)
    #[default = false]
    to_me_to_you: bool,
    /// No Parking! (paint fancy car)
    #[default = false]
    no_parking: bool,
    /// Rub-A-Dub-Dub! (put rubber duck in the pond)
    #[default = false]
    rub_a_dub_dub: bool,
    /// And Stay Out! (get kicked out of a store)
    #[default = false]
    and_stay_out: bool,
    /// Killer Kitty! (chase human danger item)
    #[default = false]
    killer_kitty: bool,
    /// Who Needs Cash? (bonk soda machine)
    #[default = false]
    who_needs_cash: bool,
    /// Little Kitty, Big City
    #[default = false]
    little_kitty_big_city: bool,
    /// Can't Stop The Feelings (use an emote)
    #[default = false]
    cant_stop_the_feelings: bool,
    /// What Sweet Music (meow 10 times)
    #[default = false]
    what_sweet_music: bool,
    /// Trip Hazard (make humans trip 20 times)
    #[default = false]
    trip_hazard: bool,
    /// Splish! (portapotty mischief)
    #[default = false]
    splish: bool,
    /// Decluttering (smash items)
    #[default = false]
    decluttering: bool,
    /// Dumpster Diving (dive trash)
    #[default = false]
    dumpster_diving: bool,
}

struct Memory {
    mono_module: Module,
    mono_image: Image,
    scene_manager: crate::scene_manager::SceneManager,

    trashcan_allow_shake: UnityPointer<3>,
    is_loading_save: UnityPointer<2>,
    is_teleporting: UnityPointer<2>,
    is_outro: UnityPointer<2>,
    quest_list: UnityPointer<1>,
    quest_secondary_list: UnityPointer<1>,

    post_eat: UnityPointer<2>,
    offset_achievement_id: usize,
    offset_achievement_completed: usize,
}

impl Memory {
    async fn init(game: &Process, _process_name: &str) -> Self {
        asr::print_message("Autosplitter loading...");

        asr::print_message("  => Loading Mono module...");
        let mono_module = Module::wait_attach_auto_detect(game).await;
        asr::print_message("    => Found Mono module");

        asr::print_message("  => Loading Assembly-CSharp.dll...");
        let mono_image = mono_module.wait_get_default_image(game).await;
        asr::print_message("    => Found Assembly-CSharp.dll");

        asr::print_message("  => Loading Scene Manager...");
        let scene_manager = SceneManager::wait_attach(game).await;
        asr::print_message("    => Found Scene Manager");

        asr::print_message("  => Setting up memory watchers...");
        let trashcan_allow_shake = UnityPointer::new(
            "CatPlayer",
            0,
            &["_instance", "trashDive_TrashCan", "allowPlayerShake"],
        );
        let is_loading_save =
            UnityPointer::new("CatSaveSystemManager", 0, &["_instance", "_isLoading"]);
        let is_teleporting = UnityPointer::new("CatPlayer", 0, &["_instance", "isTeleporting"]);
        let is_outro = UnityPointer::new("CatGameManager", 0, &["_instance", "isInOutro"]);
        let quest_list = UnityPointer::new("Journal", 0, &["achievementMaster"]);
        let quest_secondary_list = UnityPointer::new("Journal", 0, &["achievementSecondary"]);
        let post_eat = UnityPointer::new("CatPlayer", 0, &["_instance", "isPostEating"]);

        let achievement_class = mono_image
            .wait_get_class(game, &mono_module, "Achievement")
            .await;

        let offset_achievement_id = achievement_class
            .wait_get_field_offset(game, &mono_module, "id")
            .await as usize;
        let offset_achievement_completed = achievement_class
            .wait_get_field_offset(game, &mono_module, "_completed")
            .await as usize;
        asr::print_message("    => Done!");

        asr::print_limited::<24>(&" => Autosplitter ready!");

        Self {
            mono_module,
            mono_image,
            scene_manager,
            trashcan_allow_shake,
            is_loading_save,
            is_teleporting,
            is_outro,
            quest_list,
            quest_secondary_list,
            post_eat,
            offset_achievement_id,
            offset_achievement_completed,
        }
    }
}

#[derive(Default)]
struct Watchers {
    start_trigger: Watcher<bool>,
    end_trigger: Watcher<bool>,
    is_loading: Watcher<bool>,
    quest_list: Watcher<Vec<QuestData>>,
    quest_secondary_list: Watcher<Vec<QuestData>>,

    is_post_eating: Watcher<bool>,
    allow_player_shake: Watcher<bool>,
}

fn update_loop(game: &Process, memory: &Memory, watchers: &mut Watchers) {
    let current_scene = memory.scene_manager.get_current_scene_path::<128>(game);

    watchers.is_post_eating.update_infallible(
        memory
            .post_eat
            .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
            .is_some_and(|val| val != 0),
    );

    watchers.allow_player_shake.update_infallible(
        memory
            .trashcan_allow_shake
            .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
            .is_some_and(|val| val != 0),
    );

    watchers.start_trigger.update_infallible(
        current_scene
            .as_ref()
            .is_some_and(|scene| get_scene_name(scene) == b"Level_X")
            && watchers
                .allow_player_shake
                .pair
                .is_some_and(|val| val.changed_to(&true)),
    );

    watchers.end_trigger.update_infallible(
        memory
            .is_outro
            .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
            .is_some_and(|val| val != 0),
    );

    watchers.is_loading.update_infallible(
        current_scene.as_ref().is_some_and(|scene| {
            let scene_name = get_scene_name(scene);
            scene_name == b"Loading" || scene_name == b"MainMenu_LKBC"
        }) || memory
            .is_loading_save
            .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
            .is_some_and(|val| val != 0)
            || memory
                .is_teleporting
                .deref::<u8>(game, &memory.mono_module, &memory.mono_image)
                .is_some_and(|val| val != 0),
    );

    watchers.quest_list.update_infallible({
        match memory
            .quest_list
            .deref::<CSharpList<[u8; 0x68]>>(game, &memory.mono_module, &memory.mono_image)
            .map(|list| list.iter(game))
            .map(|data| {
                data.map(|item| QuestData {
                    quest_id: unsafe {
                        *(item.as_ptr().byte_add(memory.offset_achievement_id) as *const u32)
                    },
                    complete: item[memory.offset_achievement_completed] != 0,
                })
            }) {
            Some(x) => x.collect(),
            _ => Vec::with_capacity(0),
        }
    });

    watchers.quest_secondary_list.update_infallible({
        match memory
            .quest_secondary_list
            .deref::<CSharpList<[u8; 0x68]>>(game, &memory.mono_module, &memory.mono_image)
            .map(|list| list.iter(game))
            .map(|data| {
                data.map(|item| QuestData {
                    quest_id: unsafe {
                        *(item.as_ptr().byte_add(memory.offset_achievement_id) as *const u32)
                    },
                    complete: item[memory.offset_achievement_completed] != 0,
                })
            }) {
            Some(x) => x.collect(),
            _ => Vec::with_capacity(0),
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

    let quest_list = {
        let mut value = false;

        if let Some(quest) = &watchers.quest_list.pair {
            for i in &quest.current {
                let quest_id = i.quest_id;

                let split_setting = match quest_id {
                    8 => settings.catch_a_bird,
                    12 => settings.fetch_dog_balls,
                    19 => settings.bring_crow_25_shinies,
                    21 => settings.rescue_tanuki,
                    24 => settings.fetch_3_feathers,
                    28 => settings.reunite_the_family,
                    29 => settings.help_mayor,
                    32 => settings.find_crow,
                    34 => settings.become_artist,
                    36 => settings.find_chameleon_1,
                    37 => settings.find_chameleon_2,
                    38 => settings.find_chameleon_3,
                    39 => settings.sunbeam,
                    49 => settings.pose_for_beetle,
                    41 => settings.find_chameleon_4,
                    42 => settings.find_chameleon_5,
                    43 => settings.find_chameleon_6,
                    44 => settings.find_chameleon_7,
                    45 => settings.find_chameleon_8,
                    47 => settings.steal_lunch,
                    56 => settings.catch_yellow_bird,
                    _ => false,
                };

                if split_setting {
                    let old = quest
                        .old
                        .iter()
                        .find(|&val| val.quest_id.eq(&quest_id))
                        .map(|val| val.complete);

                    if old.is_some_and(|val| !val) && i.complete {
                        value = true;
                        break;
                    }
                }
            }
        }

        value
    };

    let catchievements = {
        let mut value = false;

        if let Some(quest) = &watchers.quest_secondary_list.pair {
            for i in &quest.current {
                let quest_id = i.quest_id;

                let split_setting = match quest_id {
                    1 => settings.hello_everyone,
                    2 => settings.quack_troops,
                    3 => settings.snap_happy,
                    7 => settings.capped_crusader,
                    8 => settings.world_traveler,
                    9 => settings.cat_napper,
                    10 => settings.bird_botherer,
                    11 => settings.if_i_fits_i_sits,
                    12 => settings.litter_picker,
                    13 => settings.smash_hit,
                    14 => settings.sticky_business,
                    15 => settings.give_a_dog_a_bone,
                    16 => settings.cult_of_purrsonality,
                    17 => settings.local_celebrity,
                    19 => settings.papa_cat_zi,
                    23 => settings.cat_like_reflexes,
                    24 => settings.back_of_the_net,
                    26 => settings.surprise,
                    27 => settings.fruit_fall,
                    30 => settings.industrial_artist,
                    31 => settings.checkmate,
                    32 => settings.to_me_to_you,
                    33 => settings.no_parking,
                    34 => settings.rub_a_dub_dub,
                    36 => settings.and_stay_out,
                    37 => settings.killer_kitty,
                    38 => settings.who_needs_cash,
                    39 => settings.little_kitty_big_city,
                    41 => settings.cant_stop_the_feelings,
                    42 => settings.what_sweet_music,
                    43 => settings.trip_hazard,
                    44 => settings.splish,
                    45 => settings.decluttering,
                    46 => settings.dumpster_diving,
                    _ => false,
                };

                if split_setting {
                    let old = quest
                        .old
                        .iter()
                        .find(|&val| val.quest_id.eq(&quest_id))
                        .map(|val| val.complete);

                    if old.is_some_and(|val| !val) && i.complete {
                        value = true;
                        break;
                    }
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

    end_trigger || quest_list || catchievements || post_eating
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

#[derive(Copy, Clone, Zeroable, Hash, PartialEq, Eq)]
struct QuestData {
    quest_id: u32,
    complete: bool,
}
