use crate::{csharp::CSharpList, memory::Memory, settings::Settings};
use alloc::vec::Vec;
use asr::{game_engine::unity::get_scene_name, time::Duration, watcher::Watcher, Process};
use bytemuck::Zeroable;

#[derive(Default)]
pub(super) struct Watchers {
    pub(super) start_trigger: Watcher<bool>,
    pub(super) end_trigger: Watcher<bool>,
    pub(super) is_loading: Watcher<bool>,
    pub(super) quest_list: Watcher<Vec<QuestData>>,
    pub(super) quest_secondary_list: Watcher<Vec<QuestData>>,

    pub(super) is_post_eating: Watcher<bool>,
    pub(super) allow_player_shake: Watcher<bool>,
}

#[derive(Copy, Clone, Zeroable, Hash, PartialEq, Eq)]
pub(super) struct QuestData {
    quest_id: u32,
    complete: bool,
}

pub(super) fn update(game: &Process, memory: &Memory, watchers: &mut Watchers) {
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
            .is_some_and(|scene| get_scene_name(&scene) == b"Level_X")
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
            let scene_name = get_scene_name(&scene);
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
                    quest_id: *bytemuck::checked::from_bytes(
                        &item[memory.offset_achievement_id
                            ..memory.offset_achievement_id + core::mem::size_of::<u32>()],
                    ),
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
                    quest_id: *bytemuck::checked::from_bytes(
                        &item[memory.offset_achievement_id
                            ..memory.offset_achievement_id + core::mem::size_of::<u32>()],
                    ),
                    complete: item[memory.offset_achievement_completed] != 0,
                })
            }) {
            Some(x) => x.collect(),
            _ => Vec::with_capacity(0),
        }
    });
}

pub(super) fn start(watchers: &Watchers, settings: &Settings) -> bool {
    settings.start
        && watchers
            .start_trigger
            .pair
            .is_some_and(|val| val.changed_to(&true))
}

pub(super) fn split(watchers: &Watchers, settings: &Settings) -> bool {
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
                    32 => settings.find_crow,
                    19 => settings.bring_crow_25_shinies,
                    34 => settings.become_artist,
                    8 => settings.catch_a_bird,
                    29 => settings.help_mayor,
                    21 => settings.rescue_tanuki,
                    28 => settings.reunite_the_family,
                    24 => settings.fetch_3_feathers,
                    49 => settings.pose_for_beetle,
                    12 => settings.fetch_dog_balls,
                    36 => settings.find_chameleon_1,
                    37 => settings.find_chameleon_2,
                    38 => settings.find_chameleon_3,
                    41 => settings.find_chameleon_4,
                    42 => settings.find_chameleon_5,
                    43 => settings.find_chameleon_6,
                    44 => settings.find_chameleon_7,
                    45 => settings.find_chameleon_8,
                    47 => settings.steal_lunch,
                    56 => settings.catch_yellow_bird,
                    39 => settings.sunbeam,
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

    end_trigger || quest_list || post_eating
}

pub(super) fn reset(_watchers: &Watchers, _settings: &Settings) -> bool {
    false
}

pub(super) fn is_loading(watchers: &Watchers, _settings: &Settings) -> Option<bool> {
    Some(watchers.is_loading.pair.is_some_and(|val| val.eq(&true)))
}

pub(super) fn game_time(
    _watchers: &Watchers,
    _settings: &Settings,
    _addresses: &Memory,
) -> Option<Duration> {
    None
}
