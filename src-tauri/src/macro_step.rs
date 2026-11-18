//! Actions shared by the original PC features; the desktop worker owns execution.
use crate::{navigation::Motion, vision::Rect};

pub(crate) enum Step {
    Observe { region: Option<Rect>, ocr: bool },
    Input(Motion),
    Text { value: String, submit: bool },
    Travel { place: usize },
    Wait(f64),
    Done,
    Failed(String),
}
