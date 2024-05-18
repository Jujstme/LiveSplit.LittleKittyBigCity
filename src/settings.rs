use asr::settings::{gui::Title, Gui};

#[derive(Gui)]
pub(crate) struct Settings {
    /// General settings
    pub general: Title,
    #[default = true]
    /// Enable auto start
    pub start: bool,
    /// Splitting settings
    pub split: Title,
    /// Split after eating fish
    #[default = true]
    pub eat_fish: bool,
    /// Split on game end
    #[default = true]
    pub got_home: bool,
    /// Quest list
    pub quests: Title,
    /// Find the crow
    #[default = true]
    pub find_crow: bool,
    /// Bring crow 25 shinies
    #[default = true]
    pub bring_crow_25_shinies: bool,
    /// Become an artist
    #[default = true]
    pub become_artist: bool,
    /// Catch a bird
    #[default = true]
    pub catch_a_bird: bool,
    /// Help the Mayor get some sleep
    #[default = true]
    pub help_mayor: bool,
    /// Rescue the tanuki from the pipe
    #[default = true]
    pub rescue_tanuki: bool,
    /// Reunite the duckling family
    #[default = true]
    pub reunite_the_family: bool,
    /// Fetch 3 feathers for the tanuki
    #[default = true]
    pub fetch_3_feathers: bool,
    /// Pose for Beetle
    #[default = true]
    pub pose_for_beetle: bool,
    /// Fetch the dog's balls
    #[default = true]
    pub fetch_dog_balls: bool,
    /// Find Chameleon
    #[default = true]
    pub find_chameleon_1: bool,
    /// Find Chameleon... again!
    #[default = true]
    pub find_chameleon_2: bool,
    /// Find Chameleon, part III
    #[default = true]
    pub find_chameleon_3: bool,
    /// Find Chameleon: Episode 4
    #[default = true]
    pub find_chameleon_4: bool,
    /// Find Chameleon: 5IVE!
    #[default = true]
    pub find_chameleon_5: bool,
    /// Chameleon 6: Find and Furious
    #[default = true]
    pub find_chameleon_6: bool,
    /// Find Chameleon: Chapter 7
    #[default = true]
    pub find_chameleon_7: bool,
    /// Find Chameleon: The Return of Chaml
    #[default = true]
    pub find_chameleon_8: bool,
    /// Steal the gardener's lunch
    #[default = true]
    pub steal_lunch: bool,
    /// Boss Cat vs. Ramune!
    #[default = true]
    pub catch_yellow_bird: bool,
    /// Waiting on a sumbeam
    #[default = true]
    pub sunbeam: bool,
}
