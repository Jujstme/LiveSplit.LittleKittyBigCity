pub(super) const PROCESS_NAMES: &[&str] = &["Little Kitty, Big City.exe"];

// If enabled, the main logic loop will use a workaround for hooking
// into the main process on Linux (eg. Wine) by truncating the
// process name to the 15th char.
pub(super) const USE_LINUX_WORKAROUND: bool = true;
