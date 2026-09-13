#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveDown,
    MoveUp,
    GoTop,
    GoBottom,
    FocusSidebar,
    FocusMain,
    Select,
    PlayPause,
    Next,
    Prev,
    SeekForward,
    SeekBackward,
    VolumeUp,
    VolumeDown,
    Refresh,
    Quit,
}
