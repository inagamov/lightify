#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveDown,
    MoveUp,
    GoTop,
    GoBottom,
    FocusSidebar,
    FocusMain,
    Select,
    Refresh,
    Quit,
}
