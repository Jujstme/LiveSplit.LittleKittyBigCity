use asr::Process;

use crate::{
    mono::{Image, Module, UnityPointer},
    scene_manager::SceneManager,
};

pub(super) struct Memory {
    pub(super) mono_module: Module,
    pub(super) mono_image: Image,
    pub(super) scene_manager: crate::scene_manager::SceneManager,

    pub(super) trashcan_allow_shake: UnityPointer<3>,
    pub(super) is_loading_save: UnityPointer<2>,
    pub(super) is_teleporting: UnityPointer<2>,
    pub(super) is_outro: UnityPointer<2>,
    pub(super) quest_list: UnityPointer<1>,
    pub(super) quest_secondary_list: UnityPointer<1>,

    pub(super) post_eat: UnityPointer<2>,
    pub(super) offset_achievement_id: usize,
    pub(super) offset_achievement_completed: usize,
}

impl Memory {
    pub(super) async fn init(game: &Process, _process_name: &str) -> Self {
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
