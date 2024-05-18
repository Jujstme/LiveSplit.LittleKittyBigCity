use asr::{
    file_format::pe, future::retry, signature::Signature, string::ArrayCString, Address, Address32,
    PointerSize, Process,
};

/// The scene manager allows you to easily identify the current scene loaded in
/// the attached Unity game.
///
/// It can be useful to identify splitting conditions or as an alternative to
/// the traditional class lookup in games with no useful static references.
pub struct SceneManager {
    pointer_size: PointerSize,
    address: Address,
    offsets: &'static Offsets,
}

impl SceneManager {
    /// Attaches to the scene manager in the given process.
    pub fn attach(process: &Process) -> Option<Self> {
        const SIG_64_BIT: Signature<13> = Signature::new("48 83 EC 20 4C 8B ?5 ???????? 33 F6");
        const SIG_32_1: Signature<12> = Signature::new("55 8B EC 51 A1 ???????? 53 33 DB");
        const SIG_32_2: Signature<6> = Signature::new("53 8D 41 ?? 33 DB");
        const SIG_32_3: Signature<14> = Signature::new("55 8B EC 83 EC 18 A1 ???????? 33 C9 53");

        let unity_player = process
            .get_module_address("UnityPlayer.dll")
            .ok()
            .and_then(|address| {
                Some((address, pe::read_size_of_image(process, address)? as u64))
            })?;

        let pointer_size = match pe::MachineType::read(process, unity_player.0)? {
            pe::MachineType::X86_64 => PointerSize::Bit64,
            _ => PointerSize::Bit32,
        };

        // There are multiple signatures that can be used, depending on the version of Unity
        // used in the target game.
        let base_address: Address = if pointer_size == PointerSize::Bit64 {
            let addr = SIG_64_BIT.scan_process_range(process, unity_player)? + 7;
            addr + 0x4 + process.read::<i32>(addr).ok()?
        } else if let Some(addr) = SIG_32_1.scan_process_range(process, unity_player) {
            process.read::<Address32>(addr + 5).ok()?.into()
        } else if let Some(addr) = SIG_32_2.scan_process_range(process, unity_player) {
            process.read::<Address32>(addr.add_signed(-4)).ok()?.into()
        } else if let Some(addr) = SIG_32_3.scan_process_range(process, unity_player) {
            process.read::<Address32>(addr + 7).ok()?.into()
        } else {
            return None;
        };

        let offsets = Offsets::new(pointer_size);

        // Dereferencing one level because this pointer never changes as long as the game is open.
        // It might not seem a lot, but it helps make things a bit faster when querying for scene stuff.
        let address = process
            .read_pointer(base_address, pointer_size)
            .ok()
            .filter(|val| !val.is_null())?;

        Some(Self {
            pointer_size,
            address,
            offsets,
        })
    }

    /// Attaches to the scene manager in the given process.
    ///
    /// This is the `await`able version of the [`attach`](Self::attach)
    /// function, yielding back to the runtime between each try.
    pub async fn wait_attach(process: &Process) -> SceneManager {
        retry(|| Self::attach(process)).await
    }

    /// Tries to retrieve the current active scene.
    fn get_current_scene(&self, process: &Process) -> Option<Scene> {
        Some(Scene {
            address: process
                .read_pointer(self.address + self.offsets.active_scene, self.pointer_size)
                .ok()
                .filter(|val| !val.is_null())?,
        })
    }

    /// Returns the full path to the current scene. Use [`get_scene_name`]
    /// afterwards to get the scene name.
    pub fn get_current_scene_path<const N: usize>(
        &self,
        process: &Process,
    ) -> Option<ArrayCString<N>> {
        self.get_current_scene(process)?.path(process, self)
    }
}

struct Offsets {
    active_scene: u8,
    asset_path: u8,
}

impl Offsets {
    pub const fn new(pointer_size: PointerSize) -> &'static Self {
        match pointer_size {
            PointerSize::Bit64 => &Self {
                active_scene: 0x48,
                asset_path: 0x10,
            },
            _ => &Self {
                active_scene: 0x28,
                asset_path: 0xC,
            },
        }
    }
}

/// A scene loaded in the attached game.
pub struct Scene {
    address: Address,
}

impl Scene {
    /// Returns the full path to the scene.
    pub fn path<const N: usize>(
        &self,
        process: &Process,
        scene_manager: &SceneManager,
    ) -> Option<ArrayCString<N>> {
        process
            .read_pointer_path(
                self.address,
                scene_manager.pointer_size,
                &[scene_manager.offsets.asset_path as u64, 0x0],
            )
            .ok()
    }
}
